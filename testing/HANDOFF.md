# bench project handoff

Last updated: **2026-11-12 UTC**. This is the continuity record requested by the user for a new Codex chat.

This document combines the available conversation history with the checked-in implementation and guides. It records the sequence and decisions we can establish; it is not a verbatim transcript of every earlier assistant response. Where exact early implementation details are unavailable, they are not invented. Current source and new user instructions take precedence over this snapshot.

## Read this first

- Project: **bench**, formerly **cppbench**, a Linux CLI for benchmarking native executables and Python scripts.
- Checkout: `/home/arch/Projects/Learning-Repo/testing`, inside the larger `Learning-Repo` Git repository.
- Current package/installed version: **0.11.0**. The engine remains **Rust + C**; Python is a supported workload, not a replacement implementation.
- Main command: `bench ./program` or `bench script.py`. Default report is simple; `--intermediate` and `--advanced` add detail.
- Installation: `~/.local/bin/bench`; `~/.local/bin/cppbench` is a compatibility symlink to `bench`.
- Account authority: local **Keycloak + PostgreSQL**, not a Rust password database. Benchmarks do not require login or Docker.
- The account realm, OAuth client, database volume, and credential-cache paths intentionally retain `cppbench` identifiers. Do not mechanically rename or reset them.
- The latest feature request produced version 0.11: runtime insights, Python startup calibration, histograms, local history/pins, advisory regressions, and interruptible cooldowns. See history item 12 and `docs/features.md`; deferred brainstorm ideas are in `docs/roadmap.md`.
- Last full verification: **87 tests passed** on 2026-11-12, plus Rust formatting, Clippy with warnings denied, and C/C++ header checks. Prior counts of 77/78 below describe earlier versions/tasks.

## User intent and working preferences

The user wanted a flexible benchmark program for heavy C++ codebases: pass an executable, get useful reports, and select metrics with flags. They explicitly requested Rust and C for the engine rather than a Python benchmarking wrapper. They want a straightforward terminal experience and documentation that can grow alongside the app.

They asked for a simple overview of time, memory, CPU, GPU availability, and a few other results, with progressively deeper intermediate and advanced views. They also want specialized domain flags to add relevant metrics to the normal report.

They originally requested at least 20 main domain flags with 7 subflags each, then explicitly reduced that to **10 main flags with 3 subflags each**. They supplied developer-role examples and asked for web research into real problems and useful measurements. The implemented set consolidates related roles; it does not claim to implement a distinct platform profiler for every job title.

For accounts, the user first accepted a local implementation, then explicitly requested a dedicated account system instead of Rust account/password logic. They asked to delete the old Rust accounts, retain registration/login/whoami/logout and related actions, and make accounts persistent rather than expiring. The replacement is Keycloak with durable PostgreSQL storage and long-lived, revocable CLI authorization.

They have repeatedly asked to continue implementation after interruptions. Work toward the requested outcome, avoid unnecessary permission loops, and explain genuine dependencies when needed. Their most recent feature request was to rename the CLI to `bench` and add Python support; both are complete.

## History, in order

### 1. Initial benchmarking tool and native implementation

The initial request was to pass C++ programs into a flexible benchmark tool, collect detailed statistics, select results with flags, and write documentation. The user then explicitly required the engine to be written in Rust and C, with a terminal command `cppbench PATH_TO_EXECUTABLE`.

The historical documentation refers to an earlier Python-package interface. That implementation is not the current application. Do not revive a `python3 -m cppbench` or `python3 -m bench` interface as though it were the installed CLI.

The native implementation established executable benchmarking, optional C++ source compilation, repeated runs and warmups, resource accounting, argument/environment handling, input replay, correctness checks, bounded execution, and reports.

### 2. Report levels and optional diagnostic tools

The user asked for an accessible overview, then intermediate and advanced views. The views now share the same measurements while presenting different levels of text detail.

The user asked which packages to install and then said those packages were installed. Optional integrations were added for perf counters, Intel GPU activity, heaptrack, and Valgrind; CPU hotspots were added with later experiment features. Exact original package-install advice is not fully available in this conversation, so use the dependency/tool section below rather than assuming every tool currently works.

In a reported **0.4.0** run, the user had applied `cap_perfmon=ep` to `/usr/bin/intel_gpu_top`. A very short calculator workload produced `intel_gpu_top did not return a sample array`. GPU parsing now handles multiple JSON shapes/stream fragments and distinguishes malformed output, insufficient samples, unavailable permissions, and missing target attribution. A CPU-only calculator is not expected to provide meaningful target GPU measurements merely because monitoring is enabled.

### 3. Experiment features, documented as 0.5

The user asked how to improve the app and authorized implementation. The current implementation includes saved baselines, comparison/regression gates, named workload suites, adaptive sampling, minimum measured duration, CPU affinity, progress output, CPU function hotspots/flamegraphs, target-PID Intel GPU information, offline HTML reports, and durable checkpoint recovery.

These additions preserve ordinary fixed-run benchmarking and keep instrumented diagnostic replays out of baseline resource/timing statistics.

### 4. Domain profiles, documented as 0.6

The domain request was reduced from 20×7 to 10×3. The supplied roles included frontend, backend, full-stack, mobile, game development, data science/engineering, DevOps, QA, embedded, systems, cybersecurity, AI/ML, cloud, blockchain, and architecture. The selected profiles combine related needs into ten groups.

The user requested web research; the reviewed sources and design rationale are recorded in `docs/profiles.md`, including SRE monitoring, wrk2 coordinated omission, Unity profiling, Web Vitals/WCAG, Android performance, PostgreSQL EXPLAIN, PyTorch timing, ROS real-time work, fio, Clang, and strace. They are design inputs, not a statistical claim about the most common developer errors.

The user explicitly confirmed **strace was installed**. Networking gained automatic socket tracing in a separate replay. Application-specific metrics use validated telemetry; the app does not invent request latency, FPS, battery use, or browser metrics from process lifetime.

### 5. Codebase organization

The user requested a general cleanup and a more manageable codebase. The current layout separates CLI/application orchestration, experiments, native execution, reports, comparisons, storage, diagnostics, profiles, and accounts. `docs/development.md` records the module map, execution boundaries, and extension workflow.

### 6. Local Rust accounts, then replacement in 0.8

A local Rust account implementation existed in **0.7**, with password records and a seven-day login implementation. The user explicitly rejected Rust for account management and asked for a dedicated system, local deployment first, and deletion of the old accounts.

**0.8** replaced the account/password database with Keycloak + PostgreSQL. The project account guide records that the default legacy account store was deleted during that upgrade, as requested; there was no migration of password hashes or account records into Keycloak. Custom legacy `accounts.json` files elsewhere, if any, are inert and are not accepted by the new CLI.

The user started Docker with `systemctl start docker` and confirmed doing so. Historical setup also involved Docker-access troubleshooting. Do not assume the daemon or socket permissions remain usable in a new session; inspect them only if the requested work needs the account service.

Accounts have no built-in expiration date. CLI authorization uses offline refresh, with an explicit distinction between permanent account records and revocable credentials/session policy. Details appear below.

### 7. Interactive calculator and EOF failure

The user reported **0.8.0** stopping during the first warmup with `output_limit` and exit `-9`, leaving `0/0` measured successes. Their calculator prompts for input; its invalid-input loop can repeatedly print when stdin reaches EOF.

Bench defaults stdin to `/dev/null`, which immediately reaches EOF. A prompt loop that ignores failed reads can exhaust the output cap. Increasing the cap is not the real solution: provide a complete finite `--input` file and ensure the target handles EOF. Current reporting explains this distinction, including replay files that also eventually reach EOF.

For the posted calculator, a suitable example input is `+`, `1`, `2`, and `q`, each on its own line. The exact input must match the target program. All warmups, measured runs, and diagnostic replays receive a fresh replay of the file.

### 8. Memory-accounting investigation and 0.9 fix

The user posted a C-style calculator and compared a **0.8.1** report of roughly 4 MiB RSS with GNU `size` showing `text=2815`, `data=672`, `bss=16`, total **3503 bytes**. They expected approximately 2 KiB and requested an accuracy fix.

Two separate issues mattered:

1. Executable section sizes are not runtime RAM. Libraries, loader, stack, buffers, and resident mappings contribute to process RSS. The calculator build examined in that investigation linked C++ runtime libraries despite using C-style code. The documented independent launcher observation was roughly 4 MiB; compiling that source as C measured roughly 1.7–1.8 MiB on that machine. These are historical observations, not universal expected values.
2. There was a genuine measurement bug: directly forking from the growing Rust report process could contaminate the target child's peak RSS with inherited pre-exec memory. Subtracting a guessed constant would not correctly repair a high-water maximum.

**0.9** introduced a small isolated C helper in a fresh address space, which then forks and measures the target. Rust parent/first-launcher accounting is discarded. The helper is embedded and executed from a sealed Linux memory file. Tests verify that an extra 64 MiB held by the Rust parent does not inflate the target peak, and that a target's short-lived touched allocation is reflected even after free.

Do not undo this isolation or relabel GNU `size` as runtime memory. See `docs/memory.md` for the full accounting scope and evidence.

### 9. Rename and Python support in 0.10, completed 2026-11-11

The user requested **cppbench → bench**, plus Python benchmarking. The package, installed executable, CLI messages/help, docs, reports, native symbol names, and new artifact-directory default were renamed.

Python support includes positional `.py` detection, explicit `--python SCRIPT`, `--interpreter`, repeatable `--python-arg`, target arguments after `--`, environment/cwd/input replay, correctness checks, the existing bounded native runner, mixed suites, report metadata, script hashing, and interpreter-aware comparison checks.

Virtual-environment interpreter symlinks are deliberately preserved. Canonicalizing `.venv/bin/python` to the system interpreter could lose the intended environment discovery. Scripts themselves are resolved to absolute canonical paths.

Compatibility was preserved through the old command symlink, telemetry aliases and environment variables, and unchanged account identifiers/cache. No account deletion or new registration was required for the rename.

The implementation passed 77 tests and the full checks; `make install` completed. Installed-command checks passed for native and Python targets, both command names, help, the two-case Python example suite with 10/10 runs each, and Python HTML output.

### 10. This continuity task, 2026-11-12

The user asked to write down the work so a new Codex chat can continue. This handoff and a short `AGENTS.md` pointer were added, with a README link for discovery. No application behavior or account state needs to change for this task.

### 11. General cleanup, 2026-11-12

The user requested a quick general cleanup and verification. Existing README/continuity edits and the unrelated parent-repository calculator edit were preserved. Build outputs and diagnostic artifacts were retained.

- Suite `defaults.env` is now validated before case overrides: it must be an object with string values. Previously, a case environment could silently discard an invalid default object or replace an invalid value. A regression test first reproduced the bug, then verified rejection before target execution for null, array, string, and non-string value inputs. Valid environment merging remains covered by the existing test.
- `.gitignore` now covers `.venv/`, `__pycache__/`, and Python bytecode throughout the project. EditorConfig applies two-space indentation to YAML as well as JSON.
- `make check-c` now checks both telemetry headers as C++17 using `CXX`, in addition to existing C11 checks using `CC`. Development and experiment documentation reflects these checks and native/Python suite configuration.
- `make verify` passed all **78 tests** (26 unit, 6 accounts, 16 CLI, 10 diagnostics, 6 experiments, 7 profiles, 7 Python), formatting, Clippy, and C/C++ checks. Project-scoped `git diff --check -- .` passed. Live account-service and hardware profiler checks were not needed or rerun.
- `make install` rebuilt and installed version 0.10.0 with the cleanup. Installed smoke checks passed for `bench` and `cppbench` version output, native/Python measurements, and rejection of invalid suite defaults.

### 12. Feature brainstorm and 0.11 implementation, 2026-11-12

The user requested brainstorming and implementation, suggesting runtime detection, interpreter overhead isolation, timing/signal/warmup/outlier features, charts, progress, history, warnings, and pins. Existing features were identified first, then a coherent set of additions was implemented. `docs/roadmap.md` records delivered versus future ideas; it is not a promise that deferred features exist.

- Runtime metadata classifies Python mode, ELF headers, and shebang scripts through bounded reads. No discovery executions, source-language guesses, or new implicit interpreter launching. Per-run `termination` records normal exit or signal number/common name while preserving existing status and negative exit-code semantics.
- `wall_analysis` adds MAD, up-to-eight-bin histograms, aggregate CPU/wall ratio when selected, and outlier run numbers. `wall_outlier` uses Tukey fences with at least four successful samples. Failed runs/warmups are excluded; outliers are retained. `--histogram` enables text/HTML bars. Metric selection remains respected.
- `--python-overhead` runs one empty-script warmup plus `--overhead-runs` calibration processes (default 5) after successful baseline execution. It preserves interpreter symlinks/options, env/cwd/affinity and execution limits. It does not replay the workload, pass workload arguments/input, or check workload output. Supported startup flags are explicitly validated. Calibration remains in `python_overhead`, never baseline statistics. Median subtraction is a signed estimate only, not exact isolation; RSS is never subtracted. Failure returns 1 with baseline retained; interruption preserves signal exit behavior and skips later diagnostics.
- `--cooldown-ms` (0–60000, default 0) pauses between baseline invocations outside timing. `runner::cooldown` synchronously waits with INT/TERM blocked/consumed and restores the signal mask; interruption retains completed samples. No extra threads or FFI/backend layout changes. Cooldown differences require comparison override; missing old settings mean zero.
- `--record-history` saves full reports privately and explicitly. Default storage is `$XDG_DATA_HOME/bench/history` or `~/.local/share/bench/history`; `--history-dir` overrides it. `bench history [--dir DIR] list/show/pin` supports JSON, bounded listing, and named pin snapshots. New writes are atomic/non-overwriting; directories/files are private, reads bounded, symlink/hard-link entries rejected. Failed runs can be recorded but not pinned. No upload, pruning, or automatic recording. Account storage/IDs remain unchanged.
- `--compare-pin NAME` selects a named snapshot instead of `--compare FILE`. `--warn-regression METRIC=PERCENT` records `comparison.advisory` without exit 3, while missing-data/incompatibility still causes exit 2. Hard regression gates remain available simultaneously. Suites and recovery support history, pins, warnings, and histogram presentation; Python suites accept calibration options.
- The new modules are `src/{runtime,insights,history}.rs`; root subcommands now live in `cli::Command`, preserving account behavior. Reports remain schema 3 and the backend remains `isolated-c-wait4-v1`. No dependencies added. Both feature guide and roadmap are installed with the other guides.
- Verification: `make verify` passed **87 tests** (27 unit, 6 accounts, 16 CLI, 10 diagnostics, 6 experiments, 8 features, 7 profiles, 7 Python). New tests cover calibration isolation/failure/timeout/interruptions, cooldown interruption, distribution counts, runtime/signal metadata, metric filtering, history privacy/immutability, warning versus hard thresholds, suites, and recovery. Formatting, Clippy, C/C++ checks, and project-scoped whitespace checks passed. Live Keycloak/hardware profiler tests were not rerun.
- `make install` built and installed **0.11.0**. Installed smoke checks passed for both command names, native histograms, history recording/listing/pins, immutable pin rejection, warning and hard thresholds, cooldown comparison mismatch, virtual-environment Python calibration, HTML output, and mixed-suite pinned comparisons. Smoke history/venvs/reports used temporary directories; user history and account state were not changed.

## Current command and report contract

```bash
bench ./program
bench ./program --intermediate
bench ./program --advanced --runs 30 --warmup 3
bench ./program --metrics timing,memory --format json --output report.json
bench ./program --input input.txt --expect-output expected.txt
bench ./program --timeout 120 -- --size 100000
bench --source main.cpp --cxxflag=-march=native
bench --list-metrics
bench --list-profiles
```

Defaults: one warmup, ten measured runs, ten-second per-invocation timeout, combined stdout/stderr cap of **64 MiB**, expected exit 0, one declared work unit per run. `--output-limit` is in **MiB**, not bytes. `--units N` controls throughput normalization.

The 15 baseline metric keys are `wall`, `user`, `system`, `cpu`, `cpu_percent`, `rss`, `minor_faults`, `major_faults`, `input_blocks`, `output_blocks`, `voluntary_switches`, `involuntary_switches`, `stdout_bytes`, `stderr_bytes`, and `throughput`. Groups: `timing`, `memory`, `io`, `scheduling`, `all`.

Simple overview: mean time, highest peak RSS across successful runs, mean per-run CPU percentage, GPU availability/activity when collected, throughput, output sizes, variation. Intermediate adds ranges and resource breakdown. Advanced adds distribution statistics, settings, and individual runs. CPU 100% means one core; multithreaded workloads can exceed it. Filesystem counters are operations, not byte counts. Failed runs do not enter successful-run summaries.

Formats: text, JSON, CSV, standalone HTML. Views affect text presentation; exports retain selected data. Domain/diagnostic results remain separate; CSV is baseline-oriented. Reports use **schema_version 3**, with additive metadata. Suites use config **version 1**.

Benchmark exit codes: 0 success; 1 failed/incomplete measurement or required diagnostic/profile failure; 2 usage/configuration/comparison incompatibility; 3 regression threshold exceeded; signal interruption generally 128 + signal. Failure precedence and missing-data behavior matter—see `src/comparison.rs` rather than collapsing all failures into one status.

## Domain flags and collection

| Main flag | Three focus flags |
| --- | --- |
| `--networking` | `--sockets`, `--bandwidth`, `--latency` |
| `--frontend` | `--rendering`, `--interactivity`, `--accessibility` |
| `--backend` | `--api`, `--database`, `--distributed` |
| `--mobile` | `--startup`, `--responsiveness`, `--battery` |
| `--gamedev` | `--gameplay`, `--graphics`, `--game-ai` |
| `--data` | `--pipelines`, `--analytics`, `--ml` |
| `--devops` | `--ci`, `--sre`, `--containers` |
| `--qa` | `--load-testing`, `--soak`, `--correctness` |
| `--embedded` | `--realtime`, `--iot`, `--robotics` |
| `--systems` | `--compilation`, `--storage`, `--concurrency` |

A main flag selects all its topics unless subflags narrow it. Subflags require their parent. Multiple profiles combine. There are **90 supported metric keys**, listed by the CLI and in `docs/profiles.md`. `--compiler` remains the C++ compiler selector; `--compilation` is the systems topic.

Profiles append specialized sections to the normal report. Networking adds strace; graphics requests Intel monitoring; all selected profiles share a telemetry replay. A finite application/test driver writes schema-versioned JSON to `BENCH_TELEMETRY_FILE`; `BENCH_PROFILE_FLAGS` describes the selection. The runner also supplies legacy `CPPBENCH_TELEMETRY_FILE` and `CPPBENCH_PROFILE_FLAGS` for already instrumented programs.

`include/bench_telemetry.h` is the C11/C++ writer. `include/cppbench_telemetry.h` aliases the old types/functions. Python can write the same JSON with the standard library. Keys, units, finite/nonnegative values, count/byte integrality, size/sample limits, and regular-file properties are validated. Unknown/malformed data is not silently accepted.

Unavailable metrics stay unavailable rather than becoming zero. Missing telemetry normally means a partial profile; `--require-profile-data` makes missing selected metrics a failure. Domain duration regression gates compare p95. Frame-budget/deadline statistics require actual application samples. Profiles are not security audits, accessibility certification, device emulators, or hard real-time guarantees.

## Python support and boundaries

```bash
bench workload.py
bench workload.py --interpreter .venv/bin/python --python-arg=-O
bench --python scripts/extensionless_workload -- --size 10000
bench --suite examples/python-suite.json
```

Default interpreter: `python3` from the target environment's PATH, including `--env PATH=...`. Scripts need no shebang/executable bit. `--executable` explicitly selects normal native/shebang launching. Interpreter options precede the script; arguments after `--` go to the script. The app does not install Python packages or create virtual environments automatically.

Each invocation starts a **fresh full interpreter process**. Startup, imports, execution, and shutdown are measured; RSS includes Python and native libraries. Warmups do not keep imports, JIT state, or heap objects alive for later invocations. This is not a Python-function microbenchmark or automatic Python line/object profiler.

Suites use `"python": "workload.py"`, optional `"interpreter": ".venv/bin/python"`, `"python_args": ["-O"]`, and normal `"args"`. Relative file paths resolve from the suite directory; bare interpreter names use PATH. Cases are preflighted before execution. Native and Python cases may coexist.

Reports store a `python` object with script, interpreter, interpreter arguments, and scope; native reports have null. Provenance includes the main script and interpreter hashes. Script changes produce comparison warnings; changed interpreter path/hash is an incompatibility unless explicitly overridden. Imported modules, dependency versions, and shared libraries are not exhaustively fingerprinted. Keep this limit explicit.

## Native execution architecture and invariants

`src/main.rs` → `src/app.rs` → `src/benchmark.rs` / `src/suite.rs` → `src/runner.rs` → native launcher → isolated helper → target process.

`build.rs` compiles the helper and embeds its executable bytes. Rust serializes a bounded, versioned little-endian request containing argv/environment/options into a sealed memfd. `native/launcher.c` launches the helper and forwards signals. `native/helper.c` decodes the request in a clean address space. `native/measure.c` supervises the actual target and returns wait4 usage, monotonic time, status, and cleanup results. The Rust-facing C FFI and helper protocol must remain synchronized.

Backend identifier: **`isolated-c-wait4-v1`**. The rename did not change measurement semantics or this identifier. Backend-changing work must update provenance/compatibility deliberately. Baselines from the old direct-fork backend are not automatically comparable.

Only target-child measurement enters baseline summaries. Helper loading/transport are outside measured time; target launch, startup, and shutdown remain inside. RSS includes the target's small C pre-exec footprint and may include waited-for descendants. It is not post-exec-only, PSS/USS, heap size, or a simultaneous sum across an entire process tree.

The target process group is cleaned up on timeout, output-limit failure, interruption, and leader exit. Workloads should wait for their children. Processes deliberately leaving that group can escape cleanup. C signal handling is process-wide; do not add parallel in-process invocations without redesigning it. No sandbox is provided for untrusted programs.

Metrics are indexed across Rust/C; `src/metrics.rs`, the native array, and FFI result layout must agree. Keep zero distinct from unavailable data. Keep profiler/telemetry replays outside baseline statistics and correctness checks active during replays.

## Experiments and diagnostics

- `--save-baseline`, `--compare`, repeatable `--regression METRIC=PERCENT`, and `--allow-mismatch`: preserve compatibility checks and failure precedence. `--allow-mismatch` does not waive absent required metrics or failed experiments.
- `--suite`: named sequential cases; defaults merge, environments merge by key, paths are suite-relative, unknown fields fail. Build native programs first; source compilation is not a suite option.
- `--adaptive`, `--precision`, `--min-time`, `--max-runs`: bounded extra sampling; descriptive intervals/outlier counts, not proof of significance. Outliers remain in statistics.
- `--cpu-affinity`: constrain to permitted Linux CPUs without reserving them or changing frequency policy.
- `--checkpoint`: new JSONL journal, durable completed records; `--recover` reconstructs without executing. Recovery does not resume an unfinished target.
- `--perf`, `--gpu`, `--heaptrack`, `--valgrind`, `--hotspots`: separate diagnostic passes; `--all-tools` selects these five, not all domain profiles.
- `--hotspots`: perf samples/stacks, function summaries, folded stacks, flamegraph SVG; sampled shares are not exact function durations.
- `--check-tools`: identity/fingerprint discovery and basic counter probes; does not alter capabilities or sysctls.

Intel GPU collection distinguishes device-wide engines from matching exact target-PID clients. Children are not automatically attributed. Missing samples/client data is unavailable. DRM resident bytes are not dedicated VRAM. NVIDIA/AMD monitoring and alternate Xe tooling are not implemented by this integration.

Strace numbers describe recognized socket API calls/return values, not packets on the wire, TCP RTT, retransmissions, or universal HTTP latency. Unsupported batch/async transfer accounting can make byte totals unavailable. Raw traces are retained with explicit parsing limits.

## Account system: preserve this state

| Item | Current value |
| --- | --- |
| Local service | `http://127.0.0.1:8787` |
| Issuer | `http://127.0.0.1:8787/realms/cppbench` |
| Realm | `cppbench` |
| Public OAuth client | `cppbench-cli` |
| Compose project | `cppbench-auth` |
| PostgreSQL volume | `cppbench-auth_database` |
| Default callback | `http://127.0.0.1:8766/callback` |
| Credential cache | Absolute `$XDG_DATA_HOME/cppbench/accounts`, otherwise `$HOME/.local/share/cppbench/accounts` |
| Bootstrap credentials | Private ignored `auth/.env`; never paste its contents into a chat/document |

Keycloak owns registration, passwords, browser pages, account management, and revocation. The Rust modules are OAuth clients using Authorization Code + PKCE, random state, and verified userinfo; they do not hash or store account passwords. The CLI stores a private, issuer-bound offline refresh credential, with locking, atomic writes, restrictive permissions, and filesystem validation. No embedded client secret or password grant.

Accounts persist until deleted/disabled. Access tokens expire after five minutes; that does not expire the account. Maximum offline-session lifespan is disabled, but supplied inactivity timeout is ten 365-day years, renewed by credential use. Do not promise that a credential can remain unused literally forever. Revocation, deletion, lost files, and policy changes can require fresh login.

Commands: `bench account register`, `login`, `whoami` (alias `status`), `logout`, `logout --local`, `manage`, `password`, `delete`; options include `--json`, `--no-browser`, `--data-dir`, `--issuer`, and callback settings. Management/password/delete commands open browser pages; opening a page does not establish that a mutation completed. Browser identity may differ from CLI identity. Normal failed revocation preserves the credential for retry; `--local` explicitly forgets it without server revocation.

Start locally with `./auth/scripts/setup.sh`. It generates bootstrap credentials only as needed, starts Compose, and waits for readiness. Use `docker compose -f auth/compose.yaml ps` or `stop` for inspection/stopping. **Do not use `down -v` for a routine restart: it deletes database data.** Do not regenerate existing secrets casually.

`auth/realm.json` imports only when the realm is absent. Editing it does not update an existing service. The rename changed display labels in the seed, not persistent IDs; an existing browser realm may still display its old label. Any future live configuration change must preserve users and be explicitly scoped. Email verification/password recovery via email is not configured without SMTP. This is loopback development hosting, not a completed public deployment.

## File map and documentation

| Area | Files |
| --- | --- |
| Entry/CLI/orchestration | `src/main.rs`, `src/cli.rs`, `src/app.rs` |
| Workload preparation/runs | `src/benchmark.rs`, `src/python.rs`, `src/runner.rs`, `src/suite.rs` |
| Native boundary | `build.rs`, `native/launcher.c`, `native/helper.c`, `native/measure.c`, `native/measure.h` |
| Metrics/statistics/provenance | `src/metrics.rs`, `src/stats.rs`, `src/experiment.rs` |
| Reports/comparisons/recovery | `src/report.rs`, `src/html.rs`, `src/comparison.rs`, `src/storage.rs` |
| Diagnostics | `src/diagnostics/`, `src/hotspots.rs` |
| Domain catalog/collectors | `src/profiles/{catalog,mod,telemetry,network,presentation}.rs` |
| Account client | `src/accounts/{mod,browser,provider,store}.rs` |
| Local identity service | `auth/compose.yaml`, `auth/realm.json`, `auth/scripts/setup.sh` |
| Public instrumentation | `include/bench_telemetry.h`, legacy compatibility header |
| Integration tests | `tests/{cli,accounts,experiments,profiles,diagnostics,python}.rs`, fixtures |
| Example workloads | `examples/workload.cpp`, `network_profile.cpp`, `workload.py`, suite JSON files |

Read [README.md](README.md) for usage; [development](docs/development.md) for code/workflow; [experiments](docs/experiments.md) for suites/comparisons/recovery; [profiles](docs/profiles.md) for the complete catalog and research; [accounts](docs/accounts.md) for identity setup; [memory](docs/memory.md) for RSS; [Python](docs/python.md) for interpreter behavior.

## Dependencies, builds, and validation

Build: Linux, Rust **1.89+**, Cargo, C compiler/linker. Tests also require g++, Python 3 with standard-library venv support, and normal Linux utilities. Python target dependencies belong in the user's chosen environment. The engine requires no Python runtime for native-only operation.

Optional diagnostics use `perf`, `intel_gpu_top`, `heaptrack`, `heaptrack_print`, `valgrind`, and `strace`. Availability and counter permissions are separate. The user previously installed tools and changed Intel monitoring capability, but this is not a current permission guarantee. Accounts need Docker + Compose and a browser; `xdg-open` is optional convenience. Do not require accounts or optional profilers for ordinary benchmarks/tests.

```bash
cd /home/arch/Projects/Learning-Repo/testing
git status --short
make fmt
make verify
make install
bench --version
bench /bin/true --runs 2 --warmup 0
bench examples/workload.py --runs 2 --warmup 0 --intermediate
bench --suite examples/python-suite.json
```

`make verify` runs formatting, `cargo clippy --locked --all-targets -- -D warnings`, C/header syntax checks with warnings denied, and `cargo test --locked`. `make install` builds release with LTO, installs the binary, compatibility symlink, both headers, and profile/account/memory/Python guides under `~/.local`. `PREFIX` and `DESTDIR` are supported. Release builds can take a few minutes. No Cargo dependency additions were needed for Python support.

Focused tests: `cargo test --locked --test python`, `--test cli`, `--test profiles`, `--test experiments`, `--test diagnostics`, `--test accounts`; parser/statistics/measurement tests also live in their Rust modules. Run validation appropriate to a change rather than rerunning expensive checks without a reason.

Optional real account-service integration: `cargo build --locked`, then `python3 auth/tests/smoke.py --restart` with the development service available. It creates/cleans a unique test account and restarts Keycloak, so it is not a read-only diagnostic. Ordinary tests mock HTTP and require no Docker/browser. No real account-service integration was rerun for the rename.

Previous 77-test breakdown: 26 unit tests, 6 accounts, 16 CLI, 10 diagnostics, 5 experiments, 7 profiles, 7 Python. Python tests cover arguments/input/environment/cwd, virtual environments, invalid options, failures/timeouts/output limits, mixed-suite preflight, comparisons, and telemetry. Compatibility tests cover the old header and environment variables.

Generated artifacts: `target/`, `build/`, `dist/`, and new `bench-artifacts/`; old `cppbench-artifacts/` remains ignored and untouched. Do not delete retained reports or diagnostic evidence as generic cleanup.

## Repository snapshot and safe continuation

At the start of this documentation task, branch was `main`, repository HEAD was `1912694`, and the latest commit touching this project was `5e35777` dated 2026-11-11. `testing/` was clean: the previous feature work had been committed between sessions. There was an unrelated user edit at `../projects/temp_conversion/temp_conversion.cpp`; leave it alone. Earlier sessions had other parent-repository edits, but do not treat those historical dirty lists as current state.

Installed command/version and the `cppbench -> bench` symlink were rechecked on 2026-11-12. Service health, current credentials, optional profiler permissions, and fresh benchmark/test results were not revalidated for this documentation-only task. Do not infer them from old successful runs.

To continue in a new chat:

1. Read `AGENTS.md`, this handoff, and the guide relevant to the user's next request.
2. Inspect current Git state and changed source. Use current evidence over this snapshot.
3. Keep unrelated edits intact; do not reset the repo or account database to recreate an old state.
4. Implement the next requested improvement, preserve measurement/data compatibility, and test the affected behavior.
5. When changing the installed CLI, build/install as appropriate and verify the command the user will actually run. Do not claim deployment or installation without checking it.
6. Update this record with the new behavior, decisions, validation, and any genuinely unfinished work. Do not claim there is a queued feature merely because a limitation is documented.

Version 0.11 implements the latest requested feature set described in history item 12. Future candidates are documented in `docs/roadmap.md`; broader runtime adapters, paired A/B runs, animated spinners, Python line profiling, broader GPU support, SMTP, and remote account deployment remain unimplemented.
