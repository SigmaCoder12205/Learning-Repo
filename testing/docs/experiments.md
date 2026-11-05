# cppbench experiments (0.5)

The ordinary command remains `cppbench ./program`, with `--intermediate` and `--advanced` for increasing detail. All new modes use the same Rust executable and native C measurement layer.

## Compare builds and enforce regression limits

```bash
cppbench ./program --runs 30 --save-baseline baseline.json -- --size 1000000
# Rebuild, keeping the workload the same.
cppbench ./program --runs 30 --compare baseline.json \
  --regression wall=5 --regression rss=10 --format html --output comparison.html \
  -- --size 1000000
```

`--save-baseline FILE` writes the full JSON report independently of presentation format. It requires completed, successful measurements. `--compare FILE` alone displays changes; repeat `--regression METRIC=PERCENT` to return exit code **3** when worsening exceeds a threshold. Timing and throughput compare medians; `rss` compares the highest peak. Higher throughput is better; lower resource consumption is better. `cpu_percent`, `stdout_bytes`, and `stderr_bytes` cannot be regression gates because they have no automatic improvement direction. `allocations`, `heap_peak`, and `leaked_bytes` require the corresponding diagnostics in both reports. These diagnostic figures come from one separate replay and may be rounded by the external tool.

Comparisons check arguments, input/environment fingerprints, work units, working directory, expected exit, CPU affinity, and CPU/kernel/host identity. Binary SHA-256 is recorded but may change between builds. Differences normally return exit code **2**. `--allow-mismatch` deliberately compares different hosts/workloads while preserving the differences in the report. It cannot waive failed/incomplete measurements, missing threshold metrics, or missing suite cases. Older reports without provenance produce a warning; full compatibility cannot be established. Thresholds are practical gates, not statistical significance tests. Noise and adaptive stopping affect results. Measurement/diagnostic failures take precedence over comparison exit codes.

## Workload suites

```bash
mkdir -p build
g++ -std=c++17 -O2 -g examples/workload.cpp -o build/workload
cppbench --suite examples/suite.json --progress --save-baseline suite-baseline.json
cppbench --suite examples/suite.json --compare suite-baseline.json \
  --regression wall=5 --format html --output suite.html
```

A suite is versioned JSON:

```json
{
  "version": 1,
  "defaults": {"runs": 10, "warmup": 2, "timeout": 120},
  "benchmarks": [
    {"name": "small", "executable": "./build/program", "args": ["--size", "1000"]},
    {"name": "large", "executable": "./build/program", "args": ["--size", "1000000"],
     "env": {"OMP_NUM_THREADS": "4"}, "units": 1000000}
  ]
}
```

Each benchmark needs a unique name and an executable. Paths (`executable`, `cwd`, `input`, `expect_output`, `artifacts_dir`) are relative to the **suite file's directory**, including paths inherited from defaults. The default working directory is that directory; artifacts default to its `cppbench-artifacts/`. Cases override defaults; `env` objects merge by key. Cases execute sequentially and remain separately reported and compared by name. Configuration and required file paths are checked before the first target executes. Runtime failures remain in the report and subsequent cases continue, except on interruption.

Measurement settings use CLI names with underscores: `runs`, `warmup`, `timeout`, `output_limit`, `expect_exit`, `units`, `metrics`, `cpu_affinity`, `adaptive`, `max_runs`, `precision`, `min_time`, `keep_going`, `perf`, `gpu`, `heaptrack`, `valgrind`, `hotspots`, `all_tools`, `diagnostic_timeout`, `perf_events`, `gpu_interval_ms`, and `gpu_device`. `args` is a string array, `env` maps strings to strings, and mode/tool flags are booleans. Unknown fields are errors. Build executables first; source compilation is not a suite option. Put measurement flags inside the suite, while presentation/comparison/progress/checkpoint flags remain on the CLI. Suite CSV has one row per case/run/metric; suite JSON contains a `cases` array.

## Repeatability and experiment duration

```bash
cppbench ./program --runs 10 --adaptive --precision 3 --max-runs 100 --progress
cppbench ./program --runs 10 --min-time 5 --max-runs 200 --cpu-affinity 2-3
```

Fixed-run behavior is unchanged unless `--adaptive` or `--min-time` is enabled. `--runs` then becomes the minimum measured count; `--max-runs` (default 100) bounds it. `--min-time` is the sum of successful measured wall times, not a duration for each invocation. Adaptive stopping requires at least five samples and an approximate normal 95% mean interval half-width within `--precision` percent of the mean (default 5). If both conditions are enabled, both must pass. Reaching the cap records a warning and stopping reason; it does not itself fail successful measurements. More repetitions cannot remove startup overhead from a tiny workload.

Reports include descriptive bootstrap 95% mean intervals (1000 deterministic resamples), sample variation, and Tukey 1.5-IQR outlier counts. Outliers remain included. A single sample has no interval. Small samples and adaptive stopping limit interpretation; these are not guarantees of accuracy or convergence.

`--cpu-affinity 0,2-3` pins target processes and their diagnostic wrappers to allowed Linux CPU IDs. It does not pin the compiler, reserve cores, change frequency policy, or eliminate background interference. IDs must belong to the calling process's permitted affinity mask. Reports record CPU identity, kernel, host, governor when exposed, binary/input SHA-256, and an inherited-environment fingerprint. Environment values are hashed rather than written into provenance; command arguments and program output remain visible.

## CPU function hotspots

```bash
g++ -O2 -g -fno-omit-frame-pointer program.cpp -o program
cppbench ./program --hotspots --advanced --format html --output hotspots.html
```

`--hotspots` adds one separate `perf record` replay: user-space cycles, requested frequency 99 Hz, DWARF call stacks. It needs working perf counter permissions, but does not change them. `perf script` supplies function names and source locations when debug symbols are available. A location is a sampled location, not a complete per-line profile. The report ranks up to 50 functions by self sample share and retains up to 128 frames per stack. Percentages are sample shares, not exact function timings; short targets may produce no samples.

Artifacts include `perf.data`, tool stdout/stderr, `stacks.folded`, and `flamegraph.svg`. The SVG provides hover labels; HTML embeds it without external dependencies. Width represents sample count. This pass is excluded from ordinary time/memory statistics. `--all-tools` now enables five separate replays: perf counters, GPU, heaptrack, Valgrind, and hotspots.

## Intel GPU attribution and compatibility checks

```bash
cppbench --check-tools
cppbench --check-tools --format json --output tools.json
cppbench ./program --gpu --gpu-interval-ms 50 --advanced
```

`--check-tools` finds and identifies `perf`, `intel_gpu_top`, `heaptrack`, `heaptrack_print`, and `valgrind`, records their executable fingerprints, and probes CPU/GPU counter access. It supports text and JSON, returns nonzero for unavailable tools/counters, and does not modify capabilities or sysctls. Identity/probe success cannot guarantee every feature of every tool version. Parser tests cover captured/representative output shapes, missing values, permission failures, and truncated Intel GPU streams.

The GPU supervisor captures the replayed target PID. When `intel_gpu_top` exposes matching DRM clients, cppbench aggregates their engine-class activity and peak resident DRM bytes. Device-wide activity remains separate. Only the exact target PID is attributed: child processes are excluded. Engine-class totals can exceed 100%; shared buffers across clients/regions can be counted more than once. Resident DRM bytes are **not dedicated VRAM**. A missing client is unavailable, not zero. A short CPU-only calculator often has no attributable GPU samples; increase actual GPU work per invocation to observe activity. This integration supports Intel monitoring through `intel_gpu_top`, not NVIDIA/AMD or alternate Xe tools.

## Offline HTML reports

```bash
cppbench ./program --format html --output report.html
```

Open the file in a browser. It includes time/memory/CPU cards, a chart of the first 200 successful runs, comparisons, expandable full results, errors, and an embedded flamegraph when collected. Suite entries can be filtered by name. Hover over charts for values; print or download the complete embedded JSON using the buttons. No server or network is needed. Text views and CSV/JSON exports remain available; `--name` labels a single experiment.

## Progress and recovery

```bash
cppbench ./program --progress --checkpoint experiment.jsonl --format json --output results.json
cppbench --recover experiment.jsonl --format html --output recovered.html
```

Progress goes to stderr at phase boundaries and after each completed run, with elapsed time and an estimate to the run limit. There is no percentage indicator inside a target invocation. Adaptive runs can finish earlier than the estimate.

The checkpoint must be a **new** file. Completed warmup/run records are appended and synchronized outside measured time; reports are also saved around diagnostics and at completion. Recovery reads the journal without executing anything. It ignores an unterminated final record, rebuilds summaries from complete records, and labels unfinished experiments partial (exit code 1). It does not resume workloads or reconstruct an invocation that was still running. A fully completed journal reproduces its original report. Keep checkpoint, output, and saved-baseline destinations distinct. Journaling adds between-run I/O and can affect later cache state.

JSON reports now use schema version 3. New fields include provenance, completion state, stopping reason, confidence/outlier summaries, comparisons, and optional suite cases. Diagnostic measurements remain separate from baseline statistics.
