#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "requests",
#     "pandas",
#     "matplotlib",
# ]
# ///
"""
Backtest precipitation forecast accuracy for a fixed location across models
and lead times, using Open-Meteo's Previous Runs API (forecasts) and
Historical Weather API (ERA5 reanalysis as ground truth). Defaults to
Adlershof, Berlin; pass --location-name/--latitude/--longitude for others.

Usage:
    uv run scripts/precip_backtest.py [options]

Data is cached (per location) in a local SQLite database so repeated runs
only fetch new/missing days instead of re-downloading everything. Only the
metrics table and chart are always recomputed from the full cache.

Output:
    <out-dir>/<location-name>/metrics.csv           model x lead_time x metric table
    <out-dir>/<location-name>/skill_degradation.png  line chart of MAE and CSI(>1mm) vs lead time
"""

import argparse
import datetime as dt
import logging
import sqlite3
import sys
import time
from pathlib import Path

import pandas as pd
import requests

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
DEFAULT_LOCATION_NAME = "adlershof"
ADLERSHOF_LATITUDE = 52.43
ADLERSHOF_LONGITUDE = 13.53

DEFAULT_MODELS = [
    "icon_seamless",
    "gfs_seamless",
    "ecmwf_ifs025",
    "meteofrance_arome_france",
    "meteofrance_arome_france_hd",
    "ukmo_seamless",
]
DEFAULT_LEAD_TIMES = list(range(1, 8))
DEFAULT_THRESHOLDS_MM = [0.1, 1.0, 5.0]
DEFAULT_START_DATE = dt.date(2024, 1, 1)

PREVIOUS_RUNS_URL = "https://previous-runs-api.open-meteo.com/v1/forecast"
ARCHIVE_URL = "https://archive-api.open-meteo.com/v1/archive"

MAX_CHUNK_DAYS = 366  # keep individual HTTP requests to a reasonable size
HTTP_TIMEOUT_S = 60
MAX_RETRIES = 5

log = logging.getLogger("precip_backtest")


# ---------------------------------------------------------------------------
# Date helpers
# ---------------------------------------------------------------------------
def daterange(start: dt.date, end: dt.date) -> list[dt.date]:
    return [start + dt.timedelta(days=i) for i in range((end - start).days + 1)]


def missing_ranges(
    all_dates: list[dt.date], present: set[dt.date]
) -> list[tuple[dt.date, dt.date]]:
    """Group missing dates into contiguous (start, end) ranges, capped at
    MAX_CHUNK_DAYS, to keep the number and size of HTTP requests bounded."""
    missing = sorted(d for d in all_dates if d not in present)
    ranges: list[tuple[dt.date, dt.date]] = []
    chunk_start = None
    prev = None
    for d in missing:
        if chunk_start is None:
            chunk_start = d
        elif (d - prev).days > 1 or (d - chunk_start).days >= MAX_CHUNK_DAYS:
            ranges.append((chunk_start, prev))
            chunk_start = d
        prev = d
    if chunk_start is not None:
        ranges.append((chunk_start, prev))
    return ranges


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------
def fetch_json(session: requests.Session, url: str, params: dict) -> dict:
    backoff = 2
    for attempt in range(1, MAX_RETRIES + 1):
        try:
            resp = session.get(url, params=params, timeout=HTTP_TIMEOUT_S)
        except requests.RequestException as e:
            if attempt == MAX_RETRIES:
                raise
            log.warning("request error (%s), retrying in %ss", e, backoff)
            time.sleep(backoff)
            backoff *= 2
            continue
        if resp.status_code == 200:
            return resp.json()
        if resp.status_code in (429, 500, 502, 503, 504) and attempt < MAX_RETRIES:
            log.warning(
                "HTTP %s from %s, retrying in %ss", resp.status_code, url, backoff
            )
            time.sleep(backoff)
            backoff *= 2
            continue
        resp.raise_for_status()
    raise RuntimeError(f"exhausted retries fetching {url}")


# ---------------------------------------------------------------------------
# SQLite cache
# ---------------------------------------------------------------------------
def _migrate_legacy_schema(conn: sqlite3.Connection) -> None:
    """Add the `location` column to pre-existing single-location caches,
    tagging their rows as DEFAULT_LOCATION_NAME, instead of discarding
    already-fetched data."""
    tables = {
        row[0]
        for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")
    }
    if "forecast" not in tables:
        return
    cols = {row[1] for row in conn.execute("PRAGMA table_info(forecast)")}
    if "location" in cols:
        return
    log.info("Migrating cache schema to support multiple locations")
    conn.execute("ALTER TABLE forecast RENAME TO forecast_legacy")
    conn.execute("ALTER TABLE observed RENAME TO observed_legacy")
    _create_tables(conn)
    conn.execute(
        f"""
        INSERT INTO forecast (date, location, model, lead_time, precip_mm)
        SELECT date, '{DEFAULT_LOCATION_NAME}', model, lead_time, precip_mm FROM forecast_legacy
        """
    )
    conn.execute(
        f"""
        INSERT INTO observed (date, location, precip_mm)
        SELECT date, '{DEFAULT_LOCATION_NAME}', precip_mm FROM observed_legacy
        """
    )
    conn.execute("DROP TABLE forecast_legacy")
    conn.execute("DROP TABLE observed_legacy")
    conn.commit()


def _create_tables(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS forecast (
            date TEXT NOT NULL,
            location TEXT NOT NULL,
            model TEXT NOT NULL,
            lead_time INTEGER NOT NULL,
            precip_mm REAL,
            PRIMARY KEY (date, location, model, lead_time)
        )
        """
    )
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS observed (
            date TEXT NOT NULL,
            location TEXT NOT NULL,
            precip_mm REAL,
            PRIMARY KEY (date, location)
        )
        """
    )


def open_db(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(path)
    _migrate_legacy_schema(conn)
    _create_tables(conn)
    conn.commit()
    return conn


def fetch_observed(
    conn: sqlite3.Connection,
    session: requests.Session,
    location: str,
    start: dt.date,
    end: dt.date,
    lat: float,
    lon: float,
) -> None:
    all_dates = daterange(start, end)
    cur = conn.execute(
        "SELECT date FROM observed WHERE location = ? AND date BETWEEN ? AND ?",
        (location, start.isoformat(), end.isoformat()),
    )
    present = {dt.date.fromisoformat(row[0]) for row in cur}
    for chunk_start, chunk_end in missing_ranges(all_dates, present):
        log.info(
            "Fetching ERA5 observed precipitation (%s) %s..%s",
            location,
            chunk_start,
            chunk_end,
        )
        data = fetch_json(
            session,
            ARCHIVE_URL,
            {
                "latitude": lat,
                "longitude": lon,
                "daily": "precipitation_sum",
                "start_date": chunk_start.isoformat(),
                "end_date": chunk_end.isoformat(),
                "timezone": "UTC",
            },
        )
        if data.get("error"):
            log.warning(
                "archive API error for %s..%s: %s",
                chunk_start,
                chunk_end,
                data.get("reason"),
            )
            continue
        daily = data.get("daily", {})
        rows = [
            (day, location, precip)
            for day, precip in zip(
                daily.get("time", []), daily.get("precipitation_sum", [])
            )
        ]
        conn.executemany(
            "INSERT OR REPLACE INTO observed (date, location, precip_mm) VALUES (?, ?, ?)",
            rows,
        )
        conn.commit()


def fetch_forecasts(
    conn: sqlite3.Connection,
    session: requests.Session,
    location: str,
    start: dt.date,
    end: dt.date,
    lat: float,
    lon: float,
    models: list[str],
    lead_times: list[int],
) -> None:
    all_dates = daterange(start, end)
    for model in models:
        cur = conn.execute(
            """
            SELECT date, COUNT(DISTINCT lead_time) FROM forecast
            WHERE location = ? AND model = ? AND date BETWEEN ? AND ?
            GROUP BY date
            """,
            (location, model, start.isoformat(), end.isoformat()),
        )
        complete = {
            dt.date.fromisoformat(row[0]) for row in cur if row[1] >= len(lead_times)
        }
        for chunk_start, chunk_end in missing_ranges(all_dates, complete):
            log.info(
                "Fetching %s previous-run forecasts (%s) %s..%s",
                model,
                location,
                chunk_start,
                chunk_end,
            )
            hourly_vars = ",".join(
                f"precipitation_previous_day{lt}" for lt in lead_times
            )
            data = fetch_json(
                session,
                PREVIOUS_RUNS_URL,
                {
                    "latitude": lat,
                    "longitude": lon,
                    "hourly": hourly_vars,
                    "models": model,
                    "start_date": chunk_start.isoformat(),
                    "end_date": chunk_end.isoformat(),
                    "timezone": "UTC",
                },
            )
            rows = []
            if data.get("error") or "hourly" not in data:
                # Record the whole chunk as attempted-but-unavailable (NULL)
                # so future runs don't keep re-requesting known gaps
                # (e.g. a model added after chunk_start).
                log.warning(
                    "previous-runs API error for %s %s..%s: %s",
                    model,
                    chunk_start,
                    chunk_end,
                    data.get("reason", "no hourly data returned"),
                )
                for d in daterange(chunk_start, chunk_end):
                    for lt in lead_times:
                        rows.append((d.isoformat(), location, model, lt, None))
            else:
                times = data["hourly"]["time"]
                for lt in lead_times:
                    values = data["hourly"].get(
                        f"precipitation_previous_day{lt}", [None] * len(times)
                    )
                    by_day: dict[str, list] = {}
                    for t, v in zip(times, values):
                        by_day.setdefault(t.split("T")[0], []).append(v)
                    for day, values_for_day in by_day.items():
                        if any(v is None for v in values_for_day):
                            precip = None
                        else:
                            precip = round(sum(values_for_day), 3)
                        rows.append((day, location, model, lt, precip))
            conn.executemany(
                """
                INSERT OR REPLACE INTO forecast (date, location, model, lead_time, precip_mm)
                VALUES (?, ?, ?, ?, ?)
                """,
                rows,
            )
            conn.commit()


# ---------------------------------------------------------------------------
# Metrics
# ---------------------------------------------------------------------------
def load_joined(
    conn: sqlite3.Connection, location: str, start: dt.date, end: dt.date
) -> pd.DataFrame:
    query = """
        SELECT f.date AS date, f.model AS model, f.lead_time AS lead_time,
               f.precip_mm AS forecast_mm, o.precip_mm AS observed_mm
        FROM forecast f
        JOIN observed o ON o.date = f.date AND o.location = f.location
        WHERE f.location = ? AND f.date BETWEEN ? AND ?
    """
    return pd.read_sql_query(
        query, conn, params=(location, start.isoformat(), end.isoformat())
    )


def compute_metrics(df: pd.DataFrame, thresholds: list[float]) -> pd.DataFrame:
    df = df.dropna(subset=["forecast_mm", "observed_mm"])
    results = []
    for (model, lead_time), group in df.groupby(["model", "lead_time"]):
        f = group["forecast_mm"]
        o = group["observed_mm"]
        row = {
            "model": model,
            "lead_time": lead_time,
            "n": len(group),
            "mae_mm": (f - o).abs().mean(),
            "bias_mm": (f - o).mean(),
        }
        for t in thresholds:
            f_event = f > t
            o_event = o > t
            hits = int((f_event & o_event).sum())
            misses = int((~f_event & o_event).sum())
            false_alarms = int((f_event & ~o_event).sum())
            n = len(group)
            denom_csi = hits + misses + false_alarms
            row[f"csi_{t}mm"] = hits / denom_csi if denom_csi else float("nan")
            hits_random = (hits + misses) * (hits + false_alarms) / n if n else 0.0
            denom_ets = hits + misses + false_alarms - hits_random
            row[f"ets_{t}mm"] = (
                (hits - hits_random) / denom_ets if denom_ets else float("nan")
            )
        results.append(row)
    metrics = pd.DataFrame(results)
    return metrics.sort_values(["model", "lead_time"]).reset_index(drop=True)


# ---------------------------------------------------------------------------
# Plotting
# ---------------------------------------------------------------------------
def plot_skill_degradation(
    metrics: pd.DataFrame, out_path: Path, csi_threshold: float, location: str
) -> None:
    fig, (ax_mae, ax_csi) = plt.subplots(1, 2, figsize=(12, 5))
    csi_col = f"csi_{csi_threshold}mm"

    for model, group in metrics.groupby("model"):
        group = group.sort_values("lead_time")
        ax_mae.plot(group["lead_time"], group["mae_mm"], marker="o", label=model)
        if csi_col in group:
            ax_csi.plot(group["lead_time"], group[csi_col], marker="o", label=model)

    ax_mae.set_xlabel("Lead time (days)")
    ax_mae.set_ylabel("MAE (mm)")
    ax_mae.set_title("Forecast error vs. lead time")
    ax_mae.grid(True, alpha=0.3)

    ax_csi.set_xlabel("Lead time (days)")
    ax_csi.set_ylabel(f"CSI (>{csi_threshold}mm)")
    ax_csi.set_title(f"Threshold skill (>{csi_threshold}mm) vs. lead time")
    ax_csi.grid(True, alpha=0.3)
    ax_csi.legend(loc="best", fontsize="small")

    fig.suptitle(f"Precipitation forecast skill degradation — {location}")
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------
def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--start-date", type=dt.date.fromisoformat, default=DEFAULT_START_DATE
    )
    parser.add_argument(
        "--end-date", type=dt.date.fromisoformat, default=dt.date.today()
    )
    parser.add_argument(
        "--location-name",
        default=DEFAULT_LOCATION_NAME,
        help="Short label used to key the cache and name the output subdirectory",
    )
    parser.add_argument("--latitude", type=float, default=ADLERSHOF_LATITUDE)
    parser.add_argument("--longitude", type=float, default=ADLERSHOF_LONGITUDE)
    parser.add_argument("--models", nargs="+", default=DEFAULT_MODELS)
    parser.add_argument("--lead-times", nargs="+", type=int, default=DEFAULT_LEAD_TIMES)
    parser.add_argument(
        "--thresholds", nargs="+", type=float, default=DEFAULT_THRESHOLDS_MM
    )
    parser.add_argument(
        "--db",
        type=Path,
        default=Path(__file__).parent / "precip_backtest_cache.sqlite",
        help="SQLite cache file",
    )
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=Path(__file__).parent / "precip_backtest_output",
        help="Output directory (a <location-name> subdirectory is created under it)",
    )
    parser.add_argument(
        "--skip-fetch",
        action="store_true",
        help="Only recompute metrics/chart from the existing cache",
    )
    parser.add_argument("-v", "--verbose", action="store_true")
    return parser.parse_args(argv)


def main() -> None:
    args = parse_args(sys.argv[1:])
    logging.basicConfig(
        level=logging.DEBUG if args.verbose else logging.INFO,
        format="%(levelname)s: %(message)s",
    )

    if args.start_date > args.end_date:
        log.error("--start-date must not be after --end-date")
        sys.exit(1)

    conn = open_db(args.db)
    try:
        if not args.skip_fetch:
            session = requests.Session()
            fetch_observed(
                conn,
                session,
                args.location_name,
                args.start_date,
                args.end_date,
                args.latitude,
                args.longitude,
            )
            fetch_forecasts(
                conn,
                session,
                args.location_name,
                args.start_date,
                args.end_date,
                args.latitude,
                args.longitude,
                args.models,
                args.lead_times,
            )
        else:
            log.info("Skipping fetch, using existing cache at %s", args.db)

        df = load_joined(conn, args.location_name, args.start_date, args.end_date)
        if df.empty:
            log.error(
                "No overlapping forecast/observed data found for %s in the requested range",
                args.location_name,
            )
            sys.exit(1)

        metrics = compute_metrics(df, args.thresholds)

        out_dir = args.out_dir / args.location_name
        out_dir.mkdir(parents=True, exist_ok=True)
        metrics_path = out_dir / "metrics.csv"
        metrics.to_csv(metrics_path, index=False)
        log.info("Wrote %s (%d rows)", metrics_path, len(metrics))

        csi_threshold = (
            args.thresholds[len(args.thresholds) // 2]
            if args.thresholds
            else DEFAULT_THRESHOLDS_MM[1]
        )
        chart_path = out_dir / "skill_degradation.png"
        plot_skill_degradation(metrics, chart_path, csi_threshold, args.location_name)
        log.info("Wrote %s", chart_path)

        with pd.option_context("display.max_columns", None, "display.width", 200):
            print(metrics.to_string(index=False))
    finally:
        conn.close()


if __name__ == "__main__":
    main()
