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
| stdout `bench-metric NAME VALUE [UNIT] [higher\|lower]` | Records a numeric metric for that run. The optional last word declares which direction is better |
| stdout JSON object with `"bench_validator": 1` | The structured protocol below |
| Other stdout lines | Shown as the failure message (the first 1 KiB is kept) |

Metric names use 1 to 64 letters, digits, `_`, `-` or `.`. Values must be finite numbers. A validator may report up to 32 metrics per run, each at most once. The unit is one optional word.

## Structured JSON protocol

For named checks, partial credit, or metric directions, print one JSON object instead:

```json
{"bench_validator": 1,
 "passed": true,
 "score": 0.75,
 "message": "3 of 4 answers correct",
 "checks": [{"name": "has_total", "passed": true},
            {"name": "sorted", "passed": false, "message": "row 7 out of order", "score": 0.5}],
 "metrics": {"tokens": {"value": 412, "unit": "count", "better": "lower"},
             "rows": 1000}}
```

| Field | Meaning |
| --- | --- |
| `bench_validator` | Required. Must be `1` |
| `passed` | Optional. When present, it decides the verdict, so partial credit can pass. When absent, every check must pass |
| `score` | Optional partial credit from 0 to 1. It is recorded as the metric `score`, where higher is better |
| `checks` | Up to 64 named checks with `passed`, plus an optional `message` and `score`. `validation_checks` counts passes and failures per check across measured runs |
| `metrics` | A number, or `{"value", "unit", "better"}`. `better` is `"higher"` or `"lower"` |
| `message` | Shown when the run fails |

A nonzero exit still fails the run. Unknown fields, a bad version, a score outside 0 to 1, duplicate names, and a metric named `score` next to a top-level `score` are validator errors. Output that is JSON but lacks `bench_validator` is treated as a plain message.

## Results

A failing check gives the run status `validation_failed`, and bench exits with code 1, like other output checks. Each checked run has a `validation` object: `status` (`passed`, `failed`, `error` or `interrupted`), `exit_code`, `wall_seconds`, `metrics`, `metric_units`, `message` and `stderr_preview`. The status `error` means the validator broke rather than the target. Examples are a timeout, a crash, or a malformed metric line, and `reason` says which.

`validation_metrics` summarizes each metric over successful measured runs, with count, mean, median, min, max, standard deviation, percentiles, and the declared `better` direction. A JSON-protocol run also records `protocol`, `passed`, `score`, `checks` and `failed_checks`. Text and HTML reports list these summaries and each failed check. `settings.validator` records the command and timeout. `bench history export` and `bench analyze` include validator metrics with kind `validator` (see [data analysis](analytics.md)).

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
## Regression gates on validator metrics

`--compare`, `--compare-pin` and `bench compare` compare validator metrics (their medians) whenever both reports have them. Gates use the `validator.` prefix:

```bash
bench ./model --validate ./grade.py --compare before.json --regression validator.score=2
bench compare before.json after.json --regression validator.loss:lower=5 --warn-regression validator.tokens:lower=10
```

A gate needs a direction: the `:higher` or `:lower` suffix, or a `better` declared by the validator. The suffix wins if both are given. A gate on a metric with no direction makes the comparison incompatible (exit 2) rather than guessing. If the validator command itself changed, the comparison lists that as a difference.
