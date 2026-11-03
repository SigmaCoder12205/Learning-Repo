PREFIX ?= $(HOME)/.local

.PHONY: build install test check
build:
	cargo build --release --locked

install: build
	install -Dm755 target/release/cppbench "$(DESTDIR)$(PREFIX)/bin/cppbench"

test:
	cargo test --locked

check:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
