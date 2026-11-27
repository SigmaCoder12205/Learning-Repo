# Data analysis with pandas and numpy

Version 0.14 adds two ways to analyze measurements. Both use one long-format table, with one row per measured run and metric. The benchmark engine stays Rust and C. Python is only needed when you run `bench analyze`, and only in the interpreter you choose.

## Export measurements

```bash
bench ./program --record-history          # save runs (opt-in private history)
bench history export > runs.csv           # newest 50 saved runs, oldest first
bench history export --format jsonl --limit 200 --output runs.jsonl
```

The columns are `source,recorded_unix_seconds,case,run,status,kind,metric,unit,value`:

- `source` is the history ID, or the report file name for `bench analyze`.
- `case` is the benchmark name, or the suite or input-matrix case.
- `kind` is `bench` for built-in metrics and `validator` for metrics from [your own checks](validators.md).
- Only measured runs are exported. Warmups are not.

Load the export in pandas, R, DuckDB or a spreadsheet:

```python
import pandas as pd
runs = pd.read_csv("runs.csv")
ok = runs[runs.status == "ok"]
print(ok.pivot_table(index="source", columns="metric", values="value", aggfunc="median"))
```

## Analyze with `bench analyze`

```bash
python3 -m venv .venv && .venv/bin/pip install -r examples/requirements-analytics.txt
bench ./program --runs 20 --format json --output before.json
# ... change the program ...
bench ./program --runs 20 --format json --output after.json
bench analyze before.json after.json --interpreter .venv/bin/python
bench analyze --history --limit 20 --interpreter .venv/bin/python --plot wall.png
```

bench flattens the reports to the table above. It then runs its embedded `bench_analysis.py` with your interpreter (`python3` by default), so installing still needs only the `bench` binary. Using successful measured runs only, the analysis reports:

- **Summary** per case and metric: n, mean, median, standard deviation, CV, 5th/95th percentiles, IQR outlier count, and a 95% bootstrap interval for the median.
- **Change** from the oldest to the newest source, such as a report file or history run. It gives the percent change of medians and a 95% bootstrap interval for the ratio of medians. The verdict is `increased` or `decreased` only when that interval excludes 1. Otherwise it is `no clear change`. With three or more sources, the slope of medians is also reported.

| Option | Meaning |
| --- | --- |
| `REPORT.json ...` | Reports from `--format json --output`, `--save-baseline`, or `bench history show` |
| `--history`, `--history-dir DIR`, `--limit N` | Add the newest N saved history runs (default 50) |
| `--metric NAME` | Repeatable. The default is `wall`, `rss` and all validator metrics. `all` keeps every metric |
| `--format text\|json` | JSON gives `summary` and `trends` arrays for further processing |
| `--plot FILE.png` | Box plot of wall time per case and source. Needs matplotlib |
| `--interpreter PATH` | A Python with pandas and numpy. Virtual environments work |
| `--output FILE` | Write the analysis to a file |

If pandas or numpy is missing, the script prints install commands and bench exits with code 2. Bootstrap intervals use a fixed seed, so repeating an analysis gives the same numbers. They need at least three runs per group. A "change" is statistical evidence about these samples, not proof of a cause, and machine load still matters.

You can also run the script directly on an export: save `python/bench_analysis.py` from this repository, then run `python3 bench_analysis.py runs.csv`.

## Benchmarking pandas and numpy code

Data-analysis scripts are ordinary Python workloads. `examples/analytics_workload.py` is a deterministic pandas pipeline (join, clean, group, aggregate), and `examples/validators/check_analytics.py` checks its answer:

```bash
bench examples/analytics_workload.py --interpreter .venv/bin/python \
  --validate .venv/bin/python --validate-arg examples/validators/check_analytics.py \
  --runs 10 -- --rows 200000
```

Timing includes interpreter startup and importing pandas, often hundreds of milliseconds. Add `--python-overhead` to estimate the empty-interpreter part separately. Imports are still included in every run.
