#!/usr/bin/env python3
"""bench validator for examples/analytics_workload.py: check the JSON answer.

Reports the group count and grand total as metrics, and fails if the totals do
not add up, so a faster but wrong change to the workload cannot pass.
"""
import json
import math
import sys

result = json.loads(sys.stdin.read())
groups = result["group_totals"]
print(f"bench-metric groups {len(groups)} count")
print(f"bench-metric grand_total {result['grand_total']}")
if not math.isclose(sum(groups.values()), result["grand_total"], rel_tol=1e-9):
    print("group totals do not add up to the grand total")
    sys.exit(1)
if result["rows"] != result["expected_rows"]:
    print(f"processed {result['rows']} rows, expected {result['expected_rows']}")
    sys.exit(1)
