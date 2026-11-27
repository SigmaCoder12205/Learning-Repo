# Your own checks in any language

Version 0.14 adds `--validate`. You write a small check program in any language: shell, Python, C++, Rust, Go, Node, or anything executable. bench runs it after every successful warmup and measured run, outside measured time. It decides whether the run's answer is right, and it can report its own numbers as metrics.

```bash
# Python check, run with the system interpreter
bench ./sum --input n.txt --validate python3 --validate-arg examples/validators/check_sum.py

# Any executable with a shebang
bench ./report_tool --validate examples/validators/nonempty_lines.sh

# Compiled C++ check
g++ -O2 -std=c++17 examples/validators/check_sorted.cpp -o build/check_sorted
bench ./sorter --validate build/check_sorted
```

## The protocol

| Direction | What |
| --- | --- |
| stdin | The target's complete stdout for that run |
| `BENCH_INPUT_FILE` | The stdin file replayed to the target, or `/dev/null` |
| `BENCH_STDOUT_FILE`, `BENCH_STDERR_FILE` | Readable paths to the target's full stdout and stderr |
| `BENCH_TARGET_EXIT_CODE` | The target's exit code |
| `BENCH_TARGET_WALL_SECONDS` | The run's measured wall time |
| Environment and working directory | The same as the target's, including `--env` and `--cwd` |
| Exit status | `0` passes the run. Any other exit fails it |
| stdout `bench-metric NAME VALUE [UNIT]` | Records a numeric metric for that run |
| Other stdout lines | Shown as the failure message (the first 1 KiB is kept) |

Metric names use 1 to 64 letters, digits, `_`, `-` or `.`. Values must be finite numbers. A validator may report up to 32 metrics per run, each at most once. The unit is one optional word.

## Results

A failing check gives the run status `validation_failed`, and bench exits with code 1, like other output checks. Each checked run has a `validation` object: `status` (`passed`, `failed`, `error` or `interrupted`), `exit_code`, `wall_seconds`, `metrics`, `metric_units`, `message` and `stderr_preview`. The status `error` means the validator broke rather than the target. Examples are a timeout, a crash, or a malformed metric line, and `reason` says which.

`validation_metrics` summarizes each metric over successful measured runs, with count, mean, median, min, max, standard deviation and percentiles. Text and HTML reports list these summaries and each failed check. `settings.validator` records the command and timeout. `bench history export` and `bench analyze` include validator metrics with kind `validator` (see [data analysis](analytics.md)).

The validator only runs when the target itself succeeded. A timeout, output limit, unexpected exit, or `--expect-*` mismatch keeps its own status. A failed warmup stops the experiment before measured runs. Ctrl-C during a validator interrupts the experiment as usual.

## Options

| Option | Meaning |
| --- | --- |
| `--validate PROGRAM` | Executable path, or a name looked up on the target's `PATH` |
| `--validate-arg ARG` | Repeatable argument. Relative paths resolve from `--cwd` |
| `--validate-timeout SECONDS` | Deadline per validator run (default 30) |

Paths keep virtual-environment symlinks, so `--validate .venv/bin/python` runs with that environment's packages.

In suites, `validate` is a string or an array `[program, arg...]`. A relative program path containing `/` resolves from the suite file. Arguments are passed as written, and `cwd` defaults to the suite directory. `validate_timeout` is a number:

```json
{"name": "sum", "executable": "sum", "input": "n.txt",
 "validate": ["python3", "checks/check_sum.py"], "validate_timeout": 10}
```

## Limits

- Validators add work between runs and may change cache state, like other between-run checks. Measured time never includes them.
- They run once per successful run, including warmups. They do not run for compilation, Python startup calibration, or the unmeasured input-capture trial.
- Comparisons and `--regression` thresholds do not yet use validator metrics. Use `bench analyze` to track them over time.
