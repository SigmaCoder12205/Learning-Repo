PREFIX ?= $(HOME)/.local

.PHONY: build install test check
build:
	cargo build --release --locked

install: build
	install -Dm755 target/release/cppbench "$(DESTDIR)$(PREFIX)/bin/cppbench"
	install -Dm644 include/cppbench_telemetry.h "$(DESTDIR)$(PREFIX)/include/cppbench_telemetry.h"
	install -Dm644 docs/profiles.md "$(DESTDIR)$(PREFIX)/share/doc/cppbench/profiles.md"

test:
	cargo test --locked

check:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
