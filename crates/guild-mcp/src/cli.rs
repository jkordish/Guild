use std::fmt::Write as _;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};

use base64::Engine as _;
use guild_manifest::{PublisherRef, SkillManifest};
use guild_registry::{
    ExecutionPlanSignatureEnvelope, ExecutionPlanVerification, InstalledSkill,
    InstalledTrustMetadata, InstalledVerificationRecord, LocalPublisherIdentity, LocalRegistry,
    LocalSourceInstaller, OciRegistryReference, OciRegistryTransportOptions, RegistryError,
    SkillRegistry, StructuredDigest, TrustedPublisherRecord, sign_execution_plan,
    verify_execution_plan,
};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityGrantSet, EvidenceBlobRecord, EvidenceRecord, ExecutionRecord, ExecutionStatus,
    GuildResourceUri, InstalledVerificationState, LocalTrustTier, RequestedSkillRef,
    ResourceReadResult, SkillKey, VersionRequirement,
};
use serde::Serialize;
use serde_json::Value;

use crate::cli_presenter::{
    PresentationOptions, StreamKind, SupportSummary, WhySummary, color_mode, render_evidence_list,
    render_evidence_show, render_execution_show, render_execution_why, render_object_show,
    render_objects_list, render_run_porcelain, render_run_status, render_runs_list,
    render_skill_porcelain, render_skill_show, render_skill_verify, render_skills_list,
    render_verify_porcelain, render_why_porcelain,
    resolved_skill_ref as presenter_resolved_skill_ref, runtime_label as presenter_runtime_label,
    short_execution_ref, support_summary_for_execution, support_summary_for_skill, why_summary,
};
use crate::codex::{
    CodexConfigWriteResult, CodexServerConfig, DEFAULT_CODEX_SERVER_NAME,
    installed_guild_server_config, running_guild_binary, write_codex_config,
};
use crate::codex_cli::{print_setup_details, project_codex_config_path};
use crate::paths;
use crate::server::{GuildMcpServer, ServerStartupError};
use crate::{CLI_BINARY_NAME, GuildMcpFacade, InspectRequest, McpError};

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

#[derive(Debug, Clone, Default)]
struct RenderFlags {
    json_output: bool,
    porcelain_output: bool,
    verbosity: u8,
    debug: bool,
    color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct InitCodexOutput {
    guild_binary: String,
    config: CodexServerConfig,
    writes: Vec<CodexConfigWriteResult>,
}

#[derive(Debug, Clone, Serialize)]
struct InitCommandOutput {
    registry_root: String,
    created_registry_root: bool,
    codex: InitCodexOutput,
}

#[derive(Debug, Clone, Serialize)]
struct InspectCommandOutput {
    summary: String,
    record: ExecutionRecord,
}

#[derive(Debug, Clone, Serialize)]
struct ShowSkillCommandOutput {
    requested_ref: String,
    resolved_skill: String,
    display_name: String,
    description: String,
    runtime: String,
    support: SupportSummary,
    trust: InstalledTrustMetadata,
    verification: Option<InstalledVerificationRecord>,
    manifest: SkillManifest,
}

#[derive(Debug, Clone, Serialize)]
struct ShowExecutionCommandOutput {
    summary: String,
    support: SupportSummary,
    record: ExecutionRecord,
}

#[derive(Debug, Clone, Serialize)]
struct ShowEvidenceCommandOutput {
    record: EvidenceRecord,
}

#[derive(Debug, Clone, Serialize)]
struct ShowObjectCommandOutput {
    record: EvidenceBlobRecord,
}

#[derive(Debug, Clone, Serialize)]
struct WhyCommandOutput {
    summary: WhySummary,
    record: ExecutionRecord,
}

#[derive(Debug, Clone, Serialize)]
struct VerifySkillCommandOutput {
    requested_ref: String,
    resolved_skill: String,
    trust: InstalledTrustMetadata,
    verification: Option<InstalledVerificationRecord>,
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
struct ListedEvidenceOutput {
    uri: String,
    produced_by_execution: Option<String>,
    mime_type: String,
    sha256: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ListedObjectOutput {
    uri: String,
    sha256: String,
    size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ListEvidenceOutput {
    registry_root: String,
    limit: usize,
    evidence_count: usize,
    evidence: Vec<ListedEvidenceOutput>,
}

#[derive(Debug, Clone, Serialize)]
struct ListObjectsOutput {
    registry_root: String,
    limit: usize,
    object_count: usize,
    objects: Vec<ListedObjectOutput>,
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

#[derive(Debug, Clone, Serialize)]
struct TrustSignPlanOutput {
    publisher_id: String,
    output_path: String,
    signed_digest: StructuredDigest,
}

#[derive(Debug, Clone, Serialize)]
struct TrustVerifyPlanOutput {
    verified: bool,
    publisher_id: String,
    trust_tier: LocalTrustTier,
    registry_root: String,
    signed_digest: StructuredDigest,
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
enum ShowTarget {
    Skill {
        requested: String,
        installed: InstalledSkill,
    },
    Execution(ExecutionRecord),
    Evidence(EvidenceRecord),
    Object(EvidenceBlobRecord),
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
        "show" => run_show(&args[1..], &global, env_registry_root),
        "run" | "inspect" => run_run_command(&args[1..], &global, env_registry_root),
        "ls" | "list" => run_ls(&args[1..], &global, env_registry_root),
        "get" | "read" => run_get(&args[1..], &global, env_registry_root),
        "why" => run_why(&args[1..], &global, env_registry_root),
        "verify" => run_verify(&args[1..], &global, env_registry_root),
        "init" => run_init(&args[1..], &global, env_registry_root),
        "install" => run_install(&args[1..], &global, env_registry_root),
        "export" => run_export(&args[1..], &global, env_registry_root),
        "import" => run_import(&args[1..], &global, env_registry_root),
        "push" => run_push(&args[1..], &global, env_registry_root),
        "pull" => run_pull(&args[1..], &global, env_registry_root),
        "trust" => run_trust(&args[1..], &global, env_registry_root),
        "codex" => run_codex(&args[1..], &global, env_registry_root),
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

fn run_show(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_show_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
    let (render, positional) = parse_render_flags(args)?;
    if positional.len() != 1 {
        return Err(CliError::new("`guild show` requires exactly one ref"));
    }

    let target = resolve_show_target(&registry, &positional[0])?;
    if render.json_output {
        match target {
            ShowTarget::Skill {
                requested,
                installed,
            } => {
                print_json(&ShowSkillCommandOutput {
                    requested_ref: requested,
                    resolved_skill: presenter_resolved_skill_ref(&installed.resolved_ref),
                    display_name: installed.manifest.display_name.clone(),
                    description: installed.manifest.description.clone(),
                    runtime: presenter_runtime_label(&installed.manifest),
                    support: support_summary_for_skill(&installed),
                    trust: installed.trust.clone(),
                    verification: installed.verification.clone(),
                    manifest: installed.manifest.clone(),
                })?;
            }
            ShowTarget::Execution(record) => {
                print_json(&ShowExecutionCommandOutput {
                    summary: record.output.as_ref().map_or_else(
                        || record.policy_decision.summary.clone(),
                        |output| output.summary.clone(),
                    ),
                    support: support_summary_for_execution(&record),
                    record,
                })?;
            }
            ShowTarget::Evidence(record) => print_json(&ShowEvidenceCommandOutput { record })?,
            ShowTarget::Object(record) => print_json(&ShowObjectCommandOutput { record })?,
        }
        return Ok(());
    }

    if render.porcelain_output {
        let line = match target {
            ShowTarget::Skill { installed, .. } => render_skill_porcelain(&installed),
            ShowTarget::Execution(record) => format!(
                "show\texec\t{}\t{}\t{}",
                record.receipt.execution_id,
                status_label(&record.status),
                short_execution_ref(&record)
            ),
            ShowTarget::Evidence(record) => format!(
                "show\tevidence\t{}\t{}\t{}",
                record.uri, record.mime_type, record.size_bytes
            ),
            ShowTarget::Object(record) => {
                format!("show\tobject\t{}\t{}", record.sha256, record.size_bytes)
            }
        };
        println!("{line}");
        return Ok(());
    }

    let presentation = presentation_options(&render);
    let text = match target {
        ShowTarget::Skill {
            requested,
            installed,
        } => render_skill_show(&installed, &requested, presentation, StreamKind::Stdout),
        ShowTarget::Execution(record) => {
            render_execution_show(&record, presentation, StreamKind::Stdout)
        }
        ShowTarget::Evidence(record) => {
            render_evidence_show(&record, presentation, StreamKind::Stdout)
        }
        ShowTarget::Object(record) => render_object_show(&record, presentation, StreamKind::Stdout),
    };
    print!("{text}");
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn run_run_command(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_run_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let mut render = RenderFlags::default();
    let mut skill_ref = None;
    let mut positional_input = None;
    let mut input_json = None;
    let mut input_file = None;
    let mut grants_json = None;
    let mut grants_file = None;
    let mut tenant_id = DEFAULT_TENANT_ID.to_owned();
    let mut actor_id = DEFAULT_ACTOR_ID.to_owned();
    let mut index = 0;

    while index < args.len() {
        if consume_render_flag(args, &mut index, &mut render)? {
            index += 1;
            continue;
        }

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
            other if skill_ref.is_none() => {
                skill_ref = Some(other.to_owned());
            }
            other if positional_input.is_none() => {
                positional_input = Some(PathBuf::from(other));
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild run`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    validate_render_flags(&render)?;
    let requested = skill_ref.ok_or_else(|| CliError::new("`guild run` requires a skill ref"))?;
    let registry = build_existing_registry(&registry_root)?;
    let skill = resolve_requested_skill_ref(&registry, &requested)?;

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

    let input = if let Some(path) = positional_input.as_deref() {
        positional_input_value(path)?
    } else {
        read_json_input(input_json.as_deref(), input_file.as_deref())?
            .unwrap_or_else(|| serde_json::json!({}))
    };
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

    if render.json_output {
        print_json(&output)?;
        return Ok(());
    }

    let payload = run_payload_text(&response.structured_content)?;
    if !payload.is_empty() {
        print!("{payload}");
        if !payload.ends_with('\n') {
            println!();
        }
    }

    let presentation = presentation_options(&render);
    let status = if render.porcelain_output {
        render_run_porcelain(&response.structured_content)
    } else {
        render_run_status(
            &response.structured_content,
            presentation,
            StreamKind::Stderr,
        )
    };
    eprintln!("{status}");
    Ok(())
}

fn run_ls(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_ls_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
    let (render, positional, limit) = parse_ls_args(args)?;
    match positional.first().map(String::as_str) {
        Some("skills") => run_ls_skills(&registry, &registry_root, &render),
        Some("runs" | "executions") => run_ls_runs(&registry, &registry_root, &render, limit),
        Some("objects") => run_ls_objects(&registry, &registry_root, &render, limit),
        Some("evidence") => run_ls_evidence(&registry, &registry_root, &render, limit),
        None => run_ls_summary(&registry, &registry_root, &render),
        Some(other) => Err(CliError::new(format!("unknown ls category `{other}`"))),
    }
}

fn run_get(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_get_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
    let mut render = RenderFlags::default();
    let mut output_path = None;
    let mut ref_input = None;
    let mut index = 0;

    while index < args.len() {
        if consume_render_flag(args, &mut index, &mut render)? {
            index += 1;
            continue;
        }
        match args[index].as_str() {
            "--output" => {
                output_path = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            other if ref_input.is_none() => ref_input = Some(other.to_owned()),
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild get`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    validate_render_flags(&render)?;
    let ref_input = ref_input.ok_or_else(|| CliError::new("`guild get` requires a ref"))?;
    let uri = resolve_resource_ref(&registry, &ref_input)?;
    let resource = registry.read_resource(&uri)?;

    if let Some(path) = output_path {
        fs::write(&path, &resource.bytes)?;
        if render.json_output {
            let output = ReadCommandOutput {
                uri: resource.uri,
                mime_type: resource.mime_type,
                sha256: resource.sha256,
                text: None,
                bytes_base64: None,
                output_path: Some(path.display().to_string()),
            };
            print_json(&output)?;
        } else if render.porcelain_output {
            println!("get\t{}\t{}", uri, path.display());
        } else {
            println!("wrote {} to {}", uri, path.display());
        }
        return Ok(());
    }

    if render.json_output {
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
    } else if render.porcelain_output {
        println!(
            "get\t{}\t{}\t{}",
            resource.uri,
            resource.mime_type,
            resource.bytes.len()
        );
    } else {
        print_read_text(&resource)?;
    }
    Ok(())
}

fn run_why(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_why_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
    let (render, positional) = parse_render_flags(args)?;
    if positional.len() != 1 {
        return Err(CliError::new(
            "`guild why` requires exactly one execution ref",
        ));
    }

    let uri = resolve_execution_ref(&registry, &positional[0])?;
    let execution_id = execution_id_from_uri(&uri)?;
    let record = registry.load_execution_record(&execution_id)?;

    if render.json_output {
        print_json(&WhyCommandOutput {
            summary: why_summary(&record),
            record,
        })?;
    } else if render.porcelain_output {
        println!("{}", render_why_porcelain(&record));
    } else {
        let presentation = presentation_options(&render);
        print!(
            "{}",
            render_execution_why(&record, presentation, StreamKind::Stdout)
        );
    }
    Ok(())
}

fn run_verify(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if args.is_empty() || is_help(args[0].as_str()) {
        print_verify_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
    let (render, positional) = parse_render_flags(args)?;
    if positional.len() != 1 {
        return Err(CliError::new(
            "`guild verify` requires exactly one skill ref",
        ));
    }

    let requested = positional[0].clone();
    let skill = resolve_requested_skill_ref(&registry, &requested)?;
    let installed = registry.resolve(&skill)?;

    if render.json_output {
        print_json(&VerifySkillCommandOutput {
            requested_ref: requested,
            resolved_skill: presenter_resolved_skill_ref(&installed.resolved_ref),
            trust: installed.trust.clone(),
            verification: installed.verification.clone(),
        })?;
    } else if render.porcelain_output {
        println!("{}", render_verify_porcelain(&installed));
    } else {
        let presentation = presentation_options(&render);
        print!(
            "{}",
            render_skill_verify(&installed, presentation, StreamKind::Stdout)
        );
    }
    Ok(())
}

fn parse_ls_args(args: &[String]) -> Result<(RenderFlags, Vec<String>, usize), CliError> {
    let mut render = RenderFlags::default();
    let mut positional = Vec::new();
    let mut limit = DEFAULT_LIST_EXECUTIONS_LIMIT;
    let mut index = 0;

    while index < args.len() {
        if consume_render_flag(args, &mut index, &mut render)? {
            index += 1;
            continue;
        }

        match args[index].as_str() {
            "--limit" => {
                let value = next_value(args, &mut index, "--limit")?;
                limit = value.parse::<usize>().map_err(|_| {
                    CliError::new(format!(
                        "invalid value for `--limit`: `{value}` is not a positive integer"
                    ))
                })?;
                if limit == 0 {
                    return Err(CliError::new("`guild ls` requires --limit > 0"));
                }
            }
            other => positional.push(other.to_owned()),
        }
        index += 1;
    }

    validate_render_flags(&render)?;
    Ok((render, positional, limit))
}

fn parse_render_flags(args: &[String]) -> Result<(RenderFlags, Vec<String>), CliError> {
    let mut render = RenderFlags::default();
    let mut positional = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if consume_render_flag(args, &mut index, &mut render)? {
            index += 1;
            continue;
        }
        positional.push(args[index].clone());
        index += 1;
    }
    validate_render_flags(&render)?;
    Ok((render, positional))
}

fn consume_render_flag(
    args: &[String],
    index: &mut usize,
    render: &mut RenderFlags,
) -> Result<bool, CliError> {
    let argument = args[*index].as_str();
    match argument {
        "--json" => {
            render.json_output = true;
            Ok(true)
        }
        "--porcelain" => {
            render.porcelain_output = true;
            Ok(true)
        }
        "--debug" => {
            render.debug = true;
            Ok(true)
        }
        "--color" => {
            render.color = Some(next_value(args, index, "--color")?.to_owned());
            Ok(true)
        }
        _ if argument.starts_with("--color=") => {
            render.color = Some(argument.trim_start_matches("--color=").to_owned());
            Ok(true)
        }
        _ if argument.starts_with("-v") && argument.chars().skip(1).all(|ch| ch == 'v') => {
            let extra_verbosity = u8::try_from(argument.len().saturating_sub(1)).unwrap_or(u8::MAX);
            render.verbosity = render.verbosity.saturating_add(extra_verbosity);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn validate_render_flags(render: &RenderFlags) -> Result<(), CliError> {
    if render.json_output && render.porcelain_output {
        return Err(CliError::new("use either --json or --porcelain, not both"));
    }
    if let Some(color) = render.color.as_deref()
        && !matches!(color, "auto" | "always" | "never")
    {
        return Err(CliError::new(format!(
            "unsupported --color mode `{color}`; expected auto, always, or never"
        )));
    }
    Ok(())
}

fn presentation_options(render: &RenderFlags) -> PresentationOptions {
    PresentationOptions {
        verbosity: render.verbosity,
        debug: render.debug,
        color: color_mode(
            std::env::var_os("NO_COLOR").is_some(),
            render.color.as_deref(),
        ),
        stdout_is_terminal: io::stdout().is_terminal(),
        stderr_is_terminal: io::stderr().is_terminal(),
    }
}

fn run_ls_summary(
    registry: &LocalRegistry,
    registry_root: &Path,
    render: &RenderFlags,
) -> Result<(), CliError> {
    let installed = registry.installed();
    let records = registry.list_recent_execution_records(DEFAULT_LIST_SUMMARY_EXECUTION_LIMIT)?;
    if render.json_output {
        let output = ListSummaryOutput {
            registry_root: registry_root.display().to_string(),
            installed_count: installed.len(),
            installed: summarize_listed_installed_skills(installed),
            recent_execution_limit: DEFAULT_LIST_SUMMARY_EXECUTION_LIMIT,
            recent_execution_count: records.len(),
            recent_executions: summarize_listed_executions(&records),
        };
        print_json(&output)
    } else if render.porcelain_output {
        for skill in installed {
            println!("{}", render_skill_porcelain(skill));
        }
        for record in &records {
            println!("{}", render_run_porcelain(record));
        }
        Ok(())
    } else {
        let presentation = presentation_options(render);
        println!("skills");
        print!(
            "{}",
            render_skills_list(installed, presentation, StreamKind::Stdout)
        );
        println!("runs");
        print!(
            "{}",
            render_runs_list(&records, presentation, StreamKind::Stdout)
        );
        Ok(())
    }
}

fn run_ls_skills(
    registry: &LocalRegistry,
    registry_root: &Path,
    render: &RenderFlags,
) -> Result<(), CliError> {
    let installed = registry.installed();
    if render.json_output {
        let output = ListSkillsOutput {
            registry_root: registry_root.display().to_string(),
            installed_count: installed.len(),
            installed: summarize_listed_installed_skills(installed),
        };
        print_json(&output)
    } else if render.porcelain_output {
        for skill in installed {
            println!("{}", render_skill_porcelain(skill));
        }
        Ok(())
    } else {
        let presentation = presentation_options(render);
        print!(
            "{}",
            render_skills_list(installed, presentation, StreamKind::Stdout)
        );
        Ok(())
    }
}

fn run_ls_runs(
    registry: &LocalRegistry,
    registry_root: &Path,
    render: &RenderFlags,
    limit: usize,
) -> Result<(), CliError> {
    let records = registry.list_recent_execution_records(limit)?;
    if render.json_output {
        let output = ListExecutionsOutput {
            registry_root: registry_root.display().to_string(),
            limit,
            execution_count: records.len(),
            executions: summarize_listed_executions(&records),
        };
        print_json(&output)
    } else if render.porcelain_output {
        for record in &records {
            println!("{}", render_run_porcelain(record));
        }
        Ok(())
    } else {
        let presentation = presentation_options(render);
        print!(
            "{}",
            render_runs_list(&records, presentation, StreamKind::Stdout)
        );
        Ok(())
    }
}

fn run_ls_evidence(
    registry: &LocalRegistry,
    registry_root: &Path,
    render: &RenderFlags,
    limit: usize,
) -> Result<(), CliError> {
    let records = registry.list_recent_evidence_records(limit)?;
    if render.json_output {
        let output = ListEvidenceOutput {
            registry_root: registry_root.display().to_string(),
            limit,
            evidence_count: records.len(),
            evidence: records
                .iter()
                .map(|record| ListedEvidenceOutput {
                    uri: record.uri.clone(),
                    produced_by_execution: record.produced_by_execution.clone(),
                    mime_type: record.mime_type.clone(),
                    sha256: record.sha256.clone(),
                    size_bytes: record.size_bytes,
                })
                .collect(),
        };
        print_json(&output)
    } else if render.porcelain_output {
        for record in &records {
            println!(
                "evidence\t{}\t{}\t{}",
                record.uri, record.mime_type, record.size_bytes
            );
        }
        Ok(())
    } else {
        let presentation = presentation_options(render);
        print!(
            "{}",
            render_evidence_list(&records, presentation, StreamKind::Stdout)
        );
        Ok(())
    }
}

fn run_ls_objects(
    registry: &LocalRegistry,
    registry_root: &Path,
    render: &RenderFlags,
    limit: usize,
) -> Result<(), CliError> {
    let records = registry.list_object_blobs(limit)?;
    if render.json_output {
        let output = ListObjectsOutput {
            registry_root: registry_root.display().to_string(),
            limit,
            object_count: records.len(),
            objects: records
                .iter()
                .map(|record| ListedObjectOutput {
                    uri: record.uri.clone(),
                    sha256: record.sha256.clone(),
                    size_bytes: record.size_bytes,
                })
                .collect(),
        };
        print_json(&output)
    } else if render.porcelain_output {
        for record in &records {
            println!("object\t{}\t{}", record.sha256, record.size_bytes);
        }
        Ok(())
    } else {
        let presentation = presentation_options(render);
        print!(
            "{}",
            render_objects_list(&records, presentation, StreamKind::Stdout)
        );
        Ok(())
    }
}

fn run_payload_text(record: &ExecutionRecord) -> Result<String, CliError> {
    let Some(output) = &record.output else {
        return Ok(String::new());
    };
    match &output.structured {
        Value::Null => Ok(String::new()),
        Value::String(text) => Ok(text.clone()),
        value => serde_json::to_string_pretty(value).map_err(CliError::from),
    }
}

fn positional_input_value(path: &Path) -> Result<Value, CliError> {
    let bytes = fs::read(path)?;
    let text = String::from_utf8(bytes).map_err(|_| {
        CliError::new("positional run input files must be valid UTF-8 or use --input-json")
    })?;
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => Ok(value),
        Err(_) => Ok(Value::String(text)),
    }
}

fn run_init(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_init_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let created_registry_root = !registry_root.exists();
    let mut codex_name = DEFAULT_CODEX_SERVER_NAME.to_owned();
    let mut global_config = false;
    let mut project_config = false;
    let mut json_output = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--name" => {
                next_value(args, &mut index, "--name")?.clone_into(&mut codex_name);
            }
            "--global" => global_config = true,
            "--project" => project_config = true,
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild init`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    LocalRegistry::load(&registry_root)?;

    let guild_binary = running_guild_binary().map_err(|error| CliError::new(error.to_string()))?;
    let config = installed_guild_server_config(&registry_root, codex_name, &guild_binary)
        .map_err(|error| CliError::new(error.to_string()))?;
    let mut writes = Vec::new();
    if global_config {
        let global_config_path =
            paths::global_codex_config_path().map_err(|error| CliError::new(error.to_string()))?;
        writes.push(
            write_codex_config(global_config_path, &config)
                .map_err(|error| CliError::new(error.to_string()))?,
        );
    }
    if project_config {
        writes.push(
            write_codex_config(
                project_codex_config_path().map_err(|error| CliError::new(error.to_string()))?,
                &config,
            )
            .map_err(|error| CliError::new(error.to_string()))?,
        );
    }

    let output = InitCommandOutput {
        registry_root: registry_root.display().to_string(),
        created_registry_root,
        codex: InitCodexOutput {
            guild_binary: guild_binary.display().to_string(),
            config,
            writes,
        },
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_init_text(&output);
    }

    Ok(())
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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

    let registry = build_existing_registry(registry_root)?;
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

    let registry = build_existing_registry(registry_root)?;
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    let registry = build_existing_registry(&registry_root)?;
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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
        "sign-plan" => run_trust_sign_plan(&args[1..]),
        "verify-plan" => run_trust_verify_plan(&args[1..], global, env_registry_root),
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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
    let registry_root = resolve_registry_root(global, env_registry_root)?;
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    ensure_existing_registry_root(&registry_root)?;
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

fn run_trust_sign_plan(args: &[String]) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_trust_sign_plan_usage();
        return Ok(());
    }

    let mut plan_path = None;
    let mut identity_file = None;
    let mut output_path = None;
    let mut json_output = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                plan_path = Some(PathBuf::from(next_value(args, &mut index, "--plan")?));
            }
            "--identity-file" => {
                identity_file = Some(PathBuf::from(next_value(
                    args,
                    &mut index,
                    "--identity-file",
                )?));
            }
            "--output" => {
                output_path = Some(PathBuf::from(next_value(args, &mut index, "--output")?));
            }
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild trust sign-plan`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let plan_path = plan_path
        .ok_or_else(|| CliError::new("`guild trust sign-plan` requires --plan <plan.json>"))?;
    let identity_file = identity_file.ok_or_else(|| {
        CliError::new("`guild trust sign-plan` requires --identity-file <identity.json>")
    })?;
    let output_path = output_path.ok_or_else(|| {
        CliError::new("`guild trust sign-plan` requires --output <signed-plan.json>")
    })?;

    let plan: Value =
        serde_json::from_str(&fs::read_to_string(&plan_path).map_err(CliError::from)?)?;
    let signer = LocalPublisherIdentity::load(&identity_file)?;
    let signed_plan = sign_execution_plan(&plan, &signer)?;
    let signature = parse_signed_plan_signature(&signed_plan)?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, serde_json::to_vec_pretty(&signed_plan)?)?;

    let output = TrustSignPlanOutput {
        publisher_id: signer.publisher.id,
        output_path: output_path.display().to_string(),
        signed_digest: signature.signed_digest,
    };

    if json_output {
        print_json(&output)?;
    } else {
        println!(
            "signed execution plan as {} to {}",
            output.publisher_id, output.output_path
        );
    }

    Ok(())
}

fn run_trust_verify_plan(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    if !args.is_empty() && is_help(args[0].as_str()) {
        print_trust_verify_plan_usage();
        return Ok(());
    }

    let registry_root = resolve_registry_root(global, env_registry_root)?;
    ensure_existing_registry_root(&registry_root)?;
    let mut plan_path = None;
    let mut json_output = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--plan" => {
                plan_path = Some(PathBuf::from(next_value(args, &mut index, "--plan")?));
            }
            "--json" => json_output = true,
            other => {
                return Err(CliError::new(format!(
                    "unexpected argument for `guild trust verify-plan`: `{other}`"
                )));
            }
        }
        index += 1;
    }

    let plan_path = plan_path.ok_or_else(|| {
        CliError::new("`guild trust verify-plan` requires --plan <signed-plan.json>")
    })?;
    let plan: Value =
        serde_json::from_str(&fs::read_to_string(&plan_path).map_err(CliError::from)?)?;
    let verification = verify_execution_plan(&registry_root, &plan)?;
    let output = trust_verify_output(&registry_root, verification);

    if json_output {
        print_json(&output)?;
    } else {
        println!(
            "verified execution plan signed by {} ({})",
            output.publisher_id, output.trust_tier
        );
    }

    Ok(())
}

fn run_codex(
    args: &[String],
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<(), CliError> {
    let default_registry_root = resolve_registry_root(global, env_registry_root)?;
    crate::codex_cli::run_guild_subcommand(args, Some(default_registry_root))
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

    let registry_root = resolve_registry_root(global, env_registry_root)?;
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

fn resolve_registry_root(
    global: &GlobalOptions,
    env_registry_root: Option<String>,
) -> Result<PathBuf, CliError> {
    if let Some(path) = &global.registry_root {
        return Ok(path.clone());
    }

    if let Some(path) = env_registry_root {
        return Ok(PathBuf::from(path));
    }

    paths::default_registry_root().map_err(|error| CliError::new(error.to_string()))
}

fn ensure_existing_registry_root(registry_root: &Path) -> Result<(), CliError> {
    if registry_root.exists() {
        return Ok(());
    }

    Err(CliError::new(format!(
        "Guild registry root `{}` does not exist yet\nread-only commands do not initialize a new root\nrun `{CLI_BINARY_NAME} install <source-dir>` to create it, or pass `--registry-root <path>` / set `GUILD_REGISTRY_ROOT` to use an existing root",
        registry_root.display()
    )))
}

fn build_existing_registry(registry_root: &Path) -> Result<LocalRegistry, CliError> {
    LocalRegistry::load_existing(registry_root).map_err(|error| {
        if error.code == "registry-root-missing" {
            CliError::new(format!(
                "Guild registry root `{}` does not exist yet\nread-only commands do not initialize a new root\nrun `{CLI_BINARY_NAME} install <source-dir>` to create it, or pass `--registry-root <path>` / set `GUILD_REGISTRY_ROOT` to use an existing root",
                registry_root.display()
            ))
        } else {
            CliError::from(error)
        }
    })
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

fn resolve_requested_skill_ref(
    registry: &LocalRegistry,
    input: &str,
) -> Result<RequestedSkillRef, CliError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CliError::new("skill reference cannot be empty"));
    }

    if let Ok(skill) = parse_skill_ref(trimmed) {
        return Ok(skill);
    }

    let short = trimmed.strip_prefix("skill://").unwrap_or(trimmed);
    let Some((name, version_req_raw)) = short.rsplit_once('@') else {
        return parse_skill_ref(trimmed);
    };
    if name.contains('/') {
        return parse_skill_ref(trimmed);
    }

    let version_req = VersionRequirement::parse(version_req_raw).map_err(|error| {
        CliError::new(format!("skill version requirement was invalid: {error}"))
    })?;

    let mut namespaces = registry
        .installed()
        .iter()
        .filter(|installed| installed.manifest.key.name == name)
        .filter(|installed| {
            version_req
                .as_semver()
                .matches(installed.resolved_ref.version.as_semver())
        })
        .map(|installed| installed.manifest.key.namespace.clone())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();

    match namespaces.as_slice() {
        [] => Err(CliError::new(format!(
            "short skill ref `{trimmed}` did not match any installed skill"
        ))),
        [namespace] => Ok(RequestedSkillRef {
            key: SkillKey {
                namespace: namespace.clone(),
                name: name.to_owned(),
            },
            version_req,
        }),
        _ => Err(CliError::new(format!(
            "short skill ref `{trimmed}` was ambiguous across namespaces: {}",
            namespaces.join(", ")
        ))),
    }
}

fn resolve_show_target(registry: &LocalRegistry, input: &str) -> Result<ShowTarget, CliError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CliError::new("`guild show` requires a non-empty ref"));
    }

    if trimmed.starts_with("guild://")
        || trimmed.starts_with("exec:")
        || trimmed.starts_with("evidence:")
        || trimmed.starts_with("obj:")
    {
        return resolve_show_resource_target(registry, trimmed);
    }

    let requested = resolve_requested_skill_ref(registry, trimmed)?;
    let installed = registry.resolve(&requested)?;
    Ok(ShowTarget::Skill {
        requested: trimmed.to_owned(),
        installed,
    })
}

fn resolve_show_resource_target(
    registry: &LocalRegistry,
    input: &str,
) -> Result<ShowTarget, CliError> {
    if let Some(prefix) = input.strip_prefix("exec:") {
        let uri = resolve_execution_prefix(registry, prefix)?;
        let execution_id = execution_id_from_uri(&uri)?;
        return registry
            .load_execution_record(&execution_id)
            .map(ShowTarget::Execution)
            .map_err(CliError::from);
    }

    if let Some(prefix) = input.strip_prefix("evidence:") {
        return resolve_evidence_record_by_prefix(registry, prefix).map(ShowTarget::Evidence);
    }

    if let Some(prefix) = input.strip_prefix("obj:") {
        return resolve_object_blob_by_prefix(registry, prefix).map(ShowTarget::Object);
    }

    let parsed =
        GuildResourceUri::parse(input).map_err(|error| CliError::new(error.to_string()))?;
    match parsed {
        GuildResourceUri::Execution { execution_id } => registry
            .load_execution_record(&execution_id)
            .map(ShowTarget::Execution)
            .map_err(CliError::from),
        GuildResourceUri::ObjectRecord { .. } | GuildResourceUri::ObjectRecordMetadata { .. } => {
            registry
                .load_evidence_record(input)
                .map(ShowTarget::Evidence)
                .map_err(CliError::from)
        }
        GuildResourceUri::ObjectBlob { digest_hex } => {
            resolve_object_blob_exact(registry, &digest_hex).map(ShowTarget::Object)
        }
        GuildResourceUri::ExecutionQuery { .. } => Err(CliError::new(
            "`guild show` does not support execution-query refs; use `guild get`",
        )),
    }
}

fn resolve_resource_ref(registry: &LocalRegistry, input: &str) -> Result<String, CliError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CliError::new("`guild get` requires a non-empty ref"));
    }

    if trimmed.starts_with("guild://") {
        GuildResourceUri::parse(trimmed).map_err(|error| CliError::new(error.to_string()))?;
        return Ok(trimmed.to_owned());
    }
    if let Some(prefix) = trimmed.strip_prefix("exec:") {
        return resolve_execution_prefix(registry, prefix);
    }
    if let Some(prefix) = trimmed.strip_prefix("evidence:") {
        return resolve_evidence_record_uri_by_prefix(registry, prefix);
    }
    if let Some(prefix) = trimmed.strip_prefix("obj:") {
        return resolve_object_uri_by_prefix(registry, prefix);
    }

    Err(CliError::new(format!(
        "unsupported resource ref `{trimmed}`; use guild://..., exec:..., evidence:..., or obj:..."
    )))
}

fn resolve_execution_ref(registry: &LocalRegistry, input: &str) -> Result<String, CliError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(CliError::new("execution ref cannot be empty"));
    }

    if trimmed.starts_with("guild://") {
        match GuildResourceUri::parse(trimmed).map_err(|error| CliError::new(error.to_string()))? {
            GuildResourceUri::Execution { .. } => return Ok(trimmed.to_owned()),
            _ => {
                return Err(CliError::new(format!(
                    "`{trimmed}` is not an execution ref"
                )));
            }
        }
    }

    if let Some(prefix) = trimmed.strip_prefix("exec:") {
        return resolve_execution_prefix(registry, prefix);
    }

    Err(CliError::new(format!(
        "unsupported execution ref `{trimmed}`; use guild://executions/... or exec:..."
    )))
}

fn execution_id_from_uri(uri: &str) -> Result<String, CliError> {
    match GuildResourceUri::parse(uri).map_err(|error| CliError::new(error.to_string()))? {
        GuildResourceUri::Execution { execution_id } => Ok(execution_id),
        _ => Err(CliError::new(format!("`{uri}` is not an execution URI"))),
    }
}

fn resolve_execution_prefix(registry: &LocalRegistry, prefix: &str) -> Result<String, CliError> {
    let prefix = non_empty_prefix("exec", prefix)?;
    let records = registry.list_recent_execution_records(usize::MAX)?;
    let matches = records
        .into_iter()
        .filter(|record| record.receipt.execution_id.starts_with(prefix))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(CliError::new(format!(
            "execution ref `exec:{prefix}` did not match any persisted execution"
        ))),
        [record] => Ok(record.receipt.uri.clone()),
        _ => Err(CliError::new(format!(
            "execution ref `exec:{prefix}` was ambiguous: {}",
            matches
                .iter()
                .take(5)
                .map(short_execution_ref)
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn resolve_evidence_record_by_prefix(
    registry: &LocalRegistry,
    prefix: &str,
) -> Result<EvidenceRecord, CliError> {
    let prefix = non_empty_prefix("evidence", prefix)?;
    let records = registry.list_recent_evidence_records(usize::MAX)?;
    let matches = records
        .into_iter()
        .filter(|record| evidence_record_id(record).starts_with(prefix))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(CliError::new(format!(
            "evidence ref `evidence:{prefix}` did not match any stored evidence record"
        ))),
        [record] => Ok(record.clone()),
        _ => Err(CliError::new(format!(
            "evidence ref `evidence:{prefix}` was ambiguous: {}",
            matches
                .iter()
                .take(5)
                .map(|record| format!(
                    "evidence:{}",
                    shorten_prefix_match(evidence_record_id(record))
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn resolve_evidence_record_uri_by_prefix(
    registry: &LocalRegistry,
    prefix: &str,
) -> Result<String, CliError> {
    resolve_evidence_record_by_prefix(registry, prefix).map(|record| record.uri)
}

fn resolve_object_blob_exact(
    registry: &LocalRegistry,
    digest_hex: &str,
) -> Result<EvidenceBlobRecord, CliError> {
    let records = registry.list_object_blobs(usize::MAX)?;
    records
        .into_iter()
        .find(|record| record.sha256 == digest_hex)
        .ok_or_else(|| {
            CliError::new(format!(
                "object ref `guild://objects/sha256/{digest_hex}` did not match any stored object"
            ))
        })
}

fn resolve_object_blob_by_prefix(
    registry: &LocalRegistry,
    prefix: &str,
) -> Result<EvidenceBlobRecord, CliError> {
    let prefix = non_empty_prefix("obj", prefix)?;
    let records = registry.list_object_blobs(usize::MAX)?;
    let matches = records
        .into_iter()
        .filter(|record| record.sha256.starts_with(prefix))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(CliError::new(format!(
            "object ref `obj:{prefix}` did not match any stored object"
        ))),
        [record] => Ok(record.clone()),
        _ => Err(CliError::new(format!(
            "object ref `obj:{prefix}` was ambiguous: {}",
            matches
                .iter()
                .take(5)
                .map(|record| format!("obj:{}", shorten_prefix_match(&record.sha256)))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn resolve_object_uri_by_prefix(
    registry: &LocalRegistry,
    prefix: &str,
) -> Result<String, CliError> {
    resolve_object_blob_by_prefix(registry, prefix).map(|record| record.uri)
}

fn evidence_record_id(record: &EvidenceRecord) -> &str {
    record.uri.rsplit('/').next().unwrap_or(record.uri.as_str())
}

fn shorten_prefix_match(value: &str) -> String {
    value.chars().take(12).collect()
}

fn non_empty_prefix<'a>(kind: &str, prefix: &'a str) -> Result<&'a str, CliError> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        Err(CliError::new(format!("{kind} ref prefix cannot be empty")))
    } else {
        Ok(trimmed)
    }
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

fn parse_signed_plan_signature(plan: &Value) -> Result<ExecutionPlanSignatureEnvelope, CliError> {
    serde_json::from_value(plan.get("plan_signature").cloned().ok_or_else(|| {
        CliError::new("signed execution plan output did not include plan_signature")
    })?)
    .map_err(CliError::from)
}

fn trust_verify_output(
    registry_root: &Path,
    verification: ExecutionPlanVerification,
) -> TrustVerifyPlanOutput {
    TrustVerifyPlanOutput {
        verified: true,
        publisher_id: verification.publisher.id,
        trust_tier: verification.trust_tier,
        registry_root: registry_root.display().to_string(),
        signed_digest: verification.signed_digest,
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

fn print_init_text(output: &InitCommandOutput) {
    println!("Guild init ready.");
    println!("registry root: {}", output.registry_root);
    println!(
        "status: {}",
        if output.created_registry_root {
            "created"
        } else {
            "already existed"
        }
    );

    println!();
    print_setup_details(
        Path::new(&output.codex.guild_binary),
        &output.codex.config,
        &output.codex.writes,
        true,
    );
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

fn print_usage() {
    println!("usage: guild [--registry-root <path>] <command> [options]");
    println!();
    println!("commands:");
    println!("  init         create the selected Guild root and print or write Codex setup");
    println!("  show         summarize an installed skill or persisted Guild ref");
    println!("  run          execute a skill through the local inspect path");
    println!("  ls           list skills, runs, objects, or evidence");
    println!("  get          read a Guild resource ref");
    println!("  why          explain one persisted execution record");
    println!("  verify       summarize installed trust and verification state");
    println!("  install      install a source skill into a Guild root");
    println!("  export       export installed state as a signed bundle or OCI layout");
    println!("  import       import a signed bundle or OCI layout into a Guild root");
    println!("  push         publish installed state to an OCI registry");
    println!("  pull         pull and import installed state from an OCI registry");
    println!("  trust        manage local publisher identities and trust records");
    println!("  codex        run deterministic Codex dogfood and smoke helpers");
    println!("  mcp          launch the existing Guild MCP stdio server");
    println!();
    println!(
        "registry roots resolve as `--registry-root`, then `GUILD_REGISTRY_ROOT`, then `~/.guild`; there is no cwd-local `.guild/` fallback."
    );
    println!(
        "canonical skill refs use `skill://<namespace>/<name>@<version>`; bare `<namespace>/<name>@<version>` and short `<name>@<version>` are accepted when unambiguous."
    );
    println!("legacy aliases: `inspect` -> `run`, `list` -> `ls`, `read` -> `get`.");
    println!("`guild trust ...` manages local trust-store state only.");
    println!("deferred: `guild build` and `guild deploy` are intentionally not implemented.");
}

fn print_show_usage() {
    println!(
        "usage: guild [--registry-root <path>] show <ref> [--json | --porcelain] [-v|-vv|--debug] [--color auto|always|never]"
    );
    println!("`guild show` is the primary non-executing inspection command.");
    println!(
        "accepted refs: skill refs, `exec:<id-prefix>`, `evidence:<id-prefix>`, `obj:<sha-prefix>`, and full `guild://...` URIs."
    );
}

fn print_run_usage() {
    println!(
        "usage: guild [--registry-root <path>] run <skill-ref> [input-file] [--input-json <json> | --input-file <path>] [--grants-json <json> | --grants-file <path>] [--tenant-id <id>] [--actor-id <id>] [--json | --porcelain] [-v|-vv|--debug] [--color auto|always|never]"
    );
    println!("`guild run` is the primary execution command.");
    println!("legacy alias: `guild inspect ...`.");
    println!("stdout carries the result payload; stderr carries the human status summary.");
}

fn print_ls_usage() {
    println!(
        "usage: guild [--registry-root <path>] ls [skills|runs|objects|evidence] [--limit <n>] [--json | --porcelain] [-v|-vv|--debug] [--color auto|always|never]"
    );
    println!("legacy alias: `guild list ...`.");
}

fn print_get_usage() {
    println!(
        "usage: guild [--registry-root <path>] get <ref> [--output <path>] [--json | --porcelain]"
    );
    println!(
        "accepted refs: full `guild://...` URIs plus `exec:<id-prefix>`, `evidence:<id-prefix>`, and `obj:<sha-prefix>`."
    );
    println!("legacy alias: `guild read ...`.");
}

fn print_why_usage() {
    println!(
        "usage: guild [--registry-root <path>] why <exec-ref> [--json | --porcelain] [-v|-vv|--debug] [--color auto|always|never]"
    );
    println!("accepted refs: `exec:<id-prefix>` and full execution URIs.");
}

fn print_verify_usage() {
    println!(
        "usage: guild [--registry-root <path>] verify <skill-ref> [--json | --porcelain] [-v|-vv|--debug] [--color auto|always|never]"
    );
    println!("`guild verify` is intentionally limited to installed skill refs.");
    println!("signed plan verification remains under `guild trust verify-plan`.");
}

fn print_init_usage() {
    println!(
        "usage: guild [--registry-root <path>] init [--global] [--project] [--name <server>] [--json]"
    );
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
    println!(
        "usage: guild [--registry-root <path>] trust <generate|add|list|remove|sign-plan|verify-plan> ..."
    );
    println!(
        "note: `guild trust ...` manages the local trust store and signs or verifies execution plans against that same trust model."
    );
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

fn print_trust_sign_plan_usage() {
    println!(
        "usage: guild trust sign-plan --plan <unsigned-plan.json> --identity-file <identity.json> --output <signed-plan.json> [--json]"
    );
}

fn print_trust_verify_plan_usage() {
    println!(
        "usage: guild [--registry-root <path>] trust verify-plan --plan <signed-plan.json> [--json]"
    );
}

fn print_mcp_usage() {
    println!("usage: guild [--registry-root <path>] mcp serve --stdio");
}

fn print_mcp_serve_usage() {
    println!("usage: guild [--registry-root <path>] mcp serve --stdio");
}
