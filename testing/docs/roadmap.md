# Feature brainstorm and delivery map

Updated 2026-11-19. The user requested a broad feature brainstorm followed by implementation. This list distinguishes delivered behavior from future candidates.

| Idea | State | Decision |
| --- | --- | --- |
| Wall vs CPU time, warmups, exit/signal monitoring | Existing; extended in 0.11 | Keep measurement semantics; add termination metadata and aggregate CPU/wall explanation. |
| Runtime detection | Delivered in 0.11 | Identify Python mode, ELF headers, and shebang declarations without discovery executions or source-language guesses. |
| Interpreter overhead isolation | Startup estimate delivered in 0.11 | Separate empty-script calibration; preserve full-process timing. Exact isolation requires instrumentation. |
| Outlier detection | Existing counts; extended in 0.11 | Add run labels, MAD spread, and variability hints; retain every successful sample. |
| ASCII charts/histograms | Delivered in 0.11 | Bounded-width histogram with raw bin counts and JSON data; existing HTML sample chart remains. |
| Local run history and baseline pinning | Delivered in 0.11 | Opt-in private snapshots and immutable named pins, independent of accounts. |
| Regression warnings | Delivered in 0.11 | Advisory thresholds alongside hard gates with compatibility protections. |
| Workload spacing | Delivered in 0.11 | Interruptible cooldown, excluded from timing and included in compatibility checks. |
| Live progress spinner | Deferred | Per-run progress and calibration/cooldown messages remain. Continuous animation must preserve the single-threaded native launch/signal boundary and support redirected stderr. |
| Paired A/B experiments | Future candidate | Alternate or randomize two builds to reduce order effects; retain pairing, seed, provenance, and appropriate statistics. |
| Reproducibility manifests | Future candidate | Explicit dependency/lockfile hashes, selected runtime versions, and build commands without secret environment values. |
| History tags and retention policy | Future candidate | Search by workload/tag and explicit pruning with pins protected; never silently delete diagnostic evidence. |
| Node/JVM/.NET runtime adapters | Future candidate | Separate startup and steady-state semantics, preserve runtime arguments, and add dependency provenance. |
| Warmup convergence diagnostics | Future candidate | Plot warmup trends; fresh Python processes cannot retain imports/JIT state across runs. |
| Machine-condition snapshots | Future candidate | Permitted CPU frequency, thermal and power-state observations with source/scope labels. |
| Resource budgets | Future candidate | Optional memory/process limits with clear failure reasons; requires cgroup/platform design and descendant tests. |
| Workload parameter sweeps | Stdin/argument matrices delivered in 0.12 | Named parameter schemas and scaling plots remain future candidates. |

See [Runtime insights and local history](features.md) for delivered behavior. Version 0.11 preserves the Rust + isolated C engine, account identities, telemetry compatibility, and report schema version 3.

## Input workflow delivered in 0.12

Source/prompt type hints, answer-once terminal capture, inline stdin, bounded inferred examples, explicit stdin/argument matrices, output previews, and separate case reports are implemented. See [the input guide](inputs.md). Exhaustive input-schema discovery, branch-aware generation, and raw-terminal replay remain outside this heuristic workflow. The isolated C measurement backend and schema version 3 remain unchanged.

## Reliability brainstorm and delivery in 0.13

The next priorities came from the input-testing workflow: knowing that a program exits successfully is different from checking its answers, and a large suite should be easy to narrow while developing.

| Idea | State | Scope |
| --- | --- | --- |
| Inline expected answers | Delivered | Exact stdout text and repeated required literal substrings; full-output streaming checks, not preview matching. |
| Useful mismatch details | Delivered | Actual/expected byte counts, first differing byte, and missing substring indices in reports. |
| Output repeatability | Delivered | Opt-in full-stdout fingerprints across successful measured runs; separate per case, retained timing samples, failure exit when unstable. |
| Suite discovery and selection | Delivered | Validate/list without executing; select exact names while retaining full preflight and suite order. |
| Input-hint accuracy cleanup | Delivered | Identifier/word boundaries, yes/no suggestions, and scanf format extraction; no claim of general semantic analysis. |
| Saved interactive input sessions | Future candidate | Explicitly save/replay named answer sequences, with overwrite protection and branch-aware editing. |
| Numeric/JSON assertions | Future candidate | Structured values, absolute/relative tolerances, and clear NaN/infinity semantics. |
| Warmup trend diagnostics | Future candidate | Report cache/startup trends separately without claiming process-local state persists. |
| Paired build comparisons | Future candidate | Alternate builds, preserve pairing/order/seed, and compare matched workloads. |

See [correctness and focused suites](correctness.md) and the runnable `examples/correctness-suite.json`. No new dependencies or timed-backend changes were needed. General semantic input discovery, random fuzzing, saved sessions, and tolerance-based comparisons are not implemented by these features.

## Custom checks and data analysis brainstorm, 0.14

Updated 2026-11-28. The user asked for checks written in any language and for pandas/numpy data analysis. These were the brainstorm options:

| Idea | State | Scope |
| --- | --- | --- |
| Validators in any language (`--validate`) | Delivered | Executable check per successful run, outside timing. stdin is target stdout, exit 0 passes, and environment variables describe the run. Suites support it too. |
| Validator metrics | Delivered | `bench-metric NAME VALUE [UNIT]` lines become per-run metrics, summarized in `validation_metrics`, text and HTML. |
| Long-format export (`bench history export`) | Delivered | CSV/JSONL, one row per measured run and metric, oldest history first, for pandas, R or DuckDB. |
| `bench analyze` with pandas/numpy | Delivered | Embedded script with a user-chosen interpreter. Medians, bootstrap intervals, outliers, oldest-to-newest change verdicts, optional matplotlib plot. |
| pandas/numpy workload example | Delivered | `examples/analytics_workload.py`, with a validator for its answer. |
| Regression gates on validator metrics | Delivered in 0.15 | `--regression validator.NAME[:higher\|lower]=PCT`, with the direction from the suffix or declared by the validator. A gate without a direction is incompatible, never guessed. |
| Parquet export | Delivered in 0.15 | `history export --format parquet` and `analyze --save-data FILE.parquet`, written by pandas/pyarrow in the chosen interpreter. No Arrow code in the binary. |
| Notebook export | Delivered in 0.15 | `analyze --notebook FILE.ipynb`: a self-contained nbformat 4 notebook with embedded data. Writing it needs no Python, and every cell was executed with nbclient during verification. |
| Rank-based A/B test in Rust | Delivered in 0.15 | Mann-Whitney U, exact or normal with tie correction, on every comparison, plus `bench compare A.json B.json`. Checked against scipy. |
| JSON validator protocol | Delivered in 0.15 | `{"bench_validator": 1, ...}` with an explicit verdict, partial-credit `score`, named checks with pass-rate summaries, and metric directions. The line protocol also gained `[higher\|lower]`. |
