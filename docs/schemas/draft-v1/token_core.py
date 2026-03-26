from __future__ import annotations

import base64
import hashlib
import hmac
import json
from copy import deepcopy
from datetime import UTC, datetime, timedelta
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from admission_core import (
    build_registry,
    canonical_json,
    digest_struct,
    effect_covers,
    effect_is_canonical,
    effect_selector,
    host_matches_suffix,
    path_pattern_covers,
    require_valid,
    same_effect_selector,
    stable_unique_dicts,
    stable_unique_strings,
    validate_instance,
)


TOKEN_VERSION = "1.0.0"
VERIFICATION_RESULT_VERSION = "1.0.0"
TOKEN_CANONICALIZATION = "guild-json-c14n-v1"
TOKEN_PROTECTION_MODE = "hmac-sha256"
TOKEN_KIND = "guild.delegated_capability_token"
VERIFICATION_KIND = "guild.token_verification_result"
ISSUANCE_RESULT_KIND = "guild.token_issuance_result"
ACCEPTABLE_PROOF_STATUSES = {"exact_minimal", "bounded_minimal", "reduced", "no_reduction"}
PROOF_SOURCE_DRAFT_EXAMPLE = "draft-example"
PROOF_SOURCE_LIVE_RUNTIME = "live-runtime"


class TokenInputError(RuntimeError):
    """Raised when token issuance or verification inputs are malformed."""


def proof_source_kind(proof: dict[str, Any] | None) -> str:
    if proof is None:
        return PROOF_SOURCE_DRAFT_EXAMPLE
    return proof.get("proof_source_kind", PROOF_SOURCE_DRAFT_EXAMPLE)


def parse_timestamp(value: str) -> datetime:
    normalized = value.replace("Z", "+00:00")
    return datetime.fromisoformat(normalized).astimezone(UTC)


def format_timestamp(value: datetime) -> str:
    return value.astimezone(UTC).isoformat().replace("+00:00", "Z")


def add_seconds(value: str, seconds: int) -> str:
    return format_timestamp(parse_timestamp(value) + timedelta(seconds=seconds))


def coerce_path(value: str | Path | None) -> Path | None:
    if value is None:
        return None
    return Path(value)


def stable_unique_resource_bindings(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return stable_unique_dicts([deepcopy(value) for value in values])


def stable_unique_host_exact_bindings(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return stable_unique_dicts([deepcopy(value) for value in values])


def resource_binding_selector_field(binding: dict[str, Any]) -> str:
    if "family" in binding:
        return "family"
    return "effect_class"


def resource_binding_selector(binding: dict[str, Any]) -> str:
    return binding[resource_binding_selector_field(binding)]


def default_port_for_scheme(scheme: str) -> int | None:
    if scheme == "https":
        return 443
    if scheme == "http":
        return 80
    return None


def parse_http_request_descriptor(value: str) -> tuple[str, Any] | None:
    method, sep, raw_url = value.partition(":")
    if not sep:
        return None
    parsed = urlparse(raw_url)
    if not parsed.scheme or not parsed.hostname:
        return None
    port = parsed.port or default_port_for_scheme(parsed.scheme)
    if port is None:
        return None
    path = parsed.path or "/"
    return method.upper(), {
        "scheme": parsed.scheme,
        "hostname": parsed.hostname,
        "port": port,
        "path": path,
    }


def resource_kind_for_uri(uri: str) -> str | None:
    if uri.startswith("guild://executions/"):
        return "execution"
    if uri.startswith("guild://objects/sha256/") or uri.startswith("guild://objects/records/"):
        return "object"
    if uri.startswith("guild://queries/executions/"):
        return "query"
    return None


def parse_emit_evidence_descriptor(value: str) -> tuple[str | None, str | None]:
    audience = None
    redaction = None
    for chunk in value.split(";"):
        key, _, item_value = chunk.partition("=")
        if key == "audience":
            audience = item_value or None
        elif key == "redaction":
            redaction = item_value or None
    return audience, redaction


def emit_evidence_exact_binding_matches_grant(binding: dict[str, Any], grant: dict[str, Any]) -> bool:
    if grant.get("family") != "emit-evidence":
        return False
    scope = grant.get("scope")
    cardinality = grant.get("cardinality")
    if not isinstance(scope, dict) or not isinstance(cardinality, dict):
        return False
    if scope.get("kind") != "evidence":
        return False
    audiences = stable_unique_strings(sorted(scope.get("audiences") or []))
    redactions = stable_unique_strings(sorted(scope.get("redactions") or []))
    if audiences != [binding["audience"]] or redactions != [binding["redaction"]]:
        return False
    if cardinality.get("max_calls") != binding["emission_count"]:
        return False
    if cardinality.get("max_bytes") != binding["size_bytes"]:
        return False
    return True


def host_exact_bindings_match_authority(
    bindings: list[dict[str, Any]],
    authority_plan: dict[str, Any],
) -> bool:
    grants = authority_plan.get("grants", [])
    if not isinstance(grants, list):
        return False
    for binding in bindings:
        family = binding.get("family")
        if family != "emit-evidence":
            return False
        if not any(
            isinstance(grant, dict) and emit_evidence_exact_binding_matches_grant(binding, grant)
            for grant in grants
        ):
            return False
    return True


def issuance_result(
    decision: str,
    issued: bool,
    reason_codes: list[str],
    *,
    token: dict[str, Any] | None = None,
    message: str,
    detail: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "kind": ISSUANCE_RESULT_KIND,
        "version": TOKEN_VERSION,
        "decision": decision,
        "issued": issued,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
        "message": message,
        "detail": detail or {},
        "token": token,
    }


def verify_result(
    *,
    decision: str,
    verified: bool,
    verification_time: str,
    token: dict[str, Any] | None,
    reason_codes: list[str],
    holder_id: str | None,
    audiences: list[str],
    resources: list[dict[str, Any]],
    runtime_guarantee_id: str | None,
    call_chain_digest: dict[str, Any] | None,
    replay_mode: str,
    replay_checked: bool,
    replay_cache_key: dict[str, Any] | None,
    notes: str | None = None,
) -> dict[str, Any]:
    issuer_id = None
    key_id = None
    token_id = None
    token_digest = None
    if token is not None:
        issuer = token.get("issuer", {})
        issuer_id = issuer.get("issuer_id")
        key_id = issuer.get("key_id")
        token_id = token.get("token_id")
        token_digest = digest_struct(token)
    result = {
        "kind": VERIFICATION_KIND,
        "version": VERIFICATION_RESULT_VERSION,
        "decision": decision,
        "verified": verified,
        "verification_time": verification_time,
        "token_id": token_id,
        "token_digest": token_digest,
        "issuer_id": issuer_id,
        "key_id": key_id,
        "reason_codes": stable_unique_strings(sorted(reason_codes)),
        "bound_context": {
            "holder_id": holder_id,
            "audiences": stable_unique_strings(sorted(audiences)),
            "resources": stable_unique_resource_bindings(resources),
            "runtime_guarantee_id": runtime_guarantee_id,
            "call_chain_digest": call_chain_digest,
        },
        "replay_state": {
            "mode": replay_mode,
            "checked": replay_checked,
            "replay_cache_key": replay_cache_key,
        },
    }
    if notes is not None:
        result["notes"] = notes
    return result


def token_payload_without_protection(token: dict[str, Any]) -> dict[str, Any]:
    payload = deepcopy(token)
    payload.pop("protection", None)
    return payload


def protection_for_payload(payload: dict[str, Any], shared_secret: str) -> dict[str, Any]:
    payload_bytes = canonical_json(payload).encode("utf-8")
    mac_value = hmac.new(shared_secret.encode("utf-8"), payload_bytes, hashlib.sha256).digest()
    return {
        "mode": TOKEN_PROTECTION_MODE,
        "canonicalization": TOKEN_CANONICALIZATION,
        "claims_digest": digest_struct(payload),
        "mac_base64": base64.b64encode(mac_value).decode("ascii"),
    }


def attach_protection(token: dict[str, Any], shared_secret: str) -> dict[str, Any]:
    payload = token_payload_without_protection(token)
    protected = deepcopy(payload)
    protected["protection"] = protection_for_payload(payload, shared_secret)
    return protected


def validate_issuer_config(issuer: dict[str, Any]) -> None:
    required = ("issuer_id", "key_id", "shared_secret")
    for key in required:
        if not issuer.get(key):
            raise TokenInputError(f"missing issuer field {key!r}")
    issuer_epoch = issuer.get("issuer_epoch", 0)
    if not isinstance(issuer_epoch, int) or issuer_epoch < 0:
        raise TokenInputError("issuer_epoch must be a non-negative integer")


def validate_plan_contract_alignment(plan: dict[str, Any], contract: dict[str, Any]) -> list[str]:
    reason_codes: list[str] = []
    if plan.get("decision") == "refuse" or plan.get("chosen_runtime") is None:
        reason_codes.append("PLAN_NOT_ADMISSIBLE")
    if plan.get("contract_id") != contract.get("contract_id"):
        reason_codes.append("PLAN_CONTRACT_MISMATCH")
    if plan.get("contract_digest") != digest_struct(contract):
        reason_codes.append("PLAN_CONTRACT_MISMATCH")
    if plan.get("component_digest") != contract.get("component", {}).get("digest"):
        reason_codes.append("PLAN_CONTRACT_MISMATCH")
    if plan.get("export_name") != contract.get("export", {}).get("name"):
        reason_codes.append("PLAN_CONTRACT_MISMATCH")
    if plan.get("input_class_fingerprint") != contract.get("input_class_fingerprint"):
        reason_codes.append("PLAN_CONTRACT_MISMATCH")
    return stable_unique_strings(sorted(reason_codes))


def validate_proof_alignment(plan: dict[str, Any], contract: dict[str, Any], proof: dict[str, Any], now: str) -> list[str]:
    reason_codes: list[str] = []
    if proof.get("execution_plan_id") != plan.get("plan_id") or proof.get("execution_plan_digest") != digest_struct(plan):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    if proof.get("chosen_runtime") != plan.get("chosen_runtime"):
        reason_codes.append("PROOF_RUNTIME_MISMATCH")
    if proof.get("skill_contract_id") != contract.get("contract_id"):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    if proof.get("contract_digest") != digest_struct(contract):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    if proof.get("component_digest") != contract.get("component", {}).get("digest"):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    if proof.get("export_name") != plan.get("export_name"):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    if proof.get("input_class_fingerprint") != plan.get("input_class_fingerprint"):
        reason_codes.append("PROOF_PLAN_MISMATCH")
    proof_status = proof.get("proof_status")
    if proof_status not in ACCEPTABLE_PROOF_STATUSES:
        reason_codes.append("PROOF_NOT_ACCEPTABLE")
    proof_expires = proof.get("expires_at")
    if proof_expires is not None and parse_timestamp(now) > parse_timestamp(proof_expires):
        reason_codes.append("PROOF_NOT_ACCEPTABLE")
    return stable_unique_strings(sorted(reason_codes))


def effect_within_scope(binding: dict[str, Any], grant: dict[str, Any]) -> bool:
    if resource_binding_selector_field(binding) != ("family" if effect_is_canonical(grant) else "effect_class"):
        return False
    if resource_binding_selector(binding) != effect_selector(grant):
        return False

    scope = grant["scope"]
    kind = scope["kind"]
    resource = binding["resource"]
    selector = effect_selector(grant)

    if effect_is_canonical(grant):
        if selector == "http-request":
            parsed = parse_http_request_descriptor(resource)
            if parsed is None:
                return False
            method, request = parsed
            schemes = scope.get("allowed_schemes")
            if schemes is not None and request["scheme"] not in schemes:
                return False
            hosts = scope.get("allowed_hosts")
            host_suffixes = scope.get("allowed_host_suffixes")
            if hosts is not None and request["hostname"] not in hosts:
                if host_suffixes is None or not any(host_matches_suffix(request["hostname"], suffix) for suffix in host_suffixes):
                    return False
            elif hosts is None and host_suffixes is not None and not any(
                host_matches_suffix(request["hostname"], suffix) for suffix in host_suffixes
            ):
                return False
            ports = scope.get("allowed_ports")
            if ports is not None and request["port"] not in ports:
                return False
            methods = scope.get("allowed_methods")
            if methods is not None and method not in methods:
                return False
            prefixes = scope.get("allowed_path_prefixes")
            if prefixes is not None and not any(request["path"].startswith(prefix) for prefix in prefixes):
                return False
            return True

        if selector == "read-resource":
            prefixes = scope.get("uri_prefixes")
            if prefixes is not None and not any(resource.startswith(prefix) for prefix in prefixes):
                return False
            resource_kinds = scope.get("resource_kinds")
            if resource_kinds is not None:
                resource_kind = resource_kind_for_uri(resource)
                if resource_kind is None or resource_kind not in resource_kinds:
                    return False
            return True

        if selector == "invoke-skill":
            aliases = scope.get("aliases")
            return aliases is None or resource in aliases

        if selector == "emit-evidence":
            audience, redaction = parse_emit_evidence_descriptor(resource)
            if audience is None or redaction is None:
                return False
            audiences = scope.get("audiences")
            redactions = scope.get("redactions")
            return (audiences is None or audience in audiences) and (redactions is None or redaction in redactions)

        if selector == "log-write":
            levels = scope.get("levels")
            return levels is None or resource in levels

    if kind == "filesystem":
        return any(path_pattern_covers(pattern, resource) for pattern in scope["paths"])

    if kind == "network":
        parsed = urlparse(resource)
        if not parsed.scheme or not parsed.hostname:
            return False
        port = parsed.port
        if port is None:
            if parsed.scheme == "https":
                port = 443
            elif parsed.scheme == "http":
                port = 80
        for audience in scope["audiences"]:
            host_ok = audience["host"] == "*" or audience["host"] == parsed.hostname
            ports = audience.get("ports", ["*"])
            port_ok = "*" in ports or port in ports
            schemes = audience.get("schemes")
            scheme_ok = schemes is None or "*" in schemes or parsed.scheme in schemes
            prefixes = audience.get("path_prefixes")
            path_ok = prefixes is None or any(parsed.path.startswith(prefix) for prefix in prefixes)
            if host_ok and port_ok and scheme_ok and path_ok:
                return True
        return False

    if kind == "secret":
        return resource in scope.get("secret_ids", [])

    if kind == "component":
        return resource in scope.get("exports", [])

    if kind == "environment":
        return resource in scope.get("names", [])

    if kind == "delegation":
        return binding["audience"] in scope.get("audiences", [])

    return False


def resource_bindings_within_grants(bindings: list[dict[str, Any]], grants: list[dict[str, Any]]) -> bool:
    for binding in bindings:
        if not any(effect_within_scope(binding, grant) for grant in grants):
            return False
    return True


def resource_binding_narrower_or_equal(candidate: dict[str, Any], envelope: dict[str, Any]) -> bool:
    if (
        resource_binding_selector_field(candidate) != resource_binding_selector_field(envelope)
        or resource_binding_selector(candidate) != resource_binding_selector(envelope)
        or candidate["audience"] != envelope["audience"]
    ):
        return False

    effect_class = resource_binding_selector(candidate)
    candidate_resource = candidate["resource"]
    envelope_resource = envelope["resource"]

    if resource_binding_selector_field(candidate) == "family":
        if effect_class == "http-request":
            candidate_descriptor = parse_http_request_descriptor(candidate_resource)
            envelope_descriptor = parse_http_request_descriptor(envelope_resource)
            if candidate_descriptor is None or envelope_descriptor is None:
                return False
            candidate_method, candidate_request = candidate_descriptor
            envelope_method, envelope_request = envelope_descriptor
            return (
                candidate_method == envelope_method
                and candidate_request["scheme"] == envelope_request["scheme"]
                and candidate_request["hostname"] == envelope_request["hostname"]
                and candidate_request["port"] == envelope_request["port"]
                and candidate_request["path"].startswith(envelope_request["path"])
            )
        if effect_class == "read-resource":
            return candidate_resource.startswith(envelope_resource)
        if effect_class == "invoke-skill":
            return candidate_resource == envelope_resource
        if effect_class == "emit-evidence":
            return candidate_resource == envelope_resource
        if effect_class == "log-write":
            return candidate_resource == envelope_resource
        return False

    if effect_class.startswith("net."):
        candidate_url = urlparse(candidate_resource)
        envelope_url = urlparse(envelope_resource)
        candidate_port = candidate_url.port or (443 if candidate_url.scheme == "https" else 80 if candidate_url.scheme == "http" else None)
        envelope_port = envelope_url.port or (443 if envelope_url.scheme == "https" else 80 if envelope_url.scheme == "http" else None)
        return (
            candidate_url.scheme == envelope_url.scheme
            and candidate_url.hostname == envelope_url.hostname
            and candidate_port == envelope_port
            and candidate_url.path.startswith(envelope_url.path)
        )

    if effect_class.startswith("fs."):
        return path_pattern_covers(envelope_resource, candidate_resource) or candidate_resource.startswith(envelope_resource.rstrip("/") + "/")

    return candidate_resource == envelope_resource


def resource_bindings_subset(candidate: list[dict[str, Any]], envelope: list[dict[str, Any]]) -> bool:
    return all(
        any(resource_binding_narrower_or_equal(candidate_item, envelope_item) for envelope_item in envelope)
        for candidate_item in candidate
    )


def audience_subset(candidate: list[str], envelope: list[str]) -> bool:
    envelope_set = set(envelope)
    return all(item in envelope_set for item in candidate)


def scope_kind_for_effect(effect_class: str) -> str:
    if effect_class == "http-request":
        return "network"
    if effect_class == "read-resource":
        return "resource"
    if effect_class == "invoke-skill":
        return "skill"
    if effect_class == "emit-evidence":
        return "evidence"
    if effect_class == "log-write":
        return "log"
    if effect_class.startswith("fs."):
        return "filesystem"
    if effect_class.startswith("net."):
        return "network"
    if effect_class.startswith("proc."):
        return "process"
    if effect_class == "secret.read":
        return "secret"
    if effect_class == "env.read":
        return "environment"
    if effect_class in {"clock.read", "random.read"}:
        return "system"
    if effect_class == "component.invoke":
        return "component"
    if effect_class == "capability.delegate":
        return "delegation"
    return "unknown"


def delegation_policy_within(candidate: dict[str, Any], envelope: dict[str, Any]) -> bool:
    envelope_mode = envelope["mode"]
    candidate_mode = candidate["mode"]
    if envelope_mode == "forbidden":
        return candidate_mode == "forbidden" and candidate.get("max_hops", 0) == 0
    if envelope_mode == "same_invocation_only":
        return candidate_mode in {"forbidden", "same_invocation_only"} and candidate.get("max_hops", 0) == 0
    if candidate_mode not in {"forbidden", "same_invocation_only", "bounded_hops"}:
        return False
    if candidate_mode == "bounded_hops":
        if candidate.get("max_hops") is None or envelope.get("max_hops") is None:
            return False
        if candidate["max_hops"] > envelope["max_hops"]:
            return False
        candidate_effects = candidate.get("allowed_authority_selectors", candidate.get("allowed_effect_classes", []))
        envelope_effects = envelope.get("allowed_authority_selectors", envelope.get("allowed_effect_classes", []))
        if not set(candidate_effects).issubset(set(envelope_effects)):
            return False
    if envelope.get("audience_binding_required") and not candidate.get("audience_binding_required"):
        return False
    if envelope.get("call_chain_binding_required") and not candidate.get("call_chain_binding_required"):
        return False
    if envelope.get("anti_replay_required") and not candidate.get("anti_replay_required"):
        return False
    if candidate.get("ttl_seconds_max", 0) > envelope.get("ttl_seconds_max", 0):
        return False
    return True


def authority_plan_within(candidate: dict[str, Any], envelope: dict[str, Any]) -> bool:
    if candidate.get("ttl_seconds", 0) > envelope.get("ttl_seconds", 0):
        return False
    if not delegation_policy_within(candidate["delegation_policy"], envelope["delegation_policy"]):
        return False
    for candidate_grant in candidate.get("grants", []):
        if not any(effect_covers(envelope_grant, candidate_grant) for envelope_grant in envelope.get("grants", [])):
            return False
    return True


def max_expiry_for_issue(
    issued_at: str,
    authority_ttl_seconds: int,
    plan_ttl_seconds: int,
    proof_expires_at: str | None,
    parent_expires_at: str | None,
) -> str:
    expires = add_seconds(issued_at, min(authority_ttl_seconds, plan_ttl_seconds))
    if proof_expires_at is not None and parse_timestamp(proof_expires_at) < parse_timestamp(expires):
        expires = proof_expires_at
    if parent_expires_at is not None and parse_timestamp(parent_expires_at) < parse_timestamp(expires):
        expires = parent_expires_at
    return expires


def issued_passthrough_policy(value: str | None) -> str:
    return value or "forbidden"


def delegation_descriptor_from_policy(policy: dict[str, Any], available_hops: int) -> dict[str, Any]:
    if available_hops <= 0 or policy["mode"] != "bounded_hops":
        return {"mode": "none", "max_hops": 0, "remaining_hops": 0}
    if available_hops == 1:
        return {"mode": "one_hop", "max_hops": 1, "remaining_hops": 1}
    return {"mode": "bounded_hops", "max_hops": available_hops, "remaining_hops": available_hops}


def root_available_hops(plan: dict[str, Any], authority_plan: dict[str, Any]) -> int:
    policy = authority_plan["delegation_policy"]
    if policy["mode"] != "bounded_hops":
        return 0
    requested = plan["delegation_token_policy_inputs"]["delegation_chain_input"]["requested_max_hops"]
    return min(policy["max_hops"], requested)


def replay_cache_key(token_id: str, chain_digest: dict[str, Any], holder_id: str) -> dict[str, Any] | None:
    return digest_struct(
        {
            "token_id": token_id,
            "holder_id": holder_id,
            "chain_digest": chain_digest,
        }
    )


def choose_bindings(
    *,
    plan: dict[str, Any],
    authority_plan: dict[str, Any],
    audiences: list[str] | None,
    resource_bindings: list[dict[str, Any]] | None,
) -> tuple[list[str], list[dict[str, Any]], list[str]]:
    reason_codes: list[str] = []
    expected_audiences = stable_unique_strings(
        sorted(audiences if audiences is not None else plan["delegation_token_policy_inputs"]["audience_binding_inputs"])
    )
    expected_resources = stable_unique_resource_bindings(
        resource_bindings
        if resource_bindings is not None
        else plan["delegation_token_policy_inputs"]["resource_binding_inputs"]
    )

    if not expected_audiences and expected_resources:
        reason_codes.append("TOKEN_AUDIENCE_BINDING_REQUIRED")

    plan_audiences = plan["delegation_token_policy_inputs"]["audience_binding_inputs"]
    if plan_audiences and not audience_subset(expected_audiences, plan_audiences):
        reason_codes.append("TOKEN_SCOPE_EXCEEDS_PLAN")

    plan_resources = plan["delegation_token_policy_inputs"]["resource_binding_inputs"]
    if plan_resources and not resource_bindings_subset(expected_resources, plan_resources):
        reason_codes.append("TOKEN_SCOPE_EXCEEDS_PLAN")

    if expected_resources and not resource_bindings_within_grants(expected_resources, authority_plan["grants"]):
        reason_codes.append("TOKEN_SCOPE_EXCEEDS_PLAN")

    if any(binding["audience"] not in expected_audiences for binding in expected_resources):
        reason_codes.append("TOKEN_AUDIENCE_BINDING_REQUIRED")

    return expected_audiences, expected_resources, stable_unique_strings(sorted(reason_codes))


def resolve_authority_basis(
    plan: dict[str, Any],
    contract: dict[str, Any],
    proof: dict[str, Any] | None,
    *,
    allow_upper_bound: bool,
    now: str,
    required_proof_source_kind: str | None = None,
) -> tuple[
    dict[str, Any] | None,
    str | None,
    str | None,
    str | None,
    str | None,
    list[dict[str, Any]],
    list[str],
]:
    if proof is not None:
        proof_errors = validate_proof_alignment(plan, contract, proof, now)
        source_kind = proof_source_kind(proof)
        if required_proof_source_kind is not None and source_kind != required_proof_source_kind:
            proof_errors.append("PROOF_NOT_ACCEPTABLE")
        if not proof_errors:
            authority_plan = deepcopy(proof["proven_authority_plan"])
            host_exact_bindings = stable_unique_host_exact_bindings(
                proof.get("host_exact_bindings", [])
            )
            if host_exact_bindings and not host_exact_bindings_match_authority(
                host_exact_bindings,
                authority_plan,
            ):
                proof_errors.append("PROOF_NOT_ACCEPTABLE")
            if authority_plan_within(authority_plan, plan["granted_authority"]):
                return (
                    authority_plan,
                    "m5_proven_subset",
                    proof["proof_id"],
                    proof["proof_status"],
                    source_kind,
                    host_exact_bindings,
                    [],
                )
            return (
                None,
                None,
                None,
                None,
                None,
                [],
                ["TOKEN_SCOPE_EXCEEDS_PLAN", "TOKEN_SCOPE_EXCEEDS_PROOF"],
            )

    reason_codes = ["PROOF_NOT_ACCEPTABLE"]
    if proof is not None:
        reason_codes.extend(validate_proof_alignment(plan, contract, proof, now))
        if required_proof_source_kind is not None and proof_source_kind(proof) != required_proof_source_kind:
            reason_codes.append("PROOF_NOT_ACCEPTABLE")
    if allow_upper_bound:
        return deepcopy(plan["granted_authority"]), "m4_upper_bound", None, None, None, [], []
    reason_codes.append("UPPER_BOUND_ISSUANCE_DISALLOWED")
    return None, None, None, None, None, [], stable_unique_strings(sorted(reason_codes))


def chain_context_for_root(plan: dict[str, Any], chain_links: list[str] | None) -> tuple[dict[str, Any] | None, list[str]]:
    expected_links = chain_links
    if expected_links is None:
        chain_input = plan["delegation_token_policy_inputs"]["delegation_chain_input"]
        expected_links = deepcopy(chain_input["caller_chain"]) if chain_input is not None else []
    if plan["delegation_token_policy_inputs"]["call_chain_binding_required"] and not expected_links:
        return None, ["TOKEN_CALL_CHAIN_BINDING_REQUIRED"]
    if not expected_links:
        expected_links = [plan["request_id"]]
    chain_id = f"{plan['request_id']}:call-chain"
    chain_payload = {"chain_id": chain_id, "links": expected_links}
    return {
        "chain_id": chain_id,
        "links": expected_links,
        "chain_digest": digest_struct(chain_payload),
    }, []


def create_root_token(
    plan: dict[str, Any],
    contract: dict[str, Any],
    issuer: dict[str, Any],
    *,
    holder_id: str,
    issued_at: str,
    proof: dict[str, Any] | None = None,
    allow_upper_bound: bool = False,
    required_proof_source_kind: str | None = None,
    audiences: list[str] | None = None,
    resource_bindings: list[dict[str, Any]] | None = None,
    token_id: str | None = None,
    not_before: str | None = None,
    expires_at: str | None = None,
    chain_links: list[str] | None = None,
    passthrough_policy: str | None = None,
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("execution_plan.schema.json", plan, registry, "execution plan")
    require_valid("skill_contract.schema.json", contract, registry, "skill contract")
    validate_issuer_config(issuer)

    reason_codes = validate_plan_contract_alignment(plan, contract)
    if reason_codes:
        return issuance_result(
            "refuse",
            False,
            reason_codes,
            message="root token issuance failed because the supplied plan was not an admissible contract-aligned input.",
        )

    authority_plan, issuance_basis, proof_id, proof_status, proof_source, host_exact_bindings, basis_errors = resolve_authority_basis(
        plan,
        contract,
        proof,
        allow_upper_bound=allow_upper_bound,
        now=issued_at,
        required_proof_source_kind=required_proof_source_kind,
    )
    if authority_plan is None or issuance_basis is None:
        return issuance_result(
            "refuse",
            False,
            basis_errors,
            message="root token issuance failed because no acceptable authority basis was available.",
        )

    chosen_audiences, chosen_resources, binding_errors = choose_bindings(
        plan=plan,
        authority_plan=authority_plan,
        audiences=audiences,
        resource_bindings=resource_bindings,
    )
    if binding_errors:
        return issuance_result(
            "refuse",
            False,
            binding_errors,
            message="root token issuance failed because the requested audience or resource bindings exceeded the admissible authority envelope.",
        )

    chain_context, chain_errors = chain_context_for_root(plan, chain_links)
    if chain_context is None:
        return issuance_result(
            "refuse",
            False,
            chain_errors,
            message="root token issuance failed because the required call-chain binding was missing.",
        )

    root_token_id = token_id or f"{plan['plan_id']}:root-token"
    if not holder_id:
        return issuance_result(
            "refuse",
            False,
            ["TOKEN_HOLDER_BINDING_REQUIRED"],
            message="root token issuance failed because holder binding was omitted.",
        )

    max_expiry = max_expiry_for_issue(
        issued_at,
        authority_plan["ttl_seconds"],
        plan["plan_validity"]["ttl_seconds"],
        proof.get("expires_at") if proof is not None else None,
        None,
    )
    if expires_at is not None and parse_timestamp(expires_at) > parse_timestamp(max_expiry):
        return issuance_result(
            "refuse",
            False,
            ["TOKEN_EXPIRY_EXCEEDS_PROOF" if proof is not None else "TOKEN_EXPIRY_EXCEEDS_PLAN"],
            message="root token issuance failed because the requested expiry exceeded the admissible authority lifetime.",
        )
    effective_expires_at = expires_at or max_expiry
    effective_not_before = not_before or issued_at

    replay_mode = "single_use" if plan["delegation_token_policy_inputs"]["anti_replay_required"] else "none"
    delegation = delegation_descriptor_from_policy(authority_plan["delegation_policy"], root_available_hops(plan, authority_plan))

    token = {
        "kind": TOKEN_KIND,
        "version": TOKEN_VERSION,
        "token_id": root_token_id,
        "issuer": {
            "issuer_id": issuer["issuer_id"],
            "key_id": issuer["key_id"],
            "issuer_epoch": issuer.get("issuer_epoch", 0),
        },
        "issued_at": issued_at,
        "not_before": effective_not_before,
        "expires_at": effective_expires_at,
        "request_id": plan["request_id"],
        "execution_plan_id": plan["plan_id"],
        "execution_plan_digest": digest_struct(plan),
        "proof_id": proof_id,
        "proof_status": proof_status,
        "host_exact_bindings": host_exact_bindings,
        "issuance_basis": issuance_basis,
        "skill_contract_id": contract["contract_id"],
        "contract_digest": digest_struct(contract),
        "component_digest": deepcopy(contract["component"]["digest"]),
        "export_name": plan["export_name"],
        "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
        "chosen_runtime": deepcopy(plan["chosen_runtime"]),
        "audience_binding": {
            "audiences": chosen_audiences,
            "resources": chosen_resources,
        },
        "holder_binding": {
            "holder_id": holder_id,
            "issued_for_delegatee": False,
        },
        "granted_authority": deepcopy(authority_plan),
        "parent_token": None,
        "call_chain": chain_context,
        "delegation": delegation,
        "passthrough_policy": issued_passthrough_policy(passthrough_policy),
        "replay_protection": {
            "mode": replay_mode,
            "replay_cache_key": replay_cache_key(root_token_id, chain_context["chain_digest"], holder_id)
            if replay_mode == "single_use"
            else None,
        },
        "revocation_hooks": {
            "issuer_epoch": issuer.get("issuer_epoch", 0),
        },
        "notes": (
            "Issued from the M5 proven authority subset backed by a live-runtime proof."
            if issuance_basis == "m5_proven_subset" and proof_source == PROOF_SOURCE_LIVE_RUNTIME
            else "Issued from the M5 proven authority subset."
            if issuance_basis == "m5_proven_subset"
            else "Issued from the M4 upper-bound grant set because explicit upper-bound issuance was allowed."
        ),
    }
    if proof_source == PROOF_SOURCE_LIVE_RUNTIME:
        token["proof_source_kind"] = proof_source
    token = attach_protection(token, issuer["shared_secret"])
    require_valid("delegated_capability_token.schema.json", token, registry, "delegated capability token")
    return token


def create_child_token(
    parent_token: dict[str, Any],
    plan: dict[str, Any],
    contract: dict[str, Any],
    child_authority_plan: dict[str, Any],
    issuer: dict[str, Any],
    *,
    holder_id: str,
    issued_at: str,
    proof: dict[str, Any] | None = None,
    required_proof_source_kind: str | None = None,
    audiences: list[str] | None = None,
    resource_bindings: list[dict[str, Any]] | None = None,
    token_id: str | None = None,
    not_before: str | None = None,
    expires_at: str | None = None,
    passthrough_policy: str | None = None,
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("delegated_capability_token.schema.json", parent_token, registry, "parent token")
    require_valid("execution_plan.schema.json", plan, registry, "execution plan")
    require_valid("skill_contract.schema.json", contract, registry, "skill contract")
    validate_issuer_config(issuer)

    reason_codes = validate_plan_contract_alignment(plan, contract)
    if reason_codes:
        return issuance_result(
            "refuse",
            False,
            reason_codes,
            message="child token issuance failed because the supplied plan was not admissible or contract-aligned.",
        )

    allow_upper_bound = parent_token["issuance_basis"] == "m4_upper_bound"
    authority_envelope, issuance_basis, proof_id, proof_status, proof_source, host_exact_bindings, basis_errors = resolve_authority_basis(
        plan,
        contract,
        proof,
        allow_upper_bound=allow_upper_bound,
        now=issued_at,
        required_proof_source_kind=required_proof_source_kind,
    )
    if authority_envelope is None or issuance_basis is None:
        return issuance_result(
            "refuse",
            False,
            basis_errors,
            message="child token issuance failed because no applicable authority envelope was available.",
        )

    if parent_token["delegation"]["remaining_hops"] <= 0:
        return issuance_result(
            "refuse",
            False,
            ["DELEGATION_HOPS_EXHAUSTED"],
            message="child token issuance failed because the parent token had no remaining delegation hops.",
        )

    child_refusal_reason_codes: list[str] = []
    if not authority_plan_within(child_authority_plan, authority_envelope):
        child_refusal_reason_codes.append(
            "TOKEN_SCOPE_EXCEEDS_PROOF" if issuance_basis == "m5_proven_subset" else "TOKEN_SCOPE_EXCEEDS_PLAN"
        )

    if not authority_plan_within(child_authority_plan, parent_token["granted_authority"]):
        child_refusal_reason_codes.append("PARENT_CHILD_SCOPE_BROADENING")
        if not delegation_policy_within(
            child_authority_plan["delegation_policy"],
            parent_token["granted_authority"]["delegation_policy"],
        ):
            child_refusal_reason_codes.append("PARENT_CHILD_DELEGATION_BROADENING")

    if host_exact_bindings and not host_exact_bindings_match_authority(
        host_exact_bindings,
        child_authority_plan,
    ):
        child_refusal_reason_codes.append("TOKEN_SCOPE_EXCEEDS_PROOF")

    parent_exact_bindings = stable_unique_host_exact_bindings(
        parent_token.get("host_exact_bindings", [])
    )
    if host_exact_bindings != parent_exact_bindings:
        child_refusal_reason_codes.append("PARENT_CHILD_SCOPE_BROADENING")

    child_audiences = stable_unique_strings(
        sorted(audiences if audiences is not None else parent_token["audience_binding"]["audiences"])
    )
    child_resources = stable_unique_resource_bindings(
        resource_bindings if resource_bindings is not None else parent_token["audience_binding"]["resources"]
    )

    child_binding_errors: list[str] = []
    if not audience_subset(child_audiences, parent_token["audience_binding"]["audiences"]):
        child_binding_errors.append("PARENT_CHILD_AUDIENCE_BROADENING")
    if not resource_bindings_subset(child_resources, parent_token["audience_binding"]["resources"]):
        child_binding_errors.append("PARENT_CHILD_SCOPE_BROADENING")
    if child_resources and not resource_bindings_within_grants(child_resources, child_authority_plan["grants"]):
        child_binding_errors.append(
            "TOKEN_SCOPE_EXCEEDS_PROOF" if issuance_basis == "m5_proven_subset" else "TOKEN_SCOPE_EXCEEDS_PLAN"
        )
    if any(binding["audience"] not in child_audiences for binding in child_resources):
        child_binding_errors.append("PARENT_CHILD_AUDIENCE_BROADENING")
    child_refusal_reason_codes.extend(child_binding_errors)

    remaining_parent_hops = parent_token["delegation"]["remaining_hops"] - 1
    child_requested_hops = child_authority_plan["delegation_policy"].get("max_hops", 0)
    if child_authority_plan["delegation_policy"]["mode"] == "bounded_hops" and child_requested_hops > remaining_parent_hops:
        child_refusal_reason_codes.append("PARENT_CHILD_DELEGATION_BROADENING")

    max_expiry = max_expiry_for_issue(
        issued_at,
        child_authority_plan["ttl_seconds"],
        plan["plan_validity"]["ttl_seconds"],
        proof.get("expires_at") if proof is not None else None,
        parent_token["expires_at"],
    )
    if expires_at is not None and parse_timestamp(expires_at) > parse_timestamp(max_expiry):
        child_refusal_reason_codes.append("PARENT_CHILD_TTL_BROADENING")

    child_refusal_reason_codes = stable_unique_strings(sorted(child_refusal_reason_codes))
    if child_refusal_reason_codes:
        return issuance_result(
            "refuse",
            False,
            child_refusal_reason_codes,
            message="child token issuance failed because the requested child authority or bindings exceeded the applicable parent or envelope constraints.",
        )

    child_chain_links = deepcopy(parent_token["call_chain"]["links"]) + [parent_token["token_id"], holder_id]
    child_chain = {
        "chain_id": parent_token["call_chain"]["chain_id"],
        "links": child_chain_links,
        "chain_digest": digest_struct(
            {
                "chain_id": parent_token["call_chain"]["chain_id"],
                "links": child_chain_links,
            }
        ),
    }

    child_delegation = delegation_descriptor_from_policy(
        child_authority_plan["delegation_policy"],
        min(
            remaining_parent_hops,
            child_requested_hops if child_authority_plan["delegation_policy"]["mode"] == "bounded_hops" else 0,
        ),
    )
    child_token_id = token_id or f"{parent_token['token_id']}:child:{holder_id}"
    replay_mode = "single_use" if child_authority_plan["delegation_policy"]["anti_replay_required"] else "none"

    token = {
        "kind": TOKEN_KIND,
        "version": TOKEN_VERSION,
        "token_id": child_token_id,
        "issuer": {
            "issuer_id": issuer["issuer_id"],
            "key_id": issuer["key_id"],
            "issuer_epoch": issuer.get("issuer_epoch", 0),
        },
        "issued_at": issued_at,
        "not_before": not_before or issued_at,
        "expires_at": expires_at or max_expiry,
        "request_id": plan["request_id"],
        "execution_plan_id": plan["plan_id"],
        "execution_plan_digest": digest_struct(plan),
        "proof_id": proof_id,
        "proof_status": proof_status,
        "host_exact_bindings": host_exact_bindings,
        "issuance_basis": issuance_basis,
        "skill_contract_id": contract["contract_id"],
        "contract_digest": digest_struct(contract),
        "component_digest": deepcopy(contract["component"]["digest"]),
        "export_name": plan["export_name"],
        "input_class_fingerprint": deepcopy(plan["input_class_fingerprint"]),
        "chosen_runtime": deepcopy(plan["chosen_runtime"]),
        "audience_binding": {
            "audiences": child_audiences,
            "resources": child_resources,
        },
        "holder_binding": {
            "holder_id": holder_id,
            "issued_for_delegatee": True,
        },
        "granted_authority": deepcopy(child_authority_plan),
        "parent_token": {
            "token_id": parent_token["token_id"],
            "token_digest": digest_struct(parent_token),
        },
        "call_chain": child_chain,
        "delegation": child_delegation,
        "passthrough_policy": issued_passthrough_policy(passthrough_policy),
        "replay_protection": {
            "mode": replay_mode,
            "replay_cache_key": replay_cache_key(child_token_id, child_chain["chain_digest"], holder_id)
            if replay_mode == "single_use"
            else None,
        },
        "revocation_hooks": {
            "issuer_epoch": issuer.get("issuer_epoch", 0),
        },
        "notes": (
            "Delegated child token backed by a live-runtime proof subset. The parent token remains non-pass-through and cannot be redeemed directly by the child holder."
            if issuance_basis == "m5_proven_subset" and proof_source == PROOF_SOURCE_LIVE_RUNTIME
            else "Delegated child token. The parent token remains non-pass-through and cannot be redeemed directly by the child holder."
        ),
    }
    if proof_source == PROOF_SOURCE_LIVE_RUNTIME:
        token["proof_source_kind"] = proof_source
    token = attach_protection(token, issuer["shared_secret"])
    require_valid("delegated_capability_token.schema.json", token, registry, "delegated capability token")
    return token


def load_issuer_secret(issuer_keys: dict[str, dict[str, str]], issuer_id: str, key_id: str) -> tuple[str | None, list[str]]:
    if issuer_id not in issuer_keys:
        return None, ["ISSUER_UNKNOWN"]
    keys = issuer_keys[issuer_id]
    if key_id not in keys:
        return None, ["KEY_ID_UNKNOWN"]
    return keys[key_id], []


def verify_protection(token: dict[str, Any], shared_secret: str) -> bool:
    protection = token.get("protection")
    if protection is None:
        return False
    if protection.get("mode") != TOKEN_PROTECTION_MODE:
        return False
    if protection.get("canonicalization") != TOKEN_CANONICALIZATION:
        return False
    payload = token_payload_without_protection(token)
    expected = protection_for_payload(payload, shared_secret)
    return hmac.compare_digest(expected["mac_base64"], protection.get("mac_base64", "")) and expected["claims_digest"] == protection.get("claims_digest")


def token_parent_subset_ok(token: dict[str, Any], parent_token: dict[str, Any]) -> list[str]:
    reason_codes: list[str] = []
    if token["parent_token"]["token_id"] != parent_token["token_id"] or token["parent_token"]["token_digest"] != digest_struct(parent_token):
        reason_codes.append("PARENT_TOKEN_INVALID")
    if not authority_plan_within(token["granted_authority"], parent_token["granted_authority"]):
        reason_codes.append("PARENT_CHILD_SCOPE_BROADENING")
    if not audience_subset(token["audience_binding"]["audiences"], parent_token["audience_binding"]["audiences"]):
        reason_codes.append("PARENT_CHILD_AUDIENCE_BROADENING")
    if not resource_bindings_subset(token["audience_binding"]["resources"], parent_token["audience_binding"]["resources"]):
        reason_codes.append("PARENT_CHILD_SCOPE_BROADENING")
    if parse_timestamp(token["expires_at"]) > parse_timestamp(parent_token["expires_at"]):
        reason_codes.append("PARENT_CHILD_TTL_BROADENING")
    if token["chosen_runtime"] != parent_token["chosen_runtime"]:
        reason_codes.append("PARENT_CHILD_RUNTIME_BROADENING")
    if stable_unique_host_exact_bindings(token.get("host_exact_bindings", [])) != stable_unique_host_exact_bindings(
        parent_token.get("host_exact_bindings", [])
    ):
        reason_codes.append("PARENT_CHILD_SCOPE_BROADENING")
    if token["call_chain"]["chain_id"] != parent_token["call_chain"]["chain_id"] or not token["call_chain"]["links"][: len(parent_token["call_chain"]["links"])] == parent_token["call_chain"]["links"]:
        reason_codes.append("CALL_CHAIN_MISMATCH")
    if token["delegation"]["remaining_hops"] > max(parent_token["delegation"]["remaining_hops"] - 1, 0):
        reason_codes.append("PARENT_CHILD_DELEGATION_BROADENING")
    return stable_unique_strings(sorted(reason_codes))


def replay_marker_path(state_dir: Path, replay_cache_key_value: str) -> Path:
    replay_dir = state_dir / "token-replay"
    replay_dir.mkdir(parents=True, exist_ok=True)
    return replay_dir / f"{replay_cache_key_value}.json"


def mark_replay(state_dir: Path, replay_cache_key_value: str, verification_time: str) -> None:
    marker = replay_marker_path(state_dir, replay_cache_key_value)
    marker.write_text(json.dumps({"used_at": verification_time}, indent=2, sort_keys=True) + "\n")


def replay_seen(state_dir: Path, replay_cache_key_value: str) -> bool:
    return replay_marker_path(state_dir, replay_cache_key_value).exists()


def verify_token(
    token: dict[str, Any],
    *,
    issuer_keys: dict[str, dict[str, str]],
    verification_time: str,
    expected_holder_id: str | None = None,
    expected_audiences: list[str] | None = None,
    expected_resources: list[dict[str, Any]] | None = None,
    expected_runtime_guarantee_id: str | None = None,
    expected_call_chain_links: list[str] | None = None,
    plan: dict[str, Any] | None = None,
    contract: dict[str, Any] | None = None,
    proof: dict[str, Any] | None = None,
    parent_token: dict[str, Any] | None = None,
    replay_state_dir: str | Path | None = None,
    revoked_token_ids: set[str] | None = None,
    minimum_issuer_epochs: dict[str, int] | None = None,
    check_replay: bool = True,
) -> dict[str, Any]:
    registry = build_registry()
    schema_errors = validate_instance("delegated_capability_token.schema.json", token, registry)
    if schema_errors:
        return verify_result(
            decision="deny",
            verified=False,
            verification_time=verification_time,
            token=token,
            reason_codes=["TOKEN_SCHEMA_INVALID"],
            holder_id=expected_holder_id,
            audiences=expected_audiences or [],
            resources=expected_resources or [],
            runtime_guarantee_id=expected_runtime_guarantee_id,
            call_chain_digest=digest_struct(expected_call_chain_links) if expected_call_chain_links else None,
            replay_mode="none",
            replay_checked=False,
            replay_cache_key=None,
            notes="; ".join(schema_errors),
        )

    issuer_id = token["issuer"]["issuer_id"]
    key_id = token["issuer"]["key_id"]
    shared_secret, secret_errors = load_issuer_secret(issuer_keys, issuer_id, key_id)
    if shared_secret is None:
        return verify_result(
            decision="deny",
            verified=False,
            verification_time=verification_time,
            token=token,
            reason_codes=secret_errors,
            holder_id=expected_holder_id,
            audiences=expected_audiences or [],
            resources=expected_resources or [],
            runtime_guarantee_id=expected_runtime_guarantee_id,
            call_chain_digest=digest_struct(expected_call_chain_links) if expected_call_chain_links else None,
            replay_mode=token["replay_protection"]["mode"],
            replay_checked=False,
            replay_cache_key=token["replay_protection"]["replay_cache_key"],
        )

    reason_codes: list[str] = []
    if not verify_protection(token, shared_secret):
        reason_codes.append("CRYPTO_PROTECTION_INVALID")

    now = parse_timestamp(verification_time)
    if now < parse_timestamp(token["not_before"]):
        reason_codes.append("TOKEN_NOT_YET_VALID")
    if now > parse_timestamp(token["expires_at"]):
        reason_codes.append("TOKEN_EXPIRED")

    if plan is not None and contract is not None:
        reason_codes.extend(validate_plan_contract_alignment(plan, contract))
        if token["execution_plan_id"] != plan["plan_id"] or token["execution_plan_digest"] != digest_struct(plan):
            reason_codes.append("PLAN_CONTRACT_MISMATCH")
        if token["chosen_runtime"] != plan["chosen_runtime"]:
            reason_codes.append("PLAN_RUNTIME_MISMATCH")
        if not authority_plan_within(token["granted_authority"], plan["granted_authority"]):
            reason_codes.append("TOKEN_SCOPE_EXCEEDS_PLAN")

    if token["issuance_basis"] == "m5_proven_subset":
        if proof is None or contract is None or plan is None:
            reason_codes.append("PROOF_NOT_ACCEPTABLE")
        else:
            reason_codes.extend(validate_proof_alignment(plan, contract, proof, verification_time))
            token_proof_source = token.get("proof_source_kind")
            if token_proof_source is not None and token_proof_source != proof_source_kind(proof):
                reason_codes.append("PROOF_NOT_ACCEPTABLE")
            if not authority_plan_within(token["granted_authority"], proof["proven_authority_plan"]):
                reason_codes.append("TOKEN_SCOPE_EXCEEDS_PROOF")
            proof_exact_bindings = stable_unique_host_exact_bindings(
                proof.get("host_exact_bindings", [])
            )
            token_exact_bindings = stable_unique_host_exact_bindings(
                token.get("host_exact_bindings", [])
            )
            if token_exact_bindings != proof_exact_bindings:
                reason_codes.append("PROOF_NOT_ACCEPTABLE")
            if token_exact_bindings and not host_exact_bindings_match_authority(
                token_exact_bindings,
                token["granted_authority"],
            ):
                reason_codes.append("TOKEN_SCOPE_EXCEEDS_PROOF")
    elif stable_unique_host_exact_bindings(token.get("host_exact_bindings", [])):
        reason_codes.append("PROOF_NOT_ACCEPTABLE")

    if expected_holder_id is None:
        reason_codes.append("HOLDER_BINDING_MISMATCH")
    elif expected_holder_id != token["holder_binding"]["holder_id"]:
        reason_codes.append("HOLDER_BINDING_MISMATCH")
        if token["passthrough_policy"] == "forbidden":
            reason_codes.append("TOKEN_PASSTHROUGH_FORBIDDEN")

    expected_audience_values = stable_unique_strings(sorted(expected_audiences or []))
    if token["audience_binding"]["audiences"]:
        if not expected_audience_values or not audience_subset(expected_audience_values, token["audience_binding"]["audiences"]):
            reason_codes.append("AUDIENCE_MISMATCH")

    expected_resource_values = stable_unique_resource_bindings(expected_resources or [])
    if token["audience_binding"]["resources"]:
        if not expected_resource_values or not resource_bindings_subset(expected_resource_values, token["audience_binding"]["resources"]):
            reason_codes.append("RESOURCE_BINDING_MISMATCH")

    if expected_runtime_guarantee_id is None or expected_runtime_guarantee_id != token["chosen_runtime"]["runtime_guarantee_id"]:
        reason_codes.append("RUNTIME_BINDING_MISMATCH")

    expected_chain_digest = None
    if expected_call_chain_links is not None:
        expected_chain_digest = digest_struct(
            {
                "chain_id": token["call_chain"]["chain_id"],
                "links": expected_call_chain_links,
            }
        )
    if expected_chain_digest is None or expected_chain_digest != token["call_chain"]["chain_digest"]:
        reason_codes.append("CALL_CHAIN_MISMATCH")

    if parent_token is not None:
        reason_codes.extend(token_parent_subset_ok(token, parent_token))
    elif token["parent_token"] is not None:
        reason_codes.append("PARENT_TOKEN_REQUIRED")

    revoked_token_ids = revoked_token_ids or set()
    minimum_issuer_epochs = minimum_issuer_epochs or {}
    if token["token_id"] in revoked_token_ids:
        reason_codes.append("TOKEN_REVOKED")
    if token["issuer"]["issuer_epoch"] < minimum_issuer_epochs.get(token["issuer"]["issuer_id"], 0):
        reason_codes.append("TOKEN_REVOKED")

    replay_mode = token["replay_protection"]["mode"]
    replay_key = token["replay_protection"]["replay_cache_key"]
    replay_checked = False
    state_dir = coerce_path(replay_state_dir)
    if check_replay and replay_mode == "single_use":
        if replay_key is None:
            reason_codes.append("CRYPTO_PROTECTION_INVALID")
        elif state_dir is None:
            reason_codes.append("REPLAY_STATE_UNAVAILABLE")
        else:
            replay_checked = True
            if replay_seen(state_dir, replay_key["value"]):
                reason_codes.append("TOKEN_REPLAYED")

    reason_codes = stable_unique_strings(sorted(reason_codes))
    verified = not reason_codes
    if verified and check_replay and replay_mode == "single_use" and state_dir is not None and replay_key is not None:
        mark_replay(state_dir, replay_key["value"], verification_time)

    result = verify_result(
        decision="allow" if verified else "deny",
        verified=verified,
        verification_time=verification_time,
        token=token,
        reason_codes=reason_codes,
        holder_id=expected_holder_id,
        audiences=expected_audience_values,
        resources=expected_resource_values,
        runtime_guarantee_id=expected_runtime_guarantee_id,
        call_chain_digest=expected_chain_digest,
        replay_mode=replay_mode,
        replay_checked=replay_checked,
        replay_cache_key=replay_key,
    )
    require_valid("token_verification_result.schema.json", result, registry, "token verification result")
    return result
