#!/usr/bin/env python3
"""Deprecated draft-v1 benchmark entrypoint.

The authoritative repo-native path now lives in Rust under:

    cargo run -q -p xtask -- draft-v1 benchmark write
    cargo run -q -p xtask -- draft-v1 benchmark check
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Deprecated benchmark entrypoint.")
    parser.add_argument(
        "--check-artifacts",
        action="store_true",
        help="Run the Rust-native benchmark artifact check instead of the write path.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    verb = "check" if args.check_artifacts else "write"
    print(
        "benchmark_real_path.py is deprecated. "
        f"Use `cargo run -q -p xtask -- draft-v1 benchmark {verb}` instead.",
        file=sys.stderr,
    )
    result = subprocess.run(
        ["cargo", "run", "-q", "-p", "xtask", "--", "draft-v1", "benchmark", verb],
        cwd=REPO_ROOT,
        check=False,
    )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
