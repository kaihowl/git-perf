#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "requests",
#     "pandas",
#     "matplotlib",
#     "wetterdienst",
# ]
# ///
"""
Backtest precipitation forecast accuracy for a fixed location across models
and lead times, using Open-Meteo's Previous Runs API (forecasts) and two
independent ground-truth references: the Historical Weather API (ERA5
reanalysis) and, optionally, national weather service station observations
(via wetterdienst: DWD for Germany, GeoSphere Austria/ZAMG for Austria).
Defaults to Adlershof, Berlin; pass --location-name/--latitude/--longitude
for others.

Scoring against two references matters because ERA5 is itself produced by
ECMWF's own model physics: ecmwf_ifs025 can look artificially skillful
against ERA5 simply from shared-physics agreement, not real forecast
accuracy. An independent point observation (a station) doesn't have that
bias, at the cost of being a single point rather than a grid-cell average.
Comparing both surfaces this via a delta-skill table/chart.

Usage:
    uv run scripts/precip_backtest.py [options]
    uv run scripts/precip_backtest.py --station-ids 00433 00427                       # DWD (default provider)
    uv run scripts/precip_backtest.py --station-provider geosphere --station-ids 14631 14622

Data is cached (per location) in a local SQLite database so repeated runs
only fetch new/missing days instead of re-downloading everything. Only the
metrics table and chart are always recomputed from the full cache.

Output:
    <out-dir>/<location-name>/metrics.csv            model x lead_time x metric x reference table
    <out-dir>/<location-name>/skill_degradation.png   MAE/CSI vs lead time (solid=ERA5, dashed=station)
    <out-dir>/<location-name>/delta_skill.csv         per model/lead_time: ERA5 score minus station score
    <out-dir>/<location-name>/delta_skill_ecmwf.png   ECMWF-specific verification-bias chart (only
                                                       written when --station-ids is given)
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

# Station IDs for the independent --station-ids reference, per location.
# Values from multiple station IDs are averaged per day into a single
# "station" reference (bracketing the target location/elevation from
# nearby stations, since it's rarely exactly on one).
#
# Adlershof (--station-provider dwd, the default): 00433 = Berlin-Tempelhof.
# 00427 = "Berlin Brandenburg" — DWD's current name for the long-running
# station at/near the former Berlin-Schönefeld site, renamed after the BER
# airport merger; there is no station literally named "Schönefeld" in DWD's
# registry anymore, but 00427 is its continuation (continuous record since
# 1957).
ADLERSHOF_DWD_STATION_IDS = ["00433", "00427"]

# Oetztal Alps (--station-provider geosphere): 14631 = Umhausen (1035m),
# 14622 = St. Leonhard im Pitztal (1454m) — GeoSphere Austria (ZAMG)
# stations ~8.5km/12.5km from the target point, bracketing its ERA5
# grid-cell elevation (~1175m) from below and above.
OETZTAL_GEOSPHERE_STATION_IDS = ["14631", "14622"]

STATION_PROVIDERS = {
    "dwd": (
        "wetterdienst.provider.dwd.observation",
        "DwdObservationRequest",
        "climate_summary",
    ),
    "geosphere": (
        "wetterdienst.provider.geosphere.observation",
        "GeosphereObservationRequest",
        "data",
    ),
}

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
        INSERT INTO observed (date, location, source, precip_mm)
        SELECT date, '{DEFAULT_LOCATION_NAME}', 'era5', precip_mm FROM observed_legacy
        """
    )
    conn.execute("DROP TABLE forecast_legacy")
    conn.execute("DROP TABLE observed_legacy")
    conn.commit()


def _migrate_add_reference_source(conn: sqlite3.Connection) -> None:
    """Add the `source` column to pre-existing single-reference `observed`
    caches, tagging their rows as 'era5', instead of discarding
    already-fetched data."""
    tables = {
        row[0]
        for row in conn.execute("SELECT name FROM sqlite_master WHERE type='table'")
    }
    if "observed" not in tables:
        return
    cols = {row[1] for row in conn.execute("PRAGMA table_info(observed)")}
    if "source" in cols:
        return
    log.info("Migrating cache schema to support multiple reference sources")
    conn.execute("ALTER TABLE observed RENAME TO observed_legacy")
    _create_tables(conn)
    conn.execute(
        """
        INSERT INTO observed (date, location, source, precip_mm)
        SELECT date, location, 'era5', precip_mm FROM observed_legacy
        """
    )
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
            source TEXT NOT NULL,
            precip_mm REAL,
            PRIMARY KEY (date, location, source)
        )
        """
    )


def open_db(path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(path)
    _migrate_legacy_schema(conn)
    _migrate_add_reference_source(conn)
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
        "SELECT date FROM observed WHERE location = ? AND source = 'era5' AND date BETWEEN ? AND ?",
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
            (day, location, "era5", precip)
            for day, precip in zip(
                daily.get("time", []), daily.get("precipitation_sum", [])
            )
        ]
        conn.executemany(
            "INSERT OR REPLACE INTO observed (date, location, source, precip_mm) VALUES (?, ?, ?, ?)",
            rows,
        )
        conn.commit()


def fetch_station_observed(
    conn: sqlite3.Connection,
    location: str,
    start: dt.date,
    end: dt.date,
    provider: str,
    station_ids: list[str],
) -> None:
    """Fetch daily precipitation from national weather service station
    observations (via wetterdienst) and cache it as the 'station'
    reference, averaged across all given station IDs per day
    (bracketing/interpolating the target location from nearby stations,
    since it's rarely exactly on one)."""
    import importlib

    import polars as pl

    if provider not in STATION_PROVIDERS:
        raise ValueError(
            f"unknown --station-provider {provider!r}, expected one of {sorted(STATION_PROVIDERS)}"
        )
    module_name, class_name, dataset_name = STATION_PROVIDERS[provider]
    request_cls = getattr(importlib.import_module(module_name), class_name)

    all_dates = daterange(start, end)
    cur = conn.execute(
        "SELECT date FROM observed WHERE location = ? AND source = 'station' AND date BETWEEN ? AND ?",
        (location, start.isoformat(), end.isoformat()),
    )
    present = {dt.date.fromisoformat(row[0]) for row in cur}
    for chunk_start, chunk_end in missing_ranges(all_dates, present):
        log.info(
            "Fetching %s station precipitation (%s, stations %s) %s..%s",
            provider,
            location,
            ",".join(station_ids),
            chunk_start,
            chunk_end,
        )
        request = request_cls(
            parameters=[("daily", dataset_name, "precipitation_height")],
            start_date=chunk_start.isoformat(),
            end_date=chunk_end.isoformat(),
        ).filter_by_station_id(station_id=station_ids)
        values = request.values.all().df
        daily = (
            values.with_columns(pl.col("date").dt.date().alias("day"))
            .group_by("day")
            .agg(pl.col("value").mean().alias("precip_mm"))
        )
        rows = [
            (row["day"].isoformat(), location, "station", row["precip_mm"])
            for row in daily.iter_rows(named=True)
        ]
        conn.executemany(
            "INSERT OR REPLACE INTO observed (date, location, source, precip_mm) VALUES (?, ?, ?, ?)",
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
    """Joins forecasts against every reference source cached for this
    location (era5, and station if it was fetched), long-format with one
    row per (date, model, lead_time, reference)."""
    query = """
        SELECT f.date AS date, f.model AS model, f.lead_time AS lead_time,
               f.precip_mm AS forecast_mm, o.precip_mm AS observed_mm,
               o.source AS reference
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
    for (model, lead_time, reference), group in df.groupby(
        ["model", "lead_time", "reference"]
    ):
        f = group["forecast_mm"]
        o = group["observed_mm"]
        row = {
            "model": model,
            "lead_time": lead_time,
            "reference": reference,
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
    return metrics.sort_values(["model", "lead_time", "reference"]).reset_index(
        drop=True
    )


def compute_delta_skill(metrics: pd.DataFrame) -> pd.DataFrame:
    """Per model/lead_time: ERA5 score minus station score, for every
    metric column. Surfaces whether a model (especially ecmwf_ifs025,
    which shares model physics with ERA5) looks systematically better
    against ERA5 than against the independent station reference — a sign
    of verification bias rather than real forecast skill. For mae_mm,
    negative means "looks more accurate against ERA5"; for csi/ets,
    positive means "looks more skillful against ERA5". Empty if no
    station reference was fetched.
    """
    if "station" not in set(metrics["reference"]):
        return pd.DataFrame()
    metric_cols = [
        c for c in metrics.columns if c not in ("model", "lead_time", "reference", "n")
    ]
    era5 = metrics[metrics["reference"] == "era5"].set_index(["model", "lead_time"])
    station = metrics[metrics["reference"] == "station"].set_index(
        ["model", "lead_time"]
    )
    common = era5.index.intersection(station.index)
    delta = (
        era5.loc[common, metric_cols] - station.loc[common, metric_cols]
    ).add_prefix("delta_")
    delta = delta.reset_index()
    return delta.sort_values(["model", "lead_time"]).reset_index(drop=True)


# ---------------------------------------------------------------------------
# Plotting
# ---------------------------------------------------------------------------
def plot_skill_degradation(
    metrics: pd.DataFrame, out_path: Path, csi_threshold: float, location: str
) -> None:
    fig, (ax_mae, ax_csi) = plt.subplots(1, 2, figsize=(12, 5))
    csi_col = f"csi_{csi_threshold}mm"
    multi_reference = metrics["reference"].nunique() > 1
    colors = plt.rcParams["axes.prop_cycle"].by_key()["color"]
    color_of = {
        model: colors[i % len(colors)]
        for i, model in enumerate(sorted(metrics["model"].unique()))
    }
    linestyle_of = {"era5": "-", "station": "--"}

    for (model, reference), group in metrics.groupby(["model", "reference"]):
        group = group.sort_values("lead_time")
        label = f"{model} ({reference})" if multi_reference else model
        style = dict(
            marker="o",
            color=color_of[model],
            linestyle=linestyle_of.get(reference, "-"),
            label=label,
        )
        ax_mae.plot(group["lead_time"], group["mae_mm"], **style)
        if csi_col in group:
            ax_csi.plot(group["lead_time"], group[csi_col], **style)

    ax_mae.set_xlabel("Lead time (days)")
    ax_mae.set_ylabel("MAE (mm)")
    ax_mae.set_title("Forecast error vs. lead time")
    ax_mae.grid(True, alpha=0.3)

    ax_csi.set_xlabel("Lead time (days)")
    ax_csi.set_ylabel(f"CSI (>{csi_threshold}mm)")
    ax_csi.set_title(f"Threshold skill (>{csi_threshold}mm) vs. lead time")
    ax_csi.grid(True, alpha=0.3)
    ax_csi.legend(loc="best", fontsize="small")

    title = f"Precipitation forecast skill degradation — {location}"
    if multi_reference:
        title += " (solid=ERA5, dashed=station)"
    fig.suptitle(title)
    fig.tight_layout()
    fig.savefig(out_path, dpi=150)
    plt.close(fig)


def plot_delta_skill_ecmwf(
    delta_skill: pd.DataFrame, out_path: Path, csi_threshold: float, location: str
) -> None:
    """ECMWF-specific verification-bias chart: delta-skill (ERA5 minus
    station) vs. lead time. A consistent nonzero delta for ecmwf_ifs025
    (more than for the other models) suggests ERA5 is inflating/deflating
    its apparent skill via shared model physics."""
    ecmwf = delta_skill[delta_skill["model"] == "ecmwf_ifs025"].sort_values("lead_time")
    if ecmwf.empty:
        log.warning("No ecmwf_ifs025 delta-skill rows to plot, skipping %s", out_path)
        return

    fig, (ax_mae, ax_csi) = plt.subplots(1, 2, figsize=(12, 5))
    ax_mae.axhline(0, color="gray", linewidth=0.8)
    ax_mae.bar(ecmwf["lead_time"], ecmwf["delta_mae_mm"], color="tab:blue")
    ax_mae.set_xlabel("Lead time (days)")
    ax_mae.set_ylabel("Delta MAE, ERA5 minus station (mm)")
    ax_mae.set_title("MAE delta (negative = looks more accurate vs. ERA5)")
    ax_mae.grid(True, alpha=0.3)

    csi_col = f"delta_csi_{csi_threshold}mm"
    ax_csi.axhline(0, color="gray", linewidth=0.8)
    if csi_col in ecmwf:
        ax_csi.bar(ecmwf["lead_time"], ecmwf[csi_col], color="tab:blue")
    ax_csi.set_xlabel("Lead time (days)")
    ax_csi.set_ylabel(f"Delta CSI(>{csi_threshold}mm), ERA5 minus station")
    ax_csi.set_title("CSI delta (positive = looks more skillful vs. ERA5)")
    ax_csi.grid(True, alpha=0.3)

    fig.suptitle(f"ECMWF verification-bias check (ERA5 vs. station) — {location}")
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
    parser.add_argument(
        "--station-provider",
        choices=sorted(STATION_PROVIDERS),
        default="dwd",
        help="National weather service to use for the independent station reference",
    )
    parser.add_argument(
        "--station-ids",
        nargs="+",
        default=None,
        help=(
            "Optional: also score against station observations (via "
            "wetterdienst), averaged across the given station IDs. E.g. "
            f"--station-provider dwd --station-ids {' '.join(ADLERSHOF_DWD_STATION_IDS)} "
            "for Adlershof (Berlin-Tempelhof, Berlin Brandenburg/former "
            "Schoenefeld site), or --station-provider geosphere --station-ids "
            f"{' '.join(OETZTAL_GEOSPHERE_STATION_IDS)} for the Oetztal Alps "
            "(Umhausen, St. Leonhard im Pitztal). Adds a 'reference' column "
            "to metrics.csv and writes delta_skill.csv / delta_skill_ecmwf.png."
        ),
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
            if args.station_ids:
                fetch_station_observed(
                    conn,
                    args.location_name,
                    args.start_date,
                    args.end_date,
                    args.station_provider,
                    args.station_ids,
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

        delta_skill = compute_delta_skill(metrics)
        if not delta_skill.empty:
            delta_path = out_dir / "delta_skill.csv"
            delta_skill.to_csv(delta_path, index=False)
            log.info("Wrote %s (%d rows)", delta_path, len(delta_skill))

            delta_chart_path = out_dir / "delta_skill_ecmwf.png"
            plot_delta_skill_ecmwf(
                delta_skill, delta_chart_path, csi_threshold, args.location_name
            )
            log.info("Wrote %s", delta_chart_path)

        with pd.option_context("display.max_columns", None, "display.width", 200):
            print(metrics.to_string(index=False))
            if not delta_skill.empty:
                print("\nDelta-skill (ERA5 minus station):")
                print(delta_skill.to_string(index=False))
    finally:
        conn.close()


if __name__ == "__main__":
    main()
