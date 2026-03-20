from __future__ import annotations

import argparse
import json
from pathlib import Path

from admission_core import load_json
from token_core import create_child_token, create_root_token, verify_token


def parse_resource_binding(value: str) -> dict:
    return json.loads(value)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Issue and verify Guild draft-v1 M6 delegated capability tokens.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    issue_root = subparsers.add_parser("issue-root", help="Issue one root token from an admissible execution plan")
    issue_root.add_argument("--plan", required=True, help="Path to an execution_plan JSON document")
    issue_root.add_argument("--contract", required=True, help="Path to a skill_contract JSON document")
    issue_root.add_argument("--proof", help="Optional path to a proof_record JSON document")
    issue_root.add_argument("--holder-id", required=True, help="Explicit holder identity bound into the token")
    issue_root.add_argument("--issuer-id", required=True, help="Issuer identity")
    issue_root.add_argument("--key-id", required=True, help="Issuer key identifier")
    issue_root.add_argument("--shared-secret", required=True, help="Shared secret used for draft-local HMAC MAC protection")
    issue_root.add_argument("--issuer-epoch", type=int, default=0, help="Optional local issuer epoch")
    issue_root.add_argument("--issued-at", required=True, help="RFC3339 issue timestamp")
    issue_root.add_argument("--not-before", help="Optional RFC3339 not-before timestamp")
    issue_root.add_argument("--expires-at", help="Optional RFC3339 expiry timestamp")
    issue_root.add_argument("--token-id", help="Optional explicit token identifier")
    issue_root.add_argument("--allow-upper-bound", action="store_true", help="Allow issuance from the M4 upper bound when no acceptable proof exists")
    issue_root.add_argument("--audience", action="append", default=[], help="Optional explicit audience label. Repeat for more than one")
    issue_root.add_argument("--resource-binding-json", action="append", default=[], help="Optional JSON object with effect_class/audience/resource. Repeat for more than one")
    issue_root.add_argument("--chain-link", action="append", default=[], help="Optional explicit call-chain link. Repeat to override the plan default")
    issue_root.add_argument("--output", help="Write the JSON result to this path instead of stdout")

    issue_child = subparsers.add_parser("issue-child", help="Issue one delegated child token from a validated parent token")
    issue_child.add_argument("--parent-token", required=True, help="Path to the parent token JSON document")
    issue_child.add_argument("--plan", required=True, help="Path to an execution_plan JSON document")
    issue_child.add_argument("--contract", required=True, help="Path to a skill_contract JSON document")
    issue_child.add_argument("--authority-plan", required=True, help="Path to an authority_plan JSON object for the child token")
    issue_child.add_argument("--proof", help="Optional path to a proof_record JSON document")
    issue_child.add_argument("--holder-id", required=True, help="Explicit child holder identity")
    issue_child.add_argument("--issuer-id", required=True, help="Issuer identity")
    issue_child.add_argument("--key-id", required=True, help="Issuer key identifier")
    issue_child.add_argument("--shared-secret", required=True, help="Shared secret used for draft-local HMAC MAC protection")
    issue_child.add_argument("--issuer-epoch", type=int, default=0, help="Optional local issuer epoch")
    issue_child.add_argument("--issued-at", required=True, help="RFC3339 issue timestamp")
    issue_child.add_argument("--not-before", help="Optional RFC3339 not-before timestamp")
    issue_child.add_argument("--expires-at", help="Optional RFC3339 expiry timestamp")
    issue_child.add_argument("--token-id", help="Optional explicit child token identifier")
    issue_child.add_argument("--audience", action="append", default=[], help="Optional explicit audience label. Repeat for more than one")
    issue_child.add_argument("--resource-binding-json", action="append", default=[], help="Optional JSON object with effect_class/audience/resource. Repeat for more than one")
    issue_child.add_argument("--output", help="Write the JSON result to this path instead of stdout")

    verify = subparsers.add_parser("verify", help="Verify one token against explicit runtime, holder, audience, and replay context")
    verify.add_argument("--token", required=True, help="Path to the token JSON document")
    verify.add_argument("--issuer-id", required=True, help="Issuer identity expected by the verifier")
    verify.add_argument("--key-id", required=True, help="Key identifier expected by the verifier")
    verify.add_argument("--shared-secret", required=True, help="Shared secret used for draft-local HMAC MAC protection")
    verify.add_argument("--verification-time", required=True, help="RFC3339 verification timestamp")
    verify.add_argument("--holder-id", required=True, help="Holder identity expected by the verifier")
    verify.add_argument("--runtime-guarantee-id", required=True, help="Runtime guarantee identifier expected at redemption time")
    verify.add_argument("--plan", help="Optional execution_plan JSON document for linkage verification")
    verify.add_argument("--contract", help="Optional skill_contract JSON document for linkage verification")
    verify.add_argument("--proof", help="Optional proof_record JSON document for proof-backed verification")
    verify.add_argument("--parent-token", help="Optional parent token JSON document for child-token verification")
    verify.add_argument("--audience", action="append", default=[], help="Expected audience label. Repeat for more than one")
    verify.add_argument("--resource-binding-json", action="append", default=[], help="Expected resource-binding JSON object. Repeat for more than one")
    verify.add_argument("--chain-link", action="append", default=[], help="Presented call-chain link. Repeat for more than one")
    verify.add_argument("--replay-state-dir", help="Optional local verifier state directory used for replay checks")
    verify.add_argument("--revoked-token-id", action="append", default=[], help="Optional revoked token id. Repeat for more than one")
    verify.add_argument("--minimum-issuer-epoch", type=int, default=0, help="Optional minimum issuer epoch accepted by the verifier")
    verify.add_argument("--output", help="Write the JSON result to this path instead of stdout")

    return parser.parse_args()


def render_output(payload: dict, output: str | None) -> None:
    rendered = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if output:
        Path(output).write_text(rendered)
    else:
        print(rendered, end="")


def main() -> int:
    args = parse_args()
    if args.command == "issue-root":
        payload = create_root_token(
            load_json(args.plan),
            load_json(args.contract),
            {
                "issuer_id": args.issuer_id,
                "key_id": args.key_id,
                "shared_secret": args.shared_secret,
                "issuer_epoch": args.issuer_epoch,
            },
            holder_id=args.holder_id,
            issued_at=args.issued_at,
            proof=load_json(args.proof) if args.proof else None,
            allow_upper_bound=args.allow_upper_bound,
            audiences=args.audience or None,
            resource_bindings=[parse_resource_binding(item) for item in args.resource_binding_json] or None,
            token_id=args.token_id,
            not_before=args.not_before,
            expires_at=args.expires_at,
            chain_links=args.chain_link or None,
        )
        render_output(payload, args.output)
        return 0

    if args.command == "issue-child":
        payload = create_child_token(
            load_json(args.parent_token),
            load_json(args.plan),
            load_json(args.contract),
            load_json(args.authority_plan),
            {
                "issuer_id": args.issuer_id,
                "key_id": args.key_id,
                "shared_secret": args.shared_secret,
                "issuer_epoch": args.issuer_epoch,
            },
            holder_id=args.holder_id,
            issued_at=args.issued_at,
            proof=load_json(args.proof) if args.proof else None,
            audiences=args.audience or None,
            resource_bindings=[parse_resource_binding(item) for item in args.resource_binding_json] or None,
            token_id=args.token_id,
            not_before=args.not_before,
            expires_at=args.expires_at,
        )
        render_output(payload, args.output)
        return 0

    issuer_keys = {
        args.issuer_id: {
            args.key_id: args.shared_secret,
        }
    }
    payload = verify_token(
        load_json(args.token),
        issuer_keys=issuer_keys,
        verification_time=args.verification_time,
        expected_holder_id=args.holder_id,
        expected_audiences=args.audience or None,
        expected_resources=[parse_resource_binding(item) for item in args.resource_binding_json] or None,
        expected_runtime_guarantee_id=args.runtime_guarantee_id,
        expected_call_chain_links=args.chain_link or None,
        plan=load_json(args.plan) if args.plan else None,
        contract=load_json(args.contract) if args.contract else None,
        proof=load_json(args.proof) if args.proof else None,
        parent_token=load_json(args.parent_token) if args.parent_token else None,
        replay_state_dir=args.replay_state_dir,
        revoked_token_ids=set(args.revoked_token_id),
        minimum_issuer_epochs={args.issuer_id: args.minimum_issuer_epoch},
    )
    render_output(payload, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
