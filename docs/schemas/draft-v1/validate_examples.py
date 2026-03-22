#!/usr/bin/env python3
"""Deprecated draft-v1 truth entrypoint.

The authoritative repo-native truth path now lives in Rust under:

    cargo run -q -p xtask -- draft-v1 truth check
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]


def main() -> int:
    print(
        "validate_examples.py is deprecated. "
        "Use `cargo run -q -p xtask -- draft-v1 truth check` instead.",
        file=sys.stderr,
    )
    result = subprocess.run(
        ["cargo", "run", "-q", "-p", "xtask", "--", "draft-v1", "truth", "check"],
        cwd=REPO_ROOT,
        check=False,
    )
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
