# Precipitation forecast backtest

Backtests precipitation forecast accuracy for a fixed location across
weather models and lead times, using [Open-Meteo](https://open-meteo.com/)'s
free APIs (no API key required). Defaults to Adlershof, Berlin
(52.43°N, 13.53°E):

- **Previous Runs API** — historical forecast values per lead time
  (`precipitation_previous_day1..7`) for `icon_seamless`, `gfs_seamless`,
  `ecmwf_ifs025`, `meteofrance_arome_france`, `meteofrance_arome_france_hd`,
  and `ukmo_seamless`.
- **Historical Weather API** (ERA5 reanalysis) — primary ground-truth
  reference, spatially consistent with each model's grid.
- **National weather service station observations** (optional, via
  [wetterdienst](https://github.com/earthobservations/wetterdienst)) — a
  second, independent reference. Enable with `--station-ids` (and
  `--station-provider`, default `dwd`; also supports `geosphere` for
  Austria/ZAMG, and whatever other providers wetterdienst covers).

### Why two references

ERA5 is itself produced by ECMWF's model physics, so `ecmwf_ifs025`
forecasts can look artificially skillful against it just from shared-model
agreement, not necessarily real forecast accuracy — a verification bias.
An independent point observation (a weather station) doesn't share that
bias, at the cost of being a single point rather than a grid-cell average
(and of being some distance from the target coordinates). Running both
lets you compare: `metrics.csv` gets a `reference` column (`era5` /
`station`), and `delta_skill.csv` / `delta_skill_ecmwf.png` report, per
model and lead time, "ERA5 score minus station score" — a persistently
larger gap for `ecmwf_ifs025` than for the independently-developed models
is the signature of that bias (confirmed at both Adlershof and the Oetztal
Alps: `ecmwf_ifs025` has the largest gap at every lead time in both). **When
the two references disagree on model ranking, weight the station numbers
as the tie-breaker** — they're the one reference no model's physics could
have influenced.

## Usage

```bash
uv run scripts/precip_backtest.py
```

Defaults to the full history since 2024-01-01 through today, all six
models, lead times 1-7, and thresholds 0.1/1/5 mm. Results are written to
`scripts/precip_backtest_output/<location-name>/` (`adlershof` by default):

- `metrics.csv` — one row per model × lead time × reference, with
  `mae_mm`, `bias_mm`, and `csi_<t>mm` / `ets_<t>mm` for each threshold
  (`reference` is `era5` only unless `--station-ids` was given).
- `skill_degradation.png` — MAE and CSI(>1mm) vs. lead time, one line per
  model (solid=ERA5, dashed=station, when both references are present).
- `delta_skill.csv` / `delta_skill_ecmwf.png` — only written when
  `--station-ids` was given: per model/lead time, ERA5 score minus
  station score, plus an ECMWF-focused chart (see "Why two references").

Downloaded data is cached in `scripts/precip_backtest_cache.sqlite`
(SQLite), keyed per location. Re-running the script only fetches days
missing from the cache, so it's safe to run repeatedly (e.g. daily via
cron) to extend the backtest window incrementally — metrics and the chart
are always recomputed from the full cache.

Useful flags:

```bash
uv run scripts/precip_backtest.py --start-date 2024-01-01 --end-date 2024-03-01
uv run scripts/precip_backtest.py --models icon_seamless gfs_seamless --lead-times 1 2 3
uv run scripts/precip_backtest.py --skip-fetch   # recompute metrics/chart from cache only, no network

# A second location: results land in scripts/precip_backtest_output/oetztal_alps/
uv run scripts/precip_backtest.py --location-name oetztal_alps --latitude 47.07301 --longitude 10.96844

# Add the independent station reference: DWD for Adlershof (Berlin stations)...
uv run scripts/precip_backtest.py --station-ids 00433 00427

# ...or GeoSphere Austria for the Oetztal Alps
uv run scripts/precip_backtest.py --location-name oetztal_alps --latitude 47.07301 --longitude 10.96844 \
  --station-provider geosphere --station-ids 14631 14622
```

Run `uv run scripts/precip_backtest.py --help` for the full option list.

## Notes

- Daily forecast totals per lead time are derived by summing the 24 hourly
  `precipitation_previous_dayN` values for each calendar day (UTC); a day
  is only scored if all 24 hourly values are present, otherwise it's
  treated as missing for that model/lead time and excluded from metrics.
- CSI/ETS use "greater than" threshold semantics (e.g. `>1mm`), consistent
  across forecast and observed values.
- Models with limited history (e.g. before they were added to Open-Meteo)
  will simply have fewer scored days for early lead times/dates — the
  script doesn't require full coverage.
- `meteofrance_arome_france` and `meteofrance_arome_france_hd` are
  short-range regional models with only ~day-1 forecast horizon at these
  locations; longer lead times simply won't have data for them.
- The cache is keyed by `--location-name`, so pick a distinct, stable name
  per location. Running an old cache file (from before multi-location
  support) automatically migrates it in place, tagging existing rows as
  `adlershof`.
- `--station-provider dwd --station-ids 00433 00427` gives
  Berlin-Tempelhof and "Berlin Brandenburg" — DWD's current name for the
  long-running station at the former Berlin-Schönefeld site (renamed
  after the BER airport merger; there's no station literally named
  "Schönefeld" in DWD's registry anymore). `--station-provider geosphere
  --station-ids 14631 14622` gives Umhausen (1035m) and St. Leonhard im
  Pitztal (1454m), GeoSphere Austria (ZAMG) stations bracketing the
  Oetztal Alps point's ERA5 grid-cell elevation (~1175m) from below and
  above. Values from multiple station IDs are always averaged per day
  into a single "station" reference, bracketing the target location from
  nearby stations rather than relying on just one.
