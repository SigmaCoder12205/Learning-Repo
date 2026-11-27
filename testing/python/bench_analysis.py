"""Statistics over bench measurements with pandas and numpy.

`bench analyze` flattens reports in Rust and runs this script with the chosen
interpreter. It can also be run directly on a CSV from `bench history export`:

    python3 bench_analysis.py measurements.csv [--metric wall] [--format json]

Input columns: source, recorded_unix_seconds, case, run, status, kind, metric,
unit, value. Only rows with status "ok" are analyzed.
"""

import argparse
import json
import sys

try:
    import numpy as np
    import pandas as pd
except ImportError as error:  # pragma: no cover - depends on the environment
    sys.stderr.write(
        f"bench analyze needs pandas and numpy; {error.name} is not installed in "
        f"{sys.executable}.\n"
        "  python3 -m venv .venv && .venv/bin/pip install pandas numpy\n"
        "  bench analyze --interpreter .venv/bin/python REPORT.json\n"
    )
    sys.exit(3)

BOOTSTRAP_RESAMPLES = 2000
SEED = 20261128  # fixed so repeated analyses of the same data agree


def bootstrap_median_ci(values, rng):
    """95% percentile-bootstrap interval for the median."""
    values = np.asarray(values, dtype=float)
    if len(values) < 3:
        return (np.nan, np.nan)
    samples = rng.choice(values, size=(BOOTSTRAP_RESAMPLES, len(values)), replace=True)
    low, high = np.percentile(np.median(samples, axis=1), [2.5, 97.5])
    return (float(low), float(high))


def iqr_outliers(values):
    q1, q3 = np.percentile(values, [25, 75])
    spread = q3 - q1
    return int(np.sum((values < q1 - 1.5 * spread) | (values > q3 + 1.5 * spread)))


def select(frame, metrics):
    frame = frame[frame["status"] == "ok"].copy()
    frame["value"] = pd.to_numeric(frame["value"], errors="coerce")
    frame = frame.dropna(subset=["value"])
    if "all" in metrics:
        return frame
    if metrics:
        return frame[frame["metric"].isin(metrics)]
    return frame[(frame["kind"] == "validator") | frame["metric"].isin(["wall", "rss"])]


def summarize(frame, rng):
    rows = []
    keys = ["case", "kind", "metric", "unit"]
    for key, group in frame.groupby(keys, sort=True, dropna=False):
        values = group["value"].to_numpy(dtype=float)
        mean = float(np.mean(values))
        std = float(np.std(values, ddof=1)) if len(values) > 1 else 0.0
        low, high = bootstrap_median_ci(values, rng)
        rows.append(
            dict(zip(keys, key))
            | {
                "n": len(values),
                "sources": group["source"].nunique(),
                "mean": mean,
                "median": float(np.median(values)),
                "std": std,
                "cv_percent": std / mean * 100 if mean else 0.0,
                "p05": float(np.percentile(values, 5)),
                "p95": float(np.percentile(values, 95)),
                "median_ci95_low": low,
                "median_ci95_high": high,
                "iqr_outliers": iqr_outliers(values),
            }
        )
    return pd.DataFrame(rows)


def trends(frame, rng):
    """Change between the oldest and newest source of each case and metric."""
    rows = []
    for (case, kind, metric, unit), group in frame.groupby(
        ["case", "kind", "metric", "unit"], sort=True, dropna=False
    ):
        order = (
            group.groupby("source")["recorded_unix_seconds"]
            .min()
            .sort_values(kind="stable", na_position="last")
        )
        if len(order) < 2:
            continue
        medians = [group.loc[group["source"] == s, "value"].median() for s in order.index]
        first = group.loc[group["source"] == order.index[0], "value"].to_numpy(float)
        last = group.loc[group["source"] == order.index[-1], "value"].to_numpy(float)
        ratio_low = ratio_high = np.nan
        if len(first) >= 3 and len(last) >= 3 and np.median(first) != 0:
            a = rng.choice(first, size=(BOOTSTRAP_RESAMPLES, len(first)), replace=True)
            b = rng.choice(last, size=(BOOTSTRAP_RESAMPLES, len(last)), replace=True)
            denominators = np.median(a, axis=1)
            ratios = np.median(b, axis=1)[denominators != 0] / denominators[denominators != 0]
            if len(ratios):
                ratio_low, ratio_high = np.percentile(ratios, [2.5, 97.5])
        if np.median(first) == 0:
            verdict = "oldest median is zero; ratio undefined"
        elif np.isnan(ratio_low):
            verdict = "too few runs per source for an interval"
        elif ratio_low > 1:
            verdict = "increased"
        elif ratio_high < 1:
            verdict = "decreased"
        else:
            verdict = "no clear change"
        slope = float(np.polyfit(np.arange(len(medians)), medians, 1)[0]) if len(medians) > 2 else np.nan
        rows.append(
            {
                "case": case,
                "kind": kind,
                "metric": metric,
                "unit": unit,
                "sources": len(order),
                "first_source": order.index[0],
                "latest_source": order.index[-1],
                "first_median": float(medians[0]),
                "latest_median": float(medians[-1]),
                "change_percent": (medians[-1] / medians[0] - 1) * 100 if medians[0] else np.nan,
                "ratio_ci95_low": float(ratio_low),
                "ratio_ci95_high": float(ratio_high),
                "slope_per_source": slope,
                "verdict": verdict,
            }
        )
    return pd.DataFrame(rows)


def plot(frame, path):
    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        sys.stderr.write("--plot needs matplotlib: .venv/bin/pip install matplotlib\n")
        return False
    wall = frame[(frame["kind"] == "bench") & (frame["metric"] == "wall")]
    if wall.empty:
        sys.stderr.write("--plot: no wall-time samples selected\n")
        return False
    groups = list(wall.groupby(["case", "source"], sort=False))
    figure, axis = plt.subplots(figsize=(max(6, len(groups) * 1.2), 4.5))
    axis.boxplot([g["value"].to_numpy(float) for _, g in groups])
    short = lambda text: text if len(text) <= 18 else "…" + text[-10:]
    labels = [f"{short(c)}\n{short(s)}" for (c, s), _ in groups]
    axis.set_xticks(range(1, len(groups) + 1), labels, fontsize=7, rotation=30, ha="right")
    axis.set_ylabel("wall time (s)")
    axis.set_title("bench wall time per case and source")
    figure.tight_layout()
    figure.savefig(path, dpi=120)
    return True


def records(frame):
    return json.loads(frame.to_json(orient="records")) if not frame.empty else []


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("data", help="tidy CSV of measurements")
    parser.add_argument("--metric", action="append", default=[])
    parser.add_argument("--format", choices=["text", "json"], default="text")
    parser.add_argument("--plot")
    args = parser.parse_args(argv)

    raw = pd.read_csv(args.data, dtype={"source": str, "case": str, "unit": str}, keep_default_na=False)
    raw["recorded_unix_seconds"] = pd.to_numeric(raw["recorded_unix_seconds"], errors="coerce")
    frame = select(raw, args.metric)
    if frame.empty:
        sys.stderr.write("bench analyze: no successful samples match the selected metrics\n")
        return 4
    rng = np.random.default_rng(SEED)
    summary = summarize(frame, rng)
    change = trends(frame, rng)
    plotted = plot(frame, args.plot) if args.plot else None
    if args.plot and not plotted:
        return 4

    if args.format == "json":
        json.dump(
            {
                "kind": "bench_analysis",
                "pandas": pd.__version__,
                "numpy": np.__version__,
                "method": "successful measured runs only; 95% percentile bootstrap with a fixed seed",
                "summary": records(summary),
                "trends": records(change),
                "plot": args.plot,
            },
            sys.stdout,
            indent=2,
        )
        sys.stdout.write("\n")
        return 0

    pd.set_option("display.width", 160)
    pd.set_option("display.max_columns", 20)
    print(f"bench analyze | pandas {pd.__version__} | numpy {np.__version__}")
    print(f"{len(frame)} samples from {frame['source'].nunique()} source(s); successful measured runs only.\n")
    print("Summary (median 95% CI is a percentile bootstrap; needs at least 3 runs)")
    columns = ["case", "kind", "metric", "unit", "n", "median", "median_ci95_low", "median_ci95_high", "mean", "cv_percent", "iqr_outliers"]
    print(summary[columns].to_string(index=False, float_format=lambda v: f"{v:.6g}"))
    if not change.empty:
        print("\nChange from the oldest to the newest source (ratio CI excludes 1 => verdict)")
        columns = ["case", "metric", "sources", "first_median", "latest_median", "change_percent", "ratio_ci95_low", "ratio_ci95_high", "verdict"]
        print(change[columns].to_string(index=False, float_format=lambda v: f"{v:.6g}"))
    if plotted:
        print(f"\nPlot saved to {args.plot}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
