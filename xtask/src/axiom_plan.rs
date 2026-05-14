use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use jsonschema::Validator;
use jsonschema::error::ValidationErrorKind;
use jsonschema::paths::Location;
use serde_json::Value;

const AXIOM_ROOT: &str = "docs/strategy/axiom-plan-ir";
const AXIOM_SCHEMA: &str = "docs/strategy/axiom-plan-ir/schema/axiom-plan-ir.schema.json";
const AXIOM_GOLDENS_ROOT: &str = "docs/strategy/axiom-plan-ir/goldens";
const USAGE: &str = "usage: cargo run -q -p xtask -- axiom-plan validate <path>\n       cargo run -q -p xtask -- axiom-plan validate-examples\n       cargo run -q -p xtask -- axiom-plan preview <path> [--json]\n       cargo run -q -p xtask -- axiom-plan check-goldens [--update]";
const PREVIEW_KIND: &str = "axiom.plan_preview";
const PREVIEW_STATUS: &str = "preview_only";
static AXIOM_SCHEMA_VALIDATOR: OnceLock<std::result::Result<Validator, String>> = OnceLock::new();
const FORBIDDEN_FIELDS: &[&str] = &[
    "executionId",
    "receipt",
    "grantedAuthority",
    "effectiveAuthority",
    "hostDecision",
    "runtimeStatus",
    "evidenceProduced",
    "grants",
    "grantedGrants",
    "effectiveGrants",
];

#[derive(Debug, Clone, Copy)]
enum GoldenOutput {
    HumanPreview,
    JsonPreview,
    Diagnostics,
}

#[derive(Debug, Clone, Copy)]
struct GoldenCase {
    source_path: &'static str,
    golden_path: &'static str,
    output: GoldenOutput,
}

const GOLDEN_CASES: &[GoldenCase] = &[
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/valid/basic-two-node-plan.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/preview/basic-two-node.txt",
        output: GoldenOutput::HumanPreview,
    },
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/valid/basic-two-node-plan.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/preview/basic-two-node.json",
        output: GoldenOutput::JsonPreview,
    },
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/valid/with-requested-grants.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/preview/with-requested-grants.txt",
        output: GoldenOutput::HumanPreview,
    },
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/valid/with-requested-grants.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/preview/with-requested-grants.json",
        output: GoldenOutput::JsonPreview,
    },
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/invalid/malformed-skill-ref.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/diagnostics/malformed-skill-ref.json",
        output: GoldenOutput::Diagnostics,
    },
    GoldenCase {
        source_path: "docs/strategy/axiom-plan-ir/examples/invalid/granted-authority-claim.json",
        golden_path: "docs/strategy/axiom-plan-ir/goldens/diagnostics/granted-authority-claim.json",
        output: GoldenOutput::Diagnostics,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Diagnostic {
    code: &'static str,
    severity: &'static str,
    path: String,
    message: String,
}

impl Diagnostic {
    fn error(path: impl Into<String>, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: "error",
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct NodeInfo {
    index: usize,
    id: String,
    depends_on: Vec<String>,
}

#[derive(Debug, Clone)]
enum PlanInput {
    Valid(Value),
    Invalid(Vec<Diagnostic>),
}

#[derive(Debug, Clone)]
struct PlanPreview {
    source_path: String,
    plan_kind: String,
    version: String,
    name: String,
    original_node_count: usize,
    nodes: Vec<NodePreview>,
    plan_trace: Vec<PlanTraceEntry>,
    limitations: Vec<String>,
}

#[derive(Debug, Clone)]
struct NodePreview {
    ordinal: usize,
    document_index: usize,
    id: String,
    skill: SkillPreview,
    depends_on: Vec<String>,
    args_as_declared: Option<Value>,
    requested_grants: Vec<Value>,
    expected_outputs: Vec<Value>,
    expected_evidence: Vec<Value>,
    failure_behavior: Option<String>,
    request_preview: RequestPreview,
    preview_trace: Vec<String>,
}

#[derive(Debug, Clone)]
struct SkillPreview {
    requested: String,
    digest: Option<String>,
    resolved_metadata: Option<Value>,
    object_form: bool,
}

#[derive(Debug, Clone)]
struct RequestPreview {
    status: &'static str,
    summary: String,
    args: &'static str,
    authority: &'static str,
    evidence: &'static str,
}

#[derive(Debug, Clone)]
struct PlanTraceEntry {
    node: String,
    preview_trace: Vec<String>,
}

pub fn run(mut args: impl Iterator<Item = String>) -> Result<()> {
    let Some(command) = args.next() else {
        bail!("{USAGE}");
    };

    match command.as_str() {
        "validate" => {
            let Some(path) = args.next() else {
                bail!("usage: cargo run -q -p xtask -- axiom-plan validate <path>");
            };
            if args.next().is_some() {
                bail!("unexpected extra arguments");
            }
            let diagnostics = validate_path(Path::new(&path))?;
            print_plan_result(&path, &diagnostics);
            if diagnostics.is_empty() {
                Ok(())
            } else {
                bail!("axiom plan invalid: {} diagnostic(s)", diagnostics.len());
            }
        }
        "preview" => {
            let Some(path) = args.next() else {
                bail!("usage: cargo run -q -p xtask -- axiom-plan preview <path> [--json]");
            };
            let mut json_output = false;
            for arg in args {
                if arg == "--json" {
                    if json_output {
                        bail!("duplicate --json argument");
                    }
                    json_output = true;
                } else {
                    bail!("unexpected argument `{arg}`");
                }
            }
            preview_path(Path::new(&path), &path, json_output)
        }
        "validate-examples" => {
            if args.next().is_some() {
                bail!("unexpected extra arguments");
            }
            validate_examples()
        }
        "check-goldens" => {
            let mut update = false;
            for arg in args {
                if arg == "--update" {
                    if update {
                        bail!("duplicate --update argument");
                    }
                    update = true;
                } else {
                    bail!("unexpected argument `{arg}`");
                }
            }
            check_goldens(update)
        }
        other => bail!("unknown axiom-plan command `{other}`"),
    }
}

fn validate_examples() -> Result<()> {
    let root = repo_root().join(AXIOM_ROOT);
    let valid_paths = example_paths(&root.join("examples/valid"))?;
    let invalid_paths = example_paths(&root.join("examples/invalid"))?;
    if valid_paths.is_empty() {
        bail!("no valid Axiom Plan IR examples found");
    }
    if invalid_paths.is_empty() {
        bail!("no invalid Axiom Plan IR examples found");
    }

    let mut failures = Vec::new();
    for path in &valid_paths {
        let diagnostics = validate_path(path)?;
        if diagnostics.is_empty() {
            println!("PASS valid {}", display_path(path));
        } else {
            println!("FAIL valid {}", display_path(path));
            print_diagnostics(&diagnostics);
            failures.push(format!(
                "{} should be valid but had {} diagnostic(s)",
                display_path(path),
                diagnostics.len()
            ));
        }
    }

    for path in &invalid_paths {
        let diagnostics = validate_path(path)?;
        if diagnostics.is_empty() {
            println!("FAIL invalid {} unexpectedly passed", display_path(path));
            failures.push(format!(
                "{} should be invalid but passed",
                display_path(path)
            ));
        } else {
            let codes = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>()
                .join(", ");
            if let Some(expected_codes) = expected_invalid_codes(path) {
                let missing_codes = expected_codes
                    .iter()
                    .copied()
                    .filter(|expected| {
                        !diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.code == *expected)
                    })
                    .collect::<Vec<_>>();
                if missing_codes.is_empty() {
                    println!("PASS invalid {} ({codes})", display_path(path));
                } else {
                    println!(
                        "FAIL invalid {} missing expected diagnostic code(s): {} ({codes})",
                        display_path(path),
                        missing_codes.join(", ")
                    );
                    failures.push(format!(
                        "{} should include expected diagnostic code(s): {}",
                        display_path(path),
                        missing_codes.join(", ")
                    ));
                }
            } else {
                println!(
                    "FAIL invalid {} has no expected diagnostic-code entry ({codes})",
                    display_path(path)
                );
                failures.push(format!(
                    "{} has no expected diagnostic-code entry",
                    display_path(path)
                ));
            }
        }
    }

    if failures.is_empty() {
        println!(
            "Axiom Plan IR example validation completed: {} valid passed, {} invalid failed as expected.",
            valid_paths.len(),
            invalid_paths.len()
        );
        Ok(())
    } else {
        bail!(
            "Axiom Plan IR example validation failed:\n - {}",
            failures.join("\n - ")
        );
    }
}

fn check_goldens(update: bool) -> Result<()> {
    let mut failures = Vec::new();
    for case in GOLDEN_CASES {
        let actual = render_golden_case(*case)?;
        let golden_path = repo_root().join(case.golden_path);
        if update {
            write_golden_file(&golden_path, &actual)?;
            println!("UPDATED {}", case.golden_path);
            continue;
        }

        let expected = fs::read_to_string(&golden_path)
            .with_context(|| format!("failed to read {}", golden_path.display()))?;
        if let Some(mismatch) = golden_mismatch(case.golden_path, &expected, &actual) {
            println!("FAIL {}", case.golden_path);
            failures.push(mismatch);
        } else {
            println!("PASS {}", case.golden_path);
        }
    }

    if failures.is_empty() {
        let action = if update { "updated" } else { "passed" };
        println!(
            "Axiom Plan IR golden check completed: {} {action}.",
            GOLDEN_CASES.len()
        );
        Ok(())
    } else {
        bail!(
            "Axiom Plan IR golden check failed:\n - {}",
            failures.join("\n - ")
        );
    }
}

fn render_golden_case(case: GoldenCase) -> Result<String> {
    let source_path = repo_root().join(case.source_path);
    match case.output {
        GoldenOutput::HumanPreview => {
            let preview = preview_for_golden(&source_path)?;
            render_plan_preview(&preview).map(|rendered| normalize_golden_text(&rendered))
        }
        GoldenOutput::JsonPreview => {
            let preview = preview_for_golden(&source_path)?;
            json_golden(&preview_to_json(&preview))
        }
        GoldenOutput::Diagnostics => diagnostics_golden(&source_path),
    }
}

fn preview_for_golden(path: &Path) -> Result<PlanPreview> {
    match read_validated_plan(path)? {
        PlanInput::Valid(value) => Ok(build_plan_preview(&value, &display_path(path))),
        PlanInput::Invalid(diagnostics) => bail!(
            "preview golden source {} is invalid: {} diagnostic(s)",
            display_path(path),
            diagnostics.len()
        ),
    }
}

fn diagnostics_golden(path: &Path) -> Result<String> {
    let mut diagnostics = match read_validated_plan(path)? {
        PlanInput::Valid(_) => bail!(
            "diagnostic golden source {} unexpectedly validated",
            display_path(path)
        ),
        PlanInput::Invalid(diagnostics) => diagnostics,
    };
    diagnostics.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.severity.cmp(right.severity))
            .then_with(|| left.message.cmp(&right.message))
    });

    json_golden(&serde_json::json!({
        "sourcePath": display_path(path),
        "diagnostics": diagnostics
            .iter()
            .map(|diagnostic| {
                serde_json::json!({
                    "code": diagnostic.code,
                    "severity": diagnostic.severity,
                    "path": diagnostic.path,
                    "message": stable_diagnostic_message(&diagnostic.message)
                })
            })
            .collect::<Vec<_>>()
    }))
}

fn stable_diagnostic_message(message: &str) -> String {
    let mut rendered = String::new();
    for char in message.chars() {
        if char.is_ascii() {
            rendered.push(char);
        } else {
            use std::fmt::Write as _;
            write!(&mut rendered, "\\u{{{:x}}}", u32::from(char))
                .expect("writing to String should not fail");
        }
    }
    rendered
}

fn json_golden(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map(|rendered| normalize_golden_text(&rendered))
        .map_err(Into::into)
}

fn normalize_golden_text(text: &str) -> String {
    let mut normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

fn golden_mismatch(golden_path: &str, expected: &str, actual: &str) -> Option<String> {
    if normalize_golden_text(expected) == actual {
        None
    } else {
        Some(format!(
            "{golden_path} does not match generated output; rerun `cargo run -q -p xtask -- axiom-plan check-goldens --update`"
        ))
    }
}

fn write_golden_file(path: &Path, content: &str) -> Result<()> {
    let goldens_root = repo_root().join(AXIOM_GOLDENS_ROOT);
    if !path.starts_with(&goldens_root) {
        bail!(
            "refusing to update golden outside {}: {}",
            AXIOM_GOLDENS_ROOT,
            path.display()
        );
    }
    let parent = path
        .parent()
        .with_context(|| format!("golden path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    fs::write(path, content).with_context(|| format!("failed to write {}", path.display()))
}

fn expected_invalid_codes(path: &Path) -> Option<&'static [&'static str]> {
    let file_name = path.file_name()?.to_str()?;
    match file_name {
        "bad-reference.json" => Some(&["axiom.unknown_reference", "axiom.unsupported_reference"]),
        "cycle.json" => Some(&["axiom.dependency_cycle"]),
        "duplicate-node-id.json" => Some(&["axiom.duplicate_node_id"]),
        "granted-authority-claim.json" => Some(&["axiom.forbidden_runtime_truth_field"]),
        "malformed-skill-ref.json" => Some(&["axiom.malformed_skill_ref"]),
        "missing-required-field.json" => Some(&["axiom.schema.missing_required_field"]),
        "unknown-dependency.json" => Some(&["axiom.unknown_dependency"]),
        _ => None,
    }
}

fn example_paths(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("failed to list {}", dir.display()))?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    Ok(paths)
}

fn validate_path(path: &Path) -> Result<Vec<Diagnostic>> {
    match read_validated_plan(path)? {
        PlanInput::Valid(_) => Ok(Vec::new()),
        PlanInput::Invalid(diagnostics) => Ok(diagnostics),
    }
}

fn preview_path(path: &Path, input_path: &str, json_output: bool) -> Result<()> {
    let preview = match read_validated_plan(path)? {
        PlanInput::Valid(value) => build_plan_preview(&value, &display_path(path)),
        PlanInput::Invalid(diagnostics) => {
            print_plan_result(input_path, &diagnostics);
            bail!("axiom plan invalid: {} diagnostic(s)", diagnostics.len());
        }
    };

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&preview_to_json(&preview))?
        );
    } else {
        print!("{}", render_plan_preview(&preview)?);
    }
    Ok(())
}

fn read_validated_plan(path: &Path) -> Result<PlanInput> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return Ok(PlanInput::Invalid(vec![Diagnostic::error(
                "/",
                "axiom.parse_error",
                format!("Failed to parse JSON: {error}"),
            )]));
        }
    };
    let diagnostics = validate_value(&value)?;
    if diagnostics.is_empty() {
        Ok(PlanInput::Valid(value))
    } else {
        Ok(PlanInput::Invalid(diagnostics))
    }
}

fn validate_value(value: &Value) -> Result<Vec<Diagnostic>> {
    let mut diagnostics = validate_schema(value)?;
    diagnostics.extend(validate_semantics(value));
    Ok(diagnostics)
}

fn validate_schema(value: &Value) -> Result<Vec<Diagnostic>> {
    let validator = axiom_schema_validator()?;
    Ok(validator
        .iter_errors(value)
        .map(|error| schema_diagnostic(&error))
        .collect())
}

fn axiom_schema_validator() -> Result<&'static Validator> {
    let validator = AXIOM_SCHEMA_VALIDATOR.get_or_init(|| {
        let schema_path = repo_root().join(AXIOM_SCHEMA);
        let text = fs::read_to_string(&schema_path)
            .with_context(|| format!("failed to read {}", schema_path.display()))
            .map_err(|error| format!("{error:#}"))?;
        let schema = serde_json::from_str::<Value>(&text)
            .with_context(|| format!("failed to parse {}", schema_path.display()))
            .map_err(|error| format!("{error:#}"))?;
        jsonschema::draft202012::options()
            .build(&schema)
            .with_context(|| format!("failed to compile {}", schema_path.display()))
            .map_err(|error| format!("{error:#}"))
    });
    match validator {
        Ok(validator) => Ok(validator),
        Err(message) => bail!("{message}"),
    }
}

fn schema_diagnostic(error: &jsonschema::ValidationError<'_>) -> Diagnostic {
    let code = match error.kind() {
        ValidationErrorKind::Required { .. } => "axiom.schema.missing_required_field",
        ValidationErrorKind::AdditionalProperties { .. } => "axiom.schema.additional_property",
        _ => "axiom.schema.invalid_shape",
    };
    let path = schema_error_path(error);
    let keyword = error.kind().keyword();
    Diagnostic::error(
        path,
        code,
        format!("Schema validation failed ({keyword}): {error}"),
    )
}

fn schema_error_path(error: &jsonschema::ValidationError<'_>) -> String {
    let base_path = location_to_json_pointer(error.instance_path());
    match error.kind() {
        ValidationErrorKind::Required { property } => {
            property.as_str().map_or(base_path.clone(), |property| {
                join_json_pointer(&base_path, property)
            })
        }
        ValidationErrorKind::AdditionalProperties { unexpected } if unexpected.len() == 1 => {
            join_json_pointer(&base_path, &unexpected[0])
        }
        _ => base_path,
    }
}

fn location_to_json_pointer(location: &Location) -> String {
    let rendered = location.to_string();
    if rendered.is_empty() {
        "/".to_owned()
    } else {
        rendered
    }
}

fn validate_semantics(value: &Value) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(plan) = value.as_object() else {
        diagnostics.push(Diagnostic::error(
            "/",
            "axiom.plan_type",
            "Axiom plan must be a JSON object",
        ));
        return diagnostics;
    };

    check_forbidden_runtime_truth_fields(plan, &mut diagnostics);
    check_top_level(plan, &mut diagnostics);

    let Some(nodes) = plan.get("nodes").and_then(Value::as_array) else {
        return diagnostics;
    };
    if nodes.is_empty() {
        return diagnostics;
    }

    let (node_infos, id_to_index) = collect_nodes(nodes, &mut diagnostics);
    check_dependencies(&node_infos, &id_to_index, &mut diagnostics);
    check_cycles(&node_infos, &mut diagnostics);
    check_node_references(nodes, &node_infos, &id_to_index, &mut diagnostics);

    diagnostics
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest dir has a repository root parent")
}

fn display_path(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn check_top_level(plan: &serde_json::Map<String, Value>, diagnostics: &mut Vec<Diagnostic>) {
    match plan.get("kind") {
        Some(Value::String(kind)) if kind == "axiom.plan" => {}
        Some(_) => diagnostics.push(Diagnostic::error(
            "/kind",
            "axiom.invalid_kind",
            "kind must be \"axiom.plan\"",
        )),
        None => diagnostics.push(Diagnostic::error(
            "/kind",
            "axiom.missing_kind",
            "kind is required",
        )),
    }

    match plan.get("version") {
        Some(Value::String(version)) if version == "1" => {}
        Some(_) => diagnostics.push(Diagnostic::error(
            "/version",
            "axiom.invalid_version",
            "version must be the string \"1\"",
        )),
        None => diagnostics.push(Diagnostic::error(
            "/version",
            "axiom.missing_version",
            "version is required",
        )),
    }

    match plan.get("name") {
        Some(Value::String(name)) if !name.trim().is_empty() => {}
        Some(_) => diagnostics.push(Diagnostic::error(
            "/name",
            "axiom.invalid_name",
            "name must be a non-empty string",
        )),
        None => diagnostics.push(Diagnostic::error(
            "/name",
            "axiom.missing_name",
            "name is required",
        )),
    }

    match plan.get("nodes") {
        Some(Value::Array(nodes)) if !nodes.is_empty() => {}
        Some(Value::Array(_)) => diagnostics.push(Diagnostic::error(
            "/nodes",
            "axiom.empty_nodes",
            "nodes must contain at least one node",
        )),
        Some(_) => diagnostics.push(Diagnostic::error(
            "/nodes",
            "axiom.invalid_nodes",
            "nodes must be an array",
        )),
        None => diagnostics.push(Diagnostic::error(
            "/nodes",
            "axiom.missing_nodes",
            "nodes is required",
        )),
    }
}

fn collect_nodes(
    nodes: &[Value],
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<NodeInfo>, HashMap<String, usize>) {
    let mut node_infos = Vec::new();
    let mut id_to_index = HashMap::new();

    for (index, node) in nodes.iter().enumerate() {
        let path = format!("/nodes/{index}");
        let Some(node_object) = node.as_object() else {
            diagnostics.push(Diagnostic::error(
                path,
                "axiom.node_type",
                "node must be an object",
            ));
            continue;
        };

        let Some(id) = check_node_id(index, node_object, diagnostics) else {
            check_skill(index, node_object, diagnostics);
            check_requested_grants(index, node_object, diagnostics);
            continue;
        };

        if let Some(first_index) = id_to_index.insert(id.clone(), index) {
            diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/id"),
                "axiom.duplicate_node_id",
                format!("Duplicate node id: {id} (first seen at /nodes/{first_index}/id)"),
            ));
        }

        let depends_on = check_depends_on(index, node_object, diagnostics);
        check_skill(index, node_object, diagnostics);
        check_requested_grants(index, node_object, diagnostics);
        node_infos.push(NodeInfo {
            index,
            id,
            depends_on,
        });
    }

    (node_infos, id_to_index)
}

fn check_forbidden_runtime_truth_fields(
    plan: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_forbidden_fields(plan, "", diagnostics);

    let Some(nodes) = plan.get("nodes").and_then(Value::as_array) else {
        return;
    };
    for (node_index, node) in nodes.iter().enumerate() {
        let Some(node) = node.as_object() else {
            continue;
        };
        let node_path = format!("/nodes/{node_index}");
        check_forbidden_fields(node, &node_path, diagnostics);

        if let Some(skill) = node.get("skill").and_then(Value::as_object) {
            check_forbidden_fields(skill, &format!("{node_path}/skill"), diagnostics);
        }
        check_forbidden_object_array(
            node.get("requestedGrants"),
            &format!("{node_path}/requestedGrants"),
            diagnostics,
        );
        check_forbidden_object_array(
            node.get("expectedOutputs"),
            &format!("{node_path}/expectedOutputs"),
            diagnostics,
        );
        check_forbidden_object_array(
            node.get("expectedEvidence"),
            &format!("{node_path}/expectedEvidence"),
            diagnostics,
        );
        if let Some(failure_behavior) = node.get("failureBehavior").and_then(Value::as_object) {
            check_forbidden_fields(
                failure_behavior,
                &format!("{node_path}/failureBehavior"),
                diagnostics,
            );
        }
    }
}

fn check_forbidden_object_array(
    value: Option<&Value>,
    base_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(items) = value.and_then(Value::as_array) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        if let Some(object) = item.as_object() {
            check_forbidden_fields(object, &format!("{base_path}/{index}"), diagnostics);
        }
    }
}

fn check_forbidden_fields(
    object: &serde_json::Map<String, Value>,
    base_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in FORBIDDEN_FIELDS {
        if object.contains_key(*field) {
            diagnostics.push(Diagnostic::error(
                format!("{base_path}/{field}"),
                "axiom.forbidden_runtime_truth_field",
                format!("Forbidden runtime-truth field: {field}"),
            ));
        }
    }
}

fn check_node_id(
    index: usize,
    node: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    match node.get("id") {
        Some(Value::String(id)) if is_valid_node_id(id) => Some(id.clone()),
        Some(Value::String(id)) => {
            diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/id"),
                "axiom.invalid_node_id",
                format!("Node id must match ^[a-zA-Z][a-zA-Z0-9_-]*$: {id}"),
            ));
            None
        }
        Some(_) => {
            diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/id"),
                "axiom.invalid_node_id",
                "node id must be a string",
            ));
            None
        }
        None => {
            diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/id"),
                "axiom.missing_node_id",
                "node id is required",
            ));
            None
        }
    }
}

fn check_depends_on(
    index: usize,
    node: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let Some(depends_on) = node.get("dependsOn") else {
        return Vec::new();
    };
    let Some(depends_on) = depends_on.as_array() else {
        diagnostics.push(Diagnostic::error(
            format!("/nodes/{index}/dependsOn"),
            "axiom.invalid_depends_on",
            "dependsOn must be an array",
        ));
        return Vec::new();
    };

    let mut dependencies = Vec::new();
    for (dependency_index, dependency) in depends_on.iter().enumerate() {
        match dependency {
            Value::String(id) => dependencies.push(id.clone()),
            _ => diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/dependsOn/{dependency_index}"),
                "axiom.invalid_dependency",
                "dependency must be a node id string",
            )),
        }
    }
    dependencies
}

fn check_dependencies(
    node_infos: &[NodeInfo],
    id_to_index: &HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in node_infos {
        for (dependency_index, dependency) in node.depends_on.iter().enumerate() {
            if dependency == &node.id {
                diagnostics.push(Diagnostic::error(
                    format!("/nodes/{}/dependsOn/{dependency_index}", node.index),
                    "axiom.self_dependency",
                    format!("Node cannot depend on itself: {}", node.id),
                ));
            }
            if !id_to_index.contains_key(dependency) {
                diagnostics.push(Diagnostic::error(
                    format!("/nodes/{}/dependsOn/{dependency_index}", node.index),
                    "axiom.unknown_dependency",
                    format!("Unknown dependency node id: {dependency}"),
                ));
            }
        }
    }
}

fn check_cycles(node_infos: &[NodeInfo], diagnostics: &mut Vec<Diagnostic>) {
    let graph = node_infos
        .iter()
        .map(|node| (node.id.as_str(), node.depends_on.as_slice()))
        .collect::<HashMap<_, _>>();
    let node_indexes = node_infos
        .iter()
        .map(|node| (node.id.as_str(), node.index))
        .collect::<HashMap<_, _>>();
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();

    for node in node_infos {
        if let Some(cycle_node) = find_cycle(
            &node.id,
            &graph,
            &mut visiting,
            &mut visited,
            &mut Vec::new(),
        ) {
            let index = node_indexes
                .get(cycle_node.as_str())
                .copied()
                .unwrap_or(node.index);
            diagnostics.push(Diagnostic::error(
                format!("/nodes/{index}/dependsOn"),
                "axiom.dependency_cycle",
                format!("Dependency graph contains a cycle involving node: {cycle_node}"),
            ));
            return;
        }
    }
}

fn find_cycle(
    node_id: &str,
    graph: &HashMap<&str, &[String]>,
    visiting: &mut HashSet<String>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
) -> Option<String> {
    if visited.contains(node_id) {
        return None;
    }
    if !visiting.insert(node_id.to_owned()) {
        return Some(node_id.to_owned());
    }
    stack.push(node_id.to_owned());

    if let Some(dependencies) = graph.get(node_id) {
        for dependency in *dependencies {
            if graph.contains_key(dependency.as_str())
                && let Some(cycle_node) = find_cycle(dependency, graph, visiting, visited, stack)
            {
                return Some(cycle_node);
            }
        }
    }

    stack.pop();
    visiting.remove(node_id);
    visited.insert(node_id.to_owned());
    None
}

fn check_skill(
    index: usize,
    node: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match node.get("skill") {
        Some(Value::String(skill_ref)) => {
            if !is_valid_skill_ref(skill_ref) {
                diagnostics.push(Diagnostic::error(
                    format!("/nodes/{index}/skill"),
                    "axiom.malformed_skill_ref",
                    format!("Malformed exploratory skill ref: {skill_ref}"),
                ));
            }
        }
        Some(Value::Object(skill)) => {
            match skill.get("requested") {
                Some(Value::String(skill_ref)) if is_valid_skill_ref(skill_ref) => {}
                Some(Value::String(skill_ref)) => diagnostics.push(Diagnostic::error(
                    format!("/nodes/{index}/skill/requested"),
                    "axiom.malformed_skill_ref",
                    format!("Malformed exploratory requested skill ref: {skill_ref}"),
                )),
                Some(_) => diagnostics.push(Diagnostic::error(
                    format!("/nodes/{index}/skill/requested"),
                    "axiom.invalid_skill_object",
                    "skill.requested must be a string",
                )),
                None => diagnostics.push(Diagnostic::error(
                    format!("/nodes/{index}/skill/requested"),
                    "axiom.missing_requested_skill",
                    "object skill form must include requested",
                )),
            }
            if let Some(digest) = skill.get("digest") {
                match digest {
                    Value::String(digest) if is_valid_sha256_digest(digest) => {}
                    Value::String(digest) => diagnostics.push(Diagnostic::error(
                        format!("/nodes/{index}/skill/digest"),
                        "axiom.invalid_skill_digest",
                        format!("digest must look like sha256:<64 lowercase hex chars>: {digest}"),
                    )),
                    _ => diagnostics.push(Diagnostic::error(
                        format!("/nodes/{index}/skill/digest"),
                        "axiom.invalid_skill_digest",
                        "digest must be a string",
                    )),
                }
            }
        }
        Some(_) => diagnostics.push(Diagnostic::error(
            format!("/nodes/{index}/skill"),
            "axiom.invalid_skill_ref",
            "skill must be a string or exploratory object form",
        )),
        None => diagnostics.push(Diagnostic::error(
            format!("/nodes/{index}/skill"),
            "axiom.missing_skill_ref",
            "skill is required",
        )),
    }
}

fn check_requested_grants(
    index: usize,
    node: &serde_json::Map<String, Value>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(requested_grants) = node.get("requestedGrants") else {
        return;
    };
    let Some(requested_grants) = requested_grants.as_array() else {
        diagnostics.push(Diagnostic::error(
            format!("/nodes/{index}/requestedGrants"),
            "axiom.invalid_requested_grants",
            "requestedGrants must be an array when present",
        ));
        return;
    };

    for (grant_index, grant) in requested_grants.iter().enumerate() {
        let grant_path = format!("/nodes/{index}/requestedGrants/{grant_index}");
        let Some(grant) = grant.as_object() else {
            diagnostics.push(Diagnostic::error(
                grant_path,
                "axiom.invalid_requested_grant",
                "requested grant must be an object",
            ));
            continue;
        };
        match grant.get("family") {
            Some(Value::String(family)) if !family.trim().is_empty() => {}
            Some(_) => diagnostics.push(Diagnostic::error(
                format!("{grant_path}/family"),
                "axiom.invalid_requested_grant_family",
                "requested grant family must be a non-empty string",
            )),
            None => diagnostics.push(Diagnostic::error(
                format!("{grant_path}/family"),
                "axiom.missing_requested_grant_family",
                "requested grant family is required",
            )),
        }

        match grant.get("constraints") {
            Some(Value::Object(_)) => {}
            Some(_) => diagnostics.push(Diagnostic::error(
                format!("{grant_path}/constraints"),
                "axiom.invalid_requested_grant_constraints",
                "requested grant constraints must be an object",
            )),
            None => diagnostics.push(Diagnostic::error(
                format!("{grant_path}/constraints"),
                "axiom.missing_requested_grant_constraints",
                "requested grant constraints are required",
            )),
        }
    }
}

fn check_node_references(
    nodes: &[Value],
    node_infos: &[NodeInfo],
    id_to_index: &HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let info_by_index = node_infos
        .iter()
        .map(|node| (node.index, node))
        .collect::<HashMap<_, _>>();

    for (index, node) in nodes.iter().enumerate() {
        let Some(node_info) = info_by_index.get(&index) else {
            continue;
        };
        let accessible_nodes = accessible_dependencies(node_info, node_infos);
        scan_references(
            node,
            &format!("/nodes/{index}"),
            &accessible_nodes,
            id_to_index,
            diagnostics,
        );
    }
}

fn accessible_dependencies(current: &NodeInfo, node_infos: &[NodeInfo]) -> HashSet<String> {
    let graph = node_infos
        .iter()
        .map(|node| (node.id.as_str(), node.depends_on.as_slice()))
        .collect::<HashMap<_, _>>();
    let mut accessible = HashSet::new();
    let mut stack = current.depends_on.clone();
    while let Some(node_id) = stack.pop() {
        if !accessible.insert(node_id.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(node_id.as_str()) {
            stack.extend(dependencies.iter().cloned());
        }
    }
    accessible
}

fn scan_references(
    value: &Value,
    path: &str,
    accessible_nodes: &HashSet<String>,
    id_to_index: &HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::String(text) => {
            check_reference_text(text, path, accessible_nodes, id_to_index, diagnostics);
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                scan_references(
                    item,
                    &format!("{path}/{index}"),
                    accessible_nodes,
                    id_to_index,
                    diagnostics,
                );
            }
        }
        Value::Object(object) => {
            for (key, nested) in object {
                scan_references(
                    nested,
                    &format!("{path}/{}", escape_json_pointer_segment(key)),
                    accessible_nodes,
                    id_to_index,
                    diagnostics,
                );
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn check_reference_text(
    text: &str,
    path: &str,
    accessible_nodes: &HashSet<String>,
    id_to_index: &HashMap<String, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if text.contains("${") {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.unsupported_reference",
            format!("Unsupported reference syntax: {text}"),
        ));
        return;
    }
    if !text.starts_with('$') {
        return;
    }
    if !text_matches_reference_shape(text) {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.unsupported_reference",
            format!("Unsupported reference syntax: {text}"),
        ));
        return;
    }

    let Some((root, _fields)) = text[1..].split_once('.') else {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.unsupported_reference",
            format!("Unsupported reference syntax: {text}"),
        ));
        return;
    };

    if root == "input" {
        return;
    }
    if root == "item" || root == "env" {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.unsupported_reference",
            format!("Unsupported reference root: ${root}"),
        ));
        return;
    }
    if !id_to_index.contains_key(root) {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.unknown_reference",
            format!("Reference points to unknown node: ${root}"),
        ));
        return;
    }
    if !accessible_nodes.contains(root) {
        diagnostics.push(Diagnostic::error(
            path,
            "axiom.inaccessible_reference",
            format!("Reference points to node outside this node's dependency closure: ${root}"),
        ));
    }
}

fn text_matches_reference_shape(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('$') else {
        return false;
    };
    let Some((root, fields)) = rest.split_once('.') else {
        return false;
    };
    is_valid_reference_segment(root)
        && fields
            .split('.')
            .all(|segment| !segment.is_empty() && is_valid_reference_segment(segment))
}

fn is_valid_reference_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
}

fn is_valid_node_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-')
}

fn is_valid_skill_ref(skill_ref: &str) -> bool {
    let Some(rest) = skill_ref.strip_prefix("skill://") else {
        return false;
    };
    let Some((namespace, name_and_version)) = rest.split_once('/') else {
        return false;
    };
    let Some((name, version)) = name_and_version.split_once('@') else {
        return false;
    };
    !namespace.is_empty()
        && !name.is_empty()
        && !version.is_empty()
        && namespace.chars().all(is_skill_component_char)
        && name.chars().all(is_skill_component_char)
        && !version.chars().any(char::is_whitespace)
}

fn is_skill_component_char(char: char) -> bool {
    char.is_ascii_alphanumeric() || char == '_' || char == '.' || char == '-'
}

fn is_valid_sha256_digest(digest: &str) -> bool {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .chars()
            .all(|char| char.is_ascii_hexdigit() && !char.is_ascii_uppercase())
}

fn escape_json_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn join_json_pointer(base_path: &str, segment: &str) -> String {
    if base_path == "/" {
        format!("/{}", escape_json_pointer_segment(segment))
    } else {
        format!("{base_path}/{}", escape_json_pointer_segment(segment))
    }
}

fn print_plan_result(path: &str, diagnostics: &[Diagnostic]) {
    if diagnostics.is_empty() {
        println!("valid: {path}");
    } else {
        println!("invalid: {path}");
        print_diagnostics(diagnostics);
    }
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        println!(
            "{} {} {}: {}",
            diagnostic.severity, diagnostic.code, diagnostic.path, diagnostic.message
        );
    }
}

fn build_plan_preview(value: &Value, source_path: &str) -> PlanPreview {
    let plan = value
        .as_object()
        .expect("validated Axiom Plan IR is an object");
    let nodes = plan
        .get("nodes")
        .and_then(Value::as_array)
        .expect("validated Axiom Plan IR has nodes");
    let ordered_indexes = topologically_order_node_indexes(nodes);
    let node_previews = ordered_indexes
        .iter()
        .enumerate()
        .map(|(order_index, node_index)| {
            build_node_preview(
                order_index + 1,
                *node_index,
                nodes[*node_index]
                    .as_object()
                    .expect("validated Axiom Plan IR nodes are objects"),
            )
        })
        .collect::<Vec<_>>();
    let plan_trace = node_previews
        .iter()
        .map(|node| PlanTraceEntry {
            node: node.id.clone(),
            preview_trace: vec![
                format!(
                    "pre-admission requestPreview for `{}` would request `{}`",
                    node.id, node.skill.requested
                ),
                "requested grants remain requested authority only; not admitted, not granted, not executed".to_owned(),
                "expected evidence remains expectation only".to_owned(),
            ],
        })
        .collect::<Vec<_>>();

    PlanPreview {
        source_path: source_path.to_owned(),
        plan_kind: string_field(plan, "kind")
            .unwrap_or("axiom.plan")
            .to_owned(),
        version: string_field(plan, "version").unwrap_or("1").to_owned(),
        name: string_field(plan, "name").unwrap_or("<unnamed>").to_owned(),
        original_node_count: nodes.len(),
        nodes: node_previews,
        plan_trace,
        limitations: preview_limitations(),
    }
}

fn build_node_preview(
    ordinal: usize,
    document_index: usize,
    node: &serde_json::Map<String, Value>,
) -> NodePreview {
    let id = string_field(node, "id")
        .unwrap_or("<missing-id>")
        .to_owned();
    let skill = skill_preview(node.get("skill"));
    let requested_grants = value_array(node.get("requestedGrants"));
    let expected_evidence = value_array(node.get("expectedEvidence"));
    let request_preview = RequestPreview {
        status: "pre-admission",
        summary: format!(
            "would request `{}` with args as declared; references are not evaluated by preview",
            skill.requested
        ),
        args: "args as declared; references are not evaluated by preview",
        authority: "would propose requested authority only; not admitted, not granted, not executed",
        evidence: "expected evidence is expectation only",
    };
    let preview_trace = vec![
        format!(
            "pre-admission previewTrace for `{id}` would request `{}`",
            skill.requested
        ),
        "args as declared; references are not evaluated by preview".to_owned(),
        "requested grants are requested authority only; not granted".to_owned(),
        if expected_evidence.is_empty() {
            "expected evidence: none declared; expectation only".to_owned()
        } else {
            "expected evidence declarations are expectation only".to_owned()
        },
    ];

    NodePreview {
        ordinal,
        document_index,
        id,
        skill,
        depends_on: string_array(node.get("dependsOn")),
        args_as_declared: node.get("args").cloned(),
        requested_grants,
        expected_outputs: value_array(node.get("expectedOutputs")),
        expected_evidence,
        failure_behavior: string_field(node, "failureBehavior").map(str::to_owned),
        request_preview,
        preview_trace,
    }
}

fn topologically_order_node_indexes(nodes: &[Value]) -> Vec<usize> {
    let id_to_index = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node_id_from_value(node).map(|id| (id.to_owned(), index)))
        .collect::<HashMap<_, _>>();
    let mut emitted = HashSet::new();
    let mut ordered = Vec::with_capacity(nodes.len());

    while ordered.len() < nodes.len() {
        let mut progressed = false;
        for (index, node) in nodes.iter().enumerate() {
            if emitted.contains(&index) {
                continue;
            }
            let dependencies_satisfied =
                string_array(node.as_object().and_then(|object| object.get("dependsOn")))
                    .iter()
                    .all(|dependency| {
                        id_to_index
                            .get(dependency)
                            .is_none_or(|dependency_index| emitted.contains(dependency_index))
                    });
            if dependencies_satisfied {
                emitted.insert(index);
                ordered.push(index);
                progressed = true;
            }
        }
        if !progressed {
            ordered.extend((0..nodes.len()).filter(|index| !emitted.contains(index)));
        }
    }

    ordered
}

fn node_id_from_value(node: &Value) -> Option<&str> {
    node.as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
}

fn skill_preview(skill: Option<&Value>) -> SkillPreview {
    match skill {
        Some(Value::String(requested)) => SkillPreview {
            requested: requested.clone(),
            digest: None,
            resolved_metadata: None,
            object_form: false,
        },
        Some(Value::Object(skill)) => SkillPreview {
            requested: string_field(skill, "requested")
                .unwrap_or("<missing-requested-skill>")
                .to_owned(),
            digest: string_field(skill, "digest").map(str::to_owned),
            resolved_metadata: skill.get("resolved").cloned(),
            object_form: true,
        },
        _ => SkillPreview {
            requested: "<missing-skill>".to_owned(),
            digest: None,
            resolved_metadata: None,
            object_form: false,
        },
    }
}

fn string_field<'a>(object: &'a serde_json::Map<String, Value>, field: &str) -> Option<&'a str> {
    object.get(field).and_then(Value::as_str)
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn value_array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn preview_limitations() -> Vec<String> {
    vec![
        "preview-only: pre-admission; not admitted, not granted, not executed.".to_owned(),
        "no skill availability check".to_owned(),
        "no Guild resolution".to_owned(),
        "no Guild admission".to_owned(),
        "no authority grant".to_owned(),
        "no execution".to_owned(),
        "no receipt creation".to_owned(),
        "no evidence persistence".to_owned(),
        "no full policy reduction".to_owned(),
        "Requested grants are requested authority only; Guild admission and policy remain canonical later.".to_owned(),
        "Expected evidence is expectation only.".to_owned(),
        "Args are args as declared; references are not evaluated by preview.".to_owned(),
        "Skill refs are not resolved; object-form skill metadata is not currently schema-admitted, and any resolved field shown by defensive rendering is plan-supplied metadata only, not Guild resolution, not verified by preview.".to_owned(),
    ]
}

fn preview_to_json(preview: &PlanPreview) -> Value {
    serde_json::json!({
        "kind": PREVIEW_KIND,
        "status": PREVIEW_STATUS,
        "plan": {
            "kind": preview.plan_kind,
            "version": preview.version,
            "name": preview.name,
            "sourcePath": preview.source_path,
            "nodeCount": preview.original_node_count
        },
        "nodes": preview
            .nodes
            .iter()
            .map(node_preview_to_json)
            .collect::<Vec<_>>(),
        "planTrace": preview
            .plan_trace
            .iter()
            .map(plan_trace_entry_to_json)
            .collect::<Vec<_>>(),
        "limitations": preview.limitations
    })
}

fn node_preview_to_json(node: &NodePreview) -> Value {
    serde_json::json!({
        "id": node.id,
        "ordinal": node.ordinal,
        "documentIndex": node.document_index,
        "skill": skill_preview_to_json(&node.skill),
        "dependsOn": node.depends_on,
        "argsAsDeclared": node.args_as_declared,
        "argsStatus": "args as declared; references are not evaluated by preview",
        "requestedGrants": {
            "status": "requested authority only",
            "items": node.requested_grants
        },
        "expectedOutputs": node.expected_outputs,
        "expectedEvidence": {
            "status": "expectation only",
            "items": node.expected_evidence
        },
        "failureBehavior": node.failure_behavior,
        "requestPreview": {
            "status": node.request_preview.status,
            "summary": node.request_preview.summary,
            "args": node.request_preview.args,
            "authority": node.request_preview.authority,
            "evidence": node.request_preview.evidence
        },
        "previewTrace": node.preview_trace
    })
}

fn skill_preview_to_json(skill: &SkillPreview) -> Value {
    let mut skill_json = serde_json::Map::new();
    skill_json.insert(
        "requested".to_owned(),
        Value::String(skill.requested.clone()),
    );
    if skill.object_form {
        skill_json.insert(
            "schemaStatus".to_owned(),
            Value::String("object form is not currently schema-admitted".to_owned()),
        );
    } else {
        skill_json.insert(
            "schemaStatus".to_owned(),
            Value::String("string-form skill ref is current schema-admitted form".to_owned()),
        );
    }
    if let Some(digest) = &skill.digest {
        skill_json.insert("digest".to_owned(), Value::String(digest.clone()));
    }
    if let Some(resolved_metadata) = &skill.resolved_metadata {
        skill_json.insert(
            "resolved".to_owned(),
            serde_json::json!({
                "status": "plan-supplied resolved metadata; pre-admission; not Guild resolution; not verified by preview",
                "value": resolved_metadata
            }),
        );
    }
    Value::Object(skill_json)
}

fn plan_trace_entry_to_json(entry: &PlanTraceEntry) -> Value {
    serde_json::json!({
        "node": entry.node,
        "previewTrace": entry.preview_trace
    })
}

fn render_plan_preview(preview: &PlanPreview) -> Result<String> {
    let mut output = String::new();
    push_line(&mut output, "Axiom Plan Preview");
    push_line(&mut output, format!("kind: {PREVIEW_KIND}"));
    push_line(&mut output, format!("status: {PREVIEW_STATUS}"));
    push_line(&mut output, format!("source: {}", preview.source_path));
    push_line(&mut output, format!("plan: {}", preview.name));
    push_line(&mut output, format!("plan kind: {}", preview.plan_kind));
    push_line(&mut output, format!("version: {}", preview.version));
    push_line(
        &mut output,
        format!("nodes: {}", preview.original_node_count),
    );
    push_line(&mut output, "");

    push_line(&mut output, "ordered nodes:");
    for node in &preview.nodes {
        render_node_preview(&mut output, node)?;
    }

    push_line(&mut output, "planTrace:");
    for entry in &preview.plan_trace {
        push_line(&mut output, format!("  - node {}:", entry.node));
        for trace in &entry.preview_trace {
            push_line(&mut output, format!("    - {trace}"));
        }
    }
    push_line(&mut output, "");

    push_line(&mut output, "limitations:");
    for limitation in &preview.limitations {
        push_line(&mut output, format!("  - {limitation}"));
    }

    Ok(output)
}

fn render_node_preview(output: &mut String, node: &NodePreview) -> Result<()> {
    push_line(
        output,
        format!(
            "{}. {} (document index {})",
            node.ordinal, node.id, node.document_index
        ),
    );
    push_line(output, format!("  skill: {}", node.skill.requested));
    if node.skill.object_form {
        push_line(
            output,
            "  skill schema status: object form is not currently schema-admitted",
        );
    }
    if let Some(digest) = &node.skill.digest {
        push_line(output, format!("  skill digest: {digest}"));
    }
    if let Some(resolved_metadata) = &node.skill.resolved_metadata {
        push_line(
            output,
            "  resolved: plan-supplied pre-admission metadata; not Guild resolution; not verified by preview",
        );
        push_json_block(output, resolved_metadata, 4)?;
    }
    if node.depends_on.is_empty() {
        push_line(output, "  dependsOn: []");
    } else {
        push_line(
            output,
            format!("  dependsOn: {}", node.depends_on.join(", ")),
        );
    }

    push_line(output, "  args as declared:");
    if let Some(args) = &node.args_as_declared {
        push_json_block(output, args, 4)?;
    } else {
        push_line(output, "    not declared");
    }

    render_value_list(
        output,
        "requested grants",
        "requested authority only",
        &node.requested_grants,
    )?;
    render_value_list(
        output,
        "expected outputs",
        "planner expectation",
        &node.expected_outputs,
    )?;
    render_value_list(
        output,
        "expected evidence",
        "expectation only",
        &node.expected_evidence,
    )?;
    push_line(
        output,
        format!(
            "  failure behavior: {}",
            node.failure_behavior.as_deref().unwrap_or("not declared")
        ),
    );
    push_line(output, "  requestPreview:");
    push_line(
        output,
        format!("    status: {}", node.request_preview.status),
    );
    push_line(
        output,
        format!("    summary: {}", node.request_preview.summary),
    );
    push_line(output, format!("    args: {}", node.request_preview.args));
    push_line(
        output,
        format!("    authority: {}", node.request_preview.authority),
    );
    push_line(
        output,
        format!("    evidence: {}", node.request_preview.evidence),
    );
    push_line(output, "  previewTrace:");
    for trace in &node.preview_trace {
        push_line(output, format!("    - {trace}"));
    }
    push_line(output, "");
    Ok(())
}

fn render_value_list(
    output: &mut String,
    label: &str,
    status: &str,
    items: &[Value],
) -> Result<()> {
    if items.is_empty() {
        push_line(output, format!("  {label}: none ({status})"));
        return Ok(());
    }
    push_line(output, format!("  {label} ({status}):"));
    for item in items {
        push_json_block(output, item, 4)?;
    }
    Ok(())
}

fn push_json_block(output: &mut String, value: &Value, indent: usize) -> Result<()> {
    let indent_text = " ".repeat(indent);
    let rendered = serde_json::to_string_pretty(value)?;
    for line in rendered.lines() {
        push_line(output, format!("{indent_text}{line}"));
    }
    Ok(())
}

fn push_line(output: &mut String, line: impl AsRef<str>) {
    output.push_str(line.as_ref());
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        Diagnostic, build_plan_preview, diagnostics_golden, golden_mismatch, preview_to_json,
        render_plan_preview, repo_root, validate_value,
    };

    fn validate(plan: &serde_json::Value) -> Vec<Diagnostic> {
        validate_value(plan).expect("Axiom Plan IR schema should compile")
    }

    fn preview(plan: &serde_json::Value) -> super::PlanPreview {
        assert_eq!(validate(plan), Vec::new());
        build_plan_preview(plan, "test-plan.json")
    }

    fn diagnostic_codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }

    fn forbidden_runtime_truth_json_keys() -> &'static [&'static str] {
        &[
            "executionId",
            "receipt",
            "grantedAuthority",
            "effectiveAuthority",
            "hostDecision",
            "runtimeStatus",
            "evidenceProduced",
            "grantedGrants",
            "effectiveGrants",
        ]
    }

    fn assert_no_forbidden_runtime_truth_json_keys(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, nested) in object {
                    assert!(
                        !forbidden_runtime_truth_json_keys().contains(&key.as_str()),
                        "runtime-truth JSON key leaked: {key}"
                    );
                    assert_no_forbidden_runtime_truth_json_keys(nested);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    assert_no_forbidden_runtime_truth_json_keys(item);
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn base_plan(nodes: serde_json::Value) -> serde_json::Value {
        json!({
            "kind": "axiom.plan",
            "version": "1",
            "name": "test plan",
            "nodes": nodes
        })
    }

    #[allow(clippy::needless_pass_by_value)]
    fn node(id: &str, depends_on: serde_json::Value, args: serde_json::Value) -> serde_json::Value {
        json!({
            "id": id,
            "skill": "skill://example/render-report@^0.1",
            "args": args,
            "dependsOn": depends_on,
            "requestedGrants": [],
            "expectedOutputs": [],
            "expectedEvidence": [],
            "failureBehavior": "stopPlan"
        })
    }

    #[test]
    fn accepts_basic_plan() {
        let plan = base_plan(json!([
            node("draft", json!([]), json!({"title": "$input.title"})),
            node(
                "final",
                json!(["draft"]),
                json!({"source": "$draft.output"})
            )
        ]));

        assert_eq!(validate(&plan), Vec::new());
    }

    #[test]
    fn accepts_minimal_node_contract() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "skill://example/render-report@^0.1"
        }]));

        assert_eq!(validate(&plan), Vec::new());
    }

    #[test]
    fn rejects_duplicate_node_ids() {
        let plan = base_plan(json!([
            node("brief", json!([]), json!({})),
            node("brief", json!([]), json!({}))
        ]));

        assert!(diagnostic_codes(&validate(&plan)).contains(&"axiom.duplicate_node_id"));
    }

    #[test]
    fn rejects_unknown_dependencies() {
        let plan = base_plan(json!([node("brief", json!(["missing"]), json!({}))]));

        assert!(diagnostic_codes(&validate(&plan)).contains(&"axiom.unknown_dependency"));
    }

    #[test]
    fn rejects_cycles() {
        let plan = base_plan(json!([
            node("first", json!(["second"]), json!({})),
            node("second", json!(["first"]), json!({}))
        ]));

        assert!(diagnostic_codes(&validate(&plan)).contains(&"axiom.dependency_cycle"));
    }

    #[test]
    fn rejects_bad_references() {
        let plan = base_plan(json!([node(
            "brief",
            json!([]),
            json!({
                "missing": "$missing.output",
                "env": "$env.SECRET",
                "call": "$brief.output()"
            })
        )]));

        let codes = diagnostic_codes(&validate(&plan));
        assert!(codes.contains(&"axiom.unknown_reference"));
        assert!(codes.contains(&"axiom.unsupported_reference"));
    }

    #[test]
    fn rejects_forbidden_runtime_truth() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "skill://example/render-report@^0.1",
            "args": {},
            "dependsOn": [],
            "requestedGrants": [],
            "grantedAuthority": [],
            "expectedOutputs": [],
            "expectedEvidence": [],
            "failureBehavior": "stopPlan"
        }]));

        assert!(
            diagnostic_codes(&validate(&plan)).contains(&"axiom.forbidden_runtime_truth_field")
        );
    }

    #[test]
    fn rejects_malformed_skill_refs() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "example/render-report@^0.1",
            "args": {},
            "dependsOn": [],
            "requestedGrants": [],
            "expectedOutputs": [],
            "expectedEvidence": [],
            "failureBehavior": "stopPlan"
        }]));

        assert!(diagnostic_codes(&validate(&plan)).contains(&"axiom.malformed_skill_ref"));
    }

    #[test]
    fn rejects_bad_requested_grant_shape() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "skill://example/render-report@^0.1",
            "args": {},
            "dependsOn": [],
            "requestedGrants": [
                {
                    "family": "",
                    "constraints": "not an object"
                }
            ],
            "expectedOutputs": [],
            "expectedEvidence": [],
            "failureBehavior": "stopPlan"
        }]));

        let codes = diagnostic_codes(&validate(&plan));
        assert!(codes.contains(&"axiom.invalid_requested_grant_family"));
        assert!(codes.contains(&"axiom.invalid_requested_grant_constraints"));
    }

    #[test]
    fn maps_schema_required_failures_to_stable_code() {
        let plan = base_plan(json!([{
            "id": "brief"
        }]));

        let codes = diagnostic_codes(&validate(&plan));
        assert!(codes.contains(&"axiom.schema.missing_required_field"));
    }

    #[test]
    fn maps_schema_additional_properties_to_stable_code() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "skill://example/render-report@^0.1",
            "surprise": true
        }]));

        let codes = diagnostic_codes(&validate(&plan));
        assert!(codes.contains(&"axiom.schema.additional_property"));
    }

    #[test]
    fn does_not_police_forbidden_words_inside_skill_args_payload() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": "skill://example/render-report@^0.1",
            "args": {
                "grantedAuthority": {
                    "note": "skill-owned payload, not Axiom runtime truth"
                }
            }
        }]));

        assert!(
            !diagnostic_codes(&validate(&plan)).contains(&"axiom.forbidden_runtime_truth_field")
        );
    }

    #[test]
    fn valid_preview_succeeds() {
        let plan = base_plan(json!([
            node("draft", json!([]), json!({"title": "$input.title"})),
            node(
                "final",
                json!(["draft"]),
                json!({"source": "$draft.output"})
            )
        ]));

        let preview = preview(&plan);

        assert_eq!(preview.name, "test plan");
        assert_eq!(preview.version, "1");
        assert_eq!(preview.nodes.len(), 2);
        assert_eq!(preview.nodes[0].id, "draft");
        assert_eq!(preview.nodes[1].id, "final");
    }

    #[test]
    fn invalid_preview_fails_before_rendering_and_preserves_diagnostics() {
        let plan = base_plan(json!([{
            "id": "brief"
        }]));

        let diagnostics = validate(&plan);

        assert!(diagnostic_codes(&diagnostics).contains(&"axiom.schema.missing_required_field"));
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == "/nodes/0/skill")
        );
    }

    #[test]
    fn preview_orders_dependencies_before_dependents_with_stable_independent_order() {
        let plan = base_plan(json!([
            node("child", json!(["source"]), json!({})),
            node("unrelated", json!([]), json!({})),
            node("source", json!([]), json!({}))
        ]));

        let preview = preview(&plan);
        let ordered_ids = preview
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(ordered_ids, vec!["unrelated", "source", "child"]);
    }

    #[test]
    fn preview_includes_required_boundary_phrases() {
        let plan = base_plan(json!([{
            "id": "evidence",
            "skill": "skill://example/emit-evidence-exact@^0.1",
            "args": {
                "message": "planned evidence payload"
            },
            "requestedGrants": [
                {
                    "family": "emit-evidence",
                    "constraints": {
                        "audiences": ["user"]
                    }
                }
            ],
            "expectedEvidence": [
                {
                    "kind": "summary"
                }
            ]
        }]));
        let rendered = render_plan_preview(&preview(&plan)).expect("preview renders");

        for phrase in [
            "pre-admission",
            "not admitted",
            "not granted",
            "not executed",
            "no Guild resolution",
            "no receipt creation",
            "no evidence persistence",
            "requested authority only",
            "expectation only",
            "args as declared",
            "would request",
            "would propose",
        ] {
            assert!(rendered.contains(phrase), "missing phrase: {phrase}");
        }
    }

    #[test]
    fn preview_avoids_positive_runtime_truth_phrases() {
        let plan = base_plan(json!([node("brief", json!([]), json!({}))]));
        let preview = preview(&plan);
        let rendered = render_plan_preview(&preview).expect("preview renders");
        let rendered_json =
            serde_json::to_string_pretty(&preview_to_json(&preview)).expect("preview JSON renders");
        let combined = format!("{rendered}\n{rendered_json}");

        for phrase in [
            "admitted by Guild",
            "granted by Guild",
            "executed by Guild",
            "receipt created",
            "evidence produced",
            "resolved by Guild",
            "runtime journal",
        ] {
            assert!(
                !combined.contains(phrase),
                "positive runtime-truth phrase leaked: {phrase}"
            );
        }
    }

    #[test]
    fn json_preview_has_preview_shape_and_avoids_runtime_truth_fields() {
        let plan = base_plan(json!([node("brief", json!([]), json!({}))]));
        let rendered = preview_to_json(&preview(&plan));
        let rendered_text =
            serde_json::to_string_pretty(&rendered).expect("preview JSON should render");

        assert_eq!(rendered["kind"], "axiom.plan_preview");
        assert_eq!(rendered["status"], "preview_only");
        assert!(rendered.get("plan").is_some());
        assert!(rendered.get("nodes").is_some());
        assert!(rendered.get("planTrace").is_some());
        assert!(rendered.get("limitations").is_some());

        assert_no_forbidden_runtime_truth_json_keys(&rendered);

        for limitation in [
            "no skill availability check",
            "no Guild resolution",
            "no Guild admission",
            "no authority grant",
            "no execution",
            "no receipt creation",
            "no evidence persistence",
            "no full policy reduction",
        ] {
            assert!(
                rendered["limitations"]
                    .as_array()
                    .expect("limitations should be an array")
                    .iter()
                    .any(|value| value == limitation),
                "missing limitation: {limitation}"
            );
        }

        assert!(rendered_text.contains("\"requestPreview\""));
        assert!(rendered_text.contains("\"previewTrace\""));
    }

    #[test]
    fn defensive_object_form_renderer_labels_resolved_as_plan_supplied_metadata() {
        let plan = base_plan(json!([{
            "id": "brief",
            "skill": {
                "requested": "skill://example/render-report@^0.1",
                "digest": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "resolved": {
                    "name": "render-report",
                    "version": "0.1.0"
                }
            },
            "args": {}
        }]));

        let preview = build_plan_preview(&plan, "defensive-object-form.json");
        let rendered = render_plan_preview(&preview).expect("preview renders");
        let rendered_json =
            serde_json::to_string_pretty(&preview_to_json(&preview)).expect("preview JSON renders");

        assert!(rendered.contains("object form is not currently schema-admitted"));
        assert!(rendered.contains("resolved: plan-supplied pre-admission metadata"));
        assert!(rendered.contains("not Guild resolution"));
        assert!(rendered.contains("not verified by preview"));
        assert!(rendered_json.contains("\"resolved\""));
        assert!(rendered_json.contains("plan-supplied resolved metadata"));
        assert!(!rendered.contains("resolved by Guild"));
        assert!(!rendered_json.contains("resolved by Guild"));
    }

    #[test]
    fn diagnostic_golden_normalization_keeps_stable_semantic_codes() {
        let malformed = diagnostics_golden(
            &repo_root()
                .join("docs/strategy/axiom-plan-ir/examples/invalid/malformed-skill-ref.json"),
        )
        .expect("malformed diagnostic golden renders");
        let malformed_json: serde_json::Value =
            serde_json::from_str(&malformed).expect("malformed diagnostic golden is JSON");
        let malformed_codes = malformed_json["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .iter()
            .map(|diagnostic| {
                diagnostic["code"]
                    .as_str()
                    .expect("diagnostic code should be a string")
            })
            .collect::<Vec<_>>();

        assert_eq!(
            malformed_json["sourcePath"],
            "docs/strategy/axiom-plan-ir/examples/invalid/malformed-skill-ref.json"
        );
        assert!(malformed_codes.contains(&"axiom.schema.invalid_shape"));
        assert!(malformed_codes.contains(&"axiom.malformed_skill_ref"));

        let granted_authority = diagnostics_golden(
            &repo_root()
                .join("docs/strategy/axiom-plan-ir/examples/invalid/granted-authority-claim.json"),
        )
        .expect("granted-authority diagnostic golden renders");
        let granted_authority_json: serde_json::Value =
            serde_json::from_str(&granted_authority).expect("granted diagnostic golden is JSON");
        let granted_authority_codes = granted_authority_json["diagnostics"]
            .as_array()
            .expect("diagnostics should be an array")
            .iter()
            .map(|diagnostic| {
                diagnostic["code"]
                    .as_str()
                    .expect("diagnostic code should be a string")
            })
            .collect::<Vec<_>>();

        assert!(granted_authority_codes.contains(&"axiom.schema.additional_property"));
        assert!(granted_authority_codes.contains(&"axiom.forbidden_runtime_truth_field"));
    }

    #[test]
    fn golden_mismatch_reports_comparison_failures() {
        assert!(golden_mismatch("example.golden", "same\r\n", "same\n").is_none());
        assert!(golden_mismatch("example.golden", "old\n", "new\n").is_some());
    }
}
