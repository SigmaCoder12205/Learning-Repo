# Working on cppbench

cppbench is a Linux command-line executable built from Rust and a small C measurement layer. Build requirements are Rust/Cargo, a C compiler, and a linker. The integration tests also need `g++` and standard Linux shell utilities. Tool-format tests use fixtures and fake launchers; the normal test suite does not require GPU hardware, elevated permissions, or installed profilers.

## Local workflow

Run commands from this directory:

```bash
make fmt       # Format Rust sources
make verify    # Formatting, Clippy, C/header checks, and all tests
make build     # Optimized executable at target/release/cppbench
```

For narrower checks:

```bash
make check                       # Rust formatting and Clippy
make check-c                     # C measurement code and public telemetry header
cargo test --locked --test cli    # Public CLI behavior
cargo test --locked --test profiles
cargo test --locked diagnostics::gpu
cargo run --locked -- /bin/true --runs 2 --warmup 0
```

`make install` installs the release binary, telemetry header, and profile/account/memory guides under `~/.local` by default. `PREFIX` and `DESTDIR` retain their usual meanings. `CC` selects the compiler for the C checks. Keep dependency changes deliberate and update `Cargo.lock` when dependencies change.

Generated executables belong in `build/`. Cargo outputs go in `target/`. Diagnostic sessions live in `cppbench-artifacts/` and are ignored at any directory depth, including suite-relative artifact directories. These directories can contain useful benchmark evidence; the development commands do not delete them. `.editorconfig` defines basic whitespace conventions; `cargo fmt` is authoritative for Rust formatting.

## Code map

| Area | Responsibility |
| --- | --- |
| `src/main.rs` | Entrypoint and top-level error-to-exit-code conversion |
| `src/cli.rs` | Shared CLI option definitions and value parsing |
| `src/app.rs` | Mode selection, output destinations, comparisons, and final output |
| `src/benchmark.rs` | Compile-once support, warmups, measured invocations, adaptive stopping, and diagnostic handoff |
| `src/runner.rs` | Embedded helper, sealed request transport, C FFI ownership, output capture, and correctness checks |
| `native/launcher.c` | Fresh C helper launch, signal forwarding, and result transport (its own resource usage is discarded) |
| `native/helper.c` | Bounded versioned request decoding in a clean C address space |
| `native/measure.c`, `native/measure.h` | Target launch, wait4 accounting, deadlines, affinity, signals, and cleanup |
| `src/metrics.rs` | Baseline metric IDs, units, and selection groups |
| `src/stats.rs` | Percentiles, descriptive summaries, bootstrap intervals, and precision estimates |
| `src/experiment.rs` | Settings validation, provenance, and successful-run aggregation |
| `src/storage.rs` | Atomic JSON writes, checkpoint append/sync, and recovery |
| `src/accounts/{mod,browser,provider,store}.rs` | OAuth CLI commands, PKCE callback, Keycloak requests, and private refresh-token cache |
| `src/suite.rs` | Suite parsing, preflight, sequential execution, and suite exports |
| `src/comparison.rs` | Compatibility checks, regression thresholds, and experiment exit codes |
| `src/report.rs`, `src/html.rs` | Text/CSV/JSON presentation and standalone HTML |
| `src/diagnostics/mod.rs` | Select and sequence optional diagnostic replays |
| `src/diagnostics/support.rs` | Tool discovery, bounded artifact reads, shared invocation/error handling |
| `src/diagnostics/{perf,gpu,heaptrack,valgrind}.rs` | Each tool's command, parser, and colocated parser tests |
| `src/diagnostics/check.rs` | Installed-tool identity and counter-access checks |
| `src/hotspots.rs` | CPU stack sampling, function summaries, and flamegraph SVG |
| `src/profiles/mod.rs` | Profile flag registration, selection, and report assembly |
| `src/profiles/catalog.rs` | Profile/topic definitions and typed metric kinds |
| `src/profiles/telemetry.rs` | Application telemetry validation and its separate replay |
| `src/profiles/network.rs` | Socket tracing and strace parsing |
| `src/profiles/presentation.rs` | Domain text sections shared by terminal and HTML reports |
| `include/cppbench_telemetry.h` | Optional public C11/C++ telemetry writer |

## Execution flow and boundaries

1. The application parses options, validates mode restrictions and destinations, and loads any comparison baseline.
2. A single experiment resolves or compiles its target. Suites validate their entries first, then use the same experiment implementation for each case.
3. The runner serializes a versioned request (little-endian scalar fields and length-prefixed NUL-terminated byte strings), seals it in a memfd, and launches the embedded C helper through the C FFI. The helper decodes it and supervises a fresh target child. Only that child's usage is reported; helper startup/transport are outside measured time. Rust owns descriptors and result storage until the synchronous call returns. C signal handling is process-wide; do not parallelize target invocations inside this process without redesigning that boundary.
4. Warmups and measured runs receive correctness checks. Only successful measured runs enter baseline summaries. Journaling happens outside measured execution time.
5. Optional tools and telemetry execute as separate replays. They retain their own artifacts and never enter baseline timing/resource aggregates.
6. The application compares results, completes the journal, and renders the requested format. Recovery reads completed records without running the target again.

The baseline metric order is part of the Rust/C interface: `metrics::METRICS`, the C measurement array, and the Rust FFI layout must agree. The FFI safety comment in `runner.rs` describes the required lifetimes. Any layout or process-lifecycle change needs focused validation of timeouts, interruption, descendant cleanup, and output handling.

Reports use schema-versioned JSON values. Internal report builders establish their shape; parsers validate external tool output, telemetry, and checkpoints at their boundaries. Preserve units, source labels, completion state, and the difference between missing data and measured zero. The report format is consumed by saved baselines, recovery, suites, and HTML downloads as well as terminal output.

## Adding or changing a profile

Start in `profiles/catalog.rs`. A profile declares its main flag, baseline context, three topics, and each topic's metrics. Use `Metric::counter`, `Metric::gauge`, or `Metric::samples` so unsupported metric kinds cannot be introduced as spelling mistakes. The `MetricKind` enum has stable serialized names: `counter`, `gauge`, and `samples`.

The catalog drives CLI registration, parent/subflag validation, discovery output, and telemetry validation. Keep flags and metric keys unique. Preserve existing keys/units unless making an explicit compatibility change. Current tests enforce the public 10-profile, 3-topic, 3-metric structure; update that contract deliberately when expanding the catalog.

Use `profiles/telemetry.rs` for new application-derived calculations and `profiles/presentation.rs` for their presentation. Update [the profile guide](profiles.md), including its metric table and measurement limitations. Supply a realistic example when the metric requires application instrumentation. Do not infer request latency, FPS, GPU time, or device energy from process lifetime.

## Adding or changing a diagnostic tool

Keep command construction and parsing in the same tool module under `diagnostics/`. Reuse the shared runner and bounded readers rather than creating another process/timeout implementation. Register the tool in `diagnostics/mod.rs`; add presentation in `report.rs` where needed. CPU hotspots and network profiles have their own specialized modules listed above.

Keep the parser independently testable. Add a small representative fixture under `tests/fixtures/`, retaining output quirks that matter while removing unrelated paths or sensitive content. Test missing fields, unavailable values, malformed/truncated output, and status propagation. Preserve raw artifacts when parsing cannot establish a valid result. Document whether a metric is target-specific, device-wide, or supplied by application telemetry.

## Testing expectations

- Unit tests live beside statistics, parsers, catalog rules, and presentation code.
- `tests/accounts.rs` covers OAuth browser callbacks/PKCE, token refresh/revocation, issuer binding, and storage protections.
- `tests/cli.rs` covers invocation, compilation, metric selection, output checking, timeouts, and interruption.
- `tests/diagnostics.rs` exercises tool wrappers with fake launchers and verifies replay isolation and cleanup.
- `tests/experiments.rs` covers comparisons, suites, affinity, adaptive limits, HTML escaping, and recovery.
- `tests/profiles.rs` covers domain selection, telemetry, networking fixtures, profile regression gates, and the public header in C/C++.

For structural changes, preserve CLI help, the profile catalog, serialized field names, and exit behavior. Existing integration tests provide the main behavior checks; add tests for new behavior or uncovered failures rather than mirroring private implementation details. Real profiler smoke checks complement fixtures when changing a tool integration, but should stay optional for contributors without the hardware or permissions.

## Account service

Keycloak owns all account/password logic; keep the Rust account module a protocol client. `auth/compose.yaml` supplies the local Keycloak/PostgreSQL services, and `auth/realm.json` seeds configuration. No password grants or admin credentials belong in the CLI. The client uses userinfo from the configured issuer rather than trusting decoded JWT claims. Offline refresh credentials are bound to that issuer and saved under private filesystem permissions. All benchmark paths stay independent of the account service.

`auth/tests/smoke.py --restart` is an optional real-service check (Python standard library only, development tooling). It creates and cleans up a unique test account. Run it after `cargo build` and `./auth/scripts/setup.sh`. The service guide explains persistence, bootstrap secrets, session policy, and live-realm changes.

## Measurement helper

`build.rs` compiles the target-platform helper executable and embeds its bytes through `include_bytes!`. It also links the small launcher FFI library. `native/measure.h` defines both the Rust-facing result layout and the helper reply. Keep protocol encoding in `runner.rs` and decoding in `helper.c` in agreement, incrementing the request version if its layout changes. The helper executes with a clean environment; the original target environment travels in the sealed request, not in argv. Preserve early INT/TERM handling across exec, propagation to the target, and cleanup/reaping. See [memory accounting](memory.md) for the residual C pre-exec footprint and waited-child scope. Changing this backend requires a new measurement provenance identifier and compatibility checks.
