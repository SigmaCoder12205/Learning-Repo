"""A pandas/numpy data-analysis workload for benchmarking with bench.

Builds a deterministic synthetic sales table, then does a typical
clean -> join -> group -> aggregate pipeline. Prints a JSON summary that
examples/validators/check_analytics.py can verify.

    python3 -m venv .venv && .venv/bin/pip install -r examples/requirements-analytics.txt
    bench examples/analytics_workload.py --interpreter .venv/bin/python \
        --validate .venv/bin/python --validate-arg examples/validators/check_analytics.py \
        -- --rows 200000
"""
import argparse
import json

import numpy as np
import pandas as pd


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rows", type=int, default=100_000)
    parser.add_argument("--seed", type=int, default=7)
    args = parser.parse_args()
    if args.rows < 1:
        parser.error("--rows must be positive")

    rng = np.random.default_rng(args.seed)
    sales = pd.DataFrame(
        {
            "store": rng.integers(0, 50, args.rows),
            "product": rng.integers(0, 500, args.rows),
            "quantity": rng.integers(1, 10, args.rows),
            "discount": rng.choice([0.0, 0.05, 0.1, np.nan], args.rows),
        }
    )
    products = pd.DataFrame(
        {
            "product": np.arange(500),
            "category": np.array(["food", "toys", "tools", "books", "garden"])[np.arange(500) % 5],
            "price": np.round(rng.uniform(1, 100, 500), 2),
        }
    )
    sales["discount"] = sales["discount"].fillna(0.0)
    joined = sales.merge(products, on="product", how="left", validate="many_to_one")
    joined["revenue"] = joined["quantity"] * joined["price"] * (1 - joined["discount"])
    totals = joined.groupby("category", sort=True)["revenue"].sum().round(2)
    print(
        json.dumps(
            {
                "rows": int(len(joined)),
                "expected_rows": args.rows,
                "group_totals": totals.to_dict(),
                "grand_total": round(float(totals.sum()), 2),
            }
        )
    )


if __name__ == "__main__":
    main()
