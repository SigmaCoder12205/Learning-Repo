# bench

Continuing development in a new chat? Read the [project history and handoff](HANDOFF.md) and [development guide](docs/development.md).

Version **0.11** adds runtime identification, separate Python startup calibration, ASCII histograms, per-run outlier labels, private local history, named baseline pins, warning-only regression thresholds, and interruptible cooldowns. See the [feature guide](docs/features.md) and [brainstorm/roadmap](docs/roadmap.md).

```bash
bench workload.py --python-overhead --histogram --record-history
bench history list
bench history pin RUN_ID stable
bench workload.py --compare-pin stable --warn-regression wall=3 --regression wall=10
```

A native **Rust + C** command-line benchmark tool for Linux. Pass an executable or Python script path for a simple overview:

```bash
bench /path/to/your/program
bench workload.py
```

The default is one warmup followed by ten measured invocations. The overview shows average time, highest peak memory, average CPU usage, GPU availability, throughput, output sizes, and timing variation. Use `--intermediate` for resource ranges or `--advanced` for all 15 metrics, full statistics, and individual runs.

Rust handles the CLI, statistics, correctness checks, and reports. A small, isolated C helper handles process launch, monotonic timing, Linux `wait4` resource accounting, deadlines, and process-group cleanup. Its executable is embedded in bench and run from a sealed Linux memory file, so installation still needs only the bench executable. The benchmark engine itself needs no Python runtime. Python workloads run through your selected interpreter.

Version **0.10** renames `cppbench` to `bench` and adds Python script support, interpreter selection, and mixed native/Python suites. `make install` also installs a `cppbench` compatibility symlink. Existing account storage and Keycloak identifiers are preserved, and old telemetry headers/environment variables remain supported. New diagnostic artifacts use `bench-artifacts/`; existing artifacts are left in place. See [benchmarking Python](docs/python.md).

Version **0.9** isolates target launches from the Rust process's resident memory. This prevents report/history allocations from inflating a child's peak RSS across `fork`/`exec`. Reports identify the measurement backend; regenerate saved baselines when upgrading from earlier versions. See [what memory measurements mean](docs/memory.md), including why GNU `size` and runtime RSS differ.

Version 0.5 adds saved comparisons and regression gates, workload suites, CPU hotspots and flamegraphs, adaptive sampling, CPU affinity, target-PID Intel GPU metrics, offline HTML reports, and checkpoint recovery. See [the experiment guide](docs/experiments.md) for commands, configuration, and limitations.

Version **0.6** adds **10 domain flags with 3 subflags each**, including `--networking --bandwidth`, `--gamedev --graphics`, and `--backend --api`. The normal report stays intact; domain sections add traced socket statistics, GPU diagnostics, and validated application telemetry. Run `bench --list-profiles` for all 40 flags and 90 supported metric keys. See [domain profiles and instrumentation](docs/profiles.md) for the full matrix, research sources, and examples. `strace` is an optional dependency for automatic socket tracing.

Version **0.8** replaces the local Rust account database with **Keycloak + PostgreSQL**. Accounts have no expiration date; the CLI keeps a persistent, revocable login using OAuth/PKCE and offline refresh. Passwords and account management live in Keycloak's browser pages. Benchmarking works without login or the account service. See [accounts and local setup](docs/accounts.md), including the ten-year inactivity limit on saved logins.

```bash
./auth/scripts/setup.sh          # Requires Docker + Compose; first run downloads images
bench account register       # Register in your browser and sign in
bench account whoami
bench account logout
bench account login
```

## Build and install

Requirements: Linux, Rust 1.89+ and Cargo, a C compiler (`cc`, GCC, or Clang), and a linker. A C++ compiler is only needed for source-compilation mode. Cargo downloads build dependencies on the first build; `Cargo.lock` pins their versions.

```bash
make install
bench --version
bench ./build/my_program
```

`make install` builds an optimized release executable and installs it to `~/.local/bin/bench`. Ensure that directory is on your PATH:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Add that line to your shell startup file if needed. Change the destination with `make install PREFIX=/your/prefix`. To build and run without installing:

```bash
cargo build --release --locked
./target/release/bench /path/to/program
```

After changing the tool, run `make install` again. The installed binary works independently of this checkout. Invoke the installed `bench` executable directly; it is not a Python package.

## Usage

### Choose the detail level

```bash
bench ./program                 # Simple overview (default)
bench ./program --intermediate  # Resource breakdown, averages, and ranges
bench ./program --advanced      # Every statistic, configuration, and individual run
```

Alternatively, use `--view simple`, `--view intermediate`, or `--view advanced`. Choose one view flag per invocation. Views change text presentation; JSON and CSV retain all selected data.

The simple overview uses readable time and memory units. Time is the average elapsed time per successful run. Memory is the **highest peak RSS across successful runs**, not an average allocation size. CPU usage is the average of per-run CPU/wall percentages: 100% means one fully occupied core, and multithreaded workloads can exceed 100%. Throughput and output sizes are averages per run. Time variation is the coefficient of variation; one successful run is insufficient to estimate variation.

GPU activity is available with `--gpu`, using `intel_gpu_top` during an extra diagnostic run. The report distinguishes device-wide engine activity from target-PID DRM client activity, where available. Without monitoring, permissions, or complete samples, it reports an explicit unavailable reason. Choosing a view alone does not enable any external tool.

Intermediate view adds user/system CPU time and filesystem I/O operations, with average/minimum/maximum values. Advanced view includes all metric statistics, execution settings, and individual warmup and measured-run values. Failures and diagnostic notes appear at every level; if no runs succeed, the overview explicitly says that no resource summary is available.

### Benchmark an existing project

Build with your normal build system, then pass the resulting executable:

```bash
bench ./build/my_program
bench ./build/my_program --runs 30 --warmup 3 --timeout 120
bench ./build/my_program --runs 20 -- --size 10000000 --mode batch
```

Every invocation runs to completion or its deadline. Ten measured runs of a ten-minute program take roughly 100 minutes, plus warmups; choose `--runs`, `--warmup`, and `--timeout` accordingly.

Benchmark flags go before `--`. Arguments after it are passed directly to the target without shell interpretation. Quote paths containing spaces. The executable path is resolved relative to the invoking directory, even with `--cwd`. The explicit `--executable PATH` form is also supported.

### Benchmark Python

```bash
bench workload.py
bench workload.py --intermediate --runs 30 -- --size 100000
bench workload.py --interpreter .venv/bin/python --python-arg=-O
bench --python script_without_extension --input input.txt
```

Install Python and your workload's dependencies in the chosen environment. The default interpreter is `python3` from PATH. Scripts do not need an executable bit or shebang. Each run measures a fresh interpreter process, including startup, imports, script execution, and shutdown; peak memory includes the interpreter and loaded modules. See [the Python guide](docs/python.md) for suites, telemetry, and measurement scope.

### Select results

```bash
bench ./program --metrics wall,rss,major_faults
bench ./program --metrics timing,memory
bench ./program --metrics io,scheduling
bench --list-metrics
```

All 15 metrics are collected by default, while the text view chooses how much to display. An explicit `--metrics` selection overrides the view's default metric rows and filters JSON/CSV too. For example, `--metrics wall` shows only time-related results in any text view; `--metrics all` shows every metric even with simple formatting. OS resource counters are collected together regardless of selection. Duplicate selections are removed.

### Save reports

```bash
bench ./program --format json --output results.json
bench ./program --metrics wall,cpu,rss --format csv --output results.csv
bench ./program --format html --output results.html
```

Text prints the chosen detail level and failure diagnostics. JSON records settings, host metadata, compiler information when applicable, individual warmups and measured runs, metric units, summaries, GPU availability, and diagnostic previews. CSV has one row per measured invocation, including status, exit code, and selected metrics; it excludes warmups and summary statistics. Use JSON for the full record. View flags do not reduce export detail.

Reports go to stdout unless `--output` is specified. The file's parent directory must exist; an existing report file is overwritten. Commands and stderr may contain program-supplied sensitive data. Environment override keys are recorded, while values are omitted.

### CPU counters, GPU monitoring, allocations, and memory errors

```bash
bench ./program --perf
bench ./program --gpu
bench ./program --heaptrack
bench ./program --valgrind
bench ./program --hotspots
bench ./program --all-tools --advanced

bench ./program --perf --valgrind --diagnostic-timeout 300 \
  --artifacts-dir ./profiles --format json --output results.json -- --size 1000000
```

These options require the corresponding tools on PATH: `perf`, `intel_gpu_top` (Arch package `intel-gpu-tools`), `heaptrack` plus `heaptrack_print`, and `valgrind`. On Arch, install them with `sudo pacman -Syu --needed perf intel-gpu-tools heaptrack valgrind`.

**Each enabled tool replays the target once after all baseline runs succeed.** `--all-tools` can therefore run the target five extra times. The input file is reopened; target arguments, cwd, and expected exit status are reused. Programs with side effects repeat those side effects. Tool time and memory are excluded from baseline statistics. Each tool/analysis command has its own `--diagnostic-timeout` (default 120 seconds). Interrupted diagnostics stop subsequent tools and retain the baseline report; a failed tool does not prevent the other requested tools from running.

Diagnostics set `LC_ALL=C` for stable parsing, including the target during those passes. The baseline retains your original locale. Configured debuginfod access is inherited: Valgrind may need it to obtain loader/library debug symbols. Heaptrack may disable debuginfod during recording according to its own policy. Build your program with `-g` for useful source locations; optimized programs are supported, though optimization can obscure stack traces or eliminate allocations.

| Tool flag | What is reported | Scope and limits |
| --- | --- | --- |
| `--perf` | Counter values, instructions per cycle, cache miss percentage, branch miss percentage | Default events count user-space activity in an instrumented replay. Unsupported counters remain null. Multiplexing coverage is retained; derived ratios can be affected by scaling. |
| `--gpu` | Device-wide engine activity; target engine-class activity and resident DRM bytes when exposed | Exact target PID matching excludes child processes. Device totals include other clients. Shared DRM buffers may be counted more than once; resident bytes are not dedicated VRAM. |
| `--heaptrack` | Allocation call count, temporary allocations, approximate peak heap and retained bytes | Separate instrumented replay. Printed byte sizes are rounded estimates; retained allocations alone do not prove a bug. Trace and top allocation stack reports are saved. |
| `--valgrind` | Memcheck error contexts, occurrences by kind, leak bytes, and stack traces | Target process only: exec children are not traced and post-fork child reports are silenced. Still-reachable allocations are reported without failing the check. Other reported memory-error/leak contexts produce `findings`. |
| `--hotspots` | CPU function self samples, source locations, and flamegraph | Separate user-space perf replay at requested 99 Hz. Samples are not exact timings. Debug symbols improve source attribution. |

Select CPU events with `--perf-events cycles:u,instructions:u,branches:u,branch-misses:u`. Defaults also include `cache-references:u,cache-misses:u`. Hardware/kernel support varies; unavailable events are never silently reported as zero. See the [perf stat manual](https://man.archlinux.org/man/perf-stat.1.en).

GPU sampling defaults to 100 ms. Use `--gpu-interval-ms 50` to sample more frequently (allowed: 10–60000 ms), and `--gpu-device drm:/dev/dri/card1` to choose a device. The first sample is discarded as a baseline. A program lasting only a few milliseconds can exit before any complete sampling interval. In that case the report says the sampling window was too short (`reason_code: insufficient_samples`), rather than reporting zero activity or a format error. Increase the amount of work per invocation, or try `--gpu-interval-ms 10`; even that cannot guarantee useful samples from a very short run. `intel_gpu_top` must support the active Intel driver. This integration does not support NVIDIA/AMD or Intel Xe devices requiring a different monitoring tool. See the [intel_gpu_top manual](https://man.archlinux.org/man/intel_gpu_top.1.en).

The monitor's entire JSON output is captured separately from target stdout. This accommodates releases that otherwise write array brackets to stdout while sending sample objects to `-o FILE`. The parser also accepts single-object, comma-separated, and concatenated-object logs. The report retains raw-file and invocation information when JSON parsing fails. GPU activity is still device-wide: desktop applications can produce activity even when the benchmark itself does no GPU work.

If `intel_gpu_top` reports `Permission denied` / `CAP_PERFMON`, an administrator must grant monitoring access. On systems supporting that capability, an administrator can grant it to the monitoring binary:

```bash
sudo setcap cap_perfmon=ep /usr/bin/intel_gpu_top
```

This grants performance-monitoring access to that executable; it does not require running the benchmark target as root. Package updates may remove the capability. bench does not change system permissions or invoke sudo. The same restrictions may affect perf, though user-space events often work with the existing configuration. See the [kernel perf security documentation](https://www.kernel.org/doc/html/latest/admin-guide/perf-security.html).

If Valgrind cannot establish required loader redirections, it needs matching loader/library debug symbols. Preserve a working `DEBUGINFOD_URLS` configuration or install your distribution's matching debug symbols; the diagnostic timeout still applies. Invalid or incomplete tool output is reported as unavailable or failed, not as a clean result.

Every requested diagnostic session creates a unique directory under `bench-artifacts/` (override with `--artifacts-dir`). It contains `diagnostics.json`, captured stdout/stderr, and applicable `perf.csv`, `gpu.json`, `valgrind.xml`, compressed heap trace, and `heaptrack-analysis.stdout`. Raw files are retained even for failures where available. Heaptrack uses a private `/tmp` staging directory to accommodate launcher path handling, then copies its trace into the artifact directory. Its launcher mixes messages with target stdout, so exact output comparison is skipped for that pass; the baseline, perf, GPU replay, and Valgrind still honor `--expect-output`.

Text shows a concise diagnostic summary, with counter detail and more context in intermediate/advanced views. JSON embeds all parsed diagnostic results under `diagnostics.results` and GPU status under `gpu`. CSV remains a table of baseline measurements; the separate `diagnostics.json` contains tool results. Explicit `--metrics` filters baseline metrics, not separately requested tools. Diagnostic progress goes to stderr.

Tool statuses include `ok`, `partial`, `unavailable`, `failed`, `findings`, `interrupted`, and `skipped`. Any requested tool that is not `ok` makes the overall exit code nonzero. At most 16 MiB of each textual tool report is parsed; larger files remain available as artifacts. Output limits bound captured stdout/stderr on a best-effort basis, not independently written profiles/XML files. Raw artifacts can be large. Baseline failures skip diagnostic replays entirely.

### Input and correctness

```bash
bench ./solver --input cases/input.txt \
  --expect-output cases/expected.txt --expect-exit 0
```

Stdin is reopened for every invocation; the default is `/dev/null`. Expected output is compared byte-for-byte, including whitespace, final newlines, and binary data. Comparison happens after measured execution and uses fixed-size buffers. Without `--expect-output`, output correctness is not checked. Exit status is always checked; zero is expected by default, and negative codes represent signals.

For a calculator that reads an operation, one number per line, then `q`, supply a complete session:

```bash
printf '+\n2\n3\nq\n' > calculator-input.txt
bench ./calculator --input calculator-input.txt --intermediate
```

The same sample is included in `examples/calculator-input.txt`. Every warmup and measured run receives the whole file from the beginning. This measures the complete scripted process invocation, including prompts and input parsing, rather than just arithmetic.

If the first warmup reaches `output_limit`, no measured runs start. A program that ignores failed reads at EOF can print prompts indefinitely, even though no person is typing. bench reports the captured byte counts, a bounded stdout preview, and input guidance. In C++, check `if (!(std::cin >> operation))` before using the operation and `if (!std::getline(std::cin, line))` before processing each line; exit or return an error on EOF instead of retrying forever. Increasing the output limit does not fix such a loop.

### Working directory, environment, and throughput

```bash
bench ./build/solver --cwd ./data --env OMP_NUM_THREADS=4 --env MODE=fast
bench ./program --units 1000000 -- --items 1000000
```

The environment is inherited, with repeatable `--env KEY=VALUE` overrides. `--cwd` changes only the target working directory. Input/expected-output paths remain relative to the invoking directory. `--units` declares work completed **per invocation**; throughput is units divided by wall seconds. The default of one reports invocations per second. The tool does not infer work from arguments or output.

### Compile C++ sources

```bash
bench --source examples/workload.cpp --runs 20 -- 50000000

bench --source main.cpp helpers.cpp \
  --compiler clang++ --std c++20 --opt 3 \
  --cxxflag=-march=native --cxxflag=-DNDEBUG --cxxflag=-Iinclude \
  --cxxflag=-pthread --ldflag=-pthread --ldflag=-lm \
  -- --size 1000000
```

The default compiler uses `g++ -std=c++17 -O2`. Each `--cxxflag` and `--ldflag` supplies one argument. Compiler flags go before source paths, linker flags after the output filename. Compilation occurs once in the invoking directory and is timed separately. The temporary executable is removed on exit. For CMake, Make, generated code, or custom toolchains, build normally and use the executable path.

## Metrics

| Name | Unit | Meaning |
| --- | --- | --- |
| `wall` | s | Elapsed process time including launch and exit observation |
| `user` | s | User-mode CPU time |
| `system` | s | Kernel-mode CPU time |
| `cpu` | s | User + system CPU time |
| `cpu_percent` | % | CPU / wall × 100; multithreaded programs can exceed 100% |
| `rss` | KiB | Linux peak resident set size; not allocation volume |
| `minor_faults` | count | Page faults without disk I/O |
| `major_faults` | count | Page faults requiring disk I/O |
| `input_blocks` | count | Filesystem input operations reported by Linux; not bytes |
| `output_blocks` | count | Filesystem output operations reported by Linux; not bytes |
| `voluntary_switches` | count | Voluntary context switches |
| `involuntary_switches` | count | Involuntary context switches |
| `stdout_bytes` | bytes | Captured stdout size |
| `stderr_bytes` | bytes | Captured stderr size |
| `throughput` | units/s | Declared work units / elapsed time |

Groups:

- `timing`: wall, user, system, cpu, cpu_percent, throughput.
- `memory`: rss, minor_faults, major_faults.
- `io`: input_blocks, output_blocks, stdout_bytes, stderr_bytes.
- `scheduling`: voluntary_switches, involuntary_switches.
- `all`: every metric, selected by default.

## Options

The new experiment/report options are listed in [the experiment guide](docs/experiments.md). `bench --help` lists every flag.

| Option | Default | Purpose |
| --- | --- | --- |
| `PATH` | — | Positional executable path |
| `--executable PATH` | — | Alternative explicit executable option |
| `--source FILE [FILE ...]` | — | Compile sources; exclusive with executable modes |
| `--compiler NAME` | `g++` | Compiler executable name or path |
| `--std STANDARD` | `c++17` | C++ standard |
| `--opt LEVEL` | `2` | `0`, `1`, `2`, `3`, `s`, `g`, or `fast` |
| `--cxxflag=FLAG` | — | Repeatable compiler argument |
| `--ldflag=FLAG` | — | Repeatable linker argument |
| `--compile-timeout SECONDS` | `120` | Compilation deadline |
| `--runs N` | `10` | Positive number of measured invocations |
| `--warmup N` | `1` | Discarded warmups; zero allowed |
| `--timeout SECONDS` | `10` | Deadline per invocation |
| `--output-limit MIB` | `64` | Combined stdout/stderr limit per invocation |
| `--input FILE` | `/dev/null` | Replayed stdin |
| `--expect-output FILE` | — | Exact expected stdout |
| `--expect-exit CODE` | `0` | Expected exit code; negative for signals |
| `--cwd DIRECTORY` | Current directory | Target working directory |
| `--env KEY=VALUE` | Inherited | Repeatable override for target and compiler |
| `--units NUMBER` | `1` | Positive work units per invocation |
| `--view LEVEL` | `simple` | `simple`, `intermediate`, or `advanced` text detail |
| `--intermediate` | Off | Shortcut for `--view intermediate` |
| `--advanced` | Off | Shortcut for `--view advanced` |
| `--metrics LIST` | View-dependent text rows; all in exports | Explicit comma-separated names/groups override the view's rows |
| `--format FORMAT` | `text` | `text`, `json`, `csv`, or `html` |
| `--output FILE` | stdout | Report destination |
| `--keep-going` | Off | Continue after failed measured runs |
| `--perf` | Off | One extra replay with CPU hardware counters |
| `--gpu` | Off | One extra replay with Intel device-wide GPU monitoring |
| `--heaptrack` | Off | One extra replay with heap allocation profiling |
| `--valgrind` | Off | One extra replay with Memcheck |
| `--all-tools` | Off | Enable all five diagnostic passes |
| `--diagnostic-timeout SECONDS` | `120` | Deadline for each diagnostic/analysis command |
| `--artifacts-dir DIRECTORY` | `bench-artifacts` | Parent for unique retained diagnostic sessions |
| `--perf-events LIST` | Six user-space hardware events | Comma-separated perf events |
| `--gpu-interval-ms N` | `100` | GPU sampling interval, 10–60000 ms |
| `--gpu-device SELECTOR` | Tool default | Intel GPU device selector |
| `--list-metrics` | — | List metrics and exit |
| `--version` | — | Show version |
| `--help` | — | Show usage |

## Statistics and failures

Every invocation starts a fresh process. Warmups are checked but excluded from statistics. Failed warmups stop the benchmark even with `--keep-going`. Measured failures stop subsequent invocations unless `--keep-going` is supplied. Interruptions always stop execution.

Only successful measured invocations contribute to summaries. Statistics include sample standard deviation, linearly interpolated p90/p95, and coefficient of variation (standard deviation / mean × 100). Single-sample standard deviation and zero-mean CV are zero by convention. No outliers are removed. JSON and detailed views include descriptive 95% mean bootstrap intervals (1000 deterministic resamples) and Tukey 1.5-IQR outlier counts. A single sample has no interval; adaptive stopping can affect interval coverage. Small-sample percentiles are descriptive and do not establish tail behavior.

Statuses: `ok`, `timeout`, `output_limit`, `unexpected_exit`, `output_mismatch`, and `interrupted`. Failed runs retain metrics that may describe incomplete work. Stderr previews contain the first 4096 bytes. Failed invocations additionally include `stdout_preview` (up to 512 bytes), `stdout_preview_truncated`, and `captured_output` with stdout/stderr byte counts, even when `--metrics` excludes I/O. Invalid UTF-8 is replaced, and terminal previews are escaped. Previews are read after measurement; successful stdout is not added to reports. Launch/setup failures are errors, distinguished from a program that legitimately exits with code 127.

Exit codes:

- `0`: every requested measurement and diagnostic passed, or an informational command succeeded.
- `1`: a warmup/measurement failed, or a requested diagnostic was unavailable, partial, failed, or found memory errors; a report is produced.
- `2`: invalid arguments, setup, compilation, report-writing failure, or incompatible comparison.
- `3`: a regression threshold was exceeded (provided measurements and diagnostics otherwise passed).
- `130`: interrupted by SIGINT (Ctrl+C).
- `143`: interrupted by SIGTERM.

Target interruptions produce a partial report; compilation interruptions emit a diagnostic without a benchmark report.

## Measurement methodology and limits

The C layer uses `fork`/`execve`, `CLOCK_MONOTONIC`, and per-invocation `wait4`. SIGCHLD wakes the waiter immediately on target exit. A 5 ms safety-check interval monitors output size and deadlines; normal completion does not wait for a polling tick. Timing includes launch, dynamic loading, initialization, program output, shutdown, and parent scheduling latency. Input setup, correctness comparison, statistics, and rendering happen outside measured time.

A native harness reduces interpreter startup and orchestration overhead; it does not make the target program itself faster. Short process benchmarks still include launch and scheduling costs. Make each invocation do enough work, keep workloads and compiler flags consistent, and inspect variation. Sub-10 ms runs get a note when wall time is selected.

Linux resource accounting may include descendants the target waits for; this is not a full process-tree profiler. RSS is a peak, not the sum of simultaneous child memory, and may include inherited pre-exec memory. CPU counters have OS-dependent granularity. Logical CPU count reflects Rust's estimate of parallelism available to this process.

Output is captured in temporary files, so printing can affect timing and filesystem counters. Output limits are best-effort and may overshoot; they are not hard disk quotas. Compiler output is limited to 16 MiB. Temporary storage follows system configuration, including `TMPDIR`. Deadlines are not hard real-time guarantees, especially for processes blocked in kernel operations.

Programs execute with your permissions; this is **not a sandbox for untrusted code**. The target process group is killed on timeout, output-limit breach, interruption, and leader exit. Descendants that deliberately leave the group can escape cleanup. Programs must wait for their own children before returning. Programs requiring a live terminal and persistent services are not suitable targets; prompt-based programs can be benchmarked with a complete, finite `--input` script.

Warmups may prime filesystem caches but do not transfer process-local state to later invocations. CPU scaling, thermals, caches, background tasks, and scheduling affect results. The tool does not flush caches or change power policies; CPU pinning is available through --cpu-affinity. Baseline metrics do not include allocations, leaks, hardware counters, or GPU activity; those require the optional diagnostic passes. Energy measurement is not implemented.

## Development

```bash
make fmt
make verify
```

See [the development guide](docs/development.md) for the module map, execution flow, test commands, and instructions for adding profiles or diagnostic tools. The entrypoint delegates to separate CLI, application, benchmark, statistics, and storage modules. Each external tool owns its runner, parser, and parser tests under `src/diagnostics/`. Profile definitions, telemetry, networking, and presentation live under `src/profiles/`. The measurement boundary spans `src/runner.rs`, `native/launcher.c`, `native/helper.c`, and `native/measure.c`.

JSON uses `schema_version: 3`, with additive `gpu`, `diagnostics`, `measurement_backend`, `memory_accounting`, and `python` fields. Metric names and successful-run aggregation remain consistent. Complete statistical columns are available in advanced text view and JSON. Diagnostic measurements are nested separately and never enter baseline aggregates.
