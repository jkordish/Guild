from __future__ import annotations

from copy import deepcopy
from typing import Any

from admission_core import load_json
from token_core import attach_protection
from witness_core import generate_witness, witness_redaction_digest


def witness_issuer() -> dict[str, Any]:
    return {
        "issuer_id": "urn:guild:issuer:draft-control-plane:v1",
        "key_id": "draft-hmac-2026-03",
        "shared_secret": "guild-draft-shared-secret-2026-03",
        "issuer_epoch": 3,
    }


def witness_issuer_keys() -> dict[str, dict[str, str]]:
    issuer = witness_issuer()
    return {issuer["issuer_id"]: {issuer["key_id"]: issuer["shared_secret"]}}


def fetch_transform_full_effects(invocation: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "effect_class": "fs.read",
            "scope": {
                "kind": "filesystem",
                "paths": [invocation["config_path"]],
                "symlink_policy": "deny",
                "follow_mounts": False,
            },
            "cardinality": {
                "max_calls": 1,
                "max_bytes": 256,
            },
        },
        {
            "effect_class": "fs.write",
            "scope": {
                "kind": "filesystem",
                "paths": [invocation["output_path"]],
                "symlink_policy": "deny",
                "follow_mounts": False,
            },
            "cardinality": {
                "max_calls": 1,
                "max_bytes": 4096,
            },
        },
        {
            "effect_class": "secret.read",
            "scope": {
                "kind": "secret",
                "secret_ids": [invocation["secret_id"]],
            },
            "cardinality": {
                "max_calls": 1,
            },
        },
        {
            "effect_class": "net.connect",
            "scope": {
                "kind": "network",
                "audiences": [
                    {
                        "host": "api.vendor.example.com",
                        "ports": [443],
                        "schemes": ["https"],
                        "path_prefixes": ["/v1/source/daily.json"],
                        "methods": ["GET"],
                    }
                ],
            },
            "cardinality": {
                "max_calls": 1,
                "max_bytes": 1048576,
            },
        },
        {
            "effect_class": "clock.read",
            "scope": {
                "kind": "system",
            },
            "cardinality": {
                "max_calls": 1,
            },
        },
    ]


def fetch_transform_full_coverage(invocation: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        {
            "family": "fs.read",
            "draft_effect_class": "fs.read",
            "scope_kind": "filesystem",
            "status": "complete",
            "mapping_status": "exact",
            "scope_descriptors": [invocation["config_path"]],
            "supports_positive_facts": True,
            "supports_absence_claims": True,
            "reason_codes": [],
        },
        {
            "family": "fs.write",
            "draft_effect_class": "fs.write",
            "scope_kind": "filesystem",
            "status": "complete",
            "mapping_status": "exact",
            "scope_descriptors": [invocation["output_path"]],
            "supports_positive_facts": True,
            "supports_absence_claims": True,
            "reason_codes": [],
        },
        {
            "family": "secret.read",
            "draft_effect_class": "secret.read",
            "scope_kind": "secret",
            "status": "complete",
            "mapping_status": "exact",
            "scope_descriptors": [invocation["secret_id"]],
            "supports_positive_facts": True,
            "supports_absence_claims": True,
            "reason_codes": [],
        },
        {
            "family": "net.connect",
            "draft_effect_class": "net.connect",
            "scope_kind": "network",
            "status": "complete",
            "mapping_status": "exact",
            "scope_descriptors": [
                "GET:https://api.vendor.example.com:443/v1/source/daily.json"
            ],
            "supports_positive_facts": True,
            "supports_absence_claims": True,
            "reason_codes": [],
        },
        {
            "family": "clock.read",
            "draft_effect_class": "clock.read",
            "scope_kind": "system",
            "status": "complete",
            "mapping_status": "exact",
            "scope_descriptors": ["clock.read"],
            "supports_positive_facts": True,
            "supports_absence_claims": True,
            "reason_codes": [],
        },
    ]


def local_log_out_of_envelope_observation() -> dict[str, Any]:
    return {
        "source_id": "urn:guild:observation:fixture:local-log:out-of-envelope:v1",
        "source_kind": "bounded-observation-fixture",
        "version": "1.0.0",
        "notes": "Bounded fixture with one unauthorized network effect added to an otherwise valid local-log invocation.",
        "observed_effects": [
            {
                "effect_class": "fs.read",
                "scope": {
                    "kind": "filesystem",
                    "paths": ["/workspace/input/a.log"],
                    "symlink_policy": "deny",
                    "follow_mounts": False,
                },
                "cardinality": {
                    "max_calls": 1,
                    "max_bytes": 128,
                },
            },
            {
                "effect_class": "net.connect",
                "scope": {
                    "kind": "network",
                    "audiences": [
                        {
                            "host": "telemetry.example.net",
                            "ports": [443],
                            "schemes": ["https"],
                            "path_prefixes": ["/upload"],
                            "methods": ["POST"],
                        }
                    ],
                },
                "cardinality": {
                    "max_calls": 1,
                    "max_bytes": 4096,
                },
            },
        ],
        "coverage_families": [
            {
                "family": "fs.read",
                "draft_effect_class": "fs.read",
                "scope_kind": "filesystem",
                "status": "complete",
                "mapping_status": "exact",
                "scope_descriptors": ["/workspace/input/a.log"],
                "supports_positive_facts": True,
                "supports_absence_claims": True,
                "reason_codes": [],
            },
            {
                "family": "net.connect",
                "draft_effect_class": "net.connect",
                "scope_kind": "network",
                "status": "complete",
                "mapping_status": "exact",
                "scope_descriptors": ["POST:https://telemetry.example.net:443/upload"],
                "supports_positive_facts": True,
                "supports_absence_claims": True,
                "reason_codes": [],
            },
        ],
        "raw_trace": {
            "events": ["read", "unauthorized_egress"],
        },
    }


def fetch_transform_coverage_limited_observation(invocation: dict[str, Any]) -> dict[str, Any]:
    coverage = fetch_transform_full_coverage(invocation)
    coverage[3] = {
        "family": "net.connect",
        "draft_effect_class": "net.connect",
        "scope_kind": "network",
        "status": "partial",
        "mapping_status": "exact",
        "scope_descriptors": ["GET:https://api.vendor.example.com:443/v1/source/daily.json"],
        "supports_positive_facts": True,
        "supports_absence_claims": False,
        "reason_codes": [],
    }
    return {
        "source_id": "urn:guild:observation:fixture:fetch-transform:coverage-limited:v1",
        "source_kind": "bounded-observation-fixture",
        "version": "1.0.0",
        "notes": "Network coverage is partial. Positive network facts are visible, but absence claims are not provable.",
        "observed_effects": fetch_transform_full_effects(invocation),
        "coverage_families": coverage,
        "raw_trace": {
            "events": ["config_read", "write", "secret", "sampled_network", "clock"],
        },
    }


def fetch_transform_redacted_observation(invocation: dict[str, Any]) -> dict[str, Any]:
    return {
        "source_id": "urn:guild:observation:fixture:fetch-transform:redacted:v1",
        "source_kind": "bounded-observation-fixture",
        "version": "1.0.0",
        "notes": "Complete bounded observation before witness redaction removes concrete effect details.",
        "observed_effects": fetch_transform_full_effects(invocation),
        "coverage_families": fetch_transform_full_coverage(invocation),
        "raw_trace": {
            "events": ["config_read", "write", "secret", "network", "clock"],
        },
    }


def fetch_transform_blocked_attempt_observation(invocation: dict[str, Any]) -> dict[str, Any]:
    coverage = fetch_transform_full_coverage(invocation)
    coverage[3] = {
        "family": "net.connect",
        "draft_effect_class": "net.connect",
        "scope_kind": "network",
        "status": "complete",
        "mapping_status": "exact",
        "scope_descriptors": [
            "GET:https://api.vendor.example.com:443/v1/source/daily.json",
            "POST:https://exfil.example.net:443/bulk",
        ],
        "supports_positive_facts": True,
        "supports_absence_claims": True,
        "reason_codes": [],
    }
    return {
        "source_id": "urn:guild:observation:fixture:fetch-transform:blocked-attempt:v1",
        "source_kind": "bounded-observation-fixture",
        "version": "1.0.0",
        "notes": "Bounded fixture that distinguishes exercised local authority from a blocked egress attempt.",
        "observed_effects": [
            effect
            for effect in fetch_transform_full_effects(invocation)
            if effect["effect_class"] != "net.connect"
        ],
        "blocked_attempts_observable": True,
        "blocked_attempts": [
            {
                "effect": {
                    "effect_class": "net.connect",
                    "scope": {
                        "kind": "network",
                        "audiences": [
                            {
                                "host": "exfil.example.net",
                                "ports": [443],
                                "schemes": ["https"],
                                "path_prefixes": ["/bulk"],
                                "methods": ["POST"],
                            }
                        ],
                    },
                    "cardinality": {
                        "max_calls": 1,
                        "max_bytes": 4096,
                    },
                },
                "reason_code": "RUNTIME_DENIED",
                "message": "runtime blocked egress outside the admissible network envelope",
                "details": {
                    "stage": "hook",
                },
            }
        ],
        "coverage_families": coverage,
        "raw_trace": {
            "events": ["config_read", "write", "secret", "clock", "blocked_egress"],
        },
    }


def cluster_rollout_observation() -> dict[str, Any]:
    return {
        "source_id": "urn:guild:observation:fixture:cluster-rollout:v1",
        "source_kind": "bounded-observation-fixture",
        "version": "1.0.0",
        "notes": "Bounded fixture for the delegated rollout PATCH call.",
        "observed_effects": [
            {
                "effect_class": "net.connect",
                "scope": {
                    "kind": "network",
                    "audiences": [
                        {
                            "host": "kube-api.prod.example.internal",
                            "ports": [443],
                            "schemes": ["https"],
                            "path_prefixes": ["/apis/apps/v1/namespaces/prod/deployments/api"],
                            "methods": ["PATCH"],
                        }
                    ],
                },
                "cardinality": {
                    "max_calls": 1,
                    "max_bytes": 2048,
                },
            }
        ],
        "coverage_families": [
            {
                "family": "net.connect",
                "draft_effect_class": "net.connect",
                "scope_kind": "network",
                "status": "complete",
                "mapping_status": "exact",
                "scope_descriptors": [
                    "PATCH:https://kube-api.prod.example.internal:443/apis/apps/v1/namespaces/prod/deployments/api"
                ],
                "supports_positive_facts": True,
                "supports_absence_claims": True,
                "reason_codes": [],
            },
            {
                "family": "capability.delegate",
                "draft_effect_class": "capability.delegate",
                "scope_kind": "delegation",
                "status": "complete",
                "mapping_status": "exact",
                "scope_descriptors": ["delegation-chain"],
                "supports_positive_facts": True,
                "supports_absence_claims": True,
                "reason_codes": [],
            },
        ],
        "raw_trace": {
            "events": ["delegated_patch"],
        },
    }


def runtime_mapping_limited_observation() -> dict[str, Any]:
    return {
        "source_id": "urn:guild:observation:fixture:runtime-mapping-limited:v1",
        "source_kind": "live-runtime-hook",
        "version": "1.0.0",
        "notes": "Runtime-native inspect metadata could not be mapped safely into the draft-v1 vocabulary.",
        "observed_effects": [],
        "coverage_families": [
            {
                "family": "rust.inspect.http_request",
                "draft_effect_class": None,
                "scope_kind": None,
                "status": "insufficient",
                "mapping_status": "unsupported",
                "scope_descriptors": ["runtime-native:http_request"],
                "supports_positive_facts": False,
                "supports_absence_claims": False,
                "reason_codes": [],
            }
        ],
        "unmapped_observations": [
            {
                "family": "rust.inspect.http_request",
                "observed_as": "exercised",
                "details_summary": "Runtime exposed http_request metadata that draft-v1 cannot map losslessly.",
                "details": {
                    "method": "GET",
                    "surface": "inspect-hook",
                },
                "coverage_status": "insufficient",
                "reason_codes": ["RUNTIME_OBSERVATION_UNMAPPABLE"],
            }
        ],
        "raw_trace": {
            "native_event": "http_request",
        },
    }


def build_witness_fixtures() -> dict[str, dict[str, Any]]:
    issuer = witness_issuer()
    issuer_keys = witness_issuer_keys()

    local_plan = load_json("examples/local-log-analyzer.admit.plan.json")
    local_contract = load_json("examples/local-log-analyzer.contract.json")
    local_invocation = load_json("examples/local-log-analyzer.invocation.json")
    local_proof = load_json("examples/local-log-analyzer.proof.json")
    local_token = load_json("examples/local-log-analyzer.proof-backed.root-token.json")

    fetch_plan = load_json("examples/fetch-transform.no-reduction.plan.json")
    fetch_contract = load_json("examples/fetch-transform.contract.json")
    fetch_invocation = load_json("examples/fetch-transform.invocation.json")

    zero_plan = load_json("examples/zero-authority.admit.plan.json")
    zero_contract = load_json("examples/zero-authority.contract.json")
    zero_invocation = load_json("examples/zero-authority.invocation.json")
    zero_proof = load_json("examples/zero-authority.proof.json")
    zero_token = load_json("examples/zero-authority.empty-token.json")

    cluster_plan = load_json("examples/cluster-rollout.admit.plan.json")
    cluster_contract = load_json("examples/cluster-rollout.contract.json")
    cluster_root_token = load_json("examples/cluster-rollout.root-token.json")
    cluster_child_token = load_json("examples/cluster-rollout.child-token.json")

    local_within = generate_witness(
        plan=local_plan,
        contract=local_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T13:02:00Z",
        invocation_input=local_invocation,
        proof=local_proof,
        token=local_token,
        redaction_profile="summary_only",
    )

    local_out = generate_witness(
        plan=local_plan,
        contract=local_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T13:02:30Z",
        proof=local_proof,
        token=local_token,
        observation=local_log_out_of_envelope_observation(),
        redaction_profile="none",
    )

    fetch_coverage = generate_witness(
        plan=fetch_plan,
        contract=fetch_contract,
        issuer=issuer,
        issued_at="2026-03-20T13:03:00Z",
        observation=fetch_transform_coverage_limited_observation(fetch_invocation),
        redaction_profile="none",
    )

    fetch_redacted = generate_witness(
        plan=fetch_plan,
        contract=fetch_contract,
        issuer=issuer,
        issued_at="2026-03-20T13:03:40Z",
        observation=fetch_transform_redacted_observation(fetch_invocation),
        redaction_profile="counts_only",
    )

    fetch_blocked = generate_witness(
        plan=fetch_plan,
        contract=fetch_contract,
        issuer=issuer,
        issued_at="2026-03-20T13:03:30Z",
        observation=fetch_transform_blocked_attempt_observation(fetch_invocation),
        redaction_profile="none",
    )

    zero_authority = generate_witness(
        plan=zero_plan,
        contract=zero_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T13:16:00Z",
        invocation_input=zero_invocation,
        proof=zero_proof,
        token=zero_token,
        redaction_profile="summary_only",
    )

    cluster_witness = generate_witness(
        plan=cluster_plan,
        contract=cluster_contract,
        issuer=issuer,
        issuer_keys=issuer_keys,
        issued_at="2026-03-20T13:05:20Z",
        token=cluster_child_token,
        parent_token=cluster_root_token,
        observation=cluster_rollout_observation(),
        redaction_profile="none",
    )

    mapping_limited = generate_witness(
        plan=fetch_plan,
        contract=fetch_contract,
        issuer=issuer,
        issued_at="2026-03-20T13:04:00Z",
        observation=runtime_mapping_limited_observation(),
        redaction_profile="none",
    )

    runtime_mismatch = deepcopy(local_within)
    runtime_mismatch["runtime_binding"]["runtime_guarantee_id"] = "urn:guild:runtime:node-wasi-basic:v1"
    runtime_mismatch["redaction"]["redacted_content_digest"] = witness_redaction_digest(runtime_mismatch)
    runtime_mismatch = attach_protection(runtime_mismatch, issuer["shared_secret"])

    return {
        "local-log-analyzer.within-envelope.witness.json": {
            "witness": local_within,
            "plan": local_plan,
            "contract": local_contract,
            "proof": local_proof,
            "token": local_token,
            "parent_token": None,
        },
        "local-log-analyzer.out-of-envelope.witness.json": {
            "witness": local_out,
            "plan": local_plan,
            "contract": local_contract,
            "proof": local_proof,
            "token": local_token,
            "parent_token": None,
        },
        "fetch-transform.coverage-limited.witness.json": {
            "witness": fetch_coverage,
            "plan": fetch_plan,
            "contract": fetch_contract,
            "proof": None,
            "token": None,
            "parent_token": None,
        },
        "fetch-transform.redacted-claim-blocked.witness.json": {
            "witness": fetch_redacted,
            "plan": fetch_plan,
            "contract": fetch_contract,
            "proof": None,
            "token": None,
            "parent_token": None,
        },
        "fetch-transform.blocked-attempt.witness.json": {
            "witness": fetch_blocked,
            "plan": fetch_plan,
            "contract": fetch_contract,
            "proof": None,
            "token": None,
            "parent_token": None,
        },
        "zero-authority.witness.json": {
            "witness": zero_authority,
            "plan": zero_plan,
            "contract": zero_contract,
            "proof": zero_proof,
            "token": zero_token,
            "parent_token": None,
        },
        "cluster-rollout.witness.json": {
            "witness": cluster_witness,
            "plan": cluster_plan,
            "contract": cluster_contract,
            "proof": None,
            "token": cluster_child_token,
            "parent_token": cluster_root_token,
        },
        "runtime-mapping-limited.witness.json": {
            "witness": mapping_limited,
            "plan": fetch_plan,
            "contract": fetch_contract,
            "proof": None,
            "token": None,
            "parent_token": None,
        },
        "local-log-analyzer.runtime-mismatch.witness.json": {
            "witness": runtime_mismatch,
            "plan": local_plan,
            "contract": local_contract,
            "proof": local_proof,
            "token": local_token,
            "parent_token": None,
        },
    }
