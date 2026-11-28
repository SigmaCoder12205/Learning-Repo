PREFIX ?= $(HOME)/.local
# Optional Cargo features for build/install, e.g. `make install FEATURES=accounts`.
FEATURES ?=

.PHONY: build install install-files dist test fmt check-format lint check check-c verify
build:
	cargo build --release --locked --features "$(FEATURES)"

install: build
	$(MAKE) install-files BIN=target/release/bench

# Copy an already built binary plus headers, docs and license; used by install, dist and PKGBUILD.
BIN ?= target/release/bench
DOCS := docs/profiles.md docs/accounts.md docs/memory.md docs/python.md docs/features.md docs/correctness.md docs/inputs.md docs/validators.md docs/analytics.md docs/roadmap.md
install-files:
	install -Dm755 "$(BIN)" "$(DESTDIR)$(PREFIX)/bin/bench"
	ln -sfn bench "$(DESTDIR)$(PREFIX)/bin/cppbench"
	install -Dm644 include/bench_telemetry.h "$(DESTDIR)$(PREFIX)/include/bench_telemetry.h"
	install -Dm644 include/cppbench_telemetry.h "$(DESTDIR)$(PREFIX)/include/cppbench_telemetry.h"
	install -Dm644 -t "$(DESTDIR)$(PREFIX)/share/doc/bench" README.md $(DOCS)
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/bench/LICENSE"

# Release archive: dist/bench-VERSION-TARGET.tar.gz plus a .sha256 file.
# Extract into a prefix, e.g. tar -xzf ARCHIVE --strip-components=1 -C ~/.local
VERSION := $(shell sed -n 's/^version = "\(.*\)"$$/\1/p' Cargo.toml | head -n 1)
DIST_TARGET ?= $(shell rustc -vV | sed -n 's/^host: //p')
DIST_NAME = bench-$(VERSION)-$(DIST_TARGET)
dist:
	cargo build --release --locked --target "$(DIST_TARGET)" --features "$(FEATURES)"
	rm -rf "dist/$(DIST_NAME)" "dist/$(DIST_NAME).tar.gz" "dist/$(DIST_NAME).tar.gz.sha256"
	$(MAKE) install-files PREFIX= DESTDIR="dist/$(DIST_NAME)" BIN="target/$(DIST_TARGET)/release/bench"
	tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$${SOURCE_DATE_EPOCH:-0}" \
		-C dist -czf "dist/$(DIST_NAME).tar.gz" "$(DIST_NAME)"
	cd dist && sha256sum "$(DIST_NAME).tar.gz" > "$(DIST_NAME).tar.gz.sha256"
	@echo "dist/$(DIST_NAME).tar.gz"

test:
	cargo test --locked
	cargo test --locked --all-features

fmt:
	cargo fmt

check-format:
	cargo fmt --check

lint:
	cargo clippy --locked --all-targets -- -D warnings
	cargo clippy --locked --all-targets --all-features -- -D warnings

check: check-format lint

check-c:
	$(CC) -std=c11 -Wall -Wextra -Werror -fsyntax-only native/measure.c
	$(CC) -std=c11 -Wall -Wextra -Werror -fsyntax-only native/helper.c native/launcher.c
	$(CC) -std=c11 -Wall -Wextra -Werror -fsyntax-only native/interactive.c
	$(CC) -std=c11 -Wall -Wextra -Werror -x c -fsyntax-only include/bench_telemetry.h
	$(CC) -std=c11 -Wall -Wextra -Werror -x c -fsyntax-only include/cppbench_telemetry.h
	$(CXX) -std=c++17 -Wall -Wextra -Werror -x c++ -fsyntax-only include/bench_telemetry.h
	$(CXX) -std=c++17 -Wall -Wextra -Werror -x c++ -fsyntax-only include/cppbench_telemetry.h

verify: check check-c test
