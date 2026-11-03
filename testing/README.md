# cppbench

A native **Rust + C** command-line benchmark tool for Linux. Pass an executable path to get all 15 metrics and their statistics:

```bash
cppbench /path/to/your/program
```

The default is one warmup followed by ten measured invocations. All metrics are shown: timing, CPU, peak memory, page faults, filesystem I/O counters, context switches, output sizes, and throughput. Each metric includes count, mean, median, min, max, sample standard deviation, p90, p95, and coefficient of variation.

Rust handles the CLI, statistics, correctness checks, and reports. C handles process launch, monotonic timing, Linux `wait4` resource accounting, deadlines, and process-group cleanup. The C code is linked into the native executable. **No Python interpreter, wrapper, or runtime is used.**

## Build and install

Requirements: Linux, Rust/Cargo, a C compiler (`cc`, GCC, or Clang), and a linker. A C++ compiler is only needed for source-compilation mode. Cargo downloads build dependencies on the first build; `Cargo.lock` pins their versions.

```bash
make install
cppbench --version
cppbench ./build/my_program
```

`make install` builds an optimized release executable and installs it to `~/.local/bin/cppbench`. Ensure that directory is on your PATH:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Add that line to your shell startup file if needed. Change the destination with `make install PREFIX=/your/prefix`. To build and run without installing:

```bash
cargo build --release --locked
./target/release/cppbench /path/to/program
```

After changing the tool, run `make install` again. The installed binary works independently of this checkout. This version replaces the Python package and its `python3 -m cppbench` interface.

## Usage

### Benchmark an existing project

Build with your normal build system, then pass the resulting executable:

```bash
cppbench ./build/my_program
cppbench ./build/my_program --runs 30 --warmup 3 --timeout 120
cppbench ./build/my_program --runs 20 -- --size 10000000 --mode batch
```

Every invocation runs to completion or its deadline. Ten measured runs of a ten-minute program take roughly 100 minutes, plus warmups; choose `--runs`, `--warmup`, and `--timeout` accordingly.

Benchmark flags go before `--`. Arguments after it are passed directly to the target without shell interpretation. Quote paths containing spaces. The executable path is resolved relative to the invoking directory, even with `--cwd`. The explicit `--executable PATH` form is also supported.

### Select results

```bash
cppbench ./program --metrics wall,rss,major_faults
cppbench ./program --metrics timing,memory
cppbench ./program --metrics io,scheduling
cppbench --list-metrics
```

`--metrics all` is the default. Selection filters text, JSON, and CSV; OS resource counters are collected together regardless of selection. Duplicate selections are removed.

### Save reports

```bash
cppbench ./program --format json --output results.json
cppbench ./program --metrics wall,cpu,rss --format csv --output results.csv
```

Text prints a complete statistical summary and failure diagnostics. JSON records settings, host metadata, compiler information when applicable, individual warmups and measured runs, metric units, summaries, and diagnostic previews. CSV has one row per measured invocation, including status, exit code, and selected metrics; it excludes warmups and summary statistics. Use JSON for the full record.

Reports go to stdout unless `--output` is specified. The file's parent directory must exist; an existing report file is overwritten. Commands and stderr may contain program-supplied sensitive data. Environment override keys are recorded, while values are omitted.

### Input and correctness

```bash
cppbench ./solver --input cases/input.txt \
  --expect-output cases/expected.txt --expect-exit 0
```

Stdin is reopened for every invocation; the default is `/dev/null`. Expected output is compared byte-for-byte, including whitespace, final newlines, and binary data. Comparison happens after measured execution and uses fixed-size buffers. Without `--expect-output`, output correctness is not checked. Exit status is always checked; zero is expected by default, and negative codes represent signals.

### Working directory, environment, and throughput

```bash
cppbench ./build/solver --cwd ./data --env OMP_NUM_THREADS=4 --env MODE=fast
cppbench ./program --units 1000000 -- --items 1000000
```

The environment is inherited, with repeatable `--env KEY=VALUE` overrides. `--cwd` changes only the target working directory. Input/expected-output paths remain relative to the invoking directory. `--units` declares work completed **per invocation**; throughput is units divided by wall seconds. The default of one reports invocations per second. The tool does not infer work from arguments or output.

### Compile C++ sources

```bash
cppbench --source examples/workload.cpp --runs 20 -- 50000000

cppbench --source main.cpp helpers.cpp \
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
| `--metrics LIST` | `all` | Comma-separated names/groups |
| `--format FORMAT` | `text` | `text`, `json`, or `csv` |
| `--output FILE` | stdout | Report destination |
| `--keep-going` | Off | Continue after failed measured runs |
| `--list-metrics` | — | List metrics and exit |
| `--version` | — | Show version |
| `--help` | — | Show usage |

## Statistics and failures

Every invocation starts a fresh process. Warmups are checked but excluded from statistics. Failed warmups stop the benchmark even with `--keep-going`. Measured failures stop subsequent invocations unless `--keep-going` is supplied. Interruptions always stop execution.

Only successful measured invocations contribute to summaries. Statistics include sample standard deviation, linearly interpolated p90/p95, and coefficient of variation (standard deviation / mean × 100). Single-sample standard deviation and zero-mean CV are zero by convention. No outliers are removed; no confidence intervals are claimed. Small-sample percentiles are descriptive and do not establish tail behavior.

Statuses: `ok`, `timeout`, `output_limit`, `unexpected_exit`, `output_mismatch`, and `interrupted`. Failed runs retain metrics that may describe incomplete work. Stderr previews contain the first 4096 bytes, with invalid UTF-8 replaced. Launch/setup failures are errors, distinguished from a program that legitimately exits with code 127.

Exit codes:

- `0`: every requested measurement passed, or an informational command succeeded.
- `1`: a warmup or measurement failed; a report is produced.
- `2`: invalid arguments, setup, compilation, or report-writing failure.
- `130`: interrupted by SIGINT (Ctrl+C).
- `143`: interrupted by SIGTERM.

Target interruptions produce a partial report; compilation interruptions emit a diagnostic without a benchmark report.

## Measurement methodology and limits

The C layer uses `fork`/`execve`, `CLOCK_MONOTONIC`, and per-invocation `wait4`. SIGCHLD wakes the waiter immediately on target exit. A 5 ms safety-check interval monitors output size and deadlines; normal completion does not wait for a polling tick. Timing includes launch, dynamic loading, initialization, program output, shutdown, and parent scheduling latency. Input setup, correctness comparison, statistics, and rendering happen outside measured time.

A native harness reduces interpreter startup and orchestration overhead; it does not make the target program itself faster. Short process benchmarks still include launch and scheduling costs. Make each invocation do enough work, keep workloads and compiler flags consistent, and inspect variation. Sub-10 ms runs get a note when wall time is selected.

Linux resource accounting may include descendants the target waits for; this is not a full process-tree profiler. RSS is a peak, not the sum of simultaneous child memory, and may include inherited pre-exec memory. CPU counters have OS-dependent granularity. Logical CPU count reflects Rust's estimate of parallelism available to this process.

Output is captured in temporary files, so printing can affect timing and filesystem counters. Output limits are best-effort and may overshoot; they are not hard disk quotas. Compiler output is limited to 16 MiB. Temporary storage follows system configuration, including `TMPDIR`. Deadlines are not hard real-time guarantees, especially for processes blocked in kernel operations.

Programs execute with your permissions; this is **not a sandbox for untrusted code**. The target process group is killed on timeout, output-limit breach, interruption, and leader exit. Descendants that deliberately leave the group can escape cleanup. Programs must wait for their own children before returning. Interactive programs and persistent services are not suitable targets.

Warmups may prime filesystem caches but do not transfer process-local state to later invocations. CPU scaling, thermals, caches, background tasks, and scheduling affect results. The tool does not flush caches, pin CPUs, or change power policies. Allocation counts, leaks, hardware cache misses, instructions, GPU usage, and energy are not measured by this version.

## Development

```bash
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

`src/main.rs` implements CLI options, experiment coordination, statistics, and reports. `src/runner.rs` owns file/string lifetimes and the C FFI boundary. `native/measure.c` implements measurement; `native/measure.h` documents the ABI. `build.rs` compiles and links C. Integration tests exercise the actual CLI, including C++ compilation, target arguments, correctness checks, exports, failed-run exclusion, timeout cleanup, and interruption cleanup.

JSON uses `schema_version: 2`. Compared with the Python version, it adds implementation and interruption fields, removes Python host metadata, and reports every statistical column in text. Metric names and successful-run aggregation remain consistent.
