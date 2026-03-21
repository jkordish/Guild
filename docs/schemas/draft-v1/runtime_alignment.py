from __future__ import annotations

from copy import deepcopy
from typing import Any
from urllib.parse import urlparse

from admission_core import (
    digest_struct,
    effect_is_canonical,
    effect_scope_kind,
    effect_selector,
    normalize_effect,
    stable_unique_dicts,
    stable_unique_strings,
)


LIVE_RUNTIME_SOURCE_KIND = "live-runtime-hook"

MAPPING_EXACT = "exact"
MAPPING_NARROWING = "narrowing"
MAPPING_PARTIAL = "partial"
MAPPING_UNSUPPORTED = "unsupported"

COVERAGE_COMPLETE = "complete"
COVERAGE_PARTIAL = "partial"
COVERAGE_UNAVAILABLE = "insufficient"

HTTP_RUNTIME_FAMILY = "http-request"
READ_RESOURCE_RUNTIME_FAMILY = "read-resource"
INVOKE_SKILL_RUNTIME_FAMILY = "invoke-skill"
EMIT_EVIDENCE_RUNTIME_FAMILY = "emit-evidence"
LOG_WRITE_RUNTIME_FAMILY = "log-write"

ACTIVE_OBSERVABLE_FAMILIES = {
    HTTP_RUNTIME_FAMILY,
    READ_RESOURCE_RUNTIME_FAMILY,
    INVOKE_SKILL_RUNTIME_FAMILY,
    EMIT_EVIDENCE_RUNTIME_FAMILY,
    LOG_WRITE_RUNTIME_FAMILY,
}

LEGACY_EFFECT_TO_RUNTIME_FAMILY = {
    "net.connect": HTTP_RUNTIME_FAMILY,
    "net.resolve": HTTP_RUNTIME_FAMILY,
    "component.invoke": INVOKE_SKILL_RUNTIME_FAMILY,
    "fs.read": "filesystem",
    "fs.write": "filesystem",
    "fs.list": "filesystem",
    "secret.read": "get-secret",
    "clock.read": "wall-clock",
}


def mapping_reason_codes(mapping_status: str) -> list[str]:
    if mapping_status == MAPPING_NARROWING:
        return ["VOCABULARY_MAPPING_NARROWING"]
    if mapping_status == MAPPING_PARTIAL:
        return ["VOCABULARY_MAPPING_PARTIAL"]
    if mapping_status == MAPPING_UNSUPPORTED:
        return ["VOCABULARY_MAPPING_UNSUPPORTED"]
    return []


def coverage_reason_codes(status: str) -> list[str]:
    if status == COVERAGE_PARTIAL:
        return ["OBSERVATION_COVERAGE_PARTIAL", "LIVE_WITNESS_COVERAGE_PARTIAL"]
    if status == COVERAGE_UNAVAILABLE:
        return ["OBSERVATION_COVERAGE_INSUFFICIENT"]
    return []


def family_support_reason_codes(mapping_status: str, draft_effect_class: str | None) -> list[str]:
    codes: list[str] = []
    if mapping_status in {MAPPING_EXACT, MAPPING_NARROWING}:
        codes.append("CANONICAL_FAMILY_SUPPORTED")
    elif mapping_status == MAPPING_PARTIAL:
        codes.append("CANONICAL_FAMILY_PARTIAL")
    elif mapping_status == MAPPING_UNSUPPORTED:
        codes.append("CANONICAL_FAMILY_UNSUPPORTED")
    if draft_effect_class is not None:
        codes.append("COMPAT_ALIAS_USED")
        if draft_effect_class in LEGACY_EFFECT_TO_RUNTIME_FAMILY:
            codes.append("COMPAT_ALIAS_DEPRECATED")
    return stable_unique_strings(sorted(codes))


def runtime_family_for_legacy_effect(effect_class: str) -> str | None:
    return LEGACY_EFFECT_TO_RUNTIME_FAMILY.get(effect_class)


def runtime_mapping_for_effect(effect: dict[str, Any]) -> dict[str, Any]:
    if effect_is_canonical(effect):
        family = effect_selector(effect)
        return {
            "family": family,
            "draft_effect_class": None,
            "scope_kind": effect_scope_kind(effect),
            "mapping_status": MAPPING_EXACT if family in ACTIVE_OBSERVABLE_FAMILIES else MAPPING_UNSUPPORTED,
            "notes": None
            if family in ACTIVE_OBSERVABLE_FAMILIES
            else "The canonical family is not part of the active observable runtime slice in this milestone.",
        }
    return runtime_mapping_for_legacy_effect(effect)


def runtime_mapping_for_legacy_effect(effect: dict[str, Any]) -> dict[str, Any]:
    effect_class = effect["effect_class"]
    family = runtime_family_for_legacy_effect(effect_class)
    scope_kind = effect["scope"]["kind"]
    notes: str | None = None

    if effect_class == "net.connect":
        audiences = effect["scope"]["audiences"]
        http_only = True
        safe_methods = True
        for audience in audiences:
            schemes = audience.get("schemes") or ["*"]
            methods = audience.get("methods") or ["*"]
            if "*" in schemes or any(scheme not in {"http", "https"} for scheme in schemes):
                http_only = False
            if "*" in methods or any(method not in {"GET", "HEAD"} for method in methods):
                safe_methods = False

        if http_only and safe_methods:
            return {
                "family": family,
                "draft_effect_class": effect_class,
                "scope_kind": scope_kind,
                "mapping_status": MAPPING_NARROWING,
                "notes": "Draft net.connect is broader than the live runtime http-request family. This mapping is conservative and only safe for explicit HTTP(S) GET/HEAD scopes.",
            }
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_UNSUPPORTED,
            "notes": "Draft net.connect scope exceeds the live runtime http-request family. Wildcards and non-HTTP or non-GET/HEAD methods are not runtime-general here.",
        }

    if effect_class == "net.resolve":
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_UNSUPPORTED,
            "notes": "The live runtime does not expose a standalone DNS resolution family.",
        }

    if effect_class == "component.invoke":
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_NARROWING,
            "notes": "The live runtime invoke-skill family is narrower than generic component.invoke and is limited to declared dependency aliases.",
        }

    if effect_class in {"fs.read", "fs.write", "fs.list"}:
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_PARTIAL,
            "notes": "The canonical runtime family is filesystem, but the active inspect runtime still rejects filesystem before guest start and the scope models are not yet fully aligned.",
        }

    if effect_class == "secret.read":
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_PARTIAL,
            "notes": "The canonical runtime family is get-secret, but the active inspect runtime does not yet expose live enforcement or observation for it.",
        }

    if effect_class == "clock.read":
        return {
            "family": family,
            "draft_effect_class": effect_class,
            "scope_kind": scope_kind,
            "mapping_status": MAPPING_PARTIAL,
            "notes": "Draft clock.read does not distinguish wall-clock from monotonic clock. The canonical runtime surface does.",
        }

    return {
        "family": family or effect_class,
        "draft_effect_class": effect_class,
        "scope_kind": scope_kind,
        "mapping_status": MAPPING_UNSUPPORTED,
        "notes": notes or "This draft effect class does not have a safe live runtime mapping in the current repository.",
    }


def runtime_coverage_for_family(family: str, mapping_status: str) -> str:
    if family not in ACTIVE_OBSERVABLE_FAMILIES:
        return COVERAGE_UNAVAILABLE
    if mapping_status == MAPPING_EXACT or mapping_status == MAPPING_NARROWING:
        return COVERAGE_COMPLETE
    if mapping_status == MAPPING_PARTIAL:
        return COVERAGE_PARTIAL
    return COVERAGE_UNAVAILABLE


def supports_absence_claims(status: str, mapping_status: str) -> bool:
    return status == COVERAGE_COMPLETE and mapping_status in {MAPPING_EXACT, MAPPING_NARROWING}


def normalized_http_method(method: str) -> str:
    return method.upper()


def network_descriptor_for_runtime_http(request: dict[str, Any]) -> str:
    parsed = urlparse(request["url"])
    scheme = parsed.scheme or "http"
    host = parsed.hostname or ""
    port = parsed.port or (443 if scheme == "https" else 80)
    path = parsed.path or "/"
    return f"{normalized_http_method(request['method'])}:{scheme}://{host}:{port}{path}"


def emit_evidence_descriptor(detail: dict[str, Any]) -> str:
    return f"audience={detail['audience']};redaction={detail['redaction']}"


def log_write_descriptor(detail: dict[str, Any]) -> str:
    return detail["level"]


def canonical_effect_from_runtime_http(observation: dict[str, Any]) -> dict[str, Any]:
    request = observation["detail"]["request"]
    parsed = urlparse(request["url"])
    scheme = parsed.scheme or "http"
    host = parsed.hostname or ""
    port = parsed.port or (443 if scheme == "https" else 80)
    path = parsed.path or "/"
    method = normalized_http_method(request["method"])
    effect: dict[str, Any] = {
        "family": "http-request",
        "scope": {
            "kind": "network",
            "allowed_schemes": [scheme],
            "allowed_hosts": [host],
            "allowed_ports": [port],
            "allowed_methods": [method],
            "allowed_path_prefixes": [path],
        },
        "cardinality": {
            "max_calls": 1,
        },
    }
    if request.get("timeout_ms") is not None:
        effect["scope"]["max_timeout_ms"] = request["timeout_ms"]
    response_bytes = observation["detail"].get("response_bytes")
    if response_bytes is not None:
        effect["cardinality"]["max_bytes"] = response_bytes
    return normalize_effect(effect)


def canonical_effect_from_runtime_read_resource(observation: dict[str, Any]) -> dict[str, Any]:
    detail = observation["detail"]
    effect: dict[str, Any] = {
        "family": "read-resource",
        "scope": {
            "kind": "resource",
            "uri_prefixes": [detail["uri"]],
        },
        "cardinality": {
            "max_calls": 1,
        },
    }
    if detail.get("resource_kind") is not None:
        effect["scope"]["resource_kinds"] = [detail["resource_kind"]]
    if detail.get("bytes") is not None:
        effect["cardinality"]["max_bytes"] = detail["bytes"]
    return normalize_effect(effect)


def canonical_effect_from_runtime_invoke_skill(observation: dict[str, Any]) -> dict[str, Any]:
    return normalize_effect(
        {
            "family": "invoke-skill",
            "scope": {
                "kind": "skill",
                "aliases": [observation["detail"]["alias"]],
            },
            "cardinality": {
                "max_calls": 1,
            },
        }
    )


def canonical_effect_from_runtime_emit_evidence(observation: dict[str, Any]) -> dict[str, Any]:
    detail = observation["detail"]
    return normalize_effect(
        {
            "family": "emit-evidence",
            "scope": {
                "kind": "evidence",
                "audiences": [detail["audience"]],
                "redactions": [detail["redaction"]],
            },
            "cardinality": {
                "max_calls": 1,
                "max_bytes": detail["size_bytes"],
            },
        }
    )


def canonical_effect_from_runtime_log_write(observation: dict[str, Any]) -> dict[str, Any]:
    return normalize_effect(
        {
            "family": "log-write",
            "scope": {
                "kind": "log",
                "levels": [observation["detail"]["level"]],
            },
            "cardinality": {
                "max_calls": 1,
            },
        }
    )


def canonical_effect_from_runtime_observation(observation: dict[str, Any]) -> dict[str, Any]:
    family = observation["family"]
    if family == HTTP_RUNTIME_FAMILY:
        return canonical_effect_from_runtime_http(observation)
    if family == READ_RESOURCE_RUNTIME_FAMILY:
        return canonical_effect_from_runtime_read_resource(observation)
    if family == INVOKE_SKILL_RUNTIME_FAMILY:
        return canonical_effect_from_runtime_invoke_skill(observation)
    if family == EMIT_EVIDENCE_RUNTIME_FAMILY:
        return canonical_effect_from_runtime_emit_evidence(observation)
    if family == LOG_WRITE_RUNTIME_FAMILY:
        return canonical_effect_from_runtime_log_write(observation)
    raise ValueError(f"unsupported runtime family {family}")


def failure_for_observation(detail: dict[str, Any]) -> dict[str, Any]:
    denial = detail.get("denial") or detail.get("result_error") or {
        "code": "RUNTIME_OBSERVATION_UNMAPPABLE",
        "message": "runtime blocked attempt was missing denial details",
        "detail": {},
    }
    return denial


def blocked_effect_from_runtime_observation(observation: dict[str, Any]) -> dict[str, Any]:
    denial = failure_for_observation(observation["detail"])
    denial_detail = denial.get("detail")
    return {
        "effect": canonical_effect_from_runtime_observation(observation),
        "reason_code": denial["code"],
        "message": denial["message"],
        "details_digest": digest_struct(denial_detail) if denial_detail is not None else None,
    }


def scope_descriptors_for_effect(effect: dict[str, Any]) -> list[str]:
    if effect_is_canonical(effect):
        family = effect_selector(effect)
        scope = effect["scope"]
        if family == "http-request":
            descriptors: list[str] = []
            methods = scope.get("allowed_methods") or ["GET"]
            schemes = scope.get("allowed_schemes") or ["http"]
            hosts = scope.get("allowed_hosts") or scope.get("allowed_host_suffixes") or ["*"]
            ports = scope.get("allowed_ports") or ["*"]
            prefixes = scope.get("allowed_path_prefixes") or ["/"]
            for method in methods:
                for scheme in schemes:
                    for host in hosts:
                        for port in ports:
                            for prefix in prefixes:
                                descriptors.append(f"{method}:{scheme}://{host}:{port}{prefix}")
            return stable_unique_strings(sorted(descriptors))
        if family == "read-resource":
            return stable_unique_strings(sorted(scope.get("uri_prefixes", []) or ["guild://"]))
        if family == "invoke-skill":
            return stable_unique_strings(sorted(scope.get("aliases", []) or ["invoke-skill"]))
        if family == "emit-evidence":
            audiences = scope.get("audiences") or ["*"]
            redactions = scope.get("redactions") or ["*"]
            return stable_unique_strings(
                sorted(f"audience={audience};redaction={redaction}" for audience in audiences for redaction in redactions)
            )
        if family == "log-write":
            return stable_unique_strings(sorted(scope.get("levels", []) or ["log-write"]))
    return [effect_selector(effect)]


def unmapped_observation(observation: dict[str, Any], *, coverage_status: str) -> dict[str, Any]:
    family = observation["family"]
    status = observation["status"]
    detail = deepcopy(observation["detail"])
    return {
        "family": family,
        "observed_as": status,
        "details_summary": f"Live runtime family `{family}` does not yet have a safe direct draft-v1 representation.",
        "details_digest": digest_struct(detail),
        "coverage_status": coverage_status,
        "reason_codes": ["RUNTIME_OBSERVATION_UNMAPPABLE", "LIVE_RUNTIME_ALIGNMENT_INCOMPLETE"],
        "notes": "The runtime family is canonical; draft-v1 still lacks a safe direct family path for this live observation.",
    }


def merge_coverage_entry(
    entries: dict[tuple[str, str | None], dict[str, Any]],
    *,
    family: str,
    draft_effect_class: str | None,
    scope_kind: str | None,
    mapping_status: str,
    scope_descriptors: list[str],
    notes: str | None,
) -> None:
    key = (family, draft_effect_class)
    status = runtime_coverage_for_family(family, mapping_status)
    reason_codes = stable_unique_strings(
        sorted(
            family_support_reason_codes(mapping_status, draft_effect_class)
            + ([] if mapping_status == MAPPING_EXACT and draft_effect_class is None else mapping_reason_codes(mapping_status))
            + coverage_reason_codes(status)
            + (["LIVE_RUNTIME_ALIGNMENT_INCOMPLETE"] if mapping_status in {MAPPING_PARTIAL, MAPPING_UNSUPPORTED} else [])
        )
    )
    existing = entries.get(key)
    merged_descriptors = stable_unique_strings(
        sorted((existing or {}).get("scope_descriptors", []) + scope_descriptors)
    )
    candidate = {
        "family": family,
        "draft_effect_class": draft_effect_class,
        "scope_kind": scope_kind,
        "status": status,
        "mapping_status": mapping_status,
        "supports_positive_facts": True,
        "supports_absence_claims": supports_absence_claims(status, mapping_status),
        "scope_descriptors": merged_descriptors,
        "reason_codes": reason_codes,
        "notes": notes,
    }
    if existing is None:
        entries[key] = candidate
        return

    status_rank = {
        COVERAGE_COMPLETE: 0,
        COVERAGE_PARTIAL: 1,
        COVERAGE_UNAVAILABLE: 2,
    }
    mapping_rank = {
        MAPPING_EXACT: 0,
        MAPPING_NARROWING: 1,
        MAPPING_PARTIAL: 2,
        MAPPING_UNSUPPORTED: 3,
    }
    winner = candidate
    if status_rank[existing["status"]] < status_rank[candidate["status"]]:
        winner = existing
    elif status_rank[existing["status"]] == status_rank[candidate["status"]] and mapping_rank[existing["mapping_status"]] < mapping_rank[candidate["mapping_status"]]:
        winner = existing

    entries[key] = {
        **winner,
        "scope_descriptors": merged_descriptors,
        "reason_codes": stable_unique_strings(
            sorted(existing["reason_codes"] + candidate["reason_codes"])
        ),
    }


def overall_coverage_status(entries: list[dict[str, Any]], unmapped: list[dict[str, Any]]) -> str:
    statuses = [entry["status"] for entry in entries] + [item["coverage_status"] for item in unmapped]
    if not statuses:
        return COVERAGE_UNAVAILABLE
    if any(value == COVERAGE_UNAVAILABLE for value in statuses):
        return COVERAGE_UNAVAILABLE
    if any(value == COVERAGE_PARTIAL for value in statuses):
        return COVERAGE_PARTIAL
    return COVERAGE_COMPLETE


def observation_bundle_from_execution_record(
    execution_record: dict[str, Any],
    authority_plan: dict[str, Any],
) -> tuple[dict[str, Any], list[str]]:
    authority_observations = execution_record.get("authority_observations", [])
    coverage_entries: dict[tuple[str, str | None], dict[str, Any]] = {}
    observed_effects: list[dict[str, Any]] = []
    blocked_effects: list[dict[str, Any]] = []
    unmapped: list[dict[str, Any]] = []

    for grant in authority_plan.get("grants", []):
        mapping = runtime_mapping_for_effect(grant)
        merge_coverage_entry(
            coverage_entries,
            family=mapping["family"],
            draft_effect_class=mapping["draft_effect_class"],
            scope_kind=mapping["scope_kind"],
            mapping_status=mapping["mapping_status"],
            scope_descriptors=scope_descriptors_for_effect(grant),
            notes=mapping["notes"],
        )

    for observation in authority_observations:
        family = observation["family"]
        if family in ACTIVE_OBSERVABLE_FAMILIES:
            mapped_effect = canonical_effect_from_runtime_observation(observation)
            merge_coverage_entry(
                coverage_entries,
                family=family,
                draft_effect_class=None,
                scope_kind=effect_scope_kind(mapped_effect),
                mapping_status=MAPPING_EXACT,
                scope_descriptors=scope_descriptors_for_effect(mapped_effect),
                notes="Live runtime authority observations are carried directly as canonical control-plane families.",
            )
            if observation["status"] == "exercised":
                observed_effects.append(mapped_effect)
            else:
                blocked_effects.append(blocked_effect_from_runtime_observation(observation))
            continue

        merge_coverage_entry(
            coverage_entries,
            family=family,
            draft_effect_class=None,
            scope_kind=None,
            mapping_status=MAPPING_UNSUPPORTED,
            scope_descriptors=[family],
            notes="The live runtime family is canonical, but draft-v1 still lacks a safe effect-class representation for it.",
        )
        unmapped.append(
            unmapped_observation(
                observation,
                coverage_status=runtime_coverage_for_family(family, MAPPING_UNSUPPORTED),
            )
        )

    coverage = sorted(
        coverage_entries.values(),
        key=lambda item: (
            item["family"],
            item["draft_effect_class"] or "",
            item["scope_kind"] or "",
            ",".join(item["scope_descriptors"]),
        ),
    )

    return (
        {
            "source": {
                "source_id": execution_record["receipt"]["uri"],
                "source_kind": LIVE_RUNTIME_SOURCE_KIND,
                "version": "1.0.0",
                "notes": "Observation derived from the live Rust execution record authority_observations stream.",
            },
            "observed_effects": stable_unique_dicts(observed_effects),
            "blocked_effects": blocked_effects,
            "blocked_attempts_observable": True,
            "unmapped_observations": unmapped,
            "coverage_families": coverage,
            "overall_status": overall_coverage_status(coverage, unmapped),
            "raw_trace": {
                "execution_id": execution_record["receipt"]["execution_id"],
                "authority_observations": authority_observations,
            },
            "started_at": execution_record.get("provenance", {}).get("started_at_utc"),
            "finished_at": execution_record.get("provenance", {}).get("finished_at_utc"),
        },
        [],
    )
