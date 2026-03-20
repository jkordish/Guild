from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from admission_core import load_json
from witness_core import generate_witness, verify_claim, verify_witness


def load_optional_json(path: str | None) -> dict[str, Any] | None:
    if path is None:
        return None
    return load_json(path)


def emit(value: dict[str, Any], output_path: str | None) -> None:
    rendered = json.dumps(value, indent=2, sort_keys=True) + "\n"
    if output_path is None:
        sys.stdout.write(rendered)
        return
    Path(output_path).write_text(rendered)


def command_generate(args: argparse.Namespace) -> int:
    issuer = load_json(args.issuer)
    issuer_keys = load_optional_json(args.issuer_keys)
    witness = generate_witness(
        plan=load_json(args.plan),
        contract=load_json(args.contract),
        issuer=issuer,
        issued_at=args.issued_at,
        invocation_input=load_optional_json(args.invocation_input),
        proof=load_optional_json(args.proof),
        token=load_optional_json(args.token),
        parent_token=load_optional_json(args.parent_token),
        token_verification_result=load_optional_json(args.token_verification_result),
        observation=load_optional_json(args.observation),
        issuer_keys=issuer_keys,
        witness_id=args.witness_id,
        redaction_profile=args.redaction_profile,
        started_at=args.started_at,
        finished_at=args.finished_at,
        notes=args.notes,
    )
    emit(witness, args.output)
    return 0


def command_verify(args: argparse.Namespace) -> int:
    result = verify_witness(
        load_json(args.witness),
        issuer_keys=load_json(args.issuer_keys),
        verification_time=args.verification_time,
        plan=load_optional_json(args.plan),
        contract=load_optional_json(args.contract),
        proof=load_optional_json(args.proof),
        token=load_optional_json(args.token),
        parent_token=load_optional_json(args.parent_token),
        raw_trace=load_optional_json(args.raw_trace),
    )
    emit(result, args.output)
    return 0 if result["verified"] else 1


def command_verify_claim(args: argparse.Namespace) -> int:
    result = verify_claim(
        load_json(args.witness),
        load_json(args.claim),
        issuer_keys=load_json(args.issuer_keys),
        verification_time=args.verification_time,
        plan=load_optional_json(args.plan),
        contract=load_optional_json(args.contract),
        proof=load_optional_json(args.proof),
        token=load_optional_json(args.token),
        parent_token=load_optional_json(args.parent_token),
        raw_trace=load_optional_json(args.raw_trace),
    )
    emit(result, args.output)
    claim_status = result["claim_evaluation"]["status"]
    return 0 if result["verified"] and claim_status == "satisfied" else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Generate and verify draft-v1 witness records.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    generate = subparsers.add_parser("generate", help="Generate a witness record.")
    generate.add_argument("--plan", required=True)
    generate.add_argument("--contract", required=True)
    generate.add_argument("--issuer", required=True)
    generate.add_argument("--issuer-keys")
    generate.add_argument("--issued-at", required=True)
    generate.add_argument("--invocation-input")
    generate.add_argument("--proof")
    generate.add_argument("--token")
    generate.add_argument("--parent-token")
    generate.add_argument("--token-verification-result")
    generate.add_argument("--observation")
    generate.add_argument("--witness-id")
    generate.add_argument("--redaction-profile", default="summary_only")
    generate.add_argument("--started-at")
    generate.add_argument("--finished-at")
    generate.add_argument("--notes")
    generate.add_argument("--output")
    generate.set_defaults(func=command_generate)

    verify = subparsers.add_parser("verify", help="Verify a witness record.")
    verify.add_argument("--witness", required=True)
    verify.add_argument("--issuer-keys", required=True)
    verify.add_argument("--verification-time", required=True)
    verify.add_argument("--plan")
    verify.add_argument("--contract")
    verify.add_argument("--proof")
    verify.add_argument("--token")
    verify.add_argument("--parent-token")
    verify.add_argument("--raw-trace")
    verify.add_argument("--output")
    verify.set_defaults(func=command_verify)

    verify_claim_cmd = subparsers.add_parser("verify-claim", help="Verify a fixed claim against a witness.")
    verify_claim_cmd.add_argument("--witness", required=True)
    verify_claim_cmd.add_argument("--claim", required=True)
    verify_claim_cmd.add_argument("--issuer-keys", required=True)
    verify_claim_cmd.add_argument("--verification-time", required=True)
    verify_claim_cmd.add_argument("--plan")
    verify_claim_cmd.add_argument("--contract")
    verify_claim_cmd.add_argument("--proof")
    verify_claim_cmd.add_argument("--token")
    verify_claim_cmd.add_argument("--parent-token")
    verify_claim_cmd.add_argument("--raw-trace")
    verify_claim_cmd.add_argument("--output")
    verify_claim_cmd.set_defaults(func=command_verify_claim)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
