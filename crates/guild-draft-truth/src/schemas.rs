use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use jsonschema::paths::Location;
use jsonschema::{Registry, Resource, Validator};
use serde_json::Value;
use url::Url;

use crate::util::{draft_v1_dir, read_json};

fn schema_path(name: &str) -> PathBuf {
    draft_v1_dir().join(name)
}

fn schema_names() -> [&'static str; 12] {
    [
        "common.schema.json",
        "skill_contract.schema.json",
        "runtime_guarantee.schema.json",
        "comparator_profile.schema.json",
        "proof_record.schema.json",
        "witness_record.schema.json",
        "witness_verification_result.schema.json",
        "admission_request.schema.json",
        "execution_plan.schema.json",
        "delegated_capability_token.schema.json",
        "token_verification_result.schema.json",
        "benchmark_matrix.schema.json",
    ]
}

fn validator_for_schema(schema_name: &str) -> Result<Validator> {
    let root_path = schema_path(schema_name);
    let root_schema = read_json(&root_path)?;
    let mut registry = Registry::new();
    for name in schema_names() {
        let path = schema_path(name);
        if !path.exists() {
            continue;
        }
        let schema = read_json(&path)?;
        let file_url = Url::from_file_path(&path)
            .expect("schema file path converts to file URL")
            .to_string();
        registry = registry
            .add(name, Resource::from_contents(schema.clone()))
            .with_context(|| format!("failed to register schema resource {name}"))?
            .add(file_url.as_str(), Resource::from_contents(schema))
            .with_context(|| format!("failed to register schema resource {file_url}"))?;
    }
    let registry = registry
        .prepare()
        .with_context(|| "failed to prepare schema registry")?;
    jsonschema::draft202012::options()
        .with_registry(&registry)
        .build(&root_schema)
        .with_context(|| format!("failed to compile {schema_name}"))
}

pub fn validate_instance(schema_name: &str, instance: &Value) -> Result<Vec<String>> {
    let validator = validator_for_schema(schema_name)?;
    let errors = validator
        .iter_errors(instance)
        .map(render_validation_error)
        .collect();
    Ok(errors)
}

fn render_validation_error(error: jsonschema::ValidationError<'_>) -> String {
    let location = location_to_string(error.instance_path());
    format!("{location}: {}", error)
}

fn location_to_string(location: &Location) -> String {
    let rendered = location.to_string();
    if rendered.is_empty() {
        "<root>".to_owned()
    } else {
        rendered
    }
}

pub fn validate_examples(example_pairs: &[(&str, &str)]) -> Result<Vec<String>> {
    let mut failures = Vec::new();
    let mut validators = BTreeMap::<String, Validator>::new();
    for (schema_name, relative_path) in example_pairs {
        let validator = if let Some(existing) = validators.get(*schema_name) {
            existing
        } else {
            validators.insert(
                (*schema_name).to_owned(),
                validator_for_schema(schema_name)?,
            );
            validators.get(*schema_name).expect("validator inserted")
        };
        let instance = read_json(&draft_v1_dir().join(relative_path))?;
        for error in validator.iter_errors(&instance) {
            failures.push(format!(
                "{relative_path}: {}",
                render_validation_error(error)
            ));
        }
    }
    Ok(failures)
}

pub fn validate_json_file(schema_name: &str, path: &Path) -> Result<Vec<String>> {
    let value = read_json(path)?;
    validate_instance(schema_name, &value)
}
