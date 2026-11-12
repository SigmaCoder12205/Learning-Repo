# Runtime insights and local history (0.11)

This release extends the existing warmups, wall/CPU accounting, exit monitoring, adaptive sampling, outlier counts, progress output, and regression gates. The measurement backend remains `isolated-c-wait4-v1`; new report fields are additive within schema version 3.

## Runtime evidence and termination details

Reports identify Python workloads from the selected execution mode, ELF executables from their file header, and executable scripts from their shebang. Detection reads at most 512 bytes and does not execute a discovery probe. ELF metadata does not establish the source language or embedded runtime. Shebang metadata records the declaration rather than resolving `env` or shell wrappers. Explicit `--executable` continues to control launching; detection does not change commands.

Each invocation includes a `termination` object: either `{"kind":"exit","code":0}` or a signal number and, for common signals, its symbolic name. Existing `exit_code` conventions remain: normal exits are nonnegative and signal deaths are negative. `interrupt_signal` separately records interruption of bench itself. Text failures include signal names.

## Timing distribution and ASCII histograms

```bash
bench ./program --runs 30 --histogram
bench ./program --intermediate --histogram --format html --output report.html
bench ./program --metrics wall,cpu --format json
```

Successful wall-time samples produce `wall_analysis`, including a histogram, median absolute deviation (MAD), and run numbers outside Tukey's 1.5-IQR fences. Individual runs receive `wall_outlier`: true/false with at least four successful samples, or null when unavailable. Failed runs and warmups are excluded; outliers remain in every timing statistic. An outlier label describes the distribution and does not prove a measurement is wrong.

Histograms use up to eight equal-width bins. Bins are lower-inclusive and upper-exclusive except the last, which includes its upper bound. Identical measurements use one bin. Bars scale to the largest bin, with counts alongside them. `--histogram` enables text/HTML display; JSON always retains available analysis. CSV remains a baseline measurement table. Selecting only `rss` does not expose unselected wall-time analysis.

When both CPU and wall metrics are selected, reports show summed CPU divided by summed elapsed time over successful samples with both values. One means one core equivalent; values above one can reflect parallel CPU use. This differs from the existing average of per-run CPU percentages. It is not I/O-wait accounting or proof of a specific bottleneck. Text/HTML call out timing CV above 10% as a descriptive variability hint.

## Python startup calibration

```bash
bench workload.py --python-overhead --overhead-runs 5 --histogram
bench workload.py --interpreter .venv/bin/python --python-arg=-O --python-overhead
```

After successful workload measurements, bench creates a temporary empty Python script. One calibration warmup precedes `--overhead-runs` fresh interpreter processes (default 5, range 1–1000). Calibration uses the selected interpreter, supported interpreter options, environment, working directory, affinity, timeout, and output limit. It receives `/dev/null`, expects exit zero, and omits workload arguments and exact-output checks. Calibration does not execute the workload script again.

The separate `python_overhead` report retains calibration runs and summaries. With selected wall timing, it includes the full-workload median minus the empty-script median as `estimated_workload_wall_seconds`, plus the empty/full median ratio. The difference may be negative and the ratio may exceed 100%; these are preserved rather than clamped. **Full-process timing, RSS, and regression comparisons remain unchanged.** Memory is never subtracted.

This is a startup estimate, not exact interpreter-overhead isolation or function timing. The temporary script's directory differs from the workload's directory. Imports, module lookup, startup hooks, caches, scheduling, and interpreter implementation can change the result. Python documents that a script's directory affects `sys.path`; [`timeit`](https://docs.python.org/3/library/timeit.html) instead measures statements within an interpreter and treats setup separately. See the [Python command-line reference](https://docs.python.org/3/using/cmdline.html) for script and interpreter-option semantics.

Calibration accepts `-O`, `-OO`, `-B`, `-E`, `-I`, `-s`, `-S`, `-u`, `-q`, `-P`, `-b`, `-bb`, `-d`, `-v`, `-vv`, and `-W`/`-X` with attached or separate values. Execution selectors such as `-c`/`-m`, interactive mode, and other options are rejected before running the target. Availability of startup options still depends on the selected interpreter. `--python-overhead` requires Python mode.

A failed/timed-out calibration retains successful workload results and causes exit 1. Interrupted calibration returns the signal-based exit status and skips subsequent diagnostics. A failed workload skips calibration. Calibration samples do not enter baseline statistics, throughput, or adaptive stopping. Suites accept `python_overhead` and `overhead_runs` per Python case.

## Between-run cooldown

```bash
bench ./program --runs 20 --warmup 2 --cooldown-ms 100
```

The pause applies between baseline invocations, including the warmup-to-measurement transition. It does not apply before the first or after the final invocation, or to diagnostics/calibration. Range: 0–60000 ms; default zero. It is outside measured time and helps control workload spacing, but does not measure or guarantee a thermal state. INT/TERM during the pause preserves completed measurements. Comparisons treat different cooldown settings as incompatible unless `--allow-mismatch` is explicit; older baselines without this field mean zero.

## Private local history and baseline pins

```bash
bench ./program --name parser --record-history
bench history list
bench history list --json --limit 10
bench history show RUN_ID
bench history pin RUN_ID parser-stable
bench history list --pins
bench history show parser-stable --pin
bench ./program --compare-pin parser-stable --regression wall=5
```

Recording is opt-in and saves the full JSON report, including failed/partial experiments that reach report output. Saved IDs appear on stderr and under `history.id` in JSON. Suites are stored as one report. Recovery can also be recorded with `--recover FILE --record-history`. Abrupt termination before report output requires the existing `--checkpoint`/`--recover` workflow; history is not a replacement for journaling.

The default directory is `$XDG_DATA_HOME/bench/history` when XDG_DATA_HOME is absolute, otherwise `~/.local/share/bench/history`. Override it with `--history-dir DIR` during measurement/comparison, or `bench history --dir DIR ...` for history commands. Listing an absent store does not create it. Account storage is unchanged.

Directories are private (0700), files are private (0600), and writes use atomic, non-overwriting publication. Readers reject non-private storage, symlink/hard-link entries, and reports over 64 MiB. IDs/pin names contain 1–100 ASCII letters, digits, dashes, or underscores. Reports can contain target arguments and captured output; only record workloads whose output you want to retain. No upload, automatic pruning, or automatic recording occurs.

A pin is a snapshot copy of a completed successful run/suite, including successful requested diagnostics/calibration. Failed or partial runs cannot be pinned. Existing names are never overwritten by `history pin`; use a new name for a new baseline. Editing files manually can still change them. `--compare-pin` and `--compare FILE` are alternatives and use the same compatibility rules.

## Warning thresholds alongside hard gates

```bash
bench ./program --compare-pin parser-stable \
  --warn-regression wall=3 --regression wall=10
```

Worsening above 3% produces an advisory message; above 10% also triggers the existing exit-3 gate. Advisory results appear under `comparison.advisory` and in text/HTML. Warnings alone keep exit 0. Missing required metrics or incompatible comparisons still return exit 2, and measurement failures retain precedence. Both threshold flags support existing metric directions, including higher-is-better throughput and p95 domain durations. Thresholds are practical policies, not statistical significance tests.

See the [feature roadmap](roadmap.md) for the brainstorm and deferred work.
