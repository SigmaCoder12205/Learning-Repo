#!/usr/bin/env python3
"""bench validator: the target reads N on stdin and must print 1 + 2 + ... + N last.

Protocol: stdin is the target's stdout; exit 0 passes, any other exit fails.
`bench-metric NAME VALUE [UNIT]` lines become per-run metrics; other lines are the
message shown when a run fails.
"""
import os
import sys

n = int(open(os.environ["BENCH_INPUT_FILE"]).read().split()[0])
words = sys.stdin.read().split()
if not words:
    print("target printed nothing")
    sys.exit(1)
got = int(words[-1])
want = n * (n + 1) // 2
print(f"bench-metric absolute_error {abs(got - want)}")
if got != want:
    print(f"expected {want}, got {got}")
    sys.exit(1)
