#!/bin/sh
# Smoke-test a release archive: extract it, check it is static, and run real benchmarks.
set -eu
archive=$1
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tar -xzf "$archive" -C "$work"
root=$(find "$work" -mindepth 1 -maxdepth 1 -type d)
bin="$root/bin/bench"
if command -v file >/dev/null; then
    file -L "$bin"
    case $(file -L "$bin") in
        *x86-64*statically\ linked* | *x86-64*static-pie\ linked*) ;;
        *x86-64*) case "$archive" in *musl*) echo "musl archive is not static" >&2; exit 1 ;; esac ;;
    esac
fi
test -L "$root/bin/cppbench"
test -f "$root/share/licenses/bench/LICENSE"
test -f "$root/include/bench_telemetry.h"
"$bin" --version
"$root/bin/cppbench" --version
"$bin" /bin/true --runs 2 --warmup 0 --format json > "$work/true.json"
grep -q '"measurement_passed": true' "$work/true.json"
# The embedded C helper must start on this machine, including PTY-free stdin replay.
"$bin" /bin/cat --input-line hello --expect-text 'hello
' --runs 2 --warmup 0 >/dev/null
if command -v python3 >/dev/null; then
    printf 'print(sum(range(1000)))\n' > "$work/w.py"
    "$bin" "$work/w.py" --runs 2 --warmup 0 --non-interactive --expect-text '499500
' >/dev/null
fi
echo "smoke test passed: $archive"
