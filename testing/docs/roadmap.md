# Feature brainstorm and delivery map

Updated 2026-11-12. The user requested a broad feature brainstorm followed by implementation. This list distinguishes delivered behavior from future candidates.

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
| Workload parameter sweeps | Future candidate | Named cases from size/concurrency matrices with scaling plots and suite preflight. |

See [Runtime insights and local history](features.md) for delivered behavior. Version 0.11 preserves the Rust + isolated C engine, account identities, telemetry compatibility, and report schema version 3.
