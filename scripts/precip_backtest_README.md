# Precipitation forecast backtest (Adlershof, Berlin)

Backtests precipitation forecast accuracy for Adlershof, Berlin (52.43°N,
13.53°E) across weather models and lead times, using
[Open-Meteo](https://open-meteo.com/)'s free APIs (no API key required):

- **Previous Runs API** — historical forecast values per lead time
  (`precipitation_previous_day1..7`) for `icon_seamless`, `gfs_seamless`,
  `ecmwf_ifs025`, `meteofrance_arome_france`, `meteofrance_arome_france_hd`,
  and `ukmo_seamless`.
- **Historical Weather API** (ERA5 reanalysis) — ground truth daily
  precipitation.

## Usage

```bash
uv run scripts/precip_backtest.py
```

Defaults to the full history since 2024-01-01 through today, all six
models, lead times 1-7, and thresholds 0.1/1/5 mm. Results are written to
`scripts/precip_backtest_output/`:

- `metrics.csv` — one row per model × lead time, with `mae_mm`, `bias_mm`,
  and `csi_<t>mm` / `ets_<t>mm` for each threshold.
- `skill_degradation.png` — MAE and CSI(>1mm) vs. lead time, one line per
  model.

Downloaded data is cached in `scripts/precip_backtest_cache.sqlite`
(SQLite). Re-running the script only fetches days missing from the cache,
so it's safe to run repeatedly (e.g. daily via cron) to extend the
backtest window incrementally — metrics and the chart are always
recomputed from the full cache.

Useful flags:

```bash
uv run scripts/precip_backtest.py --start-date 2024-01-01 --end-date 2024-03-01
uv run scripts/precip_backtest.py --models icon_seamless gfs_seamless --lead-times 1 2 3
uv run scripts/precip_backtest.py --skip-fetch   # recompute metrics/chart from cache only, no network
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
  short-range regional models with only ~day-1 forecast horizon at this
  location; longer lead times simply won't have data for them.
