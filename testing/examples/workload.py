"""Deterministic, standard-library workload for bench's Python examples."""
import argparse
import hashlib


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--size", type=int, default=10000)
    args = parser.parse_args()
    if args.size < 1:
        parser.error("--size must be positive")
    digest = b"bench"
    for _ in range(args.size):
        digest = hashlib.sha256(digest).digest()
    print(digest.hex())


if __name__ == "__main__":
    main()
