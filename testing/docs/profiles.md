# Domain profiles (cppbench 0.6)

Profiles add specialized results after the normal time, memory, CPU, and GPU report. They do not replace the baseline metric selection or infer application behavior from process runtime. The core remains Rust + native C; no Python runtime is required.

```bash
cppbench ./program --networking
cppbench ./program --networking --sockets --bandwidth
cppbench ./game --gamedev --graphics --frame-budget-ms 16.666667
cppbench ./api_test --backend --api --format html --output api.html
cppbench ./control_test --embedded --realtime --deadline-ms 2
cppbench ./test_driver --backend --api --qa --correctness
cppbench --list-profiles
cppbench --list-profiles --format json --output catalog.json
```

A main flag selects all three of its topics. Supplying one or more of its subflags narrows that profile to those topics. Subflags require their parent; multiple main profiles can be combined. Flags before `--` belong to cppbench; flags after `--` belong to the target. The original `--compiler PATH` still selects a C++ compiler; the systems topic uses `--compilation` to avoid a collision.

| Main flag | Three subflags | Typical problems investigated |
| --- | --- | --- |
| `--networking` | `--sockets`, `--bandwidth`, `--latency` | Failed connections, transfer counts, slow requests |
| `--frontend` | `--rendering`, `--interactivity`, `--accessibility` | Slow loading, UI stalls, accessibility test failures |
| `--backend` | `--api`, `--database`, `--distributed` | Request tails, query/lock waits, RPC queues/retries |
| `--mobile` | `--startup`, `--responsiveness`, `--battery` | Slow device startup, frame stalls/ANRs, energy/wakeups |
| `--gamedev` | `--gameplay`, `--graphics`, `--game-ai` | Slow simulation, uneven frames, expensive AI updates |
| `--data` | `--pipelines`, `--analytics`, `--ml` | Pipeline bottlenecks, spills, inference/training latency |
| `--devops` | `--ci`, `--sre`, `--containers` | Build/cache behavior, service errors, throttling/OOM |
| `--qa` | `--load-testing`, `--soak`, `--correctness` | Response tails, resource growth, assertion failures |
| `--embedded` | `--realtime`, `--iot`, `--robotics` | Deadline misses, device messaging, control-loop delay |
| `--systems` | `--compilation`, `--storage`, `--concurrency` | Compiler phases, I/O latency, lock/task queues |

These groups consolidate related roles: full-stack work uses frontend + backend; ML is under data; cloud/platform work uses DevOps. They are performance profiles, not security audits, WCAG certifications, or hard real-time guarantees.

## What is measured automatically

- Every selected profile adds relevant **baseline process context**, such as user/system CPU, peak RSS, page faults, and context switches. `--metrics` still filters baseline metrics; filtered-out context remains absent.
- `--networking` adds a separate `strace` replay, when installed and permitted. It counts socket syscalls, errors by errno, connect attempts, returned send/receive byte counts, and elapsed syscall samples. It follows traced descendants and recognizes socket-annotated `read`/`write` calls. It also supports interrupted/resumed trace records.
- The graphics topic (including plain `--gamedev`) requests the existing Intel GPU monitor. Device-wide activity and exact-PID attribution retain their existing scope. Missing monitor access does not prevent the rest of a profile report. Explicit `--gpu` still has its usual required-tool exit behavior.
- All selected profiles share **one additional telemetry replay**. cppbench sets `CPPBENCH_TELEMETRY_FILE` to a fresh absolute artifact path and `CPPBENCH_PROFILE_FLAGS` to the requested comma-separated flags. If the application or test driver writes measurements there, cppbench validates and summarizes them. Baseline runs do not receive these variables from cppbench.

Thus `--backend --api` adds one replay; `--networking` adds two when strace is available; `--gamedev --graphics` adds telemetry plus GPU monitoring. Multiple profiles share these collectors. `--all-tools` continues to mean the original five diagnostics; it does not enable all profiles. `--diagnostic-timeout`, correctness checks, cwd, stdin, affinity, and artifact handling apply to profile replays too. As with other diagnostics, they inherit `LC_ALL=C`.

No instrumentation is needed for ordinary process metrics or automatic socket/GPU data. **Request latency, frame times, browser vitals, database locks, device battery data, and similar application measurements require instrumentation or a test driver.** A Linux executable wrapper can collect device/browser data and write the telemetry file. cppbench does not directly run APKs, iOS applications, browser URLs, firmware, or persistent services. Benchmark a finite executable/test driver that owns the relevant workload and shuts down cleanly.

## Networking semantics

Install optional automatic tracing on Arch with `sudo pacman -S --needed strace`; `cppbench --check-tools` now reports strace identity too. No capabilities or privilege settings are changed automatically. A separate tracing pass perturbs timing; syscall durations are **not TCP RTT or HTTP request latency**. Reported errors include normal nonblocking `EINPROGRESS`/`EAGAIN` behavior. Inspect `errors_by_errno` and workload semantics before treating them as faults.

Byte totals sum successful syscall return values for recognized socket send/receive/read/write operations. They describe socket API transfers, not link-layer traffic, retransmissions, or packet loss. UNIX/local sockets are included; both endpoints and descendants may be counted. Batch (`sendmmsg`/`recvmmsg`), observed zero-copy transfer, or `io_uring` activity causes automatic byte totals to be withheld instead of presenting incomplete totals as complete. Application telemetry can supply those metrics. Raw traces remain available, with payload strings truncated; endpoints and file paths may still appear.

Counters are zero only when collected and actually zero. No observed socket calls means syscall latency is unavailable, not zero. Missing strace, permission failures, malformed/truncated traces, and unsupported transfer accounting are explicit in the diagnostics. Traces are parsed up to 16 MiB and 100,000 recognized socket calls. Raw trace files can exceed that and are retained; parsing limits are not disk quotas. Reduce diagnostic workload duration when needed.

Try the supplied finite, local socket benchmark:

```bash
mkdir -p build
g++ -std=c++17 -O2 examples/network_profile.cpp -o build/network_profile
cppbench ./build/network_profile --networking --require-profile-data --intermediate
```

It executes 1,000 fixed-size round trips over a UNIX socket pair without contacting an external service. Both the tracer and the application count 512,000 bytes sent and received. This is an example of local IPC, not a measure of internet bandwidth.

## Application telemetry

Write exactly one JSON object to the environment-provided file before exiting. Do not print telemetry to stdout; normal output checks remain intact. Raw data is retained as `profile-telemetry.json`, validated summaries appear in `profiles.json` and the main JSON report, and separate collector output remains in `diagnostics.json`.

```json
{
  "version": 1,
  "metrics": {
    "gamedev.frame_ms": {"unit": "ms", "samples": [12.1, 13.4, 20.7]},
    "gamedev.gpu_frame_ms": {"unit": "ms", "samples": [8.2, 8.5, 9.0]},
    "gamedev.draw_calls": {"unit": "count", "samples": [120, 118, 125]}
  }
}
```

Those numbers illustrate the format, not an actual benchmark result. Use real presented-frame intervals for `frame_ms`, and GPU timestamp measurements for `gpu_frame_ms`. Cumulative counters and gauges use `"value": NUMBER` instead of `samples`. Run `--list-profiles --format json` for all 90 keys, their exact units, and kinds. Emit each key once. Known metrics outside the selected topics are validated but not displayed. Unknown keys/fields, wrong units, negative/non-finite values, fractional counts/bytes, empty samples, and inconsistent value/sample formats fail validation.

Each sample array may contain 1–100,000 entries, with 200,000 entries total and a 16 MiB file limit. Byte/count values must be exact nonnegative integers no larger than 2^53−1. Avoid serializing unbounded event streams. The file must be a regular file, not a symlink or FIFO. An invalid telemetry report fails the diagnostic; it is never reported as an empty successful result.

The supplied header supports C11 and C++ without another library:

```cpp
#include <cppbench_telemetry.h>

// Buffer actual measurements during the telemetry replay, then flush at shutdown.
// frame_times_ms, gpu_times_ms and draw_calls contain your real measurements.
auto output = cppbench_telemetry_open();
cppbench_telemetry_samples(&output, "gamedev.frame_ms", "ms",
                          frame_times_ms.data(), frame_times_ms.size());
cppbench_telemetry_samples(&output, "gamedev.gpu_frame_ms", "ms",
                          gpu_times_ms.data(), gpu_times_ms.size());
cppbench_telemetry_samples(&output, "gamedev.draw_calls", "count",
                          draw_calls.data(), draw_calls.size());
if (cppbench_telemetry_close(&output) != 0) { /* handle write failure */ }
```

`make install` also installs the header to `~/.local/include/cppbench_telemetry.h` and this guide to `~/.local/share/doc/cppbench/profiles.md` (subject to `PREFIX`). Compile with `-I"$HOME/.local/include"`, or use this checkout's `include/` directory. Check `cppbench_telemetry_enabled()` before collecting optional measurements to avoid extra sampling work in baseline runs. The header opens no file when the environment variable is absent. Use a single writer, join worker threads before flushing, keep numeric formatting in the C locale, and aggregate child-process measurements in the parent. Concurrent processes must not overwrite the same file.

If both the tracer and the application supply a key, validated application telemetry is displayed, with its source label; the independent trace is retained in diagnostics. Derived rates use the entire associated instrumented replay duration, including startup, shutdown, and tracing overhead. They are not steady-state server/device throughput.

## Statistics and failures

Domain sample distributions show count, mean, median, p90, p95, p99, min, max, standard deviation, and variation. These describe samples from **one separate replay**, not the distribution of baseline invocations. Small-sample tails are descriptive. They do not correct coordinated omission or make independence/confidence claims.

For presented game/mobile frame intervals, FPS is `1000 / mean(frame_ms)`. `--frame-budget-ms` defaults to 16.666667 ms; counts above it are reported. `--deadline-ms` requires `--embedded` and counts cycle/control samples above that deadline. Neither setting turns an observed miss into a baseline failure or establishes worst-case guarantees. Missing frame intervals never produce invented FPS.

Missing application data produces a `partial` profile and normally leaves successful baseline exit status unchanged. `--require-profile-data` returns exit code **1** if any selected domain metric is unavailable. It requires the selected metric keys, not optional device-wide GPU context. Failed telemetry invocations and invalid telemetry also return 1; ordinary target/diagnostic failures keep their existing behavior. Missing optional automatic tools do not fail a profile if the required metric data is supplied by valid application telemetry.

Duration sample metrics in milliseconds can use existing regression gates:

```bash
cppbench ./api_test --backend --api --save-baseline api-baseline.json
cppbench ./api_test --backend --api --compare api-baseline.json \
  --regression backend.request_ms=5
```

Domain gates compare p95 from the telemetry/tracing replay, with lower being better. Counts, byte totals, and gauges have workload-dependent directions and are not automatic regression gates. Missing threshold data remains incompatible even with `--allow-mismatch`; differences in profile selection and measurement source are also recorded. Existing baseline regression metrics retain their previous definitions.

Text, JSON, HTML, saved baselines, suites, and checkpoint recovery retain domain reports. CSV keeps its original baseline-only layout; use the adjacent `profiles.json` or JSON/HTML for domain results. Profiles add no metric columns to the ordinary baseline summaries.

Suite defaults/cases accept the same boolean flags with underscores:

```json
{
  "version": 1,
  "defaults": {"runs": 10, "warmup": 1, "backend": true, "api": true},
  "benchmarks": [
    {"name": "api-small", "executable": "./api_test", "args": ["100"]},
    {"name": "api-large", "executable": "./api_test", "args": ["10000"]}
  ]
}
```

For hyphenated flags use `game_ai`, `load_testing`, `require_profile_data`, `frame_budget_ms`, and `deadline_ms`. Suite defaults follow the existing merging rules.

## Research behind the selections

These are design choices informed by first-party documentation and concrete developer reports, not a claim that these are the ten most common roles or a statistical survey of bugs. Sources reviewed on 2026-11-05:

- **Services and load testing:** [Google SRE's monitoring chapter](https://sre.google/sre-book/monitoring-distributed-systems/) emphasizes latency, traffic, errors, and saturation. [wrk2's design notes](https://github.com/giltene/wrk2) explain how coordinated omission hides latency spikes. These informed separate request/error counters and the warning against deriving request latency from process time.
- **Game graphics:** [Unity's profiling guidance](https://unity.com/how-to/best-practices-for-profiling-game-performance) separates CPU/GPU frame budgets and rendering waits. The graphics topic therefore accepts real frame samples and keeps device GPU percentages separate.
- **Frontend:** [Core Web Vitals guidance](https://web.dev/articles/top-cwv) identifies loading, responsiveness, and layout stability work. [W3C's conformance guidance](https://www.w3.org/WAI/WCAG22/Understanding/conformance) explains the need for automated and human evaluation. Browser data must be supplied; accessibility counts do not certify conformance.
- **Mobile:** [Android's common performance issues](https://developer.android.com/topic/performance/issues) cover rendering stalls, ANRs, startup, memory, and background work. Device telemetry is separated from the Linux driver's resource use.
- **Databases:** [PostgreSQL EXPLAIN documentation](https://www.postgresql.org/docs/current/using-explain.html) distinguishes query timing, buffer activity, and estimated plan costs. Query/lock timings need database instrumentation, not guessed conversions from RSS.
- **ML:** A developer reported large timing differences with synchronization in [PyTorch issue #18012](https://github.com/pytorch/pytorch/issues/18012). [PyTorch benchmark documentation](https://docs.pytorch.org/docs/main/utils/benchmark.html) explains accelerator synchronization in timing. ML/GPU metrics must measure the intended completed work.
- **Embedded/robotics:** [ROS 2's real-time background](https://design.ros2.org/articles/realtime_background.html) distinguishes low latency from deadline guarantees. The embedded topics report observed cycles, jitter, and misses without claiming hard real-time certification.
- **Systems:** [fio's documentation](https://fio.readthedocs.io/en/latest/fio_doc.html) distinguishes operation latency, IOPS, and bandwidth; [Clang's user manual](https://clang.llvm.org/docs/UsersManual.html) documents compilation time traces. These motivate explicit I/O operation/byte counters and compiler-phase telemetry.
- **Tracing:** The [strace manual](https://man7.org/linux/man-pages/man1/strace.1.html) documents annotated descriptors and unfinished/resumed syscalls. Its semantics underpin the automatic network collector and its limits.

## Complete metric catalog

The following table matches `cppbench --list-profiles`. Sample metrics take an array; counters/gauges take a scalar. Units are part of the telemetry contract.

| Profile / topic | Metric key | Unit | Kind |
| --- | --- | --- | --- |
| `networking / sockets` | `networking.socket_calls` | `count` | counter |
| `networking / sockets` | `networking.socket_errors` | `count` | counter |
| `networking / sockets` | `networking.connect_calls` | `count` | counter |
| `networking / bandwidth` | `networking.sent_bytes` | `B` | counter |
| `networking / bandwidth` | `networking.received_bytes` | `B` | counter |
| `networking / bandwidth` | `networking.network_calls` | `count` | counter |
| `networking / latency` | `networking.syscall_ms` | `ms` | samples |
| `networking / latency` | `networking.request_ms` | `ms` | samples |
| `networking / latency` | `networking.timeouts` | `count` | counter |
| `frontend / rendering` | `frontend.lcp_ms` | `ms` | samples |
| `frontend / rendering` | `frontend.cls` | `score` | gauge |
| `frontend / rendering` | `frontend.long_task_ms` | `ms` | samples |
| `frontend / interactivity` | `frontend.inp_ms` | `ms` | samples |
| `frontend / interactivity` | `frontend.input_delay_ms` | `ms` | samples |
| `frontend / interactivity` | `frontend.handler_ms` | `ms` | samples |
| `frontend / accessibility` | `frontend.a11y_violations` | `count` | counter |
| `frontend / accessibility` | `frontend.a11y_checks` | `count` | counter |
| `frontend / accessibility` | `frontend.keyboard_failures` | `count` | counter |
| `backend / api` | `backend.request_ms` | `ms` | samples |
| `backend / api` | `backend.requests` | `count` | counter |
| `backend / api` | `backend.errors` | `count` | counter |
| `backend / database` | `backend.query_ms` | `ms` | samples |
| `backend / database` | `backend.queries` | `count` | counter |
| `backend / database` | `backend.lock_wait_ms` | `ms` | samples |
| `backend / distributed` | `backend.rpc_ms` | `ms` | samples |
| `backend / distributed` | `backend.queue_ms` | `ms` | samples |
| `backend / distributed` | `backend.retries` | `count` | counter |
| `mobile / startup` | `mobile.ttid_ms` | `ms` | samples |
| `mobile / startup` | `mobile.ttfd_ms` | `ms` | samples |
| `mobile / startup` | `mobile.startup_failures` | `count` | counter |
| `mobile / responsiveness` | `mobile.frame_ms` | `ms` | samples |
| `mobile / responsiveness` | `mobile.anrs` | `count` | counter |
| `mobile / responsiveness` | `mobile.dropped_frames` | `count` | counter |
| `mobile / battery` | `mobile.energy_j` | `J` | gauge |
| `mobile / battery` | `mobile.wake_lock_ms` | `ms` | samples |
| `mobile / battery` | `mobile.wakeups` | `count` | counter |
| `gamedev / gameplay` | `gamedev.tick_ms` | `ms` | samples |
| `gamedev / gameplay` | `gamedev.physics_ms` | `ms` | samples |
| `gamedev / gameplay` | `gamedev.entities` | `count` | gauge |
| `gamedev / graphics` | `gamedev.frame_ms` | `ms` | samples |
| `gamedev / graphics` | `gamedev.gpu_frame_ms` | `ms` | samples |
| `gamedev / graphics` | `gamedev.draw_calls` | `count` | samples |
| `gamedev / game-ai` | `gamedev.pathfinding_ms` | `ms` | samples |
| `gamedev / game-ai` | `gamedev.decision_ms` | `ms` | samples |
| `gamedev / game-ai` | `gamedev.nodes_expanded` | `count` | counter |
| `data / pipelines` | `data.rows` | `count` | counter |
| `data / pipelines` | `data.input_bytes` | `B` | counter |
| `data / pipelines` | `data.stage_ms` | `ms` | samples |
| `data / analytics` | `data.query_ms` | `ms` | samples |
| `data / analytics` | `data.spill_bytes` | `B` | counter |
| `data / analytics` | `data.result_rows` | `count` | counter |
| `data / ml` | `data.inference_ms` | `ms` | samples |
| `data / ml` | `data.training_step_ms` | `ms` | samples |
| `data / ml` | `data.samples` | `count` | counter |
| `devops / ci` | `devops.stage_ms` | `ms` | samples |
| `devops / ci` | `devops.cache_hits` | `count` | counter |
| `devops / ci` | `devops.cache_misses` | `count` | counter |
| `devops / sre` | `devops.request_ms` | `ms` | samples |
| `devops / sre` | `devops.requests` | `count` | counter |
| `devops / sre` | `devops.errors` | `count` | counter |
| `devops / containers` | `devops.throttled_ms` | `ms` | gauge |
| `devops / containers` | `devops.oom_events` | `count` | counter |
| `devops / containers` | `devops.restarts` | `count` | counter |
| `qa / load-testing` | `qa.response_ms` | `ms` | samples |
| `qa / load-testing` | `qa.requests` | `count` | counter |
| `qa / load-testing` | `qa.failures` | `count` | counter |
| `qa / soak` | `qa.heap_bytes` | `B` | samples |
| `qa / soak` | `qa.open_fds` | `count` | samples |
| `qa / soak` | `qa.errors` | `count` | counter |
| `qa / correctness` | `qa.assertions` | `count` | counter |
| `qa / correctness` | `qa.assertion_failures` | `count` | counter |
| `qa / correctness` | `qa.cases` | `count` | counter |
| `embedded / realtime` | `embedded.cycle_ms` | `ms` | samples |
| `embedded / realtime` | `embedded.jitter_ms` | `ms` | samples |
| `embedded / realtime` | `embedded.deadline_misses` | `count` | counter |
| `embedded / iot` | `embedded.message_ms` | `ms` | samples |
| `embedded / iot` | `embedded.messages` | `count` | counter |
| `embedded / iot` | `embedded.energy_j` | `J` | gauge |
| `embedded / robotics` | `embedded.control_ms` | `ms` | samples |
| `embedded / robotics` | `embedded.sensor_ms` | `ms` | samples |
| `embedded / robotics` | `embedded.missed_updates` | `count` | counter |
| `systems / compilation` | `systems.parse_ms` | `ms` | samples |
| `systems / compilation` | `systems.optimize_ms` | `ms` | samples |
| `systems / compilation` | `systems.codegen_ms` | `ms` | samples |
| `systems / storage` | `systems.io_ms` | `ms` | samples |
| `systems / storage` | `systems.io_operations` | `count` | counter |
| `systems / storage` | `systems.io_bytes` | `B` | counter |
| `systems / concurrency` | `systems.lock_wait_ms` | `ms` | samples |
| `systems / concurrency` | `systems.tasks` | `count` | counter |
| `systems / concurrency` | `systems.queue_ms` | `ms` | samples |
