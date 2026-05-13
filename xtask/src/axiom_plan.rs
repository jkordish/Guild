use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;

const AXIOM_ROOT: &str = "docs/strategy/axiom-plan-ir";
const USAGE: &str = "usage: cargo run -q -p xtask -- axiom-plan validate <path>\n       cargo run -q -p xtask -- axiom-plan validate-examples";
const FORBIDDEN_FIELDS: &[&str] = &[
    "executionId",
    "receipt",
    "grantedAuthority",
    "effectiveAuthority",
    "hostDecision",
    "runtimeStatus",
    "grants",
    "grantedGrants",
    "effectiveGrants",
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
        "validate-examples" => {
            if args.next().is_some() {
                bail!("unexpected extra arguments");
            }
            validate_examples()
        }
        other => bail!("unknown axiom-plan command `{other}`"),
    }
}

fn validate_examples() -> Result<()> {
    let root = Path::new(AXIOM_ROOT);
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
            println!("PASS valid {}", path.display());
        } else {
            println!("FAIL valid {}", path.display());
            print_diagnostics(&diagnostics);
            failures.push(format!(
                "{} should be valid but had {} diagnostic(s)",
                path.display(),
                diagnostics.len()
            ));
        }
    }

    for path in &invalid_paths {
        let diagnostics = validate_path(path)?;
        if diagnostics.is_empty() {
            println!("FAIL invalid {} unexpectedly passed", path.display());
            failures.push(format!("{} should be invalid but passed", path.display()));
        } else {
            let codes = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>()
                .join(", ");
            println!("PASS invalid {} ({codes})", path.display());
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
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value = match serde_json::from_str::<Value>(&text) {
        Ok(value) => value,
        Err(error) => {
            return Ok(vec![Diagnostic::error(
                "/",
                "axiom.parse_error",
                format!("Failed to parse JSON: {error}"),
            )]);
        }
    };
    Ok(validate_value(&value))
}

fn validate_value(value: &Value) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let Some(plan) = value.as_object() else {
        diagnostics.push(Diagnostic::error(
            "/",
            "axiom.plan_type",
            "Axiom plan must be a JSON object",
        ));
        return diagnostics;
    };

    check_forbidden_fields(plan, "", &mut diagnostics);
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

        check_forbidden_fields(node_object, &format!("/nodes/{index}"), diagnostics);

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

fn check_forbidden_fields(
    object: &serde_json::Map<String, Value>,
    base_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for field in FORBIDDEN_FIELDS {
        if object.contains_key(*field) {
            diagnostics.push(Diagnostic::error(
                format!("{base_path}/{field}"),
                "axiom.forbidden_runtime_truth",
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
                format!("Node id must match ^[A-Za-z][A-Za-z0-9_-]*$: {id}"),
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
        check_forbidden_fields(grant, &grant_path, diagnostics);

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Diagnostic, validate_value};

    fn diagnostic_codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
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

        assert_eq!(validate_value(&plan), Vec::new());
    }

    #[test]
    fn rejects_duplicate_node_ids() {
        let plan = base_plan(json!([
            node("brief", json!([]), json!({})),
            node("brief", json!([]), json!({}))
        ]));

        assert!(diagnostic_codes(&validate_value(&plan)).contains(&"axiom.duplicate_node_id"));
    }

    #[test]
    fn rejects_unknown_dependencies() {
        let plan = base_plan(json!([node("brief", json!(["missing"]), json!({}))]));

        assert!(diagnostic_codes(&validate_value(&plan)).contains(&"axiom.unknown_dependency"));
    }

    #[test]
    fn rejects_cycles() {
        let plan = base_plan(json!([
            node("first", json!(["second"]), json!({})),
            node("second", json!(["first"]), json!({}))
        ]));

        assert!(diagnostic_codes(&validate_value(&plan)).contains(&"axiom.dependency_cycle"));
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

        let codes = diagnostic_codes(&validate_value(&plan));
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
            diagnostic_codes(&validate_value(&plan)).contains(&"axiom.forbidden_runtime_truth")
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

        assert!(diagnostic_codes(&validate_value(&plan)).contains(&"axiom.malformed_skill_ref"));
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

        let codes = diagnostic_codes(&validate_value(&plan));
        assert!(codes.contains(&"axiom.invalid_requested_grant_family"));
        assert!(codes.contains(&"axiom.invalid_requested_grant_constraints"));
    }
}
