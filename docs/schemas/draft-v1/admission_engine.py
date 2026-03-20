from __future__ import annotations

import argparse
import json
from pathlib import Path

from admission_core import AdmissionInputError, build_execution_plan, canonical_json, load_json


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Evaluate Guild M4 admission for a contract, admission request, and one or more runtime guarantees."
    )
    parser.add_argument("--request", required=True, help="Path to an admission_request JSON document")
    parser.add_argument("--contract", help="Path to a skill_contract JSON document when the request does not embed one")
    parser.add_argument(
        "--runtime",
        action="append",
        default=[],
        help="Path to a runtime_guarantee JSON document. Repeat for multiple candidate runtimes.",
    )
    parser.add_argument("--output", help="Write the execution plan JSON to this path instead of stdout")
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    request = load_json(args.request)
    contract = load_json(args.contract) if args.contract else None
    runtimes = [load_json(runtime_path) for runtime_path in args.runtime]

    try:
        plan = build_execution_plan(contract, request, runtimes)
    except AdmissionInputError as error:
        raise SystemExit(str(error)) from error

    rendered = json.dumps(plan, indent=2, sort_keys=True) + "\n"
    if args.output:
        output_path = Path(args.output)
        output_path.write_text(rendered)
    else:
        print(rendered, end="")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
