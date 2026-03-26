use anyhow::{Result, bail};
use serde_json::{Map, Value, json};

use crate::ArtifactMode;
use crate::util::{
    draft_v1_dir, ensure_parent_dir, get_required, get_required_str, json_array, json_object,
    read_json, write_string,
};

const OUTPUT_NAME: &str = "compatibility_matrix.md";

const SKILLS: &[&str] = &[
    "examples/local-log-analyzer.contract.json",
    "examples/zero-authority.contract.json",
    "examples/fetch-transform.contract.json",
    "examples/cluster-rollout.contract.json",
    "examples/runtime-http-read.contract.json",
    "examples/runtime-http-read-default-port.contract.json",
    "examples/runtime-http-localhost.contract.json",
    "examples/runtime-http-localhost-default-port.contract.json",
    "examples/runtime-http-localhost-head.contract.json",
    "examples/runtime-http-localhost-head-default-port.contract.json",
    "examples/runtime-http-head.contract.json",
    "examples/runtime-http-head-default-port.contract.json",
    "examples/runtime-http-redirect.contract.json",
    "examples/runtime-read-resource.contract.json",
    "examples/runtime-invoke-skill.contract.json",
    "examples/runtime-emit-evidence-zero.contract.json",
    "examples/runtime-emit-evidence-exact.contract.json",
    "examples/runtime-log-write.contract.json",
];

const RUNTIMES: &[&str] = &[
    "examples/wasmtime-strict.runtime.json",
    "examples/node-wasi-basic.runtime.json",
];

pub fn run(mode: ArtifactMode) -> Result<()> {
    let rendered = render_matrix()?;
    let output_path = draft_v1_dir().join(OUTPUT_NAME);
    verify_fail_closed_wit_world_probes()?;
    match mode {
        ArtifactMode::Check => {
            let existing = std::fs::read_to_string(&output_path)?;
            if existing != rendered {
                bail!("{OUTPUT_NAME} is out of date with the Rust-native generator");
            }
            println!("{OUTPUT_NAME} validates cleanly.");
        }
        ArtifactMode::Write => {
            ensure_parent_dir(&output_path)?;
            write_string(&output_path, &rendered)?;
            println!("Wrote {}", output_path.display());
        }
    }
    Ok(())
}

pub fn render_matrix() -> Result<String> {
    let mut lines = vec![
        "# Compatibility Matrix".to_owned(),
        String::new(),
        "Deterministic hard-requirement precheck for the bundled examples.".to_owned(),
        String::new(),
        "This matrix is intentionally narrower than full M4 admission. It covers the shared fail-closed hard-requirement path used by the Rust-native truth tooling, not request-time narrowing, runtime migration, or final execution-plan derivation.".to_owned(),
        String::new(),
        "Per-family M8c layer status now lives in `family_support_matrix.json`. This derived file is only the M4 hard-requirement precheck view.".to_owned(),
        String::new(),
        "The precheck enforces component-model compatibility, explicit WIT-world publication, required authority-selector support, required-effect enforceability, and the published runtime guarantee thresholds.".to_owned(),
        String::new(),
        "Bundled runtime examples now publish two different vocabulary surfaces on purpose: `supported_canonical_families` is the live runtime truth surface, while `supported_effect_classes` remains the legacy draft-v1 compatibility surface needed by the older bounded examples.".to_owned(),
        String::new(),
        "This table therefore mixes both surfaces on purpose: the new runtime-* fixtures exercise direct canonical family support, while the older bounded fixtures still prove the explicit compatibility paths that remain in scope after M8c.".to_owned(),
        String::new(),
        "All bundled contracts in this directory now declare the live inspect world `guild-skill-inspect-v1`, so WIT-world checks here stay aligned to the real Rust inspect entrypoint rather than the older example-local names.".to_owned(),
        String::new(),
        "The compatibility precheck only requires a runtime to publish the contract's required world, but the Rust-native truth gate separately keeps the bundled runtime examples pinned to exactly that one active inspect world so they cannot silently widen away from the live inspect contract.".to_owned(),
        String::new(),
        "Published `witness_support` values in this table are M4 hard-requirement inputs only. They do not by themselves imply runtime-general M7 observation completeness.".to_owned(),
        String::new(),
        "Negative fail-closed probes for omitted and unsupported `wit_worlds` declarations are asserted by the Rust-native compatibility flow but omitted from this table because they mutate the base runtime examples.".to_owned(),
        String::new(),
        "| Skill contract | Runtime | Result | Notes |".to_owned(),
        "|---|---|---|---|".to_owned(),
    ];

    for skill_path in SKILLS {
        let skill = read_json(&draft_v1_dir().join(skill_path))?;
        for runtime_path in RUNTIMES {
            let runtime = read_json(&draft_v1_dir().join(runtime_path))?;
            let result = match_hard_requirements(&skill, &runtime)?;
            let notes = if result.unsatisfied_requirements.is_empty() {
                "all hard requirements satisfied".to_owned()
            } else {
                result
                    .unsatisfied_requirements
                    .iter()
                    .map(|item| item.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            lines.push(format!(
                "| `{}` | `{}` | {} | {} |",
                std::path::Path::new(skill_path)
                    .file_name()
                    .expect("skill file name")
                    .to_string_lossy(),
                std::path::Path::new(runtime_path)
                    .file_name()
                    .expect("runtime file name")
                    .to_string_lossy(),
                if result.ok { "PASS" } else { "FAIL" },
                notes
            ));
        }
    }

    lines.push(String::new());
    Ok(lines.join("\n"))
}

#[derive(Debug, Clone)]
struct RequirementFailure {
    reason_code: String,
    message: String,
}

#[derive(Debug, Clone)]
struct HardRequirementResult {
    ok: bool,
    unsatisfied_requirements: Vec<RequirementFailure>,
}

fn match_hard_requirements(contract: &Value, runtime: &Value) -> Result<HardRequirementResult> {
    let failures = requirement_messages(contract, runtime)?;
    Ok(HardRequirementResult {
        ok: failures.is_empty(),
        unsatisfied_requirements: failures,
    })
}

fn requirement_messages(contract: &Value, runtime: &Value) -> Result<Vec<RequirementFailure>> {
    let contract = json_object(contract, "contract")?;
    let runtime = json_object(runtime, "runtime")?;
    let req = json_object(
        get_required(contract, "required_runtime_guarantees", "contract")?,
        "contract.required_runtime_guarantees",
    )?;
    let component = json_object(
        get_required(contract, "component", "contract")?,
        "contract.component",
    )?;
    let mut failures = Vec::new();

    if let Some(component_support_value) = runtime.get("component_model_support") {
        let component_support =
            json_object(component_support_value, "runtime.component_model_support")?;
        let required_component_model =
            get_required_str(component, "component_model", "contract.component")?;
        let published_component_model = get_required_str(
            component_support,
            "component_model",
            "runtime.component_model_support",
        )?;
        if published_component_model != required_component_model {
            failures.push(failure(
                "RUNTIME_COMPONENT_MODEL_UNSUPPORTED",
                "runtime did not publish support for the required component model",
            ));
        }

        let required_version =
            get_required_str(component, "component_model_version", "contract.component")?;
        if !string_array(
            component_support,
            "supported_versions",
            "runtime.component_model_support",
        )?
        .iter()
        .any(|item| item == required_version)
        {
            failures.push(failure(
                "RUNTIME_COMPONENT_MODEL_VERSION_UNSUPPORTED",
                "runtime did not publish support for the required component model version",
            ));
        }

        let required_world = get_required_str(component, "wit_world", "contract.component")?;
        match component_support.get("wit_worlds") {
            None => failures.push(failure(
                "RUNTIME_WIT_WORLD_UNDECLARED",
                "runtime must enumerate component_model_support.wit_worlds explicitly",
            )),
            Some(worlds) => {
                let worlds = json_array(worlds, "runtime.component_model_support.wit_worlds")?
                    .iter()
                    .map(|value| value.as_str().unwrap_or_default().to_owned())
                    .collect::<Vec<_>>();
                if !worlds.iter().any(|item| item == required_world) {
                    failures.push(failure(
                        "RUNTIME_WIT_WORLD_UNSUPPORTED",
                        "runtime did not publish the required WIT world",
                    ));
                }
            }
        }
    } else {
        failures.push(failure(
            "RUNTIME_COMPONENT_MODEL_UNSUPPORTED",
            "runtime guarantee omitted component model support details",
        ));
    }

    for effect in json_array(
        get_required(contract, "required_effects", "contract")?,
        "contract.required_effects",
    )? {
        if !runtime_supports_effect(effect, runtime)? {
            failures.push(failure(
                "REQUIRED_EFFECT_UNSUPPORTED",
                "runtime did not publish support for a required effect class",
            ));
            continue;
        }
        if !runtime_can_enforce_effect(effect, runtime)? {
            failures.push(failure(
                "REQUIRED_SCOPE_NOT_ENFORCEABLE",
                "runtime cannot enforce the scope constraints on a required effect",
            ));
        }
    }

    ordered_check(
        &mut failures,
        req,
        runtime,
        "execution_isolation_assurance",
        &["none", "best_effort", "strong"],
        "RUNTIME_EXECUTION_ISOLATION_TOO_WEAK",
        "runtime execution isolation assurance was weaker than required",
    )?;
    ordered_check(
        &mut failures,
        req,
        runtime,
        "filesystem_isolation_class",
        &[
            "none",
            "path_filter",
            "preopen_only",
            "virtual_fs",
            "os_sandbox",
        ],
        "RUNTIME_FILESYSTEM_ISOLATION_TOO_WEAK",
        "runtime filesystem isolation class was weaker than required",
    )?;
    ordered_check(
        &mut failures,
        req,
        runtime,
        "network_policy_granularity",
        &["none", "binary", "domain", "host_port", "url"],
        "RUNTIME_NETWORK_GRANULARITY_TOO_WEAK",
        "runtime network policy granularity was weaker than required",
    )?;

    mode_check(
        &mut failures,
        req,
        runtime,
        "child_process_policy",
        "RUNTIME_CHILD_PROCESS_MODE_UNSUPPORTED",
    )?;
    mode_check(
        &mut failures,
        req,
        runtime,
        "token_passthrough_policy",
        "RUNTIME_TOKEN_PASSTHROUGH_MODE_UNSUPPORTED",
    )?;
    mode_check(
        &mut failures,
        req,
        runtime,
        "revocation_behavior",
        "RUNTIME_REVOCATION_MODE_UNSUPPORTED",
    )?;

    let enforcement_required = json_object(
        get_required(req, "delegation_enforcement", "required_runtime_guarantees")?,
        "required_runtime_guarantees.delegation_enforcement",
    )?;
    let enforcement_published = json_object(
        get_required(runtime, "delegation_enforcement", "runtime")?,
        "runtime.delegation_enforcement",
    )?;
    bool_check(
        &mut failures,
        enforcement_required,
        enforcement_published,
        "audience_binding_required",
        "audience_binding",
        "RUNTIME_AUDIENCE_BINDING_UNSUPPORTED",
        "runtime did not publish a required delegation enforcement guarantee",
    )?;
    bool_check(
        &mut failures,
        enforcement_required,
        enforcement_published,
        "call_chain_binding_required",
        "call_chain_binding",
        "RUNTIME_CALL_CHAIN_BINDING_UNSUPPORTED",
        "runtime did not publish a required delegation enforcement guarantee",
    )?;
    bool_check(
        &mut failures,
        enforcement_required,
        enforcement_published,
        "anti_replay_required",
        "anti_replay",
        "RUNTIME_ANTI_REPLAY_UNSUPPORTED",
        "runtime did not publish a required delegation enforcement guarantee",
    )?;
    bool_check(
        &mut failures,
        enforcement_required,
        enforcement_published,
        "max_hops_enforced_required",
        "max_hops_enforced",
        "RUNTIME_MAX_HOPS_ENFORCEMENT_UNSUPPORTED",
        "runtime did not publish a required delegation enforcement guarantee",
    )?;

    let witness_required = json_object(
        get_required(req, "witness_support", "required_runtime_guarantees")?,
        "required_runtime_guarantees.witness_support",
    )?;
    let witness_published = json_object(
        get_required(runtime, "witness_support", "runtime")?,
        "runtime.witness_support",
    )?;
    let minimum_level = get_required_str(
        witness_required,
        "minimum_level",
        "required_runtime_guarantees.witness_support",
    )?;
    let supported_levels = string_array(
        witness_published,
        "supported_levels",
        "runtime.witness_support",
    )?;
    let level_ok = supported_levels.iter().any(|level| {
        rank(level, &["summary", "decision", "hostcall", "full"])
            >= rank(minimum_level, &["summary", "decision", "hostcall", "full"])
    });
    if !level_ok {
        failures.push(failure(
            "RUNTIME_WITNESS_LEVEL_UNSUPPORTED",
            "runtime witness support was weaker than required",
        ));
    }

    if !has_intersection(
        string_array(
            witness_required,
            "acceptable_tamper_evidence_modes",
            "required_runtime_guarantees.witness_support",
        )?,
        string_array(
            witness_published,
            "tamper_evidence_modes",
            "runtime.witness_support",
        )?,
    ) {
        failures.push(failure(
            "RUNTIME_TAMPER_EVIDENCE_MODE_UNSUPPORTED",
            "runtime did not publish an acceptable tamper-evidence mode",
        ));
    }

    if !has_intersection(
        string_array(
            witness_required,
            "acceptable_signature_modes",
            "required_runtime_guarantees.witness_support",
        )?,
        string_array(
            witness_published,
            "signature_modes",
            "runtime.witness_support",
        )?,
    ) {
        failures.push(failure(
            "RUNTIME_SIGNATURE_MODE_UNSUPPORTED",
            "runtime did not publish an acceptable witness signature mode",
        ));
    }

    bool_check(
        &mut failures,
        witness_required,
        witness_published,
        "trusted_time_source_required",
        "trusted_time_source",
        "RUNTIME_TRUSTED_TIME_SOURCE_UNSUPPORTED",
        "runtime did not publish a required witness capability",
    )?;
    bool_check(
        &mut failures,
        witness_required,
        witness_published,
        "redacted_io_hashes_required",
        "redacted_io_hashes",
        "RUNTIME_REDACTED_IO_HASHES_UNSUPPORTED",
        "runtime did not publish a required witness capability",
    )?;
    bool_check(
        &mut failures,
        witness_required,
        witness_published,
        "authority_plan_digest_required",
        "authority_plan_digest",
        "RUNTIME_AUTHORITY_PLAN_DIGEST_UNSUPPORTED",
        "runtime did not publish a required witness capability",
    )?;

    Ok(failures)
}

fn failure(reason_code: &str, message: &str) -> RequirementFailure {
    RequirementFailure {
        reason_code: reason_code.to_owned(),
        message: message.to_owned(),
    }
}

fn ordered_check(
    failures: &mut Vec<RequirementFailure>,
    required: &Map<String, Value>,
    published: &Map<String, Value>,
    field: &str,
    order: &[&str],
    reason_code: &str,
    message: &str,
) -> Result<()> {
    let required_minimum = json_object(
        get_required(required, field, "required_runtime_guarantees")?,
        field,
    )?;
    let required_value = get_required_str(required_minimum, "minimum", field)?;
    let published_value = get_required_str(published, field, "runtime")?;
    if rank(published_value, order) < rank(required_value, order) {
        failures.push(failure(reason_code, message));
    }
    Ok(())
}

fn mode_check(
    failures: &mut Vec<RequirementFailure>,
    required: &Map<String, Value>,
    published: &Map<String, Value>,
    field: &str,
    reason_code: &str,
) -> Result<()> {
    let required_section = json_object(
        get_required(required, field, "required_runtime_guarantees")?,
        field,
    )?;
    let required_mode = get_required_str(required_section, "required_mode", field)?;
    let published_section = json_object(get_required(published, field, "runtime")?, field)?;
    if !string_array(published_section, "supported_modes", field)?
        .iter()
        .any(|item| item == required_mode)
    {
        failures.push(failure(
            reason_code,
            "runtime did not publish the required policy mode",
        ));
    }
    Ok(())
}

fn bool_check(
    failures: &mut Vec<RequirementFailure>,
    required: &Map<String, Value>,
    published: &Map<String, Value>,
    required_key: &str,
    published_key: &str,
    reason_code: &str,
    message: &str,
) -> Result<()> {
    let required_value = required
        .get(required_key)
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let published_value = published
        .get(published_key)
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if required_value && !published_value {
        failures.push(failure(reason_code, message));
    }
    Ok(())
}

fn rank(value: &str, order: &[&str]) -> usize {
    order
        .iter()
        .position(|candidate| *candidate == value)
        .unwrap_or(0)
}

fn string_array(object: &Map<String, Value>, key: &str, context: &str) -> Result<Vec<String>> {
    Ok(json_array(get_required(object, key, context)?, context)?
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_owned())
        .collect())
}

fn effect_selector(effect: &Value) -> Result<String> {
    let effect = json_object(effect, "effect")?;
    if let Some(family) = effect.get("family").and_then(Value::as_str) {
        return Ok(family.to_owned());
    }
    Ok(get_required_str(effect, "effect_class", "effect")?.to_owned())
}

fn effect_is_canonical(effect: &Value) -> Result<bool> {
    Ok(json_object(effect, "effect")?.contains_key("family"))
}

fn runtime_supports_effect(effect: &Value, runtime: &Map<String, Value>) -> Result<bool> {
    let selector = effect_selector(effect)?;
    if effect_is_canonical(effect)? {
        Ok(
            string_array(runtime, "supported_canonical_families", "runtime")?
                .iter()
                .any(|item| item == &selector),
        )
    } else {
        if let Some(value) = runtime.get("supported_effect_classes") {
            Ok(json_array(value, "runtime.supported_effect_classes")?
                .iter()
                .any(|item| item.as_str() == Some(selector.as_str())))
        } else {
            Ok(false)
        }
    }
}

fn runtime_can_enforce_effect(effect: &Value, runtime: &Map<String, Value>) -> Result<bool> {
    if !runtime_supports_effect(effect, runtime)? {
        return Ok(false);
    }
    let selector = effect_selector(effect)?;
    match selector.as_str() {
        "net.connect" | "net.resolve" => {
            let granularity = get_required_str(runtime, "network_policy_granularity", "runtime")?;
            let effect = json_object(effect, "effect")?;
            let scope = json_object(get_required(effect, "scope", "effect")?, "effect.scope")?;
            let audiences = json_array(
                get_required(scope, "audiences", "effect.scope")?,
                "effect.scope.audiences",
            )?;
            Ok(runtime_can_enforce_network_scope(audiences, granularity))
        }
        "fs.read" | "fs.write" | "fs.list" => {
            Ok(get_required_str(runtime, "filesystem_isolation_class", "runtime")? != "none")
        }
        "http-request" => {
            Ok(get_required_str(runtime, "network_policy_granularity", "runtime")? == "url")
        }
        _ => Ok(true),
    }
}

fn runtime_can_enforce_network_scope(audiences: &[Value], granularity: &str) -> bool {
    match granularity {
        "url" => true,
        "host_port" => !audiences.iter().any(|audience| {
            let audience = audience.as_object().expect("audience object");
            uses_any(audience.get("schemes"))
                || uses_any(audience.get("path_prefixes"))
                || uses_any(audience.get("methods"))
        }),
        "domain" => !audiences.iter().any(|audience| {
            let audience = audience.as_object().expect("audience object");
            audience.get("host").and_then(Value::as_str) == Some("*")
                || uses_any(audience.get("ports"))
                || uses_any(audience.get("schemes"))
                || uses_any(audience.get("path_prefixes"))
                || uses_any(audience.get("methods"))
        }),
        "binary" => audiences.iter().all(|audience| {
            let audience = audience.as_object().expect("audience object");
            audience.get("host").and_then(Value::as_str) == Some("*")
                && !uses_any(audience.get("ports"))
                && !uses_any(audience.get("schemes"))
                && !uses_any(audience.get("path_prefixes"))
                && !uses_any(audience.get("methods"))
        }),
        _ => false,
    }
}

fn uses_any(values: Option<&Value>) -> bool {
    values
        .and_then(Value::as_array)
        .is_some_and(|items| !items.iter().any(|item| item.as_str() == Some("*")))
}

fn has_intersection(left: Vec<String>, right: Vec<String>) -> bool {
    left.iter().any(|value| right.contains(value))
}

fn verify_fail_closed_wit_world_probes() -> Result<()> {
    let skill = read_json(&draft_v1_dir().join("examples/local-log-analyzer.contract.json"))?;
    let runtime = read_json(&draft_v1_dir().join("examples/wasmtime-strict.runtime.json"))?;
    let mut omitted = runtime.clone();
    omitted
        .get_mut("component_model_support")
        .and_then(Value::as_object_mut)
        .expect("component_model_support object")
        .remove("wit_worlds");
    assert_reason(
        &match_hard_requirements(&skill, &omitted)?,
        "RUNTIME_WIT_WORLD_UNDECLARED",
    )?;

    let mut empty = runtime.clone();
    empty
        .get_mut("component_model_support")
        .and_then(Value::as_object_mut)
        .expect("component_model_support object")
        .insert("wit_worlds".to_owned(), json!([]));
    assert_reason(
        &match_hard_requirements(&skill, &empty)?,
        "RUNTIME_WIT_WORLD_UNSUPPORTED",
    )?;

    let mut unsupported = runtime;
    unsupported
        .get_mut("component_model_support")
        .and_then(Value::as_object_mut)
        .expect("component_model_support object")
        .insert("wit_worlds".to_owned(), json!(["different-world"]));
    assert_reason(
        &match_hard_requirements(&skill, &unsupported)?,
        "RUNTIME_WIT_WORLD_UNSUPPORTED",
    )?;
    Ok(())
}

fn assert_reason(result: &HardRequirementResult, expected: &str) -> Result<()> {
    if result.ok {
        bail!("expected fail-closed reason {expected}, but probe unexpectedly passed");
    }
    if !result
        .unsatisfied_requirements
        .iter()
        .any(|item| item.reason_code == expected)
    {
        bail!(
            "expected reason code {expected}, got {:?}",
            result.unsatisfied_requirements
        );
    }
    Ok(())
}
