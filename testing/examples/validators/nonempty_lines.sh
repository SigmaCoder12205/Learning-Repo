#!/bin/sh
# bench validator in plain shell: require at least $MIN_LINES nonempty stdout lines
# (default 1) and report the line count as a metric. stdin is the target's stdout.
lines=$(grep -c . || true)
echo "bench-metric lines $lines count"
if [ "$lines" -lt "${MIN_LINES:-1}" ]; then
    echo "expected at least ${MIN_LINES:-1} nonempty lines, got $lines"
    exit 1
fi
