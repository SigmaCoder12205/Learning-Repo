PREFIX ?= $(HOME)/.local

.PHONY: build install test fmt check-format lint check check-c verify
build:
	cargo build --release --locked

install: build
	install -Dm755 target/release/cppbench "$(DESTDIR)$(PREFIX)/bin/cppbench"
	install -Dm644 include/cppbench_telemetry.h "$(DESTDIR)$(PREFIX)/include/cppbench_telemetry.h"
	install -Dm644 docs/profiles.md "$(DESTDIR)$(PREFIX)/share/doc/cppbench/profiles.md"
	install -Dm644 docs/accounts.md "$(DESTDIR)$(PREFIX)/share/doc/cppbench/accounts.md"

test:
	cargo test --locked

fmt:
	cargo fmt

check-format:
	cargo fmt --check

lint:
	cargo clippy --locked --all-targets -- -D warnings

check: check-format lint

check-c:
	$(CC) -std=c11 -Wall -Wextra -Werror -fsyntax-only native/measure.c
	$(CC) -std=c11 -Wall -Wextra -Werror -x c -fsyntax-only include/cppbench_telemetry.h

verify: check check-c test
