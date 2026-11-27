# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

**bench** (formerly `cppbench`) is a Linux-only CLI for benchmarking native executables and Python scripts. The engine is **Rust + a small C measurement layer**; the user explicitly wants it to stay that way. Python is a supported workload, not a replacement implementation.

This directory lives inside a larger parent Git repository (the repo root is `..`). Other directories there (`projects/`, `code/`, etc.) contain unrelated user work. Leave them alone, and don't commit unless asked.

## Continuity docs (read before substantial work)

- `AGENTS.md`: read `HANDOFF.md` at the start of work and update it after substantial changes. Never copy `auth/.env` contents, credentials, or tokens into docs.
- `HANDOFF.md`: user requirements, version history (currently 0.14.0), compatibility decisions, previous verification, and known limitations. It is a dated snapshot. Current code and new user instructions take precedence over it.
- `docs/development.md`: the authoritative development guide, with a full code map, execution flow, and checklists for adding profiles and diagnostic tools.
- Feature guides in `docs/` (`profiles`, `inputs`, `correctness`, `validators`, `analytics`, `python`, `memory`, `features`, `experiments`, `accounts`, `roadmap`) are installed by `make install`. Update the relevant guide when behavior changes.

## Commands

```bash
make fmt          # cargo fmt
make verify       # fmt check + clippy (-D warnings) + C/header checks + all tests; clippy and tests run for both the default build and --all-features
make build        # release binary at target/release/bench
make check        # Rust formatting + Clippy only
make check-c      # C sources and telemetry headers (C11, and C++17 for the headers)
make install      # FEATURES=accounts adds `bench account`; ~/.local/bin/bench + cppbench symlink, headers, docs (PREFIX/DESTDIR honored)

cargo test --locked --test cli          # one integration test file (tests/*.rs)
cargo test --locked diagnostics::gpu    # unit tests by module path
cargo test --locked --test cli some_test_name
cargo run --locked -- /bin/true --runs 2 --warmup 0
```

Integration tests need `g++`, `python3` with `venv`, and standard shell utilities. They do **not** need GPU hardware, root, or installed profilers, because tool output is covered by fixtures in `tests/fixtures/` and fake launchers. Always pass `--locked`, and update `Cargo.lock` only for deliberate dependency changes.

## Architecture

Call chain: `main.rs` → `app.rs` (mode selection, output, comparisons) → `benchmark.rs` / `suite.rs` → `runner.rs` → `native/launcher.c` → embedded helper (`native/helper.c` + `native/measure.c`) → target process.

- `build.rs` compiles `helper.c` and `measure.c` into a standalone executable, which is embedded with `include_bytes!` and run from a sealed memfd. It also links `launcher.c` and `interactive.c` as a static lib (plus `libutil`). Install therefore needs only the single `bench` binary.
- `runner.rs` serializes a **versioned little-endian request** that `helper.c` decodes. Keep the encoder and decoder in sync, and bump the request version whenever the layout changes.
- The **metric order is part of the Rust/C ABI**: `metrics::METRICS`, the C metric array, and the FFI result layout in `native/measure.h` must agree. See the FFI safety comment in `runner.rs`.
- The backend identifier `isolated-c-wait4-v1` is recorded in reports for baseline compatibility. Any change to measurement semantics needs a new identifier and compatibility handling.
- C signal handling is process-wide, so target invocations must stay **sequential**. Don't parallelize them in-process.
- `native/interactive.c` is a separate PTY supervisor that runs only for the **unmeasured** terminal-capture trial. Never route timed runs through it.
- Optional diagnostics (`src/diagnostics/*`, `hotspots.rs`, `profiles/network.rs`, telemetry) run as **separate replays**. Their results never enter baseline timing or resource aggregates. Each tool module keeps its command construction and parser together, and uses the shared runner and bounded readers in `diagnostics/support.rs`.
- Domain profiles are driven by `profiles/catalog.rs`, which feeds CLI registration, validation, discovery output, and telemetry validation. Tests enforce the public **10 profiles × 3 topics × 3 metrics** contract. Use `Metric::counter`, `Metric::gauge`, or `Metric::samples`.
- Reports are schema-versioned JSON. Saved baselines, recovery, suites, history, and HTML all depend on them, so preserve field names, units, and the distinction between missing data and measured zero.
- `--validate` programs run inside `runner::run_logged` after a successful target run, via the same C launcher (sequential, unmeasured). They are never counted in timing.
- `bench analyze` (`src/analysis.rs`) embeds `python/bench_analysis.py` with `include_str!` and runs it with a user-chosen interpreter. Rust flattens reports, and Python only does statistics. Tests must not require pandas. The real-pandas test runs only when `BENCH_TEST_PANDAS_PYTHON` or `python3` provides it.
- Accounts (`src/accounts/`) sit behind the optional `accounts` Cargo feature, which is off by default and enables `reqwest`, `base64` and `zeroize`. They are a pure OAuth/PKCE protocol client for the local Keycloak + PostgreSQL stack in `auth/`. Benchmarking never depends on it. The realm, client, volume, and cache paths intentionally keep their `cppbench` identifiers, so don't rename them. The optional live check is `auth/tests/smoke.py --restart`, run after `cargo build --features accounts` and `./auth/scripts/setup.sh`.

## Compatibility to preserve

CLI help text, the profile catalog, serialized field names, exit codes, the `cppbench` symlink, and the legacy `include/cppbench_telemetry.h` header and environment variables. Generated artifacts go in `bench-artifacts/`, `build/`, and `target/`. These are gitignored, and nothing should delete them automatically.

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- When graphify-out/graph.json exists, run `graphify query "<question>"` before your first grep, search or source-file read for a codebase question, and put this rule in every subagent prompt that explores code. Read raw files only for lines the graph or an earlier read already located. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. When the answer is under a Markdown heading, `graphify section "<heading>"` prints that section's text, so the .md need not be opened. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).
- A committed graphify-out/graph.json is rewritten whole on every build, so a large diff to it in a pull request is expected. Do not flag it as churn or suggest skipping the graph rebuild or its hooks.
