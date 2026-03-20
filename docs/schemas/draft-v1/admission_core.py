from __future__ import annotations

import hashlib
import json
from copy import deepcopy
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator

try:
    from referencing import Registry, Resource
except ModuleNotFoundError as exc:  # pragma: no cover - mirrored by validate_examples.py
    raise SystemExit(
        "Missing validation dependency: referencing. Install local deps with `pip install -r requirements.txt`."
    ) from exc


BASE = Path(__file__).resolve().parent
SCHEMA_FILES = [
    "common.schema.json",
    "skill_contract.schema.json",
    "runtime_guarantee.schema.json",
    "comparator_profile.schema.json",
    "proof_record.schema.json",
    "witness_record.schema.json",
    "admission_request.schema.json",
    "execution_plan.schema.json",
]

ORDER = {
    "execution_isolation_assurance": ["none", "best_effort", "strong"],
    "filesystem_isolation_class": ["none", "path_filter", "preopen_only", "virtual_fs", "os_sandbox"],
    "network_policy_granularity": ["none", "binary", "domain", "host_port", "url"],
    "witness_level": ["summary", "decision", "hostcall", "full"],
}

SYMLINK_POLICY_ORDER = ["deny", "readonly", "follow"]
DECISION_PRECEDENCE = {"admit": 0, "downgrade": 1, "migrate": 2, "refuse": 3}


class AdmissionInputError(RuntimeError):
    """Raised when the admission inputs do not validate."""


def build_registry() -> Registry:
    registry = Registry()
    for name in SCHEMA_FILES:
        path = BASE / name
        contents = json.loads(path.read_text())
        resource = Resource.from_contents(contents)
        registry = registry.with_resource(path.as_uri(), resource)
        registry = registry.with_resource(name, resource)
    return registry


def load_json(path: str | Path) -> dict[str, Any]:
    candidate = Path(path)
    if candidate.exists():
        return json.loads(candidate.read_text())
    if not candidate.is_absolute():
        candidate = BASE / candidate
    return json.loads(candidate.read_text())


def canonical_json(value: Any) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def digest_struct(value: Any) -> dict[str, str]:
    return {
        "algorithm": "sha256",
        "value": hashlib.sha256(canonical_json(value).encode("utf-8")).hexdigest(),
    }


def rank(kind: str, value: str) -> int:
    return ORDER[kind].index(value)


def validate_instance(schema_name: str, instance: dict[str, Any], registry: Registry) -> list[str]:
    schema = json.loads((BASE / schema_name).read_text())
    validator = Draft202012Validator(schema, registry=registry)
    errors = sorted(validator.iter_errors(instance), key=lambda error: list(error.path))
    return [f"{'/'.join(map(str, error.path)) or '<root>'}: {error.message}" for error in errors]


def require_valid(schema_name: str, instance: dict[str, Any], registry: Registry, label: str) -> None:
    errors = validate_instance(schema_name, instance, registry)
    if errors:
        rendered = "; ".join(errors)
        raise AdmissionInputError(f"{label} failed validation: {rendered}")


def stable_unique_strings(values: list[str]) -> list[str]:
    seen: set[str] = set()
    output: list[str] = []
    for value in values:
        if value not in seen:
            seen.add(value)
            output.append(value)
    return output


def stable_unique_dicts(values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    seen: set[str] = set()
    output: list[dict[str, Any]] = []
    for value in values:
        key = canonical_json(value)
        if key not in seen:
            seen.add(key)
            output.append(value)
    output.sort(key=canonical_json)
    return output


def reason_item(
    reason_code: str,
    message: str,
    detail: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "reason_code": reason_code,
        "message": message,
        "detail": detail or {},
    }


def unsatisfied_requirement(
    requirement_kind: str,
    subject: str,
    reason_code: str,
    message: str,
    detail: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "requirement_kind": requirement_kind,
        "subject": subject,
        "reason": reason_item(reason_code, message, detail),
    }


def denied_scope(
    phase: str,
    requested_effect: dict[str, Any],
    reason_code: str,
    message: str,
    granted_effects: list[dict[str, Any]] | None = None,
    detail: dict[str, Any] | None = None,
) -> dict[str, Any]:
    return {
        "phase": phase,
        "requested_effect": requested_effect,
        "granted_effects": stable_unique_dicts(granted_effects or []),
        "reason": reason_item(reason_code, message, detail),
    }


def prerequisite(code: str, required: bool = True, detail: dict[str, Any] | None = None) -> dict[str, Any]:
    return {
        "code": code,
        "required": required,
        "detail": detail or {},
    }


def requested_authority_plan_id(request_id: str) -> str:
    return f"{request_id}:requested"


def granted_authority_plan_id(request_id: str) -> str:
    return f"{request_id}:granted"


def plan_id_for_request(request_id: str) -> str:
    return f"{request_id}:execution-plan"


def required_effect_classes(contract: dict[str, Any]) -> list[str]:
    classes = {
        effect["effect_class"]
        for collection in (contract.get("required_effects", []), contract.get("authority_ceiling", []))
        for effect in collection
    }
    return sorted(classes)


def runtime_overview(runtime: dict[str, Any]) -> dict[str, Any]:
    return {
        "runtime_guarantee_id": runtime["runtime_guarantee_id"],
        "runtime_guarantee_digest": digest_struct(runtime),
        "runtime": runtime["runtime"],
    }


def normalize_effect(effect: dict[str, Any]) -> dict[str, Any]:
    normalized = deepcopy(effect)
    normalized.pop("justification", None)
    if "bindings" in normalized and not normalized["bindings"]:
        normalized.pop("bindings")
    if "cardinality" in normalized:
        normalized["cardinality"] = {
            key: normalized["cardinality"][key]
            for key in sorted(normalized["cardinality"])
        }
    return normalized


def effect_equal(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return canonical_json(normalize_effect(left)) == canonical_json(normalize_effect(right))


def path_pattern_covers(container: str, candidate: str) -> bool:
    if container == "*" or container == "/**":
        return True
    if container.endswith("/**"):
        prefix = container[:-3]
        return candidate == prefix or candidate.startswith(prefix.rstrip("/") + "/")
    return container == candidate


def narrower_path_pattern(left: str, right: str) -> str | None:
    if path_pattern_covers(left, right):
        return right
    if path_pattern_covers(right, left):
        return left
    return None


def intersect_path_patterns(left: list[str], right: list[str]) -> list[str]:
    reduced: list[str] = []
    for left_path in left:
        for right_path in right:
            narrower = narrower_path_pattern(left_path, right_path)
            if narrower is not None:
                reduced.append(narrower)
    return stable_unique_strings(sorted(reduced))


def intersect_scalar_scope(
    left: list[Any] | None,
    right: list[Any] | None,
    *,
    wildcard: Any = "*",
    required: bool = False,
) -> list[Any] | None:
    left_values = left or ([wildcard] if required else None)
    right_values = right or ([wildcard] if required else None)

    if left_values is None and right_values is None:
        return None
    if left_values is not None and wildcard in left_values:
        return None if right_values == [wildcard] and not required else deepcopy(right_values)
    if right_values is not None and wildcard in right_values:
        return None if left_values == [wildcard] and not required else deepcopy(left_values)
    if left_values is None:
        return deepcopy(right_values)
    if right_values is None:
        return deepcopy(left_values)

    reduced = [value for value in left_values if value in right_values]
    if not reduced:
        return []
    if all(isinstance(item, int) for item in reduced):
        return sorted(reduced)
    return stable_unique_strings(sorted(str(item) for item in reduced))


def intersect_path_prefixes(left: list[str] | None, right: list[str] | None) -> list[str] | None:
    if left is None and right is None:
        return None
    if left is None:
        return deepcopy(right)
    if right is None:
        return deepcopy(left)
    reduced: list[str] = []
    for left_prefix in left:
        for right_prefix in right:
            if left_prefix.startswith(right_prefix):
                reduced.append(left_prefix)
            elif right_prefix.startswith(left_prefix):
                reduced.append(right_prefix)
    if not reduced:
        return []
    return stable_unique_strings(sorted(reduced))


def intersect_network_audience(
    requested: dict[str, Any],
    ceiling: dict[str, Any],
) -> dict[str, Any] | None:
    requested_host = requested["host"]
    ceiling_host = ceiling["host"]
    if requested_host == "*" and ceiling_host == "*":
        host = "*"
    elif requested_host == "*":
        host = ceiling_host
    elif ceiling_host == "*":
        host = requested_host
    elif requested_host == ceiling_host:
        host = requested_host
    else:
        return None

    ports = intersect_scalar_scope(requested.get("ports"), ceiling.get("ports"), required=True)
    if ports == []:
        return None

    schemes = intersect_scalar_scope(requested.get("schemes"), ceiling.get("schemes"))
    if schemes == []:
        return None

    path_prefixes = intersect_path_prefixes(requested.get("path_prefixes"), ceiling.get("path_prefixes"))
    if path_prefixes == []:
        return None

    methods = intersect_scalar_scope(requested.get("methods"), ceiling.get("methods"))
    if methods == []:
        return None

    output: dict[str, Any] = {
        "host": host,
        "ports": ports,
    }
    if schemes is not None:
        output["schemes"] = schemes
    if path_prefixes is not None:
        output["path_prefixes"] = path_prefixes
    if methods is not None:
        output["methods"] = methods
    return output


def intersect_network_audiences(
    requested: list[dict[str, Any]],
    ceiling: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    reduced: list[dict[str, Any]] = []
    for requested_audience in requested:
        for ceiling_audience in ceiling:
            intersected = intersect_network_audience(requested_audience, ceiling_audience)
            if intersected is not None:
                reduced.append(intersected)
    return stable_unique_dicts(reduced)


def stricter_symlink_policy(requested: str | None, ceiling: str | None) -> str | None:
    if requested is None:
        return ceiling
    if ceiling is None:
        return requested
    return SYMLINK_POLICY_ORDER[min(SYMLINK_POLICY_ORDER.index(requested), SYMLINK_POLICY_ORDER.index(ceiling))]


def narrower_bool(requested: bool | None, ceiling: bool | None) -> bool | None:
    if requested is None:
        return ceiling
    if ceiling is None:
        return requested
    return requested and ceiling


def intersect_exact_strings(requested: list[str] | None, ceiling: list[str] | None) -> list[str] | None:
    return intersect_scalar_scope(requested, ceiling)


def intersect_component_digests(
    requested: list[dict[str, Any]] | None,
    ceiling: list[dict[str, Any]] | None,
) -> list[dict[str, Any]] | None:
    if requested is None and ceiling is None:
        return None
    if requested is None:
        return deepcopy(ceiling)
    if ceiling is None:
        return deepcopy(requested)
    reduced: list[dict[str, Any]] = []
    for requested_digest in requested:
        for ceiling_digest in ceiling:
            if requested_digest == ceiling_digest:
                reduced.append(deepcopy(requested_digest))
    if not reduced:
        return []
    return stable_unique_dicts(reduced)


def intersect_scope(requested: dict[str, Any], ceiling: dict[str, Any]) -> dict[str, Any] | None:
    if requested["kind"] != ceiling["kind"]:
        return None

    kind = requested["kind"]
    if kind == "filesystem":
        paths = intersect_path_patterns(requested["paths"], ceiling["paths"])
        if not paths:
            return None
        output: dict[str, Any] = {
            "kind": kind,
            "paths": paths,
        }
        symlink_policy = stricter_symlink_policy(requested.get("symlink_policy"), ceiling.get("symlink_policy"))
        follow_mounts = narrower_bool(requested.get("follow_mounts"), ceiling.get("follow_mounts"))
        if symlink_policy is not None:
            output["symlink_policy"] = symlink_policy
        if follow_mounts is not None:
            output["follow_mounts"] = follow_mounts
        return output

    if kind == "network":
        audiences = intersect_network_audiences(requested["audiences"], ceiling["audiences"])
        if not audiences:
            return None
        return {
            "kind": kind,
            "audiences": audiences,
        }

    if kind == "process":
        commands = intersect_scalar_scope(requested.get("commands"), ceiling.get("commands"), required=True)
        if commands == []:
            return None
        argv_patterns = intersect_exact_strings(requested.get("argv_patterns"), ceiling.get("argv_patterns"))
        if argv_patterns == []:
            return None
        output = {"kind": kind, "commands": commands}
        if argv_patterns is not None:
            output["argv_patterns"] = argv_patterns
        return output

    if kind == "secret":
        secret_ids = intersect_exact_strings(requested.get("secret_ids"), ceiling.get("secret_ids"))
        if secret_ids == []:
            return None
        return {"kind": kind, "secret_ids": secret_ids}

    if kind == "environment":
        names = intersect_scalar_scope(requested.get("names"), ceiling.get("names"), required=True)
        if names == []:
            return None
        return {"kind": kind, "names": names}

    if kind == "system":
        return {"kind": kind}

    if kind == "component":
        component_digests = intersect_component_digests(
            requested.get("component_digests"), ceiling.get("component_digests")
        )
        if component_digests == []:
            return None
        exports = intersect_exact_strings(requested.get("exports"), ceiling.get("exports"))
        if exports == []:
            return None
        output = {"kind": kind}
        if component_digests is not None:
            output["component_digests"] = component_digests
        if exports is not None:
            output["exports"] = exports
        return output

    if kind == "delegation":
        effect_classes = intersect_exact_strings(requested.get("effect_classes"), ceiling.get("effect_classes"))
        if effect_classes == []:
            return None
        audiences = intersect_exact_strings(requested.get("audiences"), ceiling.get("audiences"))
        if audiences == []:
            return None
        max_hops_values = [value for value in (requested.get("max_hops"), ceiling.get("max_hops")) if value is not None]
        output = {"kind": kind}
        if effect_classes is not None:
            output["effect_classes"] = effect_classes
        if audiences is not None:
            output["audiences"] = audiences
        if max_hops_values:
            output["max_hops"] = min(max_hops_values)
        return output

    raise ValueError(f"unsupported scope kind {kind}")


def intersect_cardinality(
    requested: dict[str, Any] | None,
    ceiling: dict[str, Any] | None,
) -> dict[str, Any] | None:
    keys = ("max_calls", "max_bytes", "max_items")
    output: dict[str, Any] = {}
    for key in keys:
        values = [value for value in ((requested or {}).get(key), (ceiling or {}).get(key)) if value is not None]
        if values:
            output[key] = min(values)
    return output or None


def intersect_effect(requested: dict[str, Any], ceiling: dict[str, Any]) -> dict[str, Any] | None:
    if requested["effect_class"] != ceiling["effect_class"]:
        return None
    scope = intersect_scope(requested["scope"], ceiling["scope"])
    if scope is None:
        return None
    output: dict[str, Any] = {
        "effect_class": requested["effect_class"],
        "scope": scope,
    }
    cardinality = intersect_cardinality(requested.get("cardinality"), ceiling.get("cardinality"))
    if cardinality is not None:
        output["cardinality"] = cardinality
    if requested.get("bindings"):
        output["bindings"] = deepcopy(requested["bindings"])
    return output


def cardinality_covers(grant: dict[str, Any] | None, required: dict[str, Any] | None) -> bool:
    grant = grant or {}
    required = required or {}
    for key in ("max_calls", "max_bytes", "max_items"):
        required_value = required.get(key)
        if required_value is None:
            continue
        grant_value = grant.get(key)
        if grant_value is None or grant_value < required_value:
            return False
    return True


def effect_covers(grant: dict[str, Any], required: dict[str, Any]) -> bool:
    if grant["effect_class"] != required["effect_class"]:
        return False
    reduced = intersect_effect(grant, required)
    if reduced is None:
        return False
    if not effect_equal(reduced, required):
        return False
    return cardinality_covers(grant.get("cardinality"), required.get("cardinality"))


def scope_uses_any(values: list[Any] | None) -> bool:
    return values is not None and "*" not in values


def runtime_can_enforce_network_scope(scope: dict[str, Any], granularity: str) -> bool:
    if granularity == "url":
        return True
    if granularity == "host_port":
        return not any(
            (
                scope_uses_any(audience.get("schemes")),
                scope_uses_any(audience.get("path_prefixes")),
                scope_uses_any(audience.get("methods")),
            )
            for audience in scope["audiences"]
        )
    if granularity == "domain":
        return not any(
            (
                audience["host"] == "*",
                scope_uses_any(audience.get("ports")),
                scope_uses_any(audience.get("schemes")),
                scope_uses_any(audience.get("path_prefixes")),
                scope_uses_any(audience.get("methods")),
            )
            for audience in scope["audiences"]
        )
    if granularity == "binary":
        return all(
            audience["host"] == "*"
            and not scope_uses_any(audience.get("ports"))
            and not scope_uses_any(audience.get("schemes"))
            and not scope_uses_any(audience.get("path_prefixes"))
            and not scope_uses_any(audience.get("methods"))
            for audience in scope["audiences"]
        )
    return False


def runtime_can_enforce_effect(effect: dict[str, Any], runtime: dict[str, Any]) -> bool:
    effect_class = effect["effect_class"]
    if effect_class not in runtime.get("supported_effect_classes", []):
        return False

    if effect_class in {"net.connect", "net.resolve"}:
        return runtime_can_enforce_network_scope(effect["scope"], runtime["network_policy_granularity"])

    if effect_class in {"fs.read", "fs.write", "fs.list"}:
        return runtime["filesystem_isolation_class"] != "none"

    return True


def contract_forbids_effect(contract: dict[str, Any], effect: dict[str, Any]) -> bool:
    for forbidden in contract.get("forbidden_effects", []):
        if forbidden["effect_class"] != effect["effect_class"]:
            continue
        if intersect_effect(effect, forbidden) is not None:
            return True
    return False


def requirement_messages(contract: dict[str, Any], runtime: dict[str, Any]) -> list[dict[str, Any]]:
    req = contract["required_runtime_guarantees"]
    reasons: list[dict[str, Any]] = []

    component = contract["component"]
    component_support = runtime.get("component_model_support")
    if not component_support:
        reasons.append(
            unsatisfied_requirement(
                "component_model_support",
                component["component_model"],
                "RUNTIME_COMPONENT_MODEL_UNSUPPORTED",
                "runtime guarantee omitted component model support details",
            )
        )
    else:
        if component_support.get("component_model") != component["component_model"]:
            reasons.append(
                unsatisfied_requirement(
                    "component_model_support",
                    component["component_model"],
                    "RUNTIME_COMPONENT_MODEL_UNSUPPORTED",
                    "runtime did not publish support for the required component model",
                    {
                        "required_component_model": component["component_model"],
                        "published_component_model": component_support.get("component_model"),
                    },
                )
            )
        if component["component_model_version"] not in component_support.get("supported_versions", []):
            reasons.append(
                unsatisfied_requirement(
                    "component_model_version",
                    component["component_model_version"],
                    "RUNTIME_COMPONENT_MODEL_VERSION_UNSUPPORTED",
                    "runtime did not publish support for the required component model version",
                    {
                        "required_component_model_version": component["component_model_version"],
                        "published_versions": component_support.get("supported_versions", []),
                    },
                )
            )
        declared_worlds = component_support.get("wit_worlds")
        if declared_worlds is None:
            reasons.append(
                unsatisfied_requirement(
                    "wit_world",
                    component["wit_world"],
                    "RUNTIME_WIT_WORLD_UNDECLARED",
                    "runtime must enumerate component_model_support.wit_worlds explicitly",
                )
            )
        elif component["wit_world"] not in declared_worlds:
            reasons.append(
                unsatisfied_requirement(
                    "wit_world",
                    component["wit_world"],
                    "RUNTIME_WIT_WORLD_UNSUPPORTED",
                    "runtime did not publish the required WIT world",
                    {
                        "required_wit_world": component["wit_world"],
                        "published_wit_worlds": declared_worlds,
                    },
                )
            )

    unsupported_effects = [
        effect_class
        for effect_class in required_effect_classes(contract)
        if effect_class not in runtime.get("supported_effect_classes", [])
    ]
    for effect_class in unsupported_effects:
        reasons.append(
            unsatisfied_requirement(
                "effect_class",
                effect_class,
                "REQUIRED_EFFECT_UNSUPPORTED",
                "runtime did not publish support for a required effect class",
            )
        )

    for required_effect in contract.get("required_effects", []):
        if required_effect["effect_class"] not in runtime.get("supported_effect_classes", []):
            continue
        if not runtime_can_enforce_effect(required_effect, runtime):
            reasons.append(
                unsatisfied_requirement(
                    "required_effect_scope",
                    required_effect["effect_class"],
                    "REQUIRED_SCOPE_NOT_ENFORCEABLE",
                    "runtime cannot enforce the scope constraints on a required effect",
                    {"required_effect": normalize_effect(required_effect)},
                )
            )

    ordered_checks = (
        (
            "execution_isolation_assurance",
            "RUNTIME_EXECUTION_ISOLATION_TOO_WEAK",
            "runtime execution isolation assurance was weaker than required",
        ),
        (
            "filesystem_isolation_class",
            "RUNTIME_FILESYSTEM_ISOLATION_TOO_WEAK",
            "runtime filesystem isolation class was weaker than required",
        ),
        (
            "network_policy_granularity",
            "RUNTIME_NETWORK_GRANULARITY_TOO_WEAK",
            "runtime network policy granularity was weaker than required",
        ),
    )
    for field, reason_code, message in ordered_checks:
        if rank(field, runtime[field]) < rank(field, req[field]["minimum"]):
            reasons.append(
                unsatisfied_requirement(
                    "runtime_guarantee",
                    field,
                    reason_code,
                    message,
                    {
                        "required": req[field]["minimum"],
                        "published": runtime[field],
                    },
                )
            )

    mode_checks = (
        ("child_process_policy", "required_mode", "RUNTIME_CHILD_PROCESS_MODE_UNSUPPORTED"),
        ("token_passthrough_policy", "required_mode", "RUNTIME_TOKEN_PASSTHROUGH_MODE_UNSUPPORTED"),
        ("revocation_behavior", "required_mode", "RUNTIME_REVOCATION_MODE_UNSUPPORTED"),
    )
    for field, mode_key, reason_code in mode_checks:
        required_mode = req[field][mode_key]
        if required_mode not in runtime[field]["supported_modes"]:
            reasons.append(
                unsatisfied_requirement(
                    "runtime_guarantee",
                    field,
                    reason_code,
                    "runtime did not publish the required policy mode",
                    {
                        "required_mode": required_mode,
                        "supported_modes": runtime[field]["supported_modes"],
                    },
                )
            )

    enforcement = req["delegation_enforcement"]
    published = runtime["delegation_enforcement"]
    enforcement_checks = (
        ("audience_binding_required", "audience_binding", "RUNTIME_AUDIENCE_BINDING_UNSUPPORTED"),
        ("call_chain_binding_required", "call_chain_binding", "RUNTIME_CALL_CHAIN_BINDING_UNSUPPORTED"),
        ("anti_replay_required", "anti_replay", "RUNTIME_ANTI_REPLAY_UNSUPPORTED"),
        ("max_hops_enforced_required", "max_hops_enforced", "RUNTIME_MAX_HOPS_ENFORCEMENT_UNSUPPORTED"),
    )
    for required_key, published_key, reason_code in enforcement_checks:
        if enforcement[required_key] and not published[published_key]:
            reasons.append(
                unsatisfied_requirement(
                    "delegation_enforcement",
                    published_key,
                    reason_code,
                    "runtime did not publish a required delegation enforcement guarantee",
                )
            )

    witness_required = req["witness_support"]
    witness_published = runtime["witness_support"]
    supported_level_ok = any(
        rank("witness_level", level) >= rank("witness_level", witness_required["minimum_level"])
        for level in witness_published["supported_levels"]
    )
    if not supported_level_ok:
        reasons.append(
            unsatisfied_requirement(
                "witness_support",
                "minimum_level",
                "RUNTIME_WITNESS_LEVEL_UNSUPPORTED",
                "runtime witness support was weaker than required",
                {
                    "required_level": witness_required["minimum_level"],
                    "supported_levels": witness_published["supported_levels"],
                },
            )
        )

    if not set(witness_required["acceptable_tamper_evidence_modes"]).intersection(
        witness_published["tamper_evidence_modes"]
    ):
        reasons.append(
            unsatisfied_requirement(
                "witness_support",
                "tamper_evidence_modes",
                "RUNTIME_TAMPER_EVIDENCE_MODE_UNSUPPORTED",
                "runtime did not publish an acceptable tamper-evidence mode",
            )
        )

    if not set(witness_required["acceptable_signature_modes"]).intersection(
        witness_published["signature_modes"]
    ):
        reasons.append(
            unsatisfied_requirement(
                "witness_support",
                "signature_modes",
                "RUNTIME_SIGNATURE_MODE_UNSUPPORTED",
                "runtime did not publish an acceptable witness signature mode",
            )
        )

    boolean_checks = (
        ("trusted_time_source_required", "trusted_time_source", "RUNTIME_TRUSTED_TIME_SOURCE_UNSUPPORTED"),
        ("redacted_io_hashes_required", "redacted_io_hashes", "RUNTIME_REDACTED_IO_HASHES_UNSUPPORTED"),
        ("authority_plan_digest_required", "authority_plan_digest", "RUNTIME_AUTHORITY_PLAN_DIGEST_UNSUPPORTED"),
    )
    for required_key, published_key, reason_code in boolean_checks:
        if witness_required[required_key] and not witness_published[published_key]:
            reasons.append(
                unsatisfied_requirement(
                    "witness_support",
                    published_key,
                    reason_code,
                    "runtime did not publish a required witness capability",
                )
            )

    return reasons


def match_hard_requirements(contract: dict[str, Any], runtime: dict[str, Any]) -> dict[str, Any]:
    unsatisfied = requirement_messages(contract, runtime)
    return {
        "ok": not unsatisfied,
        "unsatisfied_requirements": unsatisfied,
        "reason_codes": stable_unique_strings(
            sorted(item["reason"]["reason_code"] for item in unsatisfied)
        ),
    }


def runtime_request_witness_ok(contract: dict[str, Any], request: dict[str, Any], runtime: dict[str, Any]) -> dict[str, Any]:
    requested_level = request.get("requested_witness_level")
    if requested_level is None:
        return {"ok": True, "unsatisfied_requirements": [], "reason_codes": []}

    supported = runtime["witness_support"]["supported_levels"]
    if any(rank("witness_level", level) >= rank("witness_level", requested_level) for level in supported):
        return {"ok": True, "unsatisfied_requirements": [], "reason_codes": []}

    requirement = unsatisfied_requirement(
        "requested_witness_level",
        requested_level,
        "REQUESTED_WITNESS_LEVEL_UNAVAILABLE",
        "runtime could not satisfy the requested witness level",
        {"requested_witness_level": requested_level, "supported_levels": supported},
    )
    return {"ok": False, "unsatisfied_requirements": [requirement], "reason_codes": [requirement["reason"]["reason_code"]]}


def reduce_requested_grant(
    requested: dict[str, Any],
    contract: dict[str, Any],
    runtime: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if contract_forbids_effect(contract, requested):
        return [], [
            denied_scope(
                "contract_ceiling",
                requested,
                "REQUESTED_AUTHORITY_FORBIDDEN_BY_CONTRACT",
                "requested authority overlapped a contract-forbidden effect",
            )
        ]

    ceilings = [
        ceiling
        for ceiling in contract.get("authority_ceiling", [])
        if ceiling["effect_class"] == requested["effect_class"]
    ]
    if not ceilings:
        return [], [
            denied_scope(
                "contract_ceiling",
                requested,
                "REQUESTED_AUTHORITY_EXCEEDS_CONTRACT_CEILING",
                "requested authority was outside the contract authority ceiling",
            )
        ]

    reduced = []
    for ceiling in ceilings:
        intersected = intersect_effect(requested, ceiling)
        if intersected is not None:
            reduced.append(intersected)
    reduced = stable_unique_dicts(reduced)
    if not reduced:
        return [], [
            denied_scope(
                "contract_ceiling",
                requested,
                "REQUESTED_AUTHORITY_EXCEEDS_CONTRACT_CEILING",
                "requested authority did not intersect any contract-ceiling grant",
            )
        ]

    enforceable = [grant for grant in reduced if runtime_can_enforce_effect(grant, runtime)]
    if not enforceable:
        return [], [
            denied_scope(
                "runtime_enforcement",
                requested,
                "REQUESTED_SCOPE_NOT_ENFORCEABLE",
                "selected runtime could not enforce the requested scope safely",
                detail={"candidate_grants": reduced},
            )
        ]

    if len(enforceable) == 1 and effect_equal(enforceable[0], requested):
        return enforceable, []

    return enforceable, [
        denied_scope(
            "contract_ceiling",
            requested,
            "REQUESTED_SCOPE_REDUCED_FOR_CONTRACT",
            "requested authority was narrowed to the contract-safe upper bound",
            granted_effects=enforceable,
        )
    ]


def required_effects_satisfied(contract: dict[str, Any], grants: list[dict[str, Any]]) -> list[dict[str, Any]]:
    unsatisfied: list[dict[str, Any]] = []
    for required_effect in contract.get("required_effects", []):
        if any(effect_covers(grant, required_effect) for grant in grants):
            continue
        unsatisfied.append(
            unsatisfied_requirement(
                "required_effect",
                required_effect["effect_class"],
                "REQUESTED_AUTHORITY_OMITS_REQUIRED_EFFECT",
                "granted authority did not cover a contract-required effect",
                {"required_effect": normalize_effect(required_effect)},
            )
        )
    return unsatisfied


def requested_authority_to_plan(request_id: str, request: dict[str, Any], contract: dict[str, Any]) -> dict[str, Any]:
    requested_authority = request["requested_authority"]
    requested_ttl = requested_authority.get("ttl_seconds")
    contract_ttl = contract["delegation_policy"]["ttl_seconds_max"]
    effective_ttl = min(value for value in (requested_ttl, contract_ttl) if value is not None) if requested_ttl else contract_ttl
    return {
        "plan_id": requested_authority_plan_id(request_id),
        "grants": stable_unique_dicts([normalize_effect(grant) for grant in requested_authority.get("grants", [])]),
        "delegation_policy": deepcopy(contract["delegation_policy"]),
        "ttl_seconds": effective_ttl,
    }


def granted_authority_plan(
    request_id: str,
    contract: dict[str, Any],
    request: dict[str, Any],
    grants: list[dict[str, Any]],
) -> dict[str, Any]:
    requested_ttl = request["requested_authority"].get("ttl_seconds")
    contract_ttl = contract["delegation_policy"]["ttl_seconds_max"]
    effective_ttl = min(value for value in (requested_ttl, contract_ttl) if value is not None) if requested_ttl else contract_ttl
    return {
        "plan_id": granted_authority_plan_id(request_id),
        "grants": stable_unique_dicts([normalize_effect(grant) for grant in grants]),
        "delegation_policy": deepcopy(contract["delegation_policy"]),
        "ttl_seconds": effective_ttl,
    }


def runtime_candidate_from_request(
    candidate: dict[str, Any],
    runtime_map: dict[str, dict[str, Any]],
) -> tuple[dict[str, Any] | None, str]:
    if candidate.get("kind") == "guild.runtime_guarantee":
        return candidate, candidate["runtime_guarantee_id"]
    runtime_guarantee_id = candidate["runtime_guarantee_id"]
    return runtime_map.get(runtime_guarantee_id), runtime_guarantee_id


def contract_from_request(request: dict[str, Any], contract: dict[str, Any] | None) -> dict[str, Any]:
    requested_contract = request["contract"]
    if requested_contract.get("kind") == "guild.skill_contract":
        return requested_contract
    if contract is None:
        raise AdmissionInputError("admission request referenced a contract by identity but no external contract document was supplied")
    return contract


def verify_contract_reference(request: dict[str, Any], contract: dict[str, Any]) -> list[dict[str, Any]]:
    requested_contract = request["contract"]
    if requested_contract.get("kind") == "guild.skill_contract":
        return []

    issues: list[dict[str, Any]] = []
    if requested_contract["contract_id"] != contract["contract_id"]:
        issues.append(
            unsatisfied_requirement(
                "contract_reference",
                requested_contract["contract_id"],
                "CONTRACT_REFERENCE_MISMATCH",
                "admission request contract reference did not match the supplied contract id",
            )
        )
    if requested_contract["component_digest"] != contract["component"]["digest"]:
        issues.append(
            unsatisfied_requirement(
                "contract_reference",
                requested_contract["contract_id"],
                "CONTRACT_REFERENCE_MISMATCH",
                "admission request component digest did not match the supplied contract",
            )
        )
    return issues


def verify_request_contract_alignment(contract: dict[str, Any], request: dict[str, Any]) -> list[dict[str, Any]]:
    issues = verify_contract_reference(request, contract)
    export_override = request.get("export_name_override")
    if export_override is not None and export_override != contract["export"]["name"]:
        issues.append(
            unsatisfied_requirement(
                "export_name",
                export_override,
                "EXPORT_OVERRIDE_INVALID",
                "export name override did not match the contract export in this single-export contract shape",
                {"contract_export_name": contract["export"]["name"]},
            )
        )

    if request["input_class_fingerprint"] != contract["input_class_fingerprint"]:
        issues.append(
            unsatisfied_requirement(
                "input_class_fingerprint",
                contract["contract_id"],
                "INPUT_CLASS_MISMATCH",
                "admission request input class fingerprint did not match the contract",
                {
                    "request_input_class_fingerprint": request["input_class_fingerprint"],
                    "contract_input_class_fingerprint": contract["input_class_fingerprint"],
                },
            )
        )
    return issues


def effective_witness_level(contract: dict[str, Any], request: dict[str, Any]) -> str:
    requested_level = request.get("requested_witness_level")
    contract_level = contract["witness_level"]
    if requested_level is None:
        return contract_level
    if rank("witness_level", requested_level) > rank("witness_level", contract_level):
        return requested_level
    return contract_level


def build_witness_policy(contract: dict[str, Any], request: dict[str, Any], runtime: dict[str, Any]) -> dict[str, Any]:
    witness_required = contract["required_runtime_guarantees"]["witness_support"]
    return {
        "contract_witness_level": contract["witness_level"],
        "requested_witness_level": request.get("requested_witness_level"),
        "effective_witness_level": effective_witness_level(contract, request),
        "supported_levels": runtime["witness_support"]["supported_levels"],
        "acceptable_tamper_evidence_modes": witness_required["acceptable_tamper_evidence_modes"],
        "acceptable_signature_modes": witness_required["acceptable_signature_modes"],
        "prerequisites": [
            prerequisite(
                "TRUSTED_TIME_SOURCE",
                witness_required["trusted_time_source_required"],
                {"required": witness_required["trusted_time_source_required"]},
            ),
            prerequisite(
                "REDACTED_IO_HASHES",
                witness_required["redacted_io_hashes_required"],
                {"required": witness_required["redacted_io_hashes_required"]},
            ),
            prerequisite(
                "AUTHORITY_PLAN_DIGEST",
                witness_required["authority_plan_digest_required"],
                {"required": witness_required["authority_plan_digest_required"]},
            ),
            prerequisite(
                "WITNESS_TAMPER_EVIDENCE_MODE",
                True,
                {"acceptable_modes": witness_required["acceptable_tamper_evidence_modes"]},
            ),
            prerequisite(
                "WITNESS_SIGNATURE_MODE",
                True,
                {"acceptable_modes": witness_required["acceptable_signature_modes"]},
            ),
        ],
    }


def build_proof_prerequisites(contract: dict[str, Any], runtime: dict[str, Any]) -> list[dict[str, Any]]:
    return [
        prerequisite("CONTRACT_DIGEST_BINDING", True, {"contract_digest": digest_struct(contract)}),
        prerequisite(
            "RUNTIME_GUARANTEE_DIGEST_BINDING",
            True,
            {"runtime_guarantee_digest": digest_struct(runtime)},
        ),
        prerequisite(
            "AUTHORITY_PLAN_DIGEST_BINDING",
            contract["required_runtime_guarantees"]["witness_support"]["authority_plan_digest_required"],
            {
                "required": contract["required_runtime_guarantees"]["witness_support"]["authority_plan_digest_required"]
            },
        ),
    ]


def build_delegation_token_policy_inputs(
    contract: dict[str, Any],
    request: dict[str, Any],
    runtime: dict[str, Any] | None,
) -> dict[str, Any]:
    required_runtime_guarantees = contract["required_runtime_guarantees"]
    runtime_default_token_passthrough_mode = None
    runtime_default_revocation_mode = None
    if runtime is not None:
        runtime_default_token_passthrough_mode = runtime["token_passthrough_policy"]["default_mode"]
        runtime_default_revocation_mode = runtime["revocation_behavior"]["default_mode"]
    return {
        "contract_delegation_policy": deepcopy(contract["delegation_policy"]),
        "required_token_passthrough_mode": required_runtime_guarantees["token_passthrough_policy"]["required_mode"],
        "runtime_default_token_passthrough_mode": runtime_default_token_passthrough_mode,
        "required_revocation_mode": required_runtime_guarantees["revocation_behavior"]["required_mode"],
        "runtime_default_revocation_mode": runtime_default_revocation_mode,
        "audience_binding_required": required_runtime_guarantees["delegation_enforcement"]["audience_binding_required"],
        "call_chain_binding_required": required_runtime_guarantees["delegation_enforcement"]["call_chain_binding_required"],
        "anti_replay_required": required_runtime_guarantees["delegation_enforcement"]["anti_replay_required"],
        "max_hops_enforced_required": required_runtime_guarantees["delegation_enforcement"]["max_hops_enforced_required"],
        "audience_binding_inputs": request.get("audience_binding_inputs", []),
        "resource_binding_inputs": request.get("resource_binding_inputs", []),
        "delegation_chain_input": request.get("delegation_chain_input"),
    }


def build_plan_validity(contract: dict[str, Any], request: dict[str, Any]) -> dict[str, Any]:
    constraints = request.get("invocation_constraints", {})
    requested_ttl = request["requested_authority"].get("ttl_seconds")
    if constraints.get("valid_for_seconds") is not None:
        requested_ttl = constraints["valid_for_seconds"] if requested_ttl is None else min(
            requested_ttl, constraints["valid_for_seconds"]
        )
    contract_ttl = contract["delegation_policy"]["ttl_seconds_max"]
    effective_ttl = min(value for value in (requested_ttl, contract_ttl) if value is not None) if requested_ttl else contract_ttl
    source = "contract" if requested_ttl is None else "contract_and_request"
    output = {
        "ttl_seconds": effective_ttl,
        "contract_ttl_seconds_max": contract_ttl,
        "requested_ttl_seconds": requested_ttl,
        "source": source,
    }
    if constraints.get("not_before") is not None:
        output["not_before"] = constraints["not_before"]
    if constraints.get("not_after") is not None:
        output["not_after"] = constraints["not_after"]
    return output


def evaluate_runtime_candidate(
    contract: dict[str, Any],
    request: dict[str, Any],
    runtime_candidate: dict[str, Any],
    runtime_map: dict[str, dict[str, Any]],
    registry: Registry,
    preferred_runtime_id: str | None,
) -> dict[str, Any]:
    runtime, runtime_guarantee_id = runtime_candidate_from_request(runtime_candidate, runtime_map)
    evaluation = {
        "runtime_guarantee_id": runtime_guarantee_id,
        "runtime": None,
        "preferred": runtime_guarantee_id == preferred_runtime_id,
        "chosen": False,
        "hard_requirements_satisfied": False,
        "admissible": False,
        "reason_codes": [],
        "unsatisfied_requirements": [],
    }

    if runtime is None:
        evaluation["unsatisfied_requirements"] = [
            unsatisfied_requirement(
                "runtime_candidate",
                runtime_guarantee_id,
                "RUNTIME_CANDIDATE_UNKNOWN",
                "admission request named a runtime candidate that was not supplied to the engine",
            )
        ]
        evaluation["reason_codes"] = ["RUNTIME_CANDIDATE_UNKNOWN"]
        return evaluation

    evaluation["runtime"] = runtime["runtime"]

    runtime_validation_errors = validate_instance("runtime_guarantee.schema.json", runtime, registry)
    if runtime_validation_errors:
        evaluation["unsatisfied_requirements"] = [
            unsatisfied_requirement(
                "runtime_schema",
                runtime_guarantee_id,
                "RUNTIME_GUARANTEE_INVALID",
                "runtime guarantee document failed schema validation",
                {"validation_errors": runtime_validation_errors},
            )
        ]
        evaluation["reason_codes"] = ["RUNTIME_GUARANTEE_INVALID"]
        return evaluation

    hard_requirements = match_hard_requirements(contract, runtime)
    witness_requirements = runtime_request_witness_ok(contract, request, runtime)

    combined_unsatisfied = hard_requirements["unsatisfied_requirements"] + witness_requirements["unsatisfied_requirements"]
    evaluation["unsatisfied_requirements"] = combined_unsatisfied
    evaluation["reason_codes"] = stable_unique_strings(
        sorted(item["reason"]["reason_code"] for item in combined_unsatisfied)
    )
    evaluation["hard_requirements_satisfied"] = not combined_unsatisfied
    if combined_unsatisfied:
        return evaluation

    requested_grants = request["requested_authority"].get("grants", [])
    granted: list[dict[str, Any]] = []
    denials: list[dict[str, Any]] = []
    for requested_grant in requested_grants:
        reduced_grants, denied = reduce_requested_grant(requested_grant, contract, runtime)
        granted.extend(reduced_grants)
        denials.extend(denied)
    granted = stable_unique_dicts([normalize_effect(grant) for grant in granted])
    denials = stable_unique_dicts(denials)

    unsatisfied_required = required_effects_satisfied(contract, granted)
    evaluation["unsatisfied_requirements"] = unsatisfied_required
    evaluation["reason_codes"] = stable_unique_strings(
        sorted(
            [item["reason"]["reason_code"] for item in unsatisfied_required]
            + [item["reason"]["reason_code"] for item in denials]
        )
    )
    if unsatisfied_required:
        evaluation["admissible"] = False
        evaluation["request_narrowing_applied"] = bool(denials)
        evaluation["granted_authority"] = granted_authority_plan(request["request_id"], contract, request, granted)
        evaluation["denied_authority"] = denials
        return evaluation

    evaluation["admissible"] = True
    evaluation["request_narrowing_applied"] = bool(denials)
    evaluation["granted_authority"] = granted_authority_plan(request["request_id"], contract, request, granted)
    evaluation["denied_authority"] = denials
    return evaluation


def aggregate_reason_codes(
    evaluation: dict[str, Any] | None,
    *,
    preferred_runtime_not_admissible: bool = False,
) -> list[str]:
    codes: list[str] = []
    if evaluation is not None:
        codes.extend(evaluation.get("reason_codes", []))
        codes.extend(item["reason"]["reason_code"] for item in evaluation.get("denied_authority", []))
    if preferred_runtime_not_admissible:
        codes.append("PREFERRED_RUNTIME_NOT_ADMISSIBLE")
    return stable_unique_strings(sorted(codes))


def public_runtime_evaluation(evaluation: dict[str, Any]) -> dict[str, Any]:
    return {
        "runtime_guarantee_id": evaluation["runtime_guarantee_id"],
        "runtime": evaluation["runtime"],
        "preferred": evaluation["preferred"],
        "chosen": evaluation["chosen"],
        "hard_requirements_satisfied": evaluation["hard_requirements_satisfied"],
        "admissible": evaluation["admissible"],
        "reason_codes": evaluation["reason_codes"],
        "unsatisfied_requirements": evaluation["unsatisfied_requirements"],
    }


def choose_runtime(
    evaluations: list[dict[str, Any]],
    preferred_runtime_id: str | None,
) -> tuple[dict[str, Any] | None, bool]:
    preferred_not_admissible = False
    if preferred_runtime_id is not None:
        preferred_evaluations = [
            evaluation for evaluation in evaluations if evaluation["runtime_guarantee_id"] == preferred_runtime_id
        ]
        for evaluation in preferred_evaluations:
            if evaluation["admissible"]:
                return evaluation, False
        preferred_not_admissible = bool(preferred_evaluations)

    for evaluation in evaluations:
        if evaluation["admissible"]:
            return evaluation, preferred_not_admissible

    return None, preferred_not_admissible


def global_refusal_plan(
    contract: dict[str, Any],
    request: dict[str, Any],
    issues: list[dict[str, Any]],
) -> dict[str, Any]:
    return {
        "kind": "guild.execution_plan",
        "version": "1.0.0",
        "plan_id": plan_id_for_request(request["request_id"]),
        "request_id": request["request_id"],
        "contract_id": contract["contract_id"],
        "contract_digest": digest_struct(contract),
        "component_digest": contract["component"]["digest"],
        "export_name": contract["export"]["name"],
        "input_class_fingerprint": deepcopy(contract["input_class_fingerprint"]),
        "decision": "refuse",
        "decision_reason_codes": stable_unique_strings(sorted(item["reason"]["reason_code"] for item in issues)),
        "preferred_runtime_guarantee_id": request.get("preferred_runtime_guarantee_id"),
        "chosen_runtime": None,
        "runtime_evaluations": [],
        "requested_authority": requested_authority_to_plan(request["request_id"], request, contract),
        "granted_authority": granted_authority_plan(request["request_id"], contract, request, []),
        "denied_authority": [],
        "hard_requirement_status": {
            "satisfied": False,
            "unsatisfied_requirements": issues,
        },
        "request_narrowing_applied": False,
        "proof_prerequisites": [],
        "witness_policy": {
            "contract_witness_level": contract["witness_level"],
            "requested_witness_level": request.get("requested_witness_level"),
            "effective_witness_level": effective_witness_level(contract, request),
            "supported_levels": [],
            "acceptable_tamper_evidence_modes": contract["required_runtime_guarantees"]["witness_support"][
                "acceptable_tamper_evidence_modes"
            ],
            "acceptable_signature_modes": contract["required_runtime_guarantees"]["witness_support"][
                "acceptable_signature_modes"
            ],
            "prerequisites": [],
        },
        "delegation_token_policy_inputs": build_delegation_token_policy_inputs(contract, request, None),
        "plan_validity": build_plan_validity(contract, request),
    }


def build_execution_plan(
    contract: dict[str, Any] | None,
    request: dict[str, Any],
    runtimes: list[dict[str, Any]],
) -> dict[str, Any]:
    registry = build_registry()
    require_valid("admission_request.schema.json", request, registry, "admission request")

    resolved_contract = contract_from_request(request, contract)
    require_valid("skill_contract.schema.json", resolved_contract, registry, "skill contract")

    runtime_map = {runtime["runtime_guarantee_id"]: runtime for runtime in runtimes}
    global_issues = verify_request_contract_alignment(resolved_contract, request)
    if global_issues:
        return global_refusal_plan(resolved_contract, request, global_issues)

    candidate_items = request["runtime_candidates"]
    if not candidate_items:
        no_candidate = [
            unsatisfied_requirement(
                "runtime_candidate",
                request["request_id"],
                "NO_CANDIDATE_RUNTIME",
                "admission request did not provide any runtime candidates",
            )
        ]
        return global_refusal_plan(resolved_contract, request, no_candidate)

    preferred_runtime_id = request.get("preferred_runtime_guarantee_id")
    evaluations = [
        evaluate_runtime_candidate(
            resolved_contract,
            request,
            candidate,
            runtime_map,
            registry,
            preferred_runtime_id,
        )
        for candidate in candidate_items
    ]

    chosen, preferred_not_admissible = choose_runtime(evaluations, preferred_runtime_id)
    if chosen is None:
        no_admissible_runtime = [
            unsatisfied_requirement(
                "runtime_candidate",
                request["request_id"],
                "NO_ADMISSIBLE_RUNTIME",
                "no supplied runtime candidate satisfied the hard requirements and request-time constraints",
            )
        ]
        reason_codes = stable_unique_strings(
            sorted(
                [item["reason"]["reason_code"] for item in no_admissible_runtime]
                + [code for evaluation in evaluations for code in evaluation["reason_codes"]]
            )
        )
        return {
            "kind": "guild.execution_plan",
            "version": "1.0.0",
            "plan_id": plan_id_for_request(request["request_id"]),
            "request_id": request["request_id"],
            "contract_id": resolved_contract["contract_id"],
            "contract_digest": digest_struct(resolved_contract),
            "component_digest": resolved_contract["component"]["digest"],
            "export_name": request.get("export_name_override") or resolved_contract["export"]["name"],
            "input_class_fingerprint": deepcopy(resolved_contract["input_class_fingerprint"]),
            "decision": "refuse",
            "decision_reason_codes": reason_codes,
            "preferred_runtime_guarantee_id": preferred_runtime_id,
            "chosen_runtime": None,
            "runtime_evaluations": [public_runtime_evaluation(evaluation) for evaluation in evaluations],
            "requested_authority": requested_authority_to_plan(request["request_id"], request, resolved_contract),
            "granted_authority": granted_authority_plan(request["request_id"], resolved_contract, request, []),
            "denied_authority": [],
            "hard_requirement_status": {
                "satisfied": False,
                "unsatisfied_requirements": no_admissible_runtime,
            },
            "request_narrowing_applied": False,
            "proof_prerequisites": [],
            "witness_policy": {
                "contract_witness_level": resolved_contract["witness_level"],
                "requested_witness_level": request.get("requested_witness_level"),
                "effective_witness_level": effective_witness_level(resolved_contract, request),
                "supported_levels": [],
                "acceptable_tamper_evidence_modes": resolved_contract["required_runtime_guarantees"]["witness_support"][
                    "acceptable_tamper_evidence_modes"
                ],
                "acceptable_signature_modes": resolved_contract["required_runtime_guarantees"]["witness_support"][
                    "acceptable_signature_modes"
                ],
                "prerequisites": [],
            },
            "delegation_token_policy_inputs": build_delegation_token_policy_inputs(
                resolved_contract,
                request,
                None,
            ),
            "plan_validity": build_plan_validity(resolved_contract, request),
        }

    chosen["chosen"] = True
    decision = "admit"
    if preferred_not_admissible:
        decision = "migrate"
    elif chosen.get("request_narrowing_applied"):
        decision = "downgrade"

    return {
        "kind": "guild.execution_plan",
        "version": "1.0.0",
        "plan_id": plan_id_for_request(request["request_id"]),
        "request_id": request["request_id"],
        "contract_id": resolved_contract["contract_id"],
        "contract_digest": digest_struct(resolved_contract),
        "component_digest": resolved_contract["component"]["digest"],
        "export_name": request.get("export_name_override") or resolved_contract["export"]["name"],
        "input_class_fingerprint": deepcopy(resolved_contract["input_class_fingerprint"]),
        "decision": decision,
        "decision_reason_codes": aggregate_reason_codes(
            chosen,
            preferred_runtime_not_admissible=preferred_not_admissible,
        ),
        "preferred_runtime_guarantee_id": preferred_runtime_id,
        "chosen_runtime": runtime_overview(runtime_map[chosen["runtime_guarantee_id"]]),
        "runtime_evaluations": [public_runtime_evaluation(evaluation) for evaluation in evaluations],
        "requested_authority": requested_authority_to_plan(request["request_id"], request, resolved_contract),
        "granted_authority": chosen["granted_authority"],
        "denied_authority": chosen.get("denied_authority", []),
        "hard_requirement_status": {
            "satisfied": True,
            "unsatisfied_requirements": [],
        },
        "request_narrowing_applied": bool(chosen.get("denied_authority")),
        "proof_prerequisites": build_proof_prerequisites(resolved_contract, runtime_map[chosen["runtime_guarantee_id"]]),
        "witness_policy": build_witness_policy(
            resolved_contract,
            request,
            runtime_map[chosen["runtime_guarantee_id"]],
        ),
        "delegation_token_policy_inputs": build_delegation_token_policy_inputs(
            resolved_contract,
            request,
            runtime_map[chosen["runtime_guarantee_id"]],
        ),
        "plan_validity": build_plan_validity(resolved_contract, request),
    }
