use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use guild_manifest::PublisherRef;
use guild_registry::{
    InstalledSkill, InstalledTrustMetadata, InstalledVerificationRecord, LocalPublisherIdentity,
    LocalRegistry, LocalSourceInstaller, OciRegistryReference, OciRegistryTransportOptions,
    RegistryError, SkillRegistry, TrustedPublisherRecord,
};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityGrantSet, ExecutionRecord, ExecutionStatus, InstalledVerificationState,
    LocalTrustTier, RequestedSkillRef, ResourceReadResult,
};
use serde::Serialize;
use serde_json::Value;

use crate::server::{GuildMcpServer, ServerStartupError};
use crate::{CLI_BINARY_NAME, GuildMcpFacade, InspectRequest, InspectResponse, McpError};

const DEFAULT_TENANT_ID: &str = "local";
const DEFAULT_ACTOR_ID: &str = "guild-cli";
const DEFAULT_LIST_SUMMARY_EXECUTION_LIMIT: usize = 10;
const DEFAULT_LIST_EXECUTIONS_LIMIT: usize = 50;

#[derive(Debug)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

impl From<RegistryError> for CliError {
    fn from(value: RegistryError) -> Self {
        Self::new(format!("{value}"))
    }
}

impl From<ServerStartupError> for CliError {
    fn from(value: ServerStartupError) -> Self {
        Self::new(format!("{value}"))
    }
}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Debug, Clone)]
struct GlobalOptions {
    registry_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct InspectCommandOutput {
    summary: String,
    record: ExecutionRecord,
}

#[derive(Debug, Clone, Serialize)]
struct ReadCommandOutput {
    uri: String,
    mime_type: String,
    sha256: Option<String>,
    text: Option<String>,
    bytes_base64: Option<String>,
    output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ListedInstalledSkillOutput {
    resolved_skill: String,
    digest: String,
    trust_tier: LocalTrustTier,
    verification_state: InstalledVerificationState,
}

#[derive(Debug, Clone, Serialize)]
struct ListedExecutionOutput {
    execution_id: String,
    uri: String,
    status: ExecutionStatus,
    resolved_skill: String,
    started_at_utc: Option<String>,
    finished_at_utc: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ListSummaryOutput {
    registry_root: String,
    installed_count: usize,
    installed: Vec<ListedInstalledSkillOutput>,
    recent_execution_limit: usize,
    recent_execution_count: usize,
    recent_executions: Vec<ListedExecutionOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct ListSkillsOutput {
    registry_root: String,
    installed_count: usize,
    installed: Vec<ListedInstalledSkillOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct ListExecutionsOutput {
    registry_root: String,
    limit: usize,
    execution_count: usize,
    executions: Vec<ListedExecutionOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct InstalledSkillOutput {
    resolved_skill: String,
    digest: String,
    registry_root: String,
    root_dir: String,
    manifest_path: String,
    artifact_path: String,
    trust: InstalledTrustMetadata,
    verification: Option<InstalledVerificationRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct ImportCommandOutput {
    format: &'static str,
    registry_root: String,
    installed: Vec<InstalledSkillOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct ExportCommandOutput {
    format: &'static str,
    output_root: String,
    root_skill: String,
    publisher_id: String,
    includes_dependency_closure: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PushCommandOutput {
    reference: String,
    manifest_digest: String,
    root_skill: String,
    publisher_id: String,
    includes_dependency_closure: bool,
}

#[derive(Debug, Clone, Serialize)]
struct TrustGenerateOutput {
    publisher_id: String,
    output_path: String,
}

#[derive(Debug, Clone, Serialize)]
struct TrustAddOutput {
    publisher_id: String,
    trust_tier: LocalTrustTier,
    registry_root: String,
}

#[derive(Debug, Clone, Serialize)]
struct TrustListOutput {
    registry_root: String,
    publishers: Vec<TrustedPublisherRecord>,
}

/// Run the first-class local `guild` CLI against the current process args.
///
/// # Errors
///
/// Returns an error if argument parsing fails, required local state is missing,
/// or the selected Guild command cannot be completed.
pub fn run(
    args: impl IntoIterator<Item = String>,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let (global, args) = parse_global_options(args)?;
    let Some(command) = args.first().map(String::as_str) else {
        print_usage();
        return Ok(());
    };

    match command {
        "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "inspect" => run_inspect(&args[1..], &global, env_registry_root),
        "read" => run_read(&args[1..], &global, env_registry_root),
        "list" => run_list(&args[1..], &global, env_registry_root),
        "install" => run_install(&args[1..], &global, env_registry_root),
        "export" => run_export(&args[1..], &global, env_registry_root),
        "import" => run_import(&args[1..], &global, env_registry_root),
        "push" => run_push(&args[1..], &global, env_registry_root),
        "pull" => run_pull(&args[1..], &global, env_registry_root),
        "trust" => run_trust(&args[1..], &global, env_registry_root),
        "codex" => run_codex(&args[1..], &global),
        "mcp" => run_mcp(&args[1..], &global, env_registry_root),
        _ => Err(CliError::new(format!("unknown subcommand `{command}`"))),
    }
}

fn parse_global_options(
    args: impl IntoIterator<Item = String>,
) -> Result<(GlobalOptions, Vec<String>), CliError> {
    let mut args = args.into_iter();
    let _program = args.next();
    let mut registry_root = None;
    let mut remaining = Vec::new();

    while let Some(argument) = args.next() {
        if argument == "--registry-root" {
            let Some(value) = args.next() else {
                return Err(CliError::new(
                    "--registry-root requires a following path argument",
                ));
            };
            registry_root = Some(PathBuf::from(value));
        } else {
            remaining.push(argument);
        }
    }

    Ok((GlobalOptions { registry_root }, remaining))
}

fn run_list(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_list_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    match args.first().map(String::as_str) {
        Some("skills") => run_list_skills(&args[1..], &registry_root),
        Some("executions") => run_list_executions(&args[1..], &registry_root),
        None | Some(_) => run_list_summary(args, &registry_root),
    }
}

fn run_list_summary(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    let mut json_output = false;

    for argument in args {
        match argument.as_str() {
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild list`: `{other}`"
                )));
            }
        }
    }

    let registry = LocalRegistry::load(registry_root)?;
    let installed = summarize_listed_installed_skills(registry.installed());
    let recent_records =
        registry.list_recent_execution_records(DEFAULT_LIST_SUMMARY_EXECUTION_LIMIT)?;
    let recent_executions = summarize_listed_executions(&recent_records);
    let output = ListSummaryOutput {
        registry_root: registry_root.display().to_string(),
        installed_count: installed.len(),
        installed,
        recent_execution_limit: DEFAULT_LIST_SUMMARY_EXECUTION_LIMIT,
        recent_execution_count: recent_executions.len(),
        recent_executions,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_list_summary_text(&output);
    }

    Ok(())
}

fn run_list_skills(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    let mut json_output = false;

    for argument in args {
        match argument.as_str() {
            "--json" => json_output = true,
            "--help" | "-h" => {
                print_list_skills_usage();
                return Ok(());
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild list skills`: `{other}`"
                )));
            }
        }
    }

    let registry = LocalRegistry::load(registry_root)?;
    let installed = summarize_listed_installed_skills(registry.installed());
    let output = ListSkillsOutput {
        registry_root: registry_root.display().to_string(),
        installed_count: installed.len(),
        installed,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_list_skills_text(&output);
    }

    Ok(())
}

fn run_list_executions(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    let mut json_output = false;
    let mut limit = DEFAULT_LIST_EXECUTIONS_LIMIT;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                let value = next_value(args, &mut index, "--limit")?;
                limit = value.parse::<usize>().map_err(|_| {
                    CliError::new(format!(
                        "invalid value for `--limit`: `{value}` is not a positive integer"
                    ))
                })?;
                if limit == 0 {
                    return Err(CliError::new(
                        "`guild list executions` requires --limit to be greater than zero",
                    ));
                }
            }
            "--json" => json_output = true,
            "--help" | "-h" => {
                print_list_executions_usage();
                return Ok(());
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild list executions`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let registry = LocalRegistry::load(registry_root)?;
    let records = registry.list_recent_execution_records(limit)?;
    let executions = summarize_listed_executions(&records);
    let output = ListExecutionsOutput {
        registry_root: registry_root.display().to_string(),
        limit,
        execution_count: executions.len(),
        executions,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_list_executions_text(&output);
    }

    Ok(())
}

fn run_inspect(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_inspect_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let skill = parse_skill_ref(&args[0])?;
    let mut input_json = None;
    let mut input_file = None;
    let mut grants_json = None;
    let mut grants_file = None;
    let mut tenant_id = DEFAULT_TENANT_ID.to_owned();
    let mut actor_id = DEFAULT_ACTOR_ID.to_owned();
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--input-json" => {
                input_json = Some(next_value(args, &mut index, "--input-json")?.to_owned());
            }
            "--input-file" => {
                input_file = Some(PathBuf::from(
                    next_value(args, &mut index, "--input-file")?.to_owned(),
                ));
            }
            "--grants-json" => {
                grants_json = Some(next_value(args, &mut index, "--grants-json")?.to_owned());
            }
            "--grants-file" => {
                grants_file = Some(PathBuf::from(
                    next_value(args, &mut index, "--grants-file")?.to_owned(),
                ));
            }
            "--tenant-id" => {
                next_value(args, &mut index, "--tenant-id")?.clone_into(&mut tenant_id);
            }
            "--actor-id" => {
                next_value(args, &mut index, "--actor-id")?.clone_into(&mut actor_id);
            }
            "--json" => {
                json_output = true;
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild inspect`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    if input_json.is_some() && input_file.is_some() {
        return Err(CliError::new(
            "use either --input-json or --input-file, not both",
        ));
    }

    if grants_json.is_some() && grants_file.is_some() {
        return Err(CliError::new(
            "use either --grants-json or --grants-file, not both",
        ));
    }

    let input = read_json_input(input_json.as_deref(), input_file.as_deref())?
        .unwrap_or_else(|| serde_json::json!({}));
    let grants = read_json_value(grants_json.as_deref(), grants_file.as_deref())?
        .map(parse_capability_grants)
        .transpose()?
        .unwrap_or_default();

    let facade = build_facade(&registry_root)?;
    let response = facade
        .inspect(InspectRequest::new(
            skill, input, tenant_id, actor_id, grants,
        ))
        .map_err(cli_error_from_mcp)?;

    let output = InspectCommandOutput {
        summary: response.summary.clone(),
        record: response.structured_content.clone(),
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_inspect_text(&response);
    }

    Ok(())
}

fn run_read(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_read_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let uri = args[0].clone();
    let mut output_path = None;
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--output" => {
                output_path = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            "--json" => {
                json_output = true;
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild read`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let facade = build_facade(&registry_root)?;
    let resource = facade.read_resource(&uri).map_err(cli_error_from_mcp)?;

    if let Some(path) = output_path {
        fs::write(&path, &resource.bytes)?;
        if json_output {
            let output = ReadCommandOutput {
                uri: resource.uri,
                mime_type: resource.mime_type,
                sha256: resource.sha256,
                text: None,
                bytes_base64: None,
                output_path: Some(path.display().to_string()),
            };
            print_json(&output)?;
        } else {
            println!("wrote {} to {}", uri, path.display());
        }
        return Ok(());
    }

    if json_output {
        let text = String::from_utf8(resource.bytes.clone()).ok();
        let output = ReadCommandOutput {
            uri: resource.uri,
            mime_type: resource.mime_type,
            sha256: resource.sha256,
            text,
            bytes_base64: Some(base64::engine::general_purpose::STANDARD.encode(resource.bytes)),
            output_path: None,
        };
        print_json(&output)?;
        return Ok(());
    }

    print_read_text(&resource)
}

fn run_install(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_install_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let source_dir = PathBuf::from(&args[0]);
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild install`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let installed = LocalSourceInstaller::new(&registry_root)?.install(&source_dir)?;
    let output = summarize_installed_skill(&installed, &registry_root);

    if json_output {
        print_json(&output)?;
    } else {
        println!("installed {}", output.resolved_skill);
        println!("digest: {}", output.digest);
        println!("path: {}", output.root_dir);
    }

    Ok(())
}

fn run_export(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let Some(format) = args.first().map(String::as_str) else {
        print_export_usage();
        return Ok(());
    };
    if is_help(format) {
        print_export_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    match format {
        "bundle" => run_export_bundle(&args[1..], &registry_root),
        "oci-layout" => run_export_oci_layout(&args[1..], &registry_root),
        _ => Err(CliError::new(format!(
            "unknown export format `{format}`; expected `bundle` or `oci-layout`"
        ))),
    }
}

fn run_export_bundle(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_export_bundle_usage();
        return Ok(());
    }

    let registry = LocalRegistry::load(registry_root)?;
    let root = resolve_installed_skill(&registry, &args[0])?;
    let mut signer = None;
    let mut output_root = None;
    let mut include_dependencies = false;
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--signer" => {
                signer = Some(PathBuf::from(next_value(args, &mut index, "--signer")?));
            }
            "--output" => {
                output_root = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            "--include-dependencies" => include_dependencies = true,
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild export bundle`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let signer = load_signer_identity(signer.as_deref())?;
    let output_root = output_root
        .ok_or_else(|| CliError::new("`guild export bundle` requires --output <directory>"))?;
    registry.export_bundle(
        &root.resolved_ref,
        include_dependencies,
        &output_root,
        &signer,
    )?;

    let output = ExportCommandOutput {
        format: "bundle",
        output_root: output_root.display().to_string(),
        root_skill: format_resolved_skill_ref(&root.resolved_ref),
        publisher_id: signer.publisher.id,
        includes_dependency_closure: include_dependencies,
    };

    if json_output {
        print_json(&output)?;
    } else {
        println!("exported {} to {}", output.root_skill, output.output_root);
    }

    Ok(())
}

fn run_export_oci_layout(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_export_oci_layout_usage();
        return Ok(());
    }

    let registry = LocalRegistry::load(registry_root)?;
    let root = resolve_installed_skill(&registry, &args[0])?;
    let mut signer = None;
    let mut output_root = None;
    let mut include_dependencies = false;
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--signer" => {
                signer = Some(PathBuf::from(next_value(args, &mut index, "--signer")?));
            }
            "--output" => {
                output_root = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            "--include-dependencies" => include_dependencies = true,
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild export oci-layout`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let signer = load_signer_identity(signer.as_deref())?;
    let output_root = output_root
        .ok_or_else(|| CliError::new("`guild export oci-layout` requires --output <directory>"))?;
    registry.export_oci_layout(
        &root.resolved_ref,
        include_dependencies,
        &output_root,
        &signer,
    )?;

    let output = ExportCommandOutput {
        format: "oci-layout",
        output_root: output_root.display().to_string(),
        root_skill: format_resolved_skill_ref(&root.resolved_ref),
        publisher_id: signer.publisher.id,
        includes_dependency_closure: include_dependencies,
    };

    if json_output {
        print_json(&output)?;
    } else {
        println!("exported {} to {}", output.root_skill, output.output_root);
    }

    Ok(())
}

fn run_import(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let Some(format) = args.first().map(String::as_str) else {
        print_import_usage();
        return Ok(());
    };
    if is_help(format) {
        print_import_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    match format {
        "bundle" => run_import_bundle(&args[1..], &registry_root),
        "oci-layout" => run_import_oci_layout(&args[1..], &registry_root),
        _ => Err(CliError::new(format!(
            "unknown import format `{format}`; expected `bundle` or `oci-layout`"
        ))),
    }
}

fn run_import_bundle(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_import_bundle_usage();
        return Ok(());
    }

    let source_root = PathBuf::from(&args[0]);
    let mut json_output = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild import bundle`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let installed = LocalRegistry::import_bundle(registry_root, &source_root)?;
    let output = ImportCommandOutput {
        format: "bundle",
        registry_root: registry_root.display().to_string(),
        installed: installed
            .iter()
            .map(|skill| summarize_installed_skill(skill, registry_root))
            .collect(),
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_import_text(&output);
    }

    Ok(())
}

fn run_import_oci_layout(args: &[String], registry_root: &Path) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_import_oci_layout_usage();
        return Ok(());
    }

    let source_root = PathBuf::from(&args[0]);
    let mut json_output = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild import oci-layout`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let installed = LocalRegistry::import_oci_layout(registry_root, &source_root)?;
    let output = ImportCommandOutput {
        format: "oci-layout",
        registry_root: registry_root.display().to_string(),
        installed: installed
            .iter()
            .map(|skill| summarize_installed_skill(skill, registry_root))
            .collect(),
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_import_text(&output);
    }

    Ok(())
}

fn run_push(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_push_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let registry = LocalRegistry::load(&registry_root)?;
    let root = resolve_installed_skill(&registry, &args[0])?;
    let mut signer = None;
    let mut reference = None;
    let mut include_dependencies = false;
    let mut allow_http = false;
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--signer" => {
                signer = Some(PathBuf::from(next_value(args, &mut index, "--signer")?));
            }
            "--reference" => {
                reference = Some(parse_oci_reference(next_value(
                    args,
                    &mut index,
                    "--reference",
                )?)?);
            }
            "--include-dependencies" => include_dependencies = true,
            "--allow-http" => allow_http = true,
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild push`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let signer = load_signer_identity(signer.as_deref())?;
    let reference =
        reference.ok_or_else(|| CliError::new("`guild push` requires --reference <oci-ref>"))?;
    let published = registry.push_oci_registry(
        &root.resolved_ref,
        include_dependencies,
        &reference,
        &oci_transport_options(allow_http),
        &signer,
    )?;

    let output = PushCommandOutput {
        reference: published.reference.to_string(),
        manifest_digest: published.manifest_digest,
        root_skill: format_resolved_skill_ref(&published.bundle.root_skill),
        publisher_id: signer.publisher.id,
        includes_dependency_closure: include_dependencies,
    };

    if json_output {
        print_json(&output)?;
    } else {
        println!("pushed {} to {}", output.root_skill, output.reference);
        println!("manifest: {}", output.manifest_digest);
    }

    Ok(())
}

fn run_pull(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_pull_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let reference = parse_oci_reference(&args[0])?;
    let mut allow_http = false;
    let mut json_output = false;
    let mut index = 1;

    while index < args.len() {
        match args[index].as_str() {
            "--allow-http" => allow_http = true,
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild pull`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let installed = LocalRegistry::pull_oci_registry(
        &registry_root,
        &reference,
        &oci_transport_options(allow_http),
    )?;
    let output = ImportCommandOutput {
        format: "oci-registry",
        registry_root: registry_root.display().to_string(),
        installed: installed
            .iter()
            .map(|skill| summarize_installed_skill(skill, &registry_root))
            .collect(),
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_import_text(&output);
    }

    Ok(())
}

fn run_trust(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        print_trust_usage();
        return Ok(());
    };
    if is_help(command) {
        print_trust_usage();
        return Ok(());
    }

    match command {
        "generate" => run_trust_generate(&args[1..]),
        "add" => run_trust_add(&args[1..], global, env_registry_root),
        "list" => run_trust_list(&args[1..], global, env_registry_root),
        "remove" => run_trust_remove(&args[1..], global, env_registry_root),
        _ => Err(CliError::new(format!(
            "unknown trust subcommand `{command}`"
        ))),
    }
}

fn run_trust_generate(args: &[String]) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_trust_generate_usage();
        return Ok(());
    }

    let mut publisher_id = None;
    let mut display_name = None;
    let mut homepage = None;
    let mut output = None;
    let mut json_output = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--publisher-id" => {
                publisher_id = Some(next_value(args, &mut index, "--publisher-id")?.to_owned());
            }
            "--display-name" => {
                display_name = Some(next_value(args, &mut index, "--display-name")?.to_owned());
            }
            "--homepage" => {
                homepage = Some(next_value(args, &mut index, "--homepage")?.to_owned());
            }
            "--output" => {
                output = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild trust generate`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let publisher_id = publisher_id.ok_or_else(|| {
        CliError::new("`guild trust generate` requires --publisher-id <publisher-id>")
    })?;
    let display_name = display_name.ok_or_else(|| {
        CliError::new("`guild trust generate` requires --display-name <display-name>")
    })?;
    let output = output
        .ok_or_else(|| CliError::new("`guild trust generate` requires --output <identity.json>"))?;

    let identity = LocalPublisherIdentity::generate(PublisherRef {
        id: publisher_id.clone(),
        display_name,
        homepage,
    })?;
    identity.save(&output)?;

    let payload = TrustGenerateOutput {
        publisher_id,
        output_path: output.display().to_string(),
    };
    if json_output {
        print_json(&payload)?;
    } else {
        println!(
            "wrote publisher identity {} to {}",
            payload.publisher_id, payload.output_path
        );
    }
    Ok(())
}

fn run_trust_add(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_trust_add_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let mut identity_file = None;
    let mut record_file = None;
    let mut tier = LocalTrustTier::TrustedImported;
    let mut json_output = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--identity-file" => {
                identity_file = Some(PathBuf::from(next_value(
                    args,
                    &mut index,
                    "--identity-file",
                )?));
            }
            "--record-file" => {
                record_file = Some(PathBuf::from(next_value(
                    args,
                    &mut index,
                    "--record-file",
                )?));
            }
            "--tier" => {
                tier = next_value(args, &mut index, "--tier")?
                    .parse::<LocalTrustTier>()
                    .map_err(|error| CliError::new(error.to_string()))?;
            }
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild trust add`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    if identity_file.is_some() == record_file.is_some() {
        return Err(CliError::new(
            "`guild trust add` requires exactly one of --identity-file or --record-file",
        ));
    }

    if tier == LocalTrustTier::LocalDev {
        return Err(CliError::new(
            "`guild trust add` only accepts trusted-imported or restricted tiers",
        ));
    }

    let publisher = if let Some(path) = identity_file {
        LocalPublisherIdentity::load(path)?.trusted_record_with_tier(tier.clone())
    } else {
        let path = record_file.expect("validated above");
        let mut record: TrustedPublisherRecord =
            serde_json::from_str(&fs::read_to_string(path).map_err(CliError::from)?)?;
        record.trust_tier = tier.clone();
        record
    };

    LocalRegistry::trust_publisher(&registry_root, &publisher)?;

    let output = TrustAddOutput {
        publisher_id: publisher.publisher.id,
        trust_tier: publisher.trust_tier,
        registry_root: registry_root.display().to_string(),
    };

    if json_output {
        print_json(&output)?;
    } else {
        println!(
            "trusted publisher {} as {}",
            output.publisher_id, output.trust_tier
        );
    }

    Ok(())
}

fn run_trust_list(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let registry_root = require_registry_root(global, env_registry_root)?;
    let mut json_output = false;

    for argument in args {
        match argument.as_str() {
            "--json" => json_output = true,
            "--help" | "-h" => {
                print_trust_list_usage();
                return Ok(());
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild trust list`: `{other}`"
                )));
            }
        }
    }

    let output = TrustListOutput {
        registry_root: registry_root.display().to_string(),
        publishers: LocalRegistry::list_trusted_publishers(&registry_root)?,
    };

    if json_output {
        print_json(&output)?;
    } else if output.publishers.is_empty() {
        println!("no trusted publishers configured");
    } else {
        for publisher in &output.publishers {
            println!("{} ({})", publisher.publisher.id, publisher.trust_tier);
        }
    }

    Ok(())
}

fn run_trust_remove(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_trust_remove_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let publisher_id = args[0].clone();
    if args.len() > 1 {
        return Err(CliError::new(
            "`guild trust remove` accepts only a publisher id",
        ));
    }

    let removed = LocalRegistry::remove_trusted_publisher(&registry_root, &publisher_id)?;
    if !removed {
        return Err(CliError::new(format!(
            "trusted publisher `{publisher_id}` was not present"
        )));
    }

    println!("removed trusted publisher {publisher_id}");
    Ok(())
}

fn run_codex(args: &[String], global: &GlobalOptions) -> Result<(), CliError> {
    crate::codex_cli::run_guild_subcommand(args, global.registry_root.clone())
        .map_err(|error| CliError::new(error.to_string()))
}

fn run_mcp(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let Some(command) = args.first().map(String::as_str) else {
        print_mcp_usage();
        return Ok(());
    };
    if is_help(command) {
        print_mcp_usage();
        return Ok(());
    }

    match command {
        "serve" => run_mcp_serve(&args[1..], global, env_registry_root),
        _ => Err(CliError::new(format!("unknown mcp subcommand `{command}`"))),
    }
}

fn run_mcp_serve(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_mcp_serve_usage();
        return Ok(());
    }

    let registry_root = require_registry_root(global, env_registry_root)?;
    let mut stdio = false;

    for argument in args {
        match argument.as_str() {
            "--stdio" => stdio = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild mcp serve`: `{other}`"
                )));
            }
        }
    }

    if !stdio {
        return Err(CliError::new(
            "`guild mcp serve` currently requires --stdio",
        ));
    }

    GuildMcpServer::load(&registry_root)?.serve_stdio()?;
    Ok(())
}

fn require_registry_root(
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<PathBuf, CliError> {
    if let Some(path) = &global.registry_root {
        return Ok(path.clone());
    }

    if let Some(path) = env_registry_root {
        return Ok(PathBuf::from(path));
    }

    Err(CliError::new(format!(
        "missing registry root; pass `--registry-root <path>` or set `GUILD_REGISTRY_ROOT`\nthere is no implicit `.guild/` or `target/dev-local-registry/...` root\nexample: {CLI_BINARY_NAME} --registry-root target/dev-local-registry inspect skill://example/hello-inspect@^0.1 --input-json '{{}}'\nexample: export GUILD_REGISTRY_ROOT=target/dev-local-registry"
    )))
}

fn build_facade(
    registry_root: &Path,
) -> Result<GuildMcpFacade<LocalRegistry, WasmtimeRuntimeAdapter>, CliError> {
    let registry = LocalRegistry::load(registry_root)?;
    let runtime = WasmtimeRuntimeAdapter::new()
        .map_err(McpError::from)
        .map_err(|error| CliError::new(format!("{}: {}", error.code, error.message)))?;
    Ok(GuildMcpFacade::new(registry, runtime))
}

fn parse_skill_ref(input: &str) -> Result<RequestedSkillRef, CliError> {
    input
        .parse::<RequestedSkillRef>()
        .map_err(|error| CliError::new(error.to_string()))
}

fn resolve_installed_skill(
    registry: &LocalRegistry,
    skill: &str,
) -> Result<InstalledSkill, CliError> {
    registry
        .resolve(&parse_skill_ref(skill)?)
        .map_err(CliError::from)
}

fn parse_oci_reference(input: &str) -> Result<OciRegistryReference, CliError> {
    input
        .parse::<OciRegistryReference>()
        .map_err(CliError::from)
}

fn load_signer_identity(path: Option<&Path>) -> Result<LocalPublisherIdentity, CliError> {
    let Some(path) = path else {
        return Err(CliError::new("missing required --signer <identity.json>"));
    };
    LocalPublisherIdentity::load(path).map_err(CliError::from)
}

fn read_json_input(inline: Option<&str>, file: Option<&Path>) -> Result<Option<Value>, CliError> {
    read_json_value(inline, file)
}

fn read_json_value(inline: Option<&str>, file: Option<&Path>) -> Result<Option<Value>, CliError> {
    if let Some(inline) = inline {
        return serde_json::from_str(inline)
            .map(Some)
            .map_err(CliError::from);
    }

    if let Some(file) = file {
        return serde_json::from_str(&fs::read_to_string(file)?)
            .map(Some)
            .map_err(CliError::from);
    }

    Ok(None)
}

fn parse_capability_grants(value: Value) -> Result<CapabilityGrantSet, CliError> {
    serde_json::from_value(value).map_err(CliError::from)
}

fn cli_error_from_mcp(error: McpError) -> CliError {
    let mut message = format!("{}: {}", error.code, error.message);
    if let Some(receipt) = error.receipt {
        let _ = write!(
            message,
            " (execution: {}, status: {})",
            receipt.uri,
            status_label(&receipt.status)
        );
    }
    CliError::new(message)
}

fn oci_transport_options(allow_http: bool) -> OciRegistryTransportOptions {
    OciRegistryTransportOptions {
        allow_http,
        ..OciRegistryTransportOptions::default()
    }
}

fn summarize_installed_skill(skill: &InstalledSkill, registry_root: &Path) -> InstalledSkillOutput {
    InstalledSkillOutput {
        resolved_skill: format_resolved_skill_ref(&skill.resolved_ref),
        digest: skill.resolved_ref.digest.clone(),
        registry_root: registry_root.display().to_string(),
        root_dir: skill.root_dir.display().to_string(),
        manifest_path: skill.manifest_path.display().to_string(),
        artifact_path: skill.artifact_path.display().to_string(),
        trust: skill.trust.clone(),
        verification: skill.verification.clone(),
    }
}

fn summarize_listed_installed_skills(skills: &[InstalledSkill]) -> Vec<ListedInstalledSkillOutput> {
    skills
        .iter()
        .map(summarize_listed_installed_skill)
        .collect()
}

fn summarize_listed_installed_skill(skill: &InstalledSkill) -> ListedInstalledSkillOutput {
    ListedInstalledSkillOutput {
        resolved_skill: format_resolved_skill_ref(&skill.resolved_ref),
        digest: skill.resolved_ref.digest.clone(),
        trust_tier: skill.trust.trust_tier.clone(),
        verification_state: skill.trust.verification_state.clone(),
    }
}

fn summarize_listed_executions(records: &[ExecutionRecord]) -> Vec<ListedExecutionOutput> {
    records.iter().map(summarize_listed_execution).collect()
}

fn summarize_listed_execution(record: &ExecutionRecord) -> ListedExecutionOutput {
    ListedExecutionOutput {
        execution_id: record.receipt.execution_id.clone(),
        uri: record.receipt.uri.clone(),
        status: record.status.clone(),
        resolved_skill: format_resolved_skill_ref(&record.resolved_skill),
        started_at_utc: record.provenance.started_at_utc.clone(),
        finished_at_utc: record.provenance.finished_at_utc.clone(),
    }
}

fn format_resolved_skill_ref(skill: &guild_types::ResolvedSkillRef) -> String {
    format!(
        "skill://{}/{}@{}",
        skill.key.namespace, skill.key.name, skill.version
    )
}

fn status_label(status: &ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Succeeded => "succeeded",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::Partial => "partial",
        ExecutionStatus::Rejected => "rejected",
    }
}

fn next_value<'a>(args: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str, CliError> {
    let value_index = *index + 1;
    let Some(value) = args.get(value_index) else {
        return Err(CliError::new(format!("{flag} requires a following value")));
    };
    *index = value_index;
    Ok(value)
}

fn is_help(argument: &str) -> bool {
    matches!(argument, "--help" | "-h")
}

fn print_json(value: &impl Serialize) -> Result<(), CliError> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn print_inspect_text(response: &InspectResponse) {
    println!("{}", response.summary);
    println!("execution: {}", response.structured_content.receipt.uri);
    println!(
        "status: {}",
        status_label(&response.structured_content.status)
    );
    if !response.structured_content.emitted_evidence.is_empty() {
        println!("evidence:");
        for evidence in &response.structured_content.emitted_evidence {
            println!("  {}", evidence.uri);
        }
    }
}

fn print_read_text(resource: &ResourceReadResult) -> Result<(), CliError> {
    match String::from_utf8(resource.bytes.clone()) {
        Ok(text) => {
            print!("{text}");
            Ok(())
        }
        Err(_) => Err(CliError::new(
            "resource bytes were not valid UTF-8; use --output <path> or --json",
        )),
    }
}

fn print_import_text(output: &ImportCommandOutput) {
    if output.installed.is_empty() {
        println!("no installed skills were imported");
        return;
    }

    for skill in &output.installed {
        println!("installed {}", skill.resolved_skill);
    }
}

fn print_list_summary_text(output: &ListSummaryOutput) {
    print_list_skills_lines(&output.installed, output.installed_count);
    println!();
    print_list_execution_lines(
        &output.recent_executions,
        output.recent_execution_count,
        Some(output.recent_execution_limit),
    );
}

fn print_list_skills_text(output: &ListSkillsOutput) {
    print_list_skills_lines(&output.installed, output.installed_count);
}

fn print_list_executions_text(output: &ListExecutionsOutput) {
    print_list_execution_lines(
        &output.executions,
        output.execution_count,
        Some(output.limit),
    );
}

fn print_list_skills_lines(skills: &[ListedInstalledSkillOutput], installed_count: usize) {
    println!("installed skills ({installed_count}):");
    if skills.is_empty() {
        println!("  none");
        return;
    }

    for skill in skills {
        println!("  {}", skill.resolved_skill);
        println!("    digest: {}", skill.digest);
        println!(
            "    trust: {} / {}",
            skill.trust_tier,
            verification_state_label(&skill.verification_state)
        );
    }
}

fn print_list_execution_lines(
    executions: &[ListedExecutionOutput],
    execution_count: usize,
    limit: Option<usize>,
) {
    match limit {
        Some(limit) => println!("recent executions ({execution_count}, limit {limit}):"),
        None => println!("recent executions ({execution_count}):"),
    }

    if executions.is_empty() {
        println!("  none");
        return;
    }

    for execution in executions {
        println!(
            "  {}  {}",
            status_label(&execution.status),
            execution.resolved_skill
        );
        println!("    execution: {}", execution.uri);
        if let Some(started_at) = &execution.started_at_utc {
            println!("    started: {started_at}");
        }
        if let Some(finished_at) = &execution.finished_at_utc {
            println!("    finished: {finished_at}");
        }
    }
}

fn verification_state_label(state: &InstalledVerificationState) -> &'static str {
    match state {
        InstalledVerificationState::LocalSource => "local-source",
        InstalledVerificationState::VerifiedImport => "verified-import",
    }
}

fn print_usage() {
    println!("usage: guild [--registry-root <path>] <command> [options]");
    println!();
    println!("commands:");
    println!("  inspect      execute a skill through the local inspect path");
    println!("  read         read a Guild resource URI");
    println!("  list         list installed skills and recent persisted executions");
    println!("  install      install a source skill into a Guild root");
    println!("  export       export installed state as a signed bundle or OCI layout");
    println!("  import       import a signed bundle or OCI layout into a Guild root");
    println!("  push         publish installed state to an OCI registry");
    println!("  pull         pull and import installed state from an OCI registry");
    println!("  trust        manage local publisher identities and trust records");
    println!("  codex        bootstrap and smoke the real Codex stdio workflow");
    println!("  mcp          launch the existing Guild MCP stdio server");
    println!();
    println!(
        "registry roots are explicit: `--registry-root` wins, then `GUILD_REGISTRY_ROOT`; there is no implicit `.guild/` or `target/dev-local-registry/...` fallback."
    );
    println!(
        "canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` is accepted as operator convenience."
    );
    println!("`guild trust ...` manages local trust-store state only.");
    println!("deferred: `guild build` and `guild deploy` are intentionally not implemented.");
}

fn print_inspect_usage() {
    println!(
        "usage: guild [--registry-root <path>] inspect <skill-ref> [--input-json <json> | --input-file <path>] [--grants-json <json> | --grants-file <path>] [--tenant-id <id>] [--actor-id <id>] [--json]"
    );
    println!(
        "note: canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` is accepted as convenience."
    );
}

fn print_read_usage() {
    println!("usage: guild [--registry-root <path>] read <guild-uri> [--output <path>] [--json]");
}

fn print_list_usage() {
    println!("usage: guild [--registry-root <path>] list [skills|executions] [options]");
    println!();
    println!("`guild list` prints a summary of installed skills plus recent persisted executions.");
    println!("`guild list skills` shows installed skills only.");
    println!(
        "`guild list executions` shows recent persisted execution activity; Guild does not currently expose a live loaded-runtime module registry."
    );
}

fn print_list_skills_usage() {
    println!("usage: guild [--registry-root <path>] list skills [--json]");
}

fn print_list_executions_usage() {
    println!("usage: guild [--registry-root <path>] list executions [--limit <n>] [--json]");
}

fn print_install_usage() {
    println!("usage: guild [--registry-root <path>] install <source-dir> [--json]");
}

fn print_export_usage() {
    println!("usage: guild [--registry-root <path>] export <bundle|oci-layout> ...");
}

fn print_export_bundle_usage() {
    println!(
        "usage: guild [--registry-root <path>] export bundle <skill-ref> --signer <identity.json> --output <dir> [--include-dependencies] [--json]"
    );
    println!(
        "note: canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` is accepted as convenience."
    );
}

fn print_export_oci_layout_usage() {
    println!(
        "usage: guild [--registry-root <path>] export oci-layout <skill-ref> --signer <identity.json> --output <dir> [--include-dependencies] [--json]"
    );
    println!(
        "note: canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` is accepted as convenience."
    );
}

fn print_import_usage() {
    println!("usage: guild [--registry-root <path>] import <bundle|oci-layout> ...");
}

fn print_import_bundle_usage() {
    println!("usage: guild [--registry-root <path>] import bundle <dir> [--json]");
}

fn print_import_oci_layout_usage() {
    println!("usage: guild [--registry-root <path>] import oci-layout <dir> [--json]");
}

fn print_push_usage() {
    println!(
        "usage: guild [--registry-root <path>] push <skill-ref> --reference <oci-ref> --signer <identity.json> [--include-dependencies] [--allow-http] [--json]"
    );
    println!(
        "note: canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` is accepted as convenience."
    );
}

fn print_pull_usage() {
    println!("usage: guild [--registry-root <path>] pull <oci-ref> [--allow-http] [--json]");
}

fn print_trust_usage() {
    println!("usage: guild [--registry-root <path>] trust <generate|add|list|remove> ...");
    println!("note: `guild trust ...` manages the local trust store only.");
}

fn print_trust_generate_usage() {
    println!(
        "usage: guild trust generate --publisher-id <id> --display-name <name> [--homepage <url>] --output <identity.json> [--json]"
    );
}

fn print_trust_add_usage() {
    println!(
        "usage: guild [--registry-root <path>] trust add (--identity-file <path> | --record-file <path>) [--tier trusted-imported|restricted] [--json]"
    );
}

fn print_trust_list_usage() {
    println!("usage: guild [--registry-root <path>] trust list [--json]");
}

fn print_trust_remove_usage() {
    println!("usage: guild [--registry-root <path>] trust remove <publisher-id>");
}

fn print_mcp_usage() {
    println!("usage: guild [--registry-root <path>] mcp serve --stdio");
}

fn print_mcp_serve_usage() {
    println!("usage: guild [--registry-root <path>] mcp serve --stdio");
}
