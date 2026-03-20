from __future__ import annotations

import argparse
import json
from pathlib import Path

from admission_core import load_json
from minimization_core import build_minimization_proof


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Evaluate Guild M5 draft minimization over an admissible M4 execution plan."
    )
    parser.add_argument("--plan", required=True, help="Path to an execution_plan JSON document")
    parser.add_argument("--contract", required=True, help="Path to a skill_contract JSON document")
    parser.add_argument("--request", required=True, help="Path to the admission_request JSON document used to derive the plan")
    parser.add_argument("--runtime", required=True, help="Path to the chosen runtime_guarantee JSON document")
    parser.add_argument("--invocation-input", required=True, help="Path to a deterministic invocation input JSON document")
    parser.add_argument("--comparator-profile", required=True, help="Path to a comparator_profile JSON document")
    parser.add_argument("--created-at", required=True, help="RFC3339 timestamp to stamp into the proof record")
    parser.add_argument("--expires-at", help="Optional RFC3339 expiry timestamp for the proof record")
    parser.add_argument("--cache-dir", help="Optional directory for conservative proof-cache reuse")
    parser.add_argument("--max-candidate-plans", type=int, default=128, help="Maximum discrete candidate plans to explore")
    parser.add_argument("--output", help="Write the proof JSON to this path instead of stdout")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    proof = build_minimization_proof(
        load_json(args.plan),
        load_json(args.contract),
        load_json(args.request),
        load_json(args.runtime),
        load_json(args.invocation_input),
        load_json(args.comparator_profile),
        created_at=args.created_at,
        expires_at=args.expires_at,
        cache_dir=args.cache_dir,
        max_candidate_plans=args.max_candidate_plans,
    )

    rendered = json.dumps(proof, indent=2, sort_keys=True) + "\n"
    if args.output:
        Path(args.output).write_text(rendered)
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
