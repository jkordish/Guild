#![allow(clippy::similar_names)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use guild_manifest::SkillManifest;
use guild_mcp::protocol::{InitializeResult, ListToolsResult, PROTOCOL_VERSION_2025_11_25};
use guild_registry::LocalSourceInstaller;
use guild_types::{
    AbiVersion, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    CapabilityRequirement, EmitEvidenceConstraints, EvidenceAudience, FilesystemConstraints,
    FilesystemOperation, FilesystemRoot, GrantedCapability, HttpMethod, HttpRequestConstraints,
    HttpScheme, InvokeDependencyConstraints, LogConstraints, ReadResourceConstraints,
    RedactionClass, ResourceKind, Severity,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;
#[path = "../../../test-support/oci_registry_test_server.rs"]
mod oci_registry_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn hello_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn render_report_source_dir() -> PathBuf {
    repo_root().join("examples/skills/render-report")
}

fn composite_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-composite")
}

fn http_source_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn incident_brief_source_dir() -> PathBuf {
    repo_root().join("examples/skills/incident-brief")
}

fn evidence_summary_source_dir() -> PathBuf {
    repo_root().join("examples/skills/evidence-summary")
}

fn draft_plan_path(name: &str) -> PathBuf {
    repo_root()
        .join("docs/schemas/draft-v1/examples")
        .join(name)
}

fn assert_markdown_uses_installed_guild_cli(path: &Path) {
    let contents = fs::read_to_string(path).unwrap();
    assert!(
        !contents.contains("cargo run -q -p guild-mcp --bin guild --"),
        "{} still uses cargo-wrapped guild CLI in user-facing markdown",
        path.display()
    );
    assert!(
        !contents.contains("cargo run -p guild-mcp --bin guild --"),
        "{} still uses cargo-wrapped guild CLI in user-facing markdown",
        path.display()
    );
}

fn assert_markdown_keeps_legacy_alias_commands_out_of_examples(path: &Path) {
    let contents = fs::read_to_string(path).unwrap();
    for (index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        assert!(
            !trimmed.starts_with("guild inspect")
                && !trimmed.starts_with("guild read")
                && !trimmed.starts_with("guild list")
                && !trimmed.starts_with("guild explain"),
            "{} teaches a legacy alias as a command example on line {}: {}",
            path.display(),
            index + 1,
            line
        );
    }
}

fn assert_contains_canonical_authority_lifecycle(contents: &str, label: &str) {
    for phrase in [
        "declared authority: capabilities declared by the installed manifest and visible in `guild show`",
        "requested authority: caller-requested grants passed to `guild run`",
        "granted authority: the final capability slice the host policy allows for that run",
        "effective at runtime: the authority the guest can actually exercise during execution",
        "Guild does not hand the guest ambient authority. The host may reduce or deny caller-requested authority before guest start, and the runtime only exposes the final granted set.",
    ] {
        assert!(
            contents.contains(phrase),
            "{label} is missing canonical authority-lifecycle wording: {phrase}"
        );
    }
}

fn emit_evidence_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![GrantedCapability {
            id: CapabilityId::EmitEvidence,
            access: CapabilityAccess::Write,
            constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                max_bytes: Some(65_536),
                audiences: Some(vec![EvidenceAudience::User]),
                redactions: Some(vec![RedactionClass::None]),
            }),
        }],
    })
    .unwrap()
}

fn emit_evidence_and_log_write_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![
            GrantedCapability {
                id: CapabilityId::EmitEvidence,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                    max_bytes: Some(65_536),
                    audiences: Some(vec![EvidenceAudience::User]),
                    redactions: Some(vec![RedactionClass::None]),
                }),
            },
            GrantedCapability {
                id: CapabilityId::LogWrite,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::Log(LogConstraints {
                    levels: Some(vec![Severity::Info]),
                }),
            },
        ],
    })
    .unwrap()
}

fn emit_evidence_and_broad_log_write_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![
            GrantedCapability {
                id: CapabilityId::EmitEvidence,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                    max_bytes: Some(65_536),
                    audiences: Some(vec![EvidenceAudience::User]),
                    redactions: Some(vec![RedactionClass::None]),
                }),
            },
            GrantedCapability {
                id: CapabilityId::LogWrite,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::Log(LogConstraints {
                    levels: Some(vec![Severity::Info, Severity::Warn]),
                }),
            },
        ],
    })
    .unwrap()
}

fn emit_filesystem_rejection_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![
            GrantedCapability {
                id: CapabilityId::EmitEvidence,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                    max_bytes: Some(65_536),
                    audiences: Some(vec![EvidenceAudience::User]),
                    redactions: Some(vec![RedactionClass::None]),
                }),
            },
            GrantedCapability {
                id: CapabilityId::Filesystem,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::Filesystem(FilesystemConstraints {
                    preopened_roots: vec![FilesystemRoot {
                        name: "workspace".into(),
                        guest_path_prefix: "/workspace".into(),
                        host_path: "/var/lib/guild/workspace".into(),
                        operations: vec![FilesystemOperation::Read],
                    }],
                }),
            },
        ],
    })
    .unwrap()
}

fn emit_http_path_denial_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![GrantedCapability {
            id: CapabilityId::HttpRequest,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
                allowed_schemes: Some(vec![HttpScheme::Http]),
                allowed_hosts: Some(vec!["127.0.0.1".into()]),
                allowed_host_suffixes: None,
                allowed_ports: None,
                allowed_methods: Some(vec![HttpMethod::Get]),
                allowed_path_prefixes: Some(vec!["/allowed".into()]),
                max_timeout_ms: Some(2_000),
                max_response_bytes: Some(4_096),
                follow_redirects: Some(true),
                max_redirects: Some(2),
                allow_loopback: Some(true),
                allow_link_local: Some(false),
                allow_private_networks: Some(false),
                allow_ip_literals: Some(true),
            }),
        }],
    })
    .unwrap()
}

fn emit_http_redirect_denial_grants_json(port: u16) -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![GrantedCapability {
            id: CapabilityId::HttpRequest,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
                allowed_schemes: Some(vec![HttpScheme::Http]),
                allowed_hosts: Some(vec![http_test_server::HttpTestServer::host().into()]),
                allowed_host_suffixes: None,
                allowed_ports: Some(vec![port]),
                allowed_methods: Some(vec![HttpMethod::Get]),
                allowed_path_prefixes: Some(vec!["/redirect-json".into(), "/json".into()]),
                max_timeout_ms: Some(2_000),
                max_response_bytes: Some(8_192),
                follow_redirects: Some(false),
                max_redirects: None,
                allow_loopback: Some(true),
                allow_link_local: Some(false),
                allow_private_networks: Some(false),
                allow_ip_literals: Some(true),
            }),
        }],
    })
    .unwrap()
}

fn incident_brief_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![
            GrantedCapability {
                id: CapabilityId::ReadResource,
                access: CapabilityAccess::Read,
                constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                    uri_prefixes: Some(vec!["guild://executions/".into()]),
                    resource_kinds: Some(vec![ResourceKind::Execution]),
                }),
            },
            GrantedCapability {
                id: CapabilityId::InvokeSkill,
                access: CapabilityAccess::Invoke,
                constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
                    aliases: Some(vec!["renderer".into()]),
                }),
            },
        ],
    })
    .unwrap()
}

fn composite_invoke_and_emit_evidence_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![
            GrantedCapability {
                id: CapabilityId::InvokeSkill,
                access: CapabilityAccess::Invoke,
                constraints: CapabilityConstraints::InvokeDependency(InvokeDependencyConstraints {
                    aliases: Some(vec!["hello".into()]),
                }),
            },
            GrantedCapability {
                id: CapabilityId::EmitEvidence,
                access: CapabilityAccess::Write,
                constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
                    max_bytes: Some(65_536),
                    audiences: Some(vec![EvidenceAudience::User]),
                    redactions: Some(vec![RedactionClass::None]),
                }),
            },
        ],
    })
    .unwrap()
}

fn evidence_summary_grants_json() -> String {
    serde_json::to_string(&CapabilityGrantSet {
        grants: vec![GrantedCapability {
            id: CapabilityId::ReadResource,
            access: CapabilityAccess::Read,
            constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
                uri_prefixes: Some(vec!["guild://objects/records/".into()]),
                resource_kinds: Some(vec![ResourceKind::Object]),
            }),
        }],
    })
    .unwrap()
}

#[allow(clippy::needless_pass_by_value)]
fn command_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap()
}

fn persisted_where_uri(stderr: &str) -> String {
    stderr
        .lines()
        .find_map(|line| line.strip_prefix("where: "))
        .unwrap()
        .to_owned()
}

struct TempFixtureDir {
    path: PathBuf,
}

impl TempFixtureDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn guild_command_with_options(
    env_registry_root: Option<&Path>,
    home_dir: Option<&Path>,
    current_dir: Option<&Path>,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_guild"));
    match current_dir {
        Some(current_dir) => {
            command.current_dir(current_dir);
        }
        None => {
            command.current_dir(repo_root());
        }
    }
    if let Some(root) = env_registry_root {
        command.env("GUILD_REGISTRY_ROOT", root);
    } else {
        command.env_remove("GUILD_REGISTRY_ROOT");
    }
    if let Some(home_dir) = home_dir {
        command.env("HOME", home_dir);
        command.env_remove("USERPROFILE");
        command.env_remove("HOMEDRIVE");
        command.env_remove("HOMEPATH");
    }
    command
}

fn guild_command(env_registry_root: Option<&Path>) -> Command {
    guild_command_with_options(env_registry_root, None, None)
}

fn run_guild(args: &[&str], env_registry_root: Option<&Path>) -> Output {
    guild_command(env_registry_root)
        .args(args)
        .output()
        .unwrap()
}

fn run_guild_with_home(args: &[&str], home_dir: &Path) -> Output {
    guild_command_with_options(None, Some(home_dir), None)
        .args(args)
        .output()
        .unwrap()
}

fn run_guild_with_home_and_cwd(args: &[&str], home_dir: &Path, current_dir: &Path) -> Output {
    guild_command_with_options(None, Some(home_dir), Some(current_dir))
        .args(args)
        .output()
        .unwrap()
}

fn run_guild_success(args: &[&str], env_registry_root: Option<&Path>) -> String {
    let output = run_guild(args, env_registry_root);
    assert!(
        output.status.success(),
        "guild command failed\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_guild_success_output(args: &[&str], env_registry_root: Option<&Path>) -> Output {
    let output = run_guild(args, env_registry_root);
    assert!(
        output.status.success(),
        "guild command failed\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn run_guild_failure_output(args: &[&str], env_registry_root: Option<&Path>) -> Output {
    let output = run_guild(args, env_registry_root);
    assert!(
        !output.status.success(),
        "guild command unexpectedly succeeded\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn run_guild_success_with_home(args: &[&str], home_dir: &Path) -> String {
    let output = run_guild_with_home(args, home_dir);
    assert!(
        output.status.success(),
        "guild command failed\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).unwrap()
}

fn run_guild_success_with_home_and_cwd(
    args: &[&str],
    home_dir: &Path,
    current_dir: &Path,
) -> String {
    let output = run_guild_with_home_and_cwd(args, home_dir, current_dir);
    assert!(
        output.status.success(),
        "guild command failed\nargs: {args:?}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).unwrap()
}

fn parse_json_stdout<T: DeserializeOwned>(stdout: &str) -> T {
    serde_json::from_str(stdout).unwrap()
}

fn parse_failure_json_output(output: &Output) -> Value {
    let stderr = std::str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.trim().is_empty(),
        "expected empty stderr, got:\n{stderr}"
    );

    let stdout = std::str::from_utf8(&output.stdout).unwrap();
    let value: Value = serde_json::from_str(stdout).unwrap();
    let steps = value["error"]["next_steps"]
        .as_array()
        .expect("error.next_steps should be an array");
    for step in steps {
        let step = step
            .as_str()
            .expect("error.next_steps entries should be strings");
        assert!(!step.starts_with("Next:"), "{stdout}");
    }
    value
}

fn install_with_cli(registry_root: &Path) {
    install_source_with_cli(registry_root, &hello_source_dir());
}

fn install_source_with_cli(registry_root: &Path, source_dir: &Path) {
    let source_dir = source_dir.display().to_string();
    let root = registry_root.display().to_string();
    let _ = run_guild_success(&["--registry-root", &root, "install", &source_dir], None);
}

fn install_source_with_cli_json(registry_root: &Path, source_dir: &Path) -> Value {
    let source_dir = source_dir.display().to_string();
    let root = registry_root.display().to_string();
    let output = run_guild_success(
        &["--registry-root", &root, "install", &source_dir, "--json"],
        None,
    );
    parse_json_stdout(&output)
}

fn generate_identity_with_cli(identity_path: &Path) {
    let identity = identity_path.display().to_string();
    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );
}

fn expected_export_review_output(transport: &str, output_root: &Path) -> String {
    format!(
        concat!(
            "exported installed state\n",
            "transport: {}\n",
            "skill: skill://example/hello-inspect@0.1.0\n",
            "publisher: local.example\n",
            "contents: root skill only\n",
            "output: {}\n",
            "Next: guild import {} {}\n",
        ),
        transport,
        output_root.display(),
        transport,
        output_root.display()
    )
}

fn expected_import_review_output(registry_root: &Path, transport: &str, source: &str) -> String {
    format!(
        concat!(
            "imported installed state\n",
            "transport: {}\n",
            "source: {}\n",
            "installed: 1 skill\n",
            "\n",
            "installed skill://example/hello-inspect@0.1.0\n",
            "publisher: local.example\n",
            "status: verified-import / trusted-imported\n",
            "Next: guild --registry-root {} verify -v skill://example/hello-inspect@0.1.0\n",
        ),
        transport,
        source,
        registry_root.display()
    )
}

fn assert_import_preview_output(
    output: &str,
    transport: &str,
    source: &str,
    decision: &str,
    trust: &str,
) {
    assert!(output.contains("previewed installed state"), "{output}");
    assert!(
        output.contains(&format!("transport: {transport}")),
        "{output}"
    );
    assert!(output.contains(&format!("source: {source}")), "{output}");
    assert!(
        output.contains(&format!("decision: {decision}")),
        "{output}"
    );
    assert!(
        output.contains("skill: skill://example/hello-inspect@0.1.0"),
        "{output}"
    );
    assert!(output.contains("publisher: local.example"), "{output}");
    assert!(
        output.contains(&format!(
            "status: {} / {trust}",
            if decision == "would-import" {
                "verified"
            } else {
                "refused"
            }
        )),
        "{output}"
    );
    assert!(output.contains("scheme: ed25519"), "{output}");
    assert!(output.contains("bundle digest: sha256:"), "{output}");
    assert!(output.contains("contents: root skill only"), "{output}");
    assert!(output.contains("skills: 1 skill"), "{output}");
}

fn expected_trust_add_output(registry_root: &Path) -> String {
    format!(
        concat!(
            "trusted publisher local.example\n",
            "tier: trusted-imported\n",
            "name: Local Example\n",
            "Next: guild --registry-root {} trust list\n",
        ),
        registry_root.display()
    )
}

fn expected_trust_list_output() -> &'static str {
    concat!(
        "publisher: local.example\n",
        "tier: trusted-imported\n",
        "name: Local Example\n",
    )
}

fn copy_dir_recursive(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_recursive(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path).unwrap();
        }
    }
}

fn installed_skill_dir(registry_root: &Path, namespace: &str, name: &str) -> PathBuf {
    let version_root = registry_root
        .join("installed")
        .join(namespace)
        .join(name)
        .join("0.1.0");
    let mut entries = fs::read_dir(&version_root).unwrap();
    let install_dir = entries.next().unwrap().unwrap().path();
    assert!(
        entries.next().is_none(),
        "expected a single installed digest dir"
    );
    install_dir
}

fn write_installed_manifest(skill_dir: &Path, update: impl FnOnce(&mut SkillManifest)) {
    let manifest_path = skill_dir.join("manifest.json");
    let mut manifest: SkillManifest =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    update(&mut manifest);
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

fn duplicate_installed_hello_with_namespace(registry_root: &Path, namespace: &str) -> PathBuf {
    let source_dir = installed_skill_dir(registry_root, "example", "hello-inspect");
    let target_dir = registry_root
        .join("installed")
        .join(namespace)
        .join("hello-inspect")
        .join("0.1.0")
        .join(source_dir.file_name().unwrap());
    copy_dir_recursive(&source_dir, &target_dir);
    write_installed_manifest(&target_dir, |manifest| {
        manifest.key.namespace = namespace.into();
        manifest.display_name = format!("Hello Inspect ({namespace})");
    });
    target_dir
}

fn duplicate_installed_hello_with_filesystem(registry_root: &Path, skill_name: &str) -> PathBuf {
    let source_dir = installed_skill_dir(registry_root, "example", "hello-inspect");
    let target_dir = registry_root
        .join("installed")
        .join("example")
        .join(skill_name)
        .join("0.1.0")
        .join(source_dir.file_name().unwrap());
    copy_dir_recursive(&source_dir, &target_dir);
    write_installed_manifest(&target_dir, |manifest| {
        manifest.key.name = skill_name.into();
        manifest.display_name = "Hello Inspect Filesystem".into();
        manifest.description = "A fixture that declares the deferred filesystem contract.".into();
        manifest.capabilities.push(CapabilityRequirement {
            id: CapabilityId::Filesystem,
            access: CapabilityAccess::Read,
            required: true,
            constraints: CapabilityConstraints::Filesystem(FilesystemConstraints {
                preopened_roots: vec![FilesystemRoot {
                    name: "workspace".into(),
                    guest_path_prefix: "/workspace".into(),
                    host_path: "/var/lib/guild/workspace".into(),
                    operations: vec![FilesystemOperation::Read],
                }],
            }),
        });
    });
    target_dir
}

fn duplicate_installed_hello_with_entrypoint_mismatch(
    registry_root: &Path,
    skill_name: &str,
) -> PathBuf {
    let source_dir = installed_skill_dir(registry_root, "example", "hello-inspect");
    let target_dir = registry_root
        .join("installed")
        .join("example")
        .join(skill_name)
        .join("0.1.0")
        .join(source_dir.file_name().unwrap());
    copy_dir_recursive(&source_dir, &target_dir);
    write_installed_manifest(&target_dir, |manifest| {
        manifest.key.name = skill_name.into();
        manifest.display_name = "Hello Inspect EntryPoint Drift".into();
        manifest.description =
            "A fixture that keeps the inspect ABI but drifts the manifest entrypoint.".into();
        manifest.runtime.guest_abi_version = AbiVersion::GuildSkillInspectV1;
        manifest.runtime.entrypoint = "guild-skill".into();
    });
    target_dir
}

fn duplicate_installed_hello_with_apply_approval_mismatch(
    registry_root: &Path,
    skill_name: &str,
) -> PathBuf {
    let source_dir = installed_skill_dir(registry_root, "example", "hello-inspect");
    let target_dir = registry_root
        .join("installed")
        .join("example")
        .join(skill_name)
        .join("0.1.0")
        .join(source_dir.file_name().unwrap());
    copy_dir_recursive(&source_dir, &target_dir);
    write_installed_manifest(&target_dir, |manifest| {
        manifest.key.name = skill_name.into();
        manifest.display_name = "Hello Inspect Mode Drift".into();
        manifest.description =
            "A fixture that introduces non-runtime installed-manifest drift.".into();
        manifest.behavior.modes.apply_requires_approval = true;
    });
    target_dir
}

fn broad_world_fixture_source() -> &'static str {
    r#"use serde_json::{json, Value};
use wit_bindgen::generate;

const _: &str = include_str!("../../../../../wit/guild-skill-v1.wit");

generate!({
    path: "../../../../wit",
    world: "guild-skill",
});

use crate::exports::guild::skill::skill::{
    ExecutionContext, Guest, Json, SkillError, SkillOutput,
};
use crate::guild::skill::host;
use crate::guild::skill::types::{EvidenceAudience, EvidenceEmissionRequest, RedactionClass};

struct HelloInspectBroadImport;

impl Guest for HelloInspectBroadImport {
    fn run(ctx: ExecutionContext, input: Json) -> Result<SkillOutput, SkillError> {
        let parsed_input: Value = serde_json::from_str(&input).map_err(|error| SkillError {
            code: "invalid-input".into(),
            message: "input JSON could not be parsed".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let payload = serde_json::to_vec(&json!({
            "kind": "broad-import-fixture",
            "execution_id": ctx.execution_id,
            "input": parsed_input,
        }))
        .map_err(|error| SkillError {
            code: "evidence-payload-invalid".into(),
            message: "fixture evidence payload could not be serialized".into(),
            retryable: false,
            detail: Some(json!({ "error": error.to_string() }).to_string()),
        })?;

        let evidence = host::emit_evidence(&EvidenceEmissionRequest {
            payload,
            mime_type: "application/json".into(),
            title: Some("broad-import fixture".into()),
            audience: EvidenceAudience::User,
            redaction: RedactionClass::None,
            freshness: Some("deterministic".into()),
        })
        .map_err(|message| SkillError {
            code: "emit-evidence-failed".into(),
            message: "host failed to persist fixture evidence".into(),
            retryable: false,
            detail: Some(json!({ "error": message }).to_string()),
        })?;

        Ok(SkillOutput {
            summary: "Broad world fixture executed".into(),
            structured: json!({
                "message": "broad import fixture",
                "execution_id": ctx.execution_id,
            })
            .to_string(),
            diagnostics: Vec::new(),
            effects: Vec::new(),
            evidence: vec![evidence],
        })
    }
}

export!(HelloInspectBroadImport with_types_in self);
"#
}

fn write_broad_import_fixture(root: &Path, skill_name: &str) -> PathBuf {
    let workspace_root = root.join("workspace");
    let source_root = workspace_root.join(format!("examples/skills/{skill_name}"));
    copy_dir_recursive(&hello_source_dir(), &source_root);
    copy_dir_recursive(&repo_root().join("wit"), &workspace_root.join("wit"));

    let manifest_path = source_root.join("manifest.json");
    let mut manifest: guild_manifest::SourceSkillManifest =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest.key.name = skill_name.into();
    manifest.display_name = format!("{} Broad Import", manifest.display_name);
    manifest.description =
        "A fixture that compiles the broad Guild world under an inspect manifest.".into();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let lib_path = source_root.join("skill-rust/src/lib.rs");
    fs::write(lib_path, broad_world_fixture_source()).unwrap();

    source_root
}

fn write_required_emit_evidence_denial_policy(registry_root: &Path) {
    let policy = json!({
        "format_version": "guild-local-policy-v2",
        "default_profile": "default",
        "profiles": [
            {
                "name": "default",
                "default_action": "allow-requested-declared",
                "rules": [
                    {
                        "name": "deny-required-emit-evidence",
                        "applies_to": "required",
                        "effect": "deny",
                        "capabilities": {
                            "grants": [
                                {
                                    "id": "emit-evidence",
                                    "access": "write",
                                    "constraints": {
                                        "max_bytes": 65_536,
                                        "audiences": ["user"],
                                        "redactions": ["none"]
                                    }
                                }
                            ]
                        }
                    }
                ]
            }
        ],
        "bindings": []
    });
    fs::write(
        registry_root.join("policy.json"),
        serde_json::to_vec_pretty(&policy).unwrap(),
    )
    .unwrap();
}

fn inspect_hello_with_cli(registry_root: &Path, name: &str, skill_ref: &str) -> Value {
    let grants_json = emit_evidence_grants_json();
    let inspect_output = run_guild_success(
        &[
            "inspect",
            skill_ref,
            "--input-json",
            &command_json(json!({ "name": name })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        Some(registry_root),
    );
    parse_json_stdout(&inspect_output)
}

#[test]
fn inspect_and_read_commands_work_with_env_registry_root() {
    let temp = TempFixtureDir::new("guild-cli-inspect-read");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let grants_json = emit_evidence_grants_json();
    let inspect_output = run_guild_success(
        &[
            "inspect",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        Some(&registry_root),
    );
    let inspect_value: Value = parse_json_stdout(&inspect_output);
    assert_eq!(
        inspect_value["summary"].as_str(),
        Some("Hello, Ada. Guild inspect is working."),
    );

    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let read_output = run_guild_success(&["read", &execution_uri, "--json"], Some(&registry_root));
    let read_value: Value = parse_json_stdout(&read_output);
    assert_eq!(read_value["uri"].as_str(), Some(execution_uri.as_str()));
    assert_eq!(read_value["mime_type"].as_str(), Some("application/json"));
    assert!(read_value["bytes_base64"].as_str().is_some());
}

#[test]
fn read_command_distinguishes_evidence_payload_and_metadata_resources() {
    let temp = TempFixtureDir::new("guild-cli-evidence-metadata");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");

    let evidence_record = inspect_value["record"]["emitted_evidence"][0].clone();
    let evidence_uri = evidence_record["uri"].as_str().unwrap().to_owned();
    let metadata_uri = format!("{evidence_uri}/metadata");

    let payload_output =
        run_guild_success(&["read", &evidence_uri, "--json"], Some(&registry_root));
    let payload_value: Value = parse_json_stdout(&payload_output);
    let payload_json: Value =
        serde_json::from_str(payload_value["text"].as_str().unwrap()).unwrap();

    let metadata_output =
        run_guild_success(&["read", &metadata_uri, "--json"], Some(&registry_root));
    let metadata_value: Value = parse_json_stdout(&metadata_output);
    let metadata_json: Value =
        serde_json::from_str(metadata_value["text"].as_str().unwrap()).unwrap();

    assert_eq!(payload_value["uri"].as_str(), Some(evidence_uri.as_str()));
    assert_eq!(
        payload_value["mime_type"].as_str(),
        Some("application/json")
    );
    assert_eq!(
        payload_json["kind"].as_str(),
        Some("hello-inspect-snapshot")
    );
    assert_eq!(metadata_value["uri"].as_str(), Some(metadata_uri.as_str()));
    assert_eq!(
        metadata_value["mime_type"].as_str(),
        Some("application/json")
    );
    assert_eq!(metadata_json, evidence_record);
    assert_eq!(metadata_json["uri"].as_str(), Some(evidence_uri.as_str()));
    assert_eq!(
        metadata_json["produced_by_execution"],
        inspect_value["record"]["receipt"]["execution_id"]
    );
    assert!(
        metadata_json["blob_uri"]
            .as_str()
            .unwrap()
            .starts_with("guild://objects/sha256/")
    );
}

#[test]
fn primary_show_and_verify_commands_render_compact_human_output() {
    let temp = TempFixtureDir::new("guild-cli-show-verify");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let show_output = run_guild_success(
        &["show", "hello-inspect@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert_eq!(
        show_output,
        format!(
            concat!(
                "example/hello-inspect@0.1.0  Hello Inspect\n",
                "status: local-source / local-dev\n",
                "support: proof-backed(log-write) not_proven(emit-evidence)\n",
                "runtime: wasm-component / guild-skill-inspect-v1\n",
                "caps: emit-evidence(write,required) log-write(write)\n",
                "Next: guild --registry-root {} verify skill://example/hello-inspect@0.1.0\n",
            ),
            registry_root.display()
        )
    );

    let verify_output = run_guild_success(
        &["verify", "hello-inspect@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_output,
        format!(
            concat!(
                "example/hello-inspect@0.1.0\n",
                "publisher: local-source\n",
                "status: local-source / local-dev\n",
                "Next: guild --registry-root {} show -v skill://example/hello-inspect@0.1.0\n",
            ),
            registry_root.display()
        )
    );
}

#[test]
fn trust_add_and_list_render_review_friendly_human_output() {
    let temp = TempFixtureDir::new("guild-cli-trust-review");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();
    fs::create_dir_all(&registry_root).unwrap();

    let empty_list = run_guild_success(
        &["--registry-root", &registry_root_display, "trust", "list"],
        None,
    );
    assert_eq!(
        empty_list,
        format!(
            concat!(
                "no trusted publishers configured\n",
                "Next: guild --registry-root {} trust add --identity-file <identity.json>\n",
            ),
            registry_root.display()
        )
    );

    generate_identity_with_cli(&identity_path);

    let add_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    assert_eq!(add_output, expected_trust_add_output(&registry_root));

    let list_output = run_guild_success(
        &["--registry-root", &registry_root_display, "trust", "list"],
        None,
    );
    assert_eq!(list_output, expected_trust_list_output());
}

#[test]
fn show_execution_and_evidence_human_output_suggest_why_next_step() {
    let temp = TempFixtureDir::new("guild-cli-show-next-steps");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_id = inspect_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    let evidence_uri = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();

    let execution_show = run_guild_success(
        &["show", &exec_prefix, "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        execution_show.contains("succeeded  exec:"),
        "{execution_show}"
    );
    assert!(
        execution_show.contains(&format!(
            "Next: guild --registry-root {} why {execution_uri}",
            registry_root.display()
        )),
        "{execution_show}"
    );

    let evidence_show = run_guild_success(
        &["show", &evidence_uri, "--color", "never"],
        Some(&registry_root),
    );
    assert!(evidence_show.contains("evidence:"), "{evidence_show}");
    assert!(
        evidence_show.contains(&format!(
            "Next: guild --registry-root {} why {execution_uri}",
            registry_root.display()
        )),
        "{evidence_show}"
    );
}

#[test]
fn primary_run_command_keeps_payload_on_stdout_and_status_on_stderr() {
    let temp = TempFixtureDir::new("guild-cli-run-stdio");
    let registry_root = temp.path().join("registry");
    let input_path = temp.path().join("input.json");
    install_with_cli(&registry_root);
    fs::write(&input_path, "{\n  \"name\": \"Ada\"\n}\n").unwrap();

    let grants_json = emit_evidence_grants_json();
    let input = input_path.display().to_string();
    let output = run_guild_success_output(
        &[
            "run",
            "hello-inspect@^0.1",
            &input,
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let payload: Value = parse_json_stdout(&stdout);

    assert_eq!(payload["message"].as_str(), Some("Hello, Ada"));
    assert_eq!(payload["mode"].as_str(), Some("inspect"));
    assert!(stderr.contains("succeeded  not_proven  exec:"), "{stderr}");
    assert!(stderr.contains("example/hello-inspect@0.1.0"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} get guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stdout.contains("exec:"), "{stdout}");
    assert!(!stderr.contains("\"message\""), "{stderr}");
}

#[test]
fn starter_pack_incident_brief_runs_with_markdown_stdout() {
    let temp = TempFixtureDir::new("guild-cli-incident-brief");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &render_report_source_dir());
    install_source_with_cli(&registry_root, &incident_brief_source_dir());

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();

    let show_output = run_guild_success(
        &["show", "incident-brief@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert!(show_output.contains("example/incident-brief@0.1.0"));
    assert!(show_output.contains("support: bounded("));
    assert!(show_output.contains("invoke-skill"));
    assert!(!show_output.contains("Next: guild run "), "{show_output}");

    let verify_output = run_guild_success(
        &["verify", "incident-brief@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_output,
        format!(
            concat!(
                "example/incident-brief@0.1.0\n",
                "publisher: local-source\n",
                "status: local-source / local-dev\n",
                "Next: guild --registry-root {} show -v skill://example/incident-brief@0.1.0\n",
            ),
            registry_root.display()
        )
    );

    let grants_json = incident_brief_grants_json();
    let output = run_guild_success_output(
        &[
            "run",
            "incident-brief@^0.1",
            "--input-json",
            &command_json(json!({ "execution_uri": execution_uri })),
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.starts_with("# Incident Brief\n\n"), "{stdout}");
    assert!(stdout.contains("## Primary reason"), "{stdout}");
    assert!(stdout.contains("## Next refs"), "{stdout}");
    assert!(stderr.contains("succeeded  bounded  exec:"), "{stderr}");
    assert!(stderr.contains("example/incident-brief@0.1.0"), "{stderr}");
    assert!(!stdout.contains("\"title\""), "{stdout}");
}

#[test]
fn starter_pack_evidence_summary_runs_with_markdown_stdout() {
    let temp = TempFixtureDir::new("guild-cli-evidence-summary");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &evidence_summary_source_dir());

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let evidence_uri = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();

    let grants_json = evidence_summary_grants_json();
    let output = run_guild_success_output(
        &[
            "run",
            "evidence-summary@^0.1",
            "--input-json",
            &command_json(json!({ "evidence_uri": evidence_uri })),
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.starts_with("# Evidence Summary\n\n"), "{stdout}");
    assert!(stdout.contains("## Linkage"), "{stdout}");
    assert!(stdout.contains("## Normalized details"), "{stdout}");
    assert!(stdout.contains("hello-inspect-snapshot"), "{stdout}");
    assert!(stderr.contains("succeeded  bounded  exec:"), "{stderr}");
    assert!(
        stderr.contains("example/evidence-summary@0.1.0"),
        "{stderr}"
    );
    assert!(!stdout.contains("\"mime_type\""), "{stdout}");
}

#[test]
fn run_refusal_keeps_payload_off_stdout_and_status_on_stderr() {
    let temp = TempFixtureDir::new("guild-cli-run-refusal-stdio");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let _ = duplicate_installed_hello_with_filesystem(&registry_root, "hello-inspect-filesystem");

    let grants_json = emit_filesystem_rejection_grants_json();
    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-filesystem@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(stderr.contains("rejected  refused  exec:"), "{stderr}");
    assert!(
        stderr.contains("example/hello-inspect-filesystem@0.1.0"),
        "{stderr}"
    );
    assert!(
        stderr.contains("runtime/compatibility: filesystem capability contracts are not implemented in the active Wasm inspect slice"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: filesystem-runtime-not-supported"),
        "{stderr}"
    );
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/hello-inspect-filesystem@^0.1'",
            registry_root.display()
        )),
        "{stderr}"
    );
    let where_line = stderr
        .lines()
        .find(|line| line.starts_with("where: guild://executions/"))
        .unwrap();
    assert!(!where_line.contains(" ("), "{stderr}");
}

#[test]
fn wrong_world_manifest_rejections_stay_in_runtime_compatibility_bucket() {
    let temp = TempFixtureDir::new("guild-cli-run-entrypoint-drift");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let _ = duplicate_installed_hello_with_entrypoint_mismatch(
        &registry_root,
        "hello-inspect-entrypoint-drift",
    );

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-entrypoint-drift@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &emit_evidence_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains(
            "runtime/compatibility: guild-skill-inspect-v1 guest ABI requires runtime.entrypoint = guild-skill-inspect-v1"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("reason: invalid-manifest"), "{stderr}");
    assert!(!stderr.contains("authority denial"), "{stderr}");
    assert!(!stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: inspect the affected installed `manifest.json` under the selected Guild root, then rerun `guild --registry-root {} install <source-dir>` to repair it from source before retrying",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn non_runtime_invalid_manifest_errors_do_not_use_runtime_bucket() {
    let temp = TempFixtureDir::new("guild-cli-run-non-runtime-manifest-drift");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let _ = duplicate_installed_hello_with_apply_approval_mismatch(
        &registry_root,
        "hello-inspect-mode-drift",
    );

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-mode-drift@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &emit_evidence_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains("usage: manifest validation failed"),
        "{stderr}"
    );
    assert!(stderr.contains("reason: invalid-manifest"), "{stderr}");
    assert!(!stderr.contains("runtime/compatibility"), "{stderr}");
    assert!(!stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(!stderr.contains("guild show -v"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: inspect the affected installed `manifest.json` under the selected Guild root, then rerun `guild --registry-root {} install <source-dir>` to repair it from source before retrying",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn broader_component_import_rejections_stay_in_runtime_compatibility_bucket() {
    let temp = TempFixtureDir::new("guild-cli-broad-import-runtime-compat");
    let registry_root = temp.path().join("registry");
    let broad_source = write_broad_import_fixture(temp.path(), "hello-inspect-broad-import-cli");
    install_source_with_cli(&registry_root, &broad_source);

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-broad-import-cli@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &emit_evidence_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains("runtime/compatibility: inspect runtime rejected component import `guild:skill/host@1.0.0`"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: unsupported-runtime-surface"),
        "{stderr}"
    );
    assert!(!stderr.contains("authority denial"), "{stderr}");
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/hello-inspect-broad-import-cli@^0.1'",
            registry_root.display()
        )),
        "{stderr}"
    );

    let execution_uri = persisted_where_uri(&stderr);
    let why_output = run_guild_success(
        &["why", &execution_uri, "--json", "--color", "never"],
        Some(&registry_root),
    );
    let why_json: Value = parse_json_stdout(&why_output);
    assert_eq!(why_json["record"]["policy_decision"]["outcome"], "allowed");
    assert_eq!(
        why_json["record"]["termination"]["code"],
        "unsupported-runtime-surface"
    );
    assert_eq!(why_json["record"]["termination"]["phase"], "runtime-load");
    assert_eq!(
        why_json["record"]["termination"]["detail"]["classification"],
        "unsupported-runtime-surface"
    );
    assert_eq!(
        why_json["record"]["termination"]["detail"]["surface_kind"],
        "component-import"
    );
}

#[test]
fn policy_denial_errors_surface_actionable_follow_up_guidance() {
    let temp = TempFixtureDir::new("guild-cli-run-policy-denial");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    write_required_emit_evidence_denial_policy(&registry_root);

    let grants_json = emit_evidence_grants_json();
    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(stderr.contains("rejected  proof-backed  exec:"), "{stderr}");
    assert!(stderr.contains("policy-denied"), "{stderr}");
    assert!(
        stderr.contains("authority denial: local policy denied one or more required capabilities"),
        "{stderr}"
    );
    assert!(stderr.contains("reason: policy-denied"), "{stderr}");
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/hello-inspect@^0.1'",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn capability_denial_errors_surface_authority_follow_up_guidance() {
    let temp = TempFixtureDir::new("guild-cli-run-capability-denial");
    let registry_root = temp.path().join("registry");
    install_source_with_cli(&registry_root, &http_source_dir());

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/inspect-http-json@^0.1",
            "--input-json",
            &command_json(json!({ "url": "http://127.0.0.1/blocked.json" })),
            "--grants-json",
            &emit_http_path_denial_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains("authority denial: http-request path was not granted for this execution"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: http-request-path-not-granted"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "hint: request an `http-request` grant whose `allowed_path_prefixes` covers `/blocked.json`"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/inspect-http-json@^0.1'",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn redirect_denials_surface_family_aware_follow_up_guidance() {
    let temp = TempFixtureDir::new("guild-cli-run-http-redirect-denial");
    let registry_root = temp.path().join("registry");
    install_source_with_cli(&registry_root, &http_source_dir());
    let server = http_test_server::HttpTestServer::start();

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/inspect-http-json@^0.1",
            "--input-json",
            &command_json(json!({ "url": server.redirect_json_url() })),
            "--grants-json",
            &emit_http_redirect_denial_grants_json(server.port()),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains(
            "authority denial: http-request received a redirect but follow_redirects was not granted"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: http-request-redirect-not-allowed"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "hint: keep redirects disabled unless needed, or request `follow_redirects=true` with a bounded `max_redirects` and destination limits that still cover the redirect target"
        ),
        "{stderr}"
    );
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
}

#[test]
fn child_capability_mismatch_errors_surface_authority_follow_up_guidance() {
    let temp = TempFixtureDir::new("guild-cli-child-capability-mismatch");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &composite_source_dir());

    let child_skill_dir = installed_skill_dir(&registry_root, "example", "hello-inspect");
    write_installed_manifest(&child_skill_dir, |manifest| {
        let log_write = manifest
            .capabilities
            .iter_mut()
            .find(|capability| capability.id == CapabilityId::LogWrite)
            .expect("hello-inspect log-write capability must exist");
        log_write.required = true;
    });

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-composite@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &composite_invoke_and_emit_evidence_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );

    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(stdout.trim().is_empty(), "{stdout}");
    assert!(
        stderr.contains(
            "authority denial: child invocation required capabilities that were not granted to the parent"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: child-capability-mismatch"),
        "{stderr}"
    );
    assert!(stderr.contains("where: guild://executions/"), "{stderr}");
    assert!(
        stderr.contains(
            "hint: expand the parent request so it covers `log-write` `write`, then compare the parent and child declared capabilities with `guild show -v <skill-ref>`"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/hello-composite@^0.1'",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn json_failures_are_machine_readable_for_authority_and_runtime_failures() {
    let policy_temp = TempFixtureDir::new("guild-cli-json-failure-authority");
    let policy_root = policy_temp.path().join("registry");
    install_with_cli(&policy_root);
    write_required_emit_evidence_denial_policy(&policy_root);

    let policy_output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &emit_evidence_grants_json(),
            "--json",
        ],
        Some(&policy_root),
    );
    let policy_value = parse_failure_json_output(&policy_output);
    assert_eq!(
        policy_value["error"]["category"].as_str(),
        Some("authority denial")
    );
    assert_eq!(
        policy_value["error"]["reason_code"].as_str(),
        Some("policy-denied")
    );
    assert_eq!(
        policy_value["error"]["summary"].as_str(),
        Some("local policy denied one or more required capabilities")
    );
    assert!(
        policy_value["error"]["location"]
            .as_str()
            .is_some_and(|location| location.starts_with("guild://executions/")),
        "{policy_value}"
    );
    assert!(
        policy_value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str().is_some_and(|step| step.contains("show -v"))),
        "{policy_value}"
    );

    let runtime_temp = TempFixtureDir::new("guild-cli-json-failure-runtime");
    let runtime_root = runtime_temp.path().join("registry");
    let broad_source =
        write_broad_import_fixture(runtime_temp.path(), "hello-inspect-json-runtime");
    install_source_with_cli(&runtime_root, &broad_source);

    let runtime_output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-json-runtime@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &emit_evidence_grants_json(),
            "--json",
        ],
        Some(&runtime_root),
    );
    let runtime_value = parse_failure_json_output(&runtime_output);
    assert_eq!(
        runtime_value["error"]["category"].as_str(),
        Some("runtime/compatibility")
    );
    assert_eq!(
        runtime_value["error"]["reason_code"].as_str(),
        Some("unsupported-runtime-surface")
    );
    assert_eq!(
        runtime_value["error"]["summary"].as_str(),
        Some("inspect runtime rejected component import `guild:skill/host@1.0.0`")
    );
    assert!(
        runtime_value["error"]["location"]
            .as_str()
            .is_some_and(|location| location.starts_with("guild://executions/")),
        "{runtime_value}"
    );
    assert!(
        runtime_value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str().is_some_and(|step| step.contains("show -v"))),
        "{runtime_value}"
    );
}

#[test]
fn primary_commands_support_short_refs_and_machine_output_flags() {
    let temp = TempFixtureDir::new("guild-cli-primary-machine");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let show_json = run_guild_success(
        &["show", "hello-inspect@^0.1", "--json"],
        Some(&registry_root),
    );
    let show_value: Value = parse_json_stdout(&show_json);
    assert_eq!(
        show_value["requested_ref"].as_str(),
        Some("hello-inspect@^0.1")
    );
    assert_eq!(
        show_value["support"]["overall"].as_str(),
        Some("not_proven")
    );
    assert_eq!(
        show_value["runtime"].as_str(),
        Some("wasm-component / guild-skill-inspect-v1")
    );
    assert!(!show_json.contains("Next:"), "{show_json}");
    assert!(!show_json.contains("resolution:"), "{show_json}");

    let show_porcelain = run_guild_success(
        &["show", "hello-inspect@^0.1", "--porcelain"],
        Some(&registry_root),
    );
    assert_eq!(
        show_porcelain,
        "skill\texample/hello-inspect@0.1.0\tlocal-source\tlocal-dev\tnot_proven\n"
    );
    assert!(!show_porcelain.contains("Next:"), "{show_porcelain}");

    let verify_porcelain = run_guild_success(
        &["verify", "hello-inspect@^0.1", "--porcelain"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_porcelain,
        "verify\texample/hello-inspect@0.1.0\tlocal-source\tlocal-dev\n"
    );
    assert!(!verify_porcelain.contains("Next:"), "{verify_porcelain}");

    let verify_json = run_guild_success(
        &["verify", "hello-inspect@^0.1", "--json"],
        Some(&registry_root),
    );
    let verify_value: Value = parse_json_stdout(&verify_json);
    assert_eq!(
        verify_value["resolved_skill"].as_str(),
        Some("skill://example/hello-inspect@0.1.0")
    );
    assert!(!verify_json.contains("Next:"), "{verify_json}");

    let grants_json = emit_evidence_grants_json();
    let run_json_output = run_guild_success_output(
        &[
            "run",
            "hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        Some(&registry_root),
    );
    let run_json = String::from_utf8(run_json_output.stdout).unwrap();
    let run_json_stderr = String::from_utf8(run_json_output.stderr).unwrap();
    let run_value: Value = parse_json_stdout(&run_json);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    assert!(!run_json.contains("Next:"), "{run_json}");
    assert!(run_json_stderr.trim().is_empty(), "{run_json_stderr}");

    let why_json_output =
        run_guild_success_output(&["why", &exec_prefix, "--json"], Some(&registry_root));
    let why_json = String::from_utf8(why_json_output.stdout).unwrap();
    let why_json_stderr = String::from_utf8(why_json_output.stderr).unwrap();
    let why_value: Value = parse_json_stdout(&why_json);
    assert_eq!(why_value["summary"]["plan"].as_str(), Some("upper-bound"));
    assert_eq!(why_value["summary"]["proof"].as_str(), Some("not_proven"));
    assert_eq!(why_value["summary"]["token"].as_str(), Some("upper-bound"));
    assert_eq!(why_value["summary"]["witness"].as_str(), Some("unlinked"));
    assert!(!why_json.contains("Next:"), "{why_json}");
    assert!(why_json_stderr.trim().is_empty(), "{why_json_stderr}");

    let why_porcelain =
        run_guild_success(&["why", &exec_prefix, "--porcelain"], Some(&registry_root));
    assert!(
        why_porcelain.starts_with(&format!(
            "why\t{execution_id}\tupper-bound\tnot_proven\tupper-bound\tunlinked\t"
        )),
        "{why_porcelain}"
    );
    assert!(!why_porcelain.contains("Next:"), "{why_porcelain}");
}

#[test]
fn primary_get_ls_and_show_commands_accept_short_resource_refs() {
    let temp = TempFixtureDir::new("guild-cli-resource-short-refs");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_id = inspect_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence_uri = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let blob_uri = inspect_value["record"]["emitted_evidence"][0]["blob_uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let blob_sha = inspect_value["record"]["emitted_evidence"][0]["sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    let evidence_id = evidence_uri.rsplit('/').next().unwrap().to_owned();
    let evidence_prefix = format!("evidence:{}", &evidence_id[..12]);
    let object_prefix = format!("obj:{}", &blob_sha[..12]);

    let get_execution = run_guild_success(&["get", &exec_prefix, "--json"], Some(&registry_root));
    let get_execution_value: Value = parse_json_stdout(&get_execution);
    let get_execution_record: Value =
        serde_json::from_str(get_execution_value["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        get_execution_value["uri"].as_str(),
        Some(execution_uri.as_str())
    );
    assert_eq!(
        get_execution_record["authority_observations_recorded"].as_bool(),
        Some(true)
    );

    let show_execution = run_guild_success(&["show", &exec_prefix, "--json"], Some(&registry_root));
    let show_execution_value: Value = parse_json_stdout(&show_execution);
    assert_eq!(
        show_execution_value["record"]["receipt"]["uri"].as_str(),
        Some(execution_uri.as_str())
    );
    assert!(!show_execution.contains("Next:"), "{show_execution}");

    let evidence_list = run_guild_success(&["ls", "evidence", "--json"], Some(&registry_root));
    let evidence_list_value: Value = parse_json_stdout(&evidence_list);
    assert_eq!(evidence_list_value["evidence_count"].as_u64(), Some(1));
    assert_eq!(
        evidence_list_value["evidence"][0]["uri"].as_str(),
        Some(evidence_uri.as_str())
    );

    let objects_list = run_guild_success(&["ls", "objects", "--json"], Some(&registry_root));
    let objects_list_value: Value = parse_json_stdout(&objects_list);
    assert_eq!(objects_list_value["object_count"].as_u64(), Some(1));
    assert_eq!(
        objects_list_value["objects"][0]["sha256"].as_str(),
        Some(blob_sha.as_str())
    );

    let show_evidence =
        run_guild_success(&["show", &evidence_prefix, "--json"], Some(&registry_root));
    let show_evidence_value: Value = parse_json_stdout(&show_evidence);
    assert_eq!(
        show_evidence_value["record"]["uri"].as_str(),
        Some(evidence_uri.as_str())
    );
    assert!(!show_evidence.contains("Next:"), "{show_evidence}");

    let show_object = run_guild_success(&["show", &object_prefix, "--json"], Some(&registry_root));
    let show_object_value: Value = parse_json_stdout(&show_object);
    assert_eq!(
        show_object_value["record"]["uri"].as_str(),
        Some(blob_uri.as_str())
    );
    assert!(!show_object.contains("Next:"), "{show_object}");

    let show_execution_porcelain =
        run_guild_success(&["show", &exec_prefix, "--porcelain"], Some(&registry_root));
    assert!(
        !show_execution_porcelain.contains("Next:"),
        "{show_execution_porcelain}"
    );

    let show_evidence_porcelain = run_guild_success(
        &["show", &evidence_prefix, "--porcelain"],
        Some(&registry_root),
    );
    assert!(
        !show_evidence_porcelain.contains("Next:"),
        "{show_evidence_porcelain}"
    );

    let show_object_porcelain = run_guild_success(
        &["show", &object_prefix, "--porcelain"],
        Some(&registry_root),
    );
    assert!(
        !show_object_porcelain.contains("Next:"),
        "{show_object_porcelain}"
    );
}

#[test]
fn short_refs_fail_closed_when_ambiguous() {
    let temp = TempFixtureDir::new("guild-cli-short-ref-ambiguity");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let _ = duplicate_installed_hello_with_namespace(&registry_root, "other-example");

    let show_output =
        run_guild_failure_output(&["show", "hello-inspect@^0.1"], Some(&registry_root));
    let show_stderr = String::from_utf8(show_output.stderr).unwrap();
    assert!(
        show_stderr.contains("lookup/ambiguity: short skill ref `hello-inspect@^0.1` was ambiguous across namespaces:"),
        "{show_stderr}"
    );
    assert!(
        show_stderr.contains(
            "short skill ref `hello-inspect@^0.1` was ambiguous across namespaces: example, other-example"
        ) || show_stderr.contains(
            "short skill ref `hello-inspect@^0.1` was ambiguous across namespaces: other-example, example"
        ),
        "{show_stderr}"
    );
    assert!(
        show_stderr.contains(
            "Next: use a fully qualified skill ref such as `skill://<namespace>/<name>@<version-or-range>`"
        ),
        "{show_stderr}"
    );

    let _ = inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let _ = inspect_hello_with_cli(
        &registry_root,
        "Turing",
        "skill://other-example/hello-inspect@^0.1",
    );

    let why_output = run_guild_failure_output(&["why", "exec:0"], Some(&registry_root));
    let why_stderr = String::from_utf8(why_output.stderr).unwrap();
    assert!(
        why_stderr.contains("lookup/ambiguity: execution ref `exec:0` was ambiguous"),
        "{why_stderr}"
    );
    assert!(
        why_stderr.contains(
            "Next: use a longer `exec:` prefix or the full `guild://executions/<id>` URI"
        ),
        "{why_stderr}"
    );
}

#[test]
fn missing_execution_refs_surface_resource_guidance() {
    let temp = TempFixtureDir::new("guild-cli-missing-execution-guidance");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let why_output = run_guild_failure_output(&["why", "exec:deadbeef"], Some(&registry_root));
    let why_stderr = String::from_utf8(why_output.stderr).unwrap();
    assert!(
        why_stderr.contains(
            "resource/read: execution ref `exec:deadbeef` did not match any persisted execution"
        ),
        "{why_stderr}"
    );
    assert!(
        why_stderr.contains(&format!(
            "Next: run `guild --registry-root {} ls runs --limit 5` to find a recent execution, or use a full `guild://executions/<id>` URI",
            registry_root.display()
        )),
        "{why_stderr}"
    );

    let get_output = run_guild_failure_output(
        &["get", "guild://executions/not-real"],
        Some(&registry_root),
    );
    let get_stderr = String::from_utf8(get_output.stderr).unwrap();
    assert!(
        get_stderr
            .contains("resource/read: execution record was not found in the local execution store"),
        "{get_stderr}"
    );
    assert!(
        get_stderr.contains("reason: execution-not-found"),
        "{get_stderr}"
    );
    assert!(
        get_stderr.contains("where: guild://executions/not-real"),
        "{get_stderr}"
    );
    assert!(
        get_stderr.contains(&format!(
            "Next: run `guild --registry-root {} ls runs --limit 5` to find a recent execution, or use a full `guild://executions/<id>` URI",
            registry_root.display()
        )),
        "{get_stderr}"
    );

    let evidence_output =
        run_guild_failure_output(&["get", "evidence:deadbeef"], Some(&registry_root));
    let evidence_stderr = String::from_utf8(evidence_output.stderr).unwrap();
    assert!(
        evidence_stderr.contains(
            "resource/read: evidence ref `evidence:deadbeef` did not match any stored evidence record"
        ),
        "{evidence_stderr}"
    );
    assert!(
        evidence_stderr.contains(&format!(
            "Next: run `guild --registry-root {} ls evidence --limit 5` to inspect stored evidence",
            registry_root.display()
        )),
        "{evidence_stderr}"
    );

    let object_output = run_guild_failure_output(&["show", "obj:deadbeef"], Some(&registry_root));
    let object_stderr = String::from_utf8(object_output.stderr).unwrap();
    assert!(
        object_stderr
            .contains("resource/read: object ref `obj:deadbeef` did not match any stored object"),
        "{object_stderr}"
    );
    assert!(
        object_stderr.contains(&format!(
            "Next: run `guild --registry-root {} ls objects --limit 5` to inspect stored objects",
            registry_root.display()
        )),
        "{object_stderr}"
    );
}

#[test]
fn json_failures_are_machine_readable_for_lookup_and_resource_misses() {
    let temp = TempFixtureDir::new("guild-cli-json-failure-lookup-resource");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let verify_output = run_guild_failure_output(
        &["verify", "missing-skill@^0.1", "--json"],
        Some(&registry_root),
    );
    let verify_value = parse_failure_json_output(&verify_output);
    assert_eq!(
        verify_value["error"]["category"].as_str(),
        Some("lookup/ambiguity")
    );
    assert_eq!(
        verify_value["error"]["summary"].as_str(),
        Some("short skill ref `missing-skill@^0.1` did not match any installed skill")
    );
    assert!(
        verify_value["error"]["reason_code"].is_null(),
        "{verify_value}"
    );
    assert!(
        verify_value["error"]["location"].is_null(),
        "{verify_value}"
    );
    assert!(
        verify_value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step.as_str() == Some("run `guild ls skills` to inspect installed skills")),
        "{verify_value}"
    );

    let why_output =
        run_guild_failure_output(&["why", "exec:deadbeef", "--json"], Some(&registry_root));
    let why_value = parse_failure_json_output(&why_output);
    assert_eq!(
        why_value["error"]["category"].as_str(),
        Some("resource/read")
    );
    assert_eq!(
        why_value["error"]["summary"].as_str(),
        Some("execution ref `exec:deadbeef` did not match any persisted execution")
    );
    assert!(why_value["error"]["reason_code"].is_null(), "{why_value}");
    assert!(why_value["error"]["location"].is_null(), "{why_value}");
    assert!(
        why_value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .is_some_and(|step| step.contains("ls runs --limit 5"))),
        "{why_value}"
    );
}

#[test]
fn json_next_steps_strip_registry_root_qualifiers_with_apostrophes() {
    let temp = TempFixtureDir::new("guild-cli-json-failure-apostrophe-root");
    let registry_root = temp.path().join("O'Reilly guild root");
    install_with_cli(&registry_root);

    let verify_output = run_guild_failure_output(
        &["verify", "missing-skill@^0.1", "--json"],
        Some(&registry_root),
    );
    let verify_value = parse_failure_json_output(&verify_output);
    assert_eq!(
        verify_value["error"]["next_steps"][0].as_str(),
        Some("run `guild ls skills` to inspect installed skills")
    );
    let rendered = serde_json::to_string(&verify_value).unwrap();
    assert!(!rendered.contains("O'Reilly guild root"), "{rendered}");
}

#[test]
fn legacy_aliases_keep_the_json_failure_envelope() {
    let temp = TempFixtureDir::new("guild-cli-json-failure-aliases");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_output = run_guild_failure_output(
        &[
            "inspect",
            "missing-skill@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--json",
        ],
        Some(&registry_root),
    );
    let inspect_value = parse_failure_json_output(&inspect_output);
    assert_eq!(
        inspect_value["error"]["category"].as_str(),
        Some("lookup/ambiguity")
    );
    assert_eq!(
        inspect_value["error"]["summary"].as_str(),
        Some("short skill ref `missing-skill@^0.1` did not match any installed skill")
    );

    let read_output =
        run_guild_failure_output(&["read", "exec:deadbeef", "--json"], Some(&registry_root));
    let read_value = parse_failure_json_output(&read_output);
    assert_eq!(
        read_value["error"]["category"].as_str(),
        Some("resource/read")
    );
    assert_eq!(
        read_value["error"]["summary"].as_str(),
        Some("execution ref `exec:deadbeef` did not match any persisted execution")
    );

    let missing_root = temp.path().join("missing-root");
    let missing_root_display = missing_root.display().to_string();
    let list_output = run_guild(
        &["--registry-root", &missing_root_display, "list", "--json"],
        None,
    );
    assert!(!list_output.status.success(), "{list_output:?}");
    let list_value = parse_failure_json_output(&list_output);
    assert_eq!(list_value["error"]["category"].as_str(), Some("root/setup"));
    assert!(
        list_value["error"]["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("does not exist yet")),
        "{list_value}"
    );
}

#[test]
fn runtime_recovery_hints_shell_quote_copy_pasteable_skill_refs() {
    let temp = TempFixtureDir::new("guild-cli-runtime-hints-quoted-ref");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let _ = duplicate_installed_hello_with_filesystem(&registry_root, "hello-inspect-filesystem");

    let grants_json = emit_filesystem_rejection_grants_json();
    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/hello-inspect-filesystem@>=0.1.0",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} show -v 'skill://example/hello-inspect-filesystem@>=0.1.0'",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn verify_missing_skill_refs_surface_lookup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-missing-verify-skill");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let output = run_guild_failure_output(&["verify", "missing-skill@^0.1"], Some(&registry_root));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "lookup/ambiguity: short skill ref `missing-skill@^0.1` did not match any installed skill"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root {} ls skills` to inspect installed skills",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn color_modes_are_additive_and_not_required_for_machine_output() {
    let temp = TempFixtureDir::new("guild-cli-colors");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let color_output = guild_command(Some(&registry_root))
        .env_remove("NO_COLOR")
        .args(["show", "hello-inspect@^0.1", "--color", "always"])
        .output()
        .unwrap();
    assert!(color_output.status.success(), "{color_output:?}");
    let color_stdout = String::from_utf8(color_output.stdout).unwrap();
    assert!(color_stdout.contains("\u{1b}["), "{color_stdout}");

    let no_color_output = guild_command(Some(&registry_root))
        .env("NO_COLOR", "1")
        .args(["show", "hello-inspect@^0.1", "--color", "always"])
        .output()
        .unwrap();
    assert!(no_color_output.status.success(), "{no_color_output:?}");
    let no_color_stdout = String::from_utf8(no_color_output.stdout).unwrap();
    assert!(!no_color_stdout.contains("\u{1b}["), "{no_color_stdout}");

    let porcelain_output = run_guild_success(
        &[
            "show",
            "hello-inspect@^0.1",
            "--porcelain",
            "--color",
            "always",
        ],
        Some(&registry_root),
    );
    assert_eq!(
        porcelain_output,
        "skill\texample/hello-inspect@0.1.0\tlocal-source\tlocal-dev\tnot_proven\n"
    );
    assert!(!porcelain_output.contains("\u{1b}["), "{porcelain_output}");
}

#[test]
fn read_only_commands_do_not_create_the_default_registry_root() {
    let temp = TempFixtureDir::new("guild-cli-default-root-read");
    let home_dir = temp.path().join("home");
    fs::create_dir_all(&home_dir).unwrap();
    let default_root = home_dir.join(".guild");

    let output = run_guild_with_home(&["list", "--json"], &home_dir);
    assert!(!output.status.success());

    let value = parse_failure_json_output(&output);
    assert_eq!(value["error"]["category"].as_str(), Some("root/setup"));
    assert!(
        value["error"]["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains("Guild registry root `")),
        "{value}"
    );
    assert_eq!(
        value["error"]["detail"].as_str(),
        Some("read-only commands do not initialize a new root")
    );
    assert!(
        value["error"]["summary"]
            .as_str()
            .is_some_and(|summary| summary.contains(default_root.to_string_lossy().as_ref())),
        "{value}"
    );
    assert!(
        value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .is_some_and(|step| step.contains("guild install <source-dir>"))),
        "{value}"
    );
    assert!(!default_root.exists());
}

#[test]
fn read_only_commands_qualify_missing_non_default_root_follow_up() {
    let temp = TempFixtureDir::new("guild-cli-missing-non-default-root");
    let registry_root = temp.path().join("custom registry");
    let registry_root_display = registry_root.display().to_string();

    let output = run_guild(&["--registry-root", &registry_root_display, "ls"], None);
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(&format!(
            "root/setup: Guild registry root `{}` does not exist yet",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains("detail: read-only commands do not initialize a new root"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root '{}' install <source-dir>` to create it",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!registry_root.exists());
}

#[test]
fn install_missing_source_directories_surface_usage_guidance() {
    let temp = TempFixtureDir::new("guild-cli-install-missing-source");
    let registry_root = temp.path().join("registry");
    let missing_source = temp.path().join("missing-skill");
    let missing_source_display = missing_source.display().to_string();

    let output =
        run_guild_failure_output(&["install", &missing_source_display], Some(&registry_root));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("usage: source skill directory does not exist"),
        "{stderr}"
    );
    assert!(stderr.contains("reason: source-root-missing"), "{stderr}");
    assert!(
        stderr.contains(&format!("where: {}", missing_source.display())),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: confirm the source directory exists, then rerun `guild --registry-root {} install <source-dir>`",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn default_registry_root_is_used_when_no_override_is_present() {
    let temp = TempFixtureDir::new("guild-cli-default-root-write");
    let home_dir = temp.path().join("home");
    fs::create_dir_all(&home_dir).unwrap();
    let default_root = home_dir.join(".guild");
    let source_root = hello_source_dir().display().to_string();

    let _ = run_guild_success_with_home(&["install", &source_root], &home_dir);
    assert!(default_root.join("installed").exists());

    let grants_json = emit_evidence_grants_json();
    let inspect_output = run_guild_success_with_home(
        &[
            "inspect",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        &home_dir,
    );
    let inspect_value: Value = parse_json_stdout(&inspect_output);
    assert_eq!(
        inspect_value["summary"].as_str(),
        Some("Hello, Ada. Guild inspect is working."),
    );
}

#[test]
fn init_creates_the_default_registry_root() {
    let temp = TempFixtureDir::new("guild-cli-init-default-root");
    let home_dir = temp.path().join("home");
    let default_root = home_dir.join(".guild");
    fs::create_dir_all(&home_dir).unwrap();

    let stdout = run_guild_success_with_home(&["init"], &home_dir);
    assert!(stdout.contains("Guild init ready."));
    assert!(stdout.contains(default_root.to_string_lossy().as_ref()));
    assert!(stdout.contains("status: created"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_guild")));
    assert!(stdout.contains("Codex CLI registration:"));
    assert!(stdout.contains("Codex MCP config snippet:"));
    assert!(stdout.contains("mcp serve --stdio"));
    assert!(default_root.exists());
    assert!(default_root.join("installed").exists());
    assert!(default_root.join("executions").exists());
}

#[test]
fn init_can_fold_in_codex_setup_for_the_default_root() {
    let temp = TempFixtureDir::new("guild-cli-init-codex");
    let home_dir = temp.path().join("home");
    let project_dir = temp.path().join("project");
    let default_root = home_dir.join(".guild");
    let global_config = home_dir.join(".codex").join("config.toml");
    let project_config = project_dir.join(".codex").join("config.toml");
    fs::create_dir_all(&home_dir).unwrap();
    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(global_config.parent().unwrap()).unwrap();
    fs::create_dir_all(project_config.parent().unwrap()).unwrap();
    fs::write(&global_config, "[profiles]\ndefault = \"safe\"\n").unwrap();
    fs::write(&project_config, "[project]\nname = \"demo\"\n").unwrap();

    let args = ["init", "--global", "--project"];
    let first_stdout = run_guild_success_with_home_and_cwd(&args, &home_dir, &project_dir);
    assert!(default_root.exists());
    assert!(first_stdout.contains("Guild init ready."));
    assert!(first_stdout.contains("updated:"));
    assert!(first_stdout.contains(env!("CARGO_BIN_EXE_guild")));

    let first_global = fs::read_to_string(&global_config).unwrap();
    let first_project = fs::read_to_string(&project_config).unwrap();
    assert!(first_global.contains("[profiles]"));
    assert!(first_global.contains("[mcp_servers.guild-local]"));
    assert!(first_global.contains(env!("CARGO_BIN_EXE_guild")));
    assert!(!first_global.contains("--registry-root"));
    assert!(first_project.contains("[project]"));
    assert!(first_project.contains("[mcp_servers.guild-local]"));

    let second_stdout = run_guild_success_with_home_and_cwd(&args, &home_dir, &project_dir);
    assert!(second_stdout.contains("unchanged:"));
    assert_eq!(first_global, fs::read_to_string(&global_config).unwrap());
    assert_eq!(first_project, fs::read_to_string(&project_config).unwrap());
}

#[test]
fn env_registry_root_overrides_the_default_home_root() {
    let temp = TempFixtureDir::new("guild-cli-env-root");
    let home_dir = temp.path().join("home");
    let env_root = temp.path().join("env-root");
    let default_root = home_dir.join(".guild");
    fs::create_dir_all(&home_dir).unwrap();
    install_with_cli(&env_root);

    let grants_json = emit_evidence_grants_json();
    let output = guild_command_with_options(Some(&env_root), Some(&home_dir), None)
        .args([
            "inspect",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!default_root.exists());
}

#[test]
fn explicit_registry_root_overrides_env_and_bare_alias_remains_accepted() {
    let temp = TempFixtureDir::new("guild-cli-root-precedence");
    let explicit_root = temp.path().join("registry-explicit");
    let env_root = temp.path().join("registry-env");
    fs::create_dir_all(&env_root).unwrap();
    install_with_cli(&explicit_root);

    let grants_json = emit_evidence_grants_json();
    let explicit_root_display = explicit_root.display().to_string();
    let inspect_output = run_guild_success(
        &[
            "--registry-root",
            &explicit_root_display,
            "inspect",
            "example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        Some(&env_root),
    );
    let inspect_value: Value = parse_json_stdout(&inspect_output);
    assert_eq!(
        inspect_value["summary"].as_str(),
        Some("Hello, Ada. Guild inspect is working."),
    );
}

#[test]
fn list_commands_show_installed_skills_and_recent_executions() {
    let temp = TempFixtureDir::new("guild-cli-list");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();

    let summary_output = run_guild_success(&["list", "--json"], Some(&registry_root));
    let summary: Value = parse_json_stdout(&summary_output);
    assert_eq!(summary["installed_count"].as_u64(), Some(1));
    assert_eq!(summary["recent_execution_limit"].as_u64(), Some(10));
    assert_eq!(summary["recent_execution_count"].as_u64(), Some(1));
    assert!(
        summary["installed"][0]["resolved_skill"]
            .as_str()
            .unwrap()
            .starts_with("skill://example/hello-inspect@")
    );
    assert!(
        summary["recent_executions"][0]["resolved_skill"]
            .as_str()
            .unwrap()
            .starts_with("skill://example/hello-inspect@")
    );
    assert_eq!(
        summary["recent_executions"][0]["uri"].as_str(),
        Some(execution_uri.as_str()),
    );

    let skills_output = run_guild_success(&["list", "skills", "--json"], Some(&registry_root));
    let skills: Value = parse_json_stdout(&skills_output);
    assert_eq!(skills["installed_count"].as_u64(), Some(1));
    assert_eq!(skills["installed"].as_array().unwrap().len(), 1);

    let executions_output = run_guild_success(
        &["list", "executions", "--limit", "1", "--json"],
        Some(&registry_root),
    );
    let executions: Value = parse_json_stdout(&executions_output);
    assert_eq!(executions["limit"].as_u64(), Some(1));
    assert_eq!(executions["execution_count"].as_u64(), Some(1));
    assert_eq!(
        executions["executions"][0]["status"].as_str(),
        Some("succeeded"),
    );
    assert_eq!(
        executions["executions"][0]["uri"].as_str(),
        Some(execution_uri.as_str()),
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn install_export_import_and_trust_commands_work_for_bundle_transport() {
    let temp = TempFixtureDir::new("guild-cli-bundle");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);

    let generate_output = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
            "--json",
        ],
        None,
    );
    let generated: Value = parse_json_stdout(&generate_output);
    assert_eq!(generated["publisher_id"].as_str(), Some("local.example"));

    let export_output = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
            "--json",
        ],
        None,
    );
    let exported: Value = parse_json_stdout(&export_output);
    assert_eq!(exported["format"].as_str(), Some("bundle"));

    let trust_add_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
            "--json",
        ],
        None,
    );
    let trust_added: Value = parse_json_stdout(&trust_add_output);
    assert_eq!(trust_added["publisher_id"].as_str(), Some("local.example"));

    let trust_list_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "list",
            "--json",
        ],
        None,
    );
    let trust_list: Value = parse_json_stdout(&trust_list_output);
    assert_eq!(trust_list["publishers"].as_array().unwrap().len(), 1);
    let trust_show_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "show",
            "local.example",
            "--json",
        ],
        None,
    );
    let trust_show: Value = parse_json_stdout(&trust_show_output);
    assert_eq!(
        trust_show["publisher"]["publisher"]["id"],
        json!("local.example")
    );

    let import_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
            "--json",
        ],
        None,
    );
    let imported: Value = parse_json_stdout(&import_output);
    assert_eq!(imported["format"].as_str(), Some("bundle"));
    assert_eq!(imported["installed"].as_array().unwrap().len(), 1);

    let grants_json = emit_evidence_grants_json();
    let inspect_output = run_guild_success(
        &[
            "inspect",
            "skill://example/hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Turing" })),
            "--grants-json",
            &grants_json,
            "--json",
        ],
        Some(&registry_b),
    );
    let inspect_value: Value = parse_json_stdout(&inspect_output);
    assert_eq!(
        inspect_value["summary"].as_str(),
        Some("Hello, Turing. Guild inspect is working."),
    );

    let remove_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "remove",
            "local.example",
        ],
        None,
    );
    assert!(remove_output.contains("removed trusted publisher local.example"));
}

#[test]
fn trust_show_renders_human_review_output() {
    let temp = TempFixtureDir::new("guild-cli-trust-show-human");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "local.example",
        ],
        None,
    );

    assert_eq!(output, expected_trust_list_output());
}

#[test]
fn trust_show_json_returns_one_trusted_publisher_record() {
    let temp = TempFixtureDir::new("guild-cli-trust-show-json");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "--json",
            "local.example",
        ],
        None,
    );
    let shown: Value = parse_json_stdout(&output);

    assert_eq!(shown["registry_root"], json!(registry_root_display));
    assert_eq!(
        shown["publisher"]["publisher"]["id"],
        json!("local.example")
    );
    assert_eq!(
        shown["publisher"]["publisher"]["display_name"],
        json!("Local Example")
    );
    assert_eq!(shown["publisher"]["trust_tier"], json!("trusted-imported"));
}

#[test]
fn export_bundle_renders_human_transport_review_output() {
    let temp = TempFixtureDir::new("guild-cli-export-human-review");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_root);
    generate_identity_with_cli(&identity_path);

    let export_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );

    assert_eq!(
        export_output,
        expected_export_review_output("bundle", &bundle_root)
    );
}

#[test]
fn import_bundle_renders_human_trust_review_output() {
    let temp = TempFixtureDir::new("guild-cli-bundle-human-review");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let import_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
        ],
        None,
    );
    assert_eq!(
        import_output,
        expected_import_review_output(&registry_b, "bundle", &bundle)
    );
}

#[test]
fn import_bundle_preview_json_reports_would_import_without_installing() {
    let temp = TempFixtureDir::new("guild-cli-bundle-preview-json");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let preview_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
            "--preview",
            "--json",
        ],
        None,
    );
    let preview: Value = parse_json_stdout(&preview_output);

    assert_eq!(preview["preview"], json!(true));
    assert_eq!(preview["format"], json!("bundle"));
    assert_eq!(preview["decision"], json!("would-import"));
    assert_eq!(
        preview["root_skill"],
        json!("skill://example/hello-inspect@0.1.0")
    );
    assert_eq!(preview["publisher_id"], json!("local.example"));
    assert_eq!(preview["verified"], json!(true));
    assert_eq!(preview["trust_tier"], json!("trusted-imported"));
    assert_eq!(preview["refusal"], Value::Null);
    assert_eq!(preview["skill_count"], json!(1));

    let installed = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "ls",
            "skills",
            "--json",
        ],
        None,
    );
    let installed: Value = parse_json_stdout(&installed);
    assert_eq!(installed["installed_count"], json!(0));
}

#[test]
fn trust_remove_missing_publishers_surface_lookup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-trust-remove-missing");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "remove",
            "local.example",
        ],
        None,
    );

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "remove",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("lookup/ambiguity: trusted publisher `local.example` was not present"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root {} trust list` to inspect the current trusted publisher entries",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn trust_show_missing_publishers_surface_lookup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-trust-show-missing");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "remove",
            "local.example",
        ],
        None,
    );

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("lookup/ambiguity: trusted publisher `local.example` was not present"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root {} trust list` to inspect the current trusted publisher entries",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn trust_read_commands_do_not_initialize_existing_directories() {
    let temp = TempFixtureDir::new("guild-cli-trust-read-no-init");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    fs::create_dir_all(&registry_root).unwrap();

    let trust_list_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "list",
            "--json",
        ],
        None,
    );
    let trust_list: Value = parse_json_stdout(&trust_list_output);
    assert_eq!(trust_list["publishers"].as_array().unwrap().len(), 0);
    assert!(!registry_root.join("trust").exists());
    assert!(!registry_root.join("installed").exists());

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("lookup/ambiguity: trusted publisher `local.example` was not present"),
        "{stderr}"
    );
    assert!(!registry_root.join("trust").exists());
    assert!(!registry_root.join("installed").exists());
}

#[test]
fn broken_local_trust_records_surface_root_setup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-broken-trust-record");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();
    let trusted_record_path = registry_root
        .join("trust")
        .join("publishers")
        .join("local.example.json");

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    fs::write(&trusted_record_path, b"{not valid json").unwrap();

    let output = run_guild_failure_output(
        &["--registry-root", &registry_root_display, "trust", "list"],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("root/setup: failed to parse trusted publisher record"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: trusted-publisher-parse-failed"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: fix or remove the broken local trust record under the selected Guild root, then rerun `guild --registry-root {} trust list`",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn broken_trust_show_records_surface_root_setup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-broken-trust-show-record");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();
    let trusted_record_path = registry_root
        .join("trust")
        .join("publishers")
        .join("local.example.json");

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    fs::write(&trusted_record_path, b"{not valid json").unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("root/setup: failed to parse trusted publisher record"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: trusted-publisher-parse-failed"),
        "{stderr}"
    );
}

#[test]
fn mismatched_trust_show_records_surface_root_setup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-mismatched-trust-show-record");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();
    let trusted_record_path = registry_root
        .join("trust")
        .join("publishers")
        .join("local.example.json");

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let mut trusted_record: Value =
        serde_json::from_str(&fs::read_to_string(&trusted_record_path).unwrap()).unwrap();
    trusted_record["publisher"]["id"] = json!("other.example");
    fs::write(
        &trusted_record_path,
        serde_json::to_vec_pretty(&trusted_record).unwrap(),
    )
    .unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "show",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "root/setup: trusted publisher record did not match the requested publisher id"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: trusted-publisher-id-mismatch"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: fix or remove the broken local trust record under the selected Guild root, then rerun `guild --registry-root {} trust list`",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn trust_remove_store_io_failures_surface_root_setup_guidance() {
    let temp = TempFixtureDir::new("guild-cli-trust-remove-io-failure");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();
    let identity_path = temp.path().join("publisher.json");
    let identity = identity_path.display().to_string();
    let trusted_record_path = registry_root
        .join("trust")
        .join("publishers")
        .join("local.example.json");

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    fs::remove_file(&trusted_record_path).unwrap();
    fs::create_dir_all(&trusted_record_path).unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "remove",
            "local.example",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("root/setup: failed to remove trusted publisher record"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: trusted-publisher-remove-failed"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: fix the local trust store under the selected Guild root, then rerun `guild --registry-root {} trust list` or `guild --registry-root {} trust remove <publisher-id>`",
            registry_root.display(),
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn trust_sign_and_verify_plan_commands_work() {
    let temp = TempFixtureDir::new("guild-cli-plan-sign");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let signed_plan_path = temp.path().join("signed-plan.json");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let plan = draft_plan_path("zero-authority.admit.plan.json")
        .display()
        .to_string();
    let signed_plan = signed_plan_path.display().to_string();

    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let sign_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "sign-plan",
            "--plan",
            &plan,
            "--identity-file",
            &identity,
            "--output",
            &signed_plan,
            "--json",
        ],
        None,
    );
    let signed_output: Value = parse_json_stdout(&sign_output);
    assert_eq!(
        signed_output["publisher_id"].as_str(),
        Some("local.example")
    );
    assert_eq!(
        signed_output["signed_digest"]["algorithm"].as_str(),
        Some("sha256")
    );

    let signed_plan_json: Value =
        serde_json::from_str(&fs::read_to_string(&signed_plan_path).unwrap()).unwrap();
    assert_eq!(
        signed_plan_json["plan_signature"]["publisher_id"].as_str(),
        Some("local.example")
    );

    let verify_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
            "--plan",
            &signed_plan,
            "--json",
        ],
        None,
    );
    let verified_output: Value = parse_json_stdout(&verify_output);
    assert_eq!(verified_output["verified"].as_bool(), Some(true));
    assert_eq!(
        verified_output["publisher_id"].as_str(),
        Some("local.example")
    );
    assert_eq!(
        verified_output["signed_digest"]["algorithm"].as_str(),
        Some("sha256")
    );
}

#[test]
fn trust_sign_and_verify_plan_human_output_is_review_friendly() {
    let temp = TempFixtureDir::new("guild-cli-plan-sign-human");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let signed_plan_path = temp.path().join("signed-plan.json");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let plan = draft_plan_path("zero-authority.admit.plan.json")
        .display()
        .to_string();
    let signed_plan = signed_plan_path.display().to_string();

    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let sign_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "sign-plan",
            "--plan",
            &plan,
            "--identity-file",
            &identity,
            "--output",
            &signed_plan,
        ],
        None,
    );
    assert!(
        sign_output.contains("signed execution plan"),
        "{sign_output}"
    );
    assert!(
        sign_output.contains("publisher: local.example"),
        "{sign_output}"
    );
    assert!(sign_output.contains("digest: sha256:"), "{sign_output}");
    assert!(
        sign_output.contains(&format!("output: {}", signed_plan_path.display())),
        "{sign_output}"
    );
    assert!(
        sign_output.contains(&format!(
            "Next: guild --registry-root {} trust verify-plan --plan {}",
            registry_root_display,
            signed_plan_path.display()
        )),
        "{sign_output}"
    );

    let verify_output = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
            "--plan",
            &signed_plan,
        ],
        None,
    );
    assert!(
        verify_output.contains("verified signed execution plan"),
        "{verify_output}"
    );
    assert!(
        verify_output.contains("publisher: local.example"),
        "{verify_output}"
    );
    assert!(
        verify_output.contains("status: verified / trusted-imported"),
        "{verify_output}"
    );
    assert!(verify_output.contains("digest: sha256:"), "{verify_output}");
}

#[test]
fn trust_verify_plan_argument_errors_stay_in_usage() {
    let temp = TempFixtureDir::new("guild-cli-plan-verify-usage");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let registry_root_display = registry_root.display().to_string();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("usage: error: the following required arguments were not provided:"),
        "{stderr}"
    );
    assert!(
        stderr.contains("Usage: guild trust verify-plan --plan <PLAN>"),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn trust_verify_plan_rejects_tampered_signed_plan() {
    let temp = TempFixtureDir::new("guild-cli-plan-verify-fail");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let signed_plan_path = temp.path().join("signed-plan.json");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let plan = draft_plan_path("zero-authority.admit.plan.json")
        .display()
        .to_string();
    let signed_plan = signed_plan_path.display().to_string();

    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "trust",
            "sign-plan",
            "--plan",
            &plan,
            "--identity-file",
            &identity,
            "--output",
            &signed_plan,
        ],
        None,
    );

    let mut signed_plan_json: Value =
        serde_json::from_str(&fs::read_to_string(&signed_plan_path).unwrap()).unwrap();
    signed_plan_json["decision"] = Value::String("downgrade".into());
    fs::write(
        &signed_plan_path,
        serde_json::to_vec_pretty(&signed_plan_json).unwrap(),
    )
    .unwrap();

    let output = run_guild(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
            "--plan",
            &signed_plan,
        ],
        None,
    );
    assert!(
        !output.status.success(),
        "verify-plan unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "trust/verification: execution plan signature metadata did not match the execution plan bytes"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: execution-plan-signature-digest-mismatch"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: confirm the signed plan file was not modified after signing, or rerun `guild --registry-root {} trust sign-plan --plan <plan.json> --identity-file <identity.json> --output <signed-plan.json>`",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn json_failures_are_machine_readable_for_signed_plan_verification_failures() {
    let temp = TempFixtureDir::new("guild-cli-json-plan-verify-fail");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let signed_plan_path = temp.path().join("signed-plan.json");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let plan = draft_plan_path("zero-authority.admit.plan.json")
        .display()
        .to_string();
    let signed_plan = signed_plan_path.display().to_string();

    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "trust",
            "sign-plan",
            "--plan",
            &plan,
            "--identity-file",
            &identity,
            "--output",
            &signed_plan,
        ],
        None,
    );

    let mut signed_plan_json: Value =
        serde_json::from_str(&fs::read_to_string(&signed_plan_path).unwrap()).unwrap();
    signed_plan_json["decision"] = Value::String("downgrade".into());
    fs::write(
        &signed_plan_path,
        serde_json::to_vec_pretty(&signed_plan_json).unwrap(),
    )
    .unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
            "--plan",
            &signed_plan,
            "--json",
        ],
        None,
    );
    let value = parse_failure_json_output(&output);
    assert_eq!(
        value["error"]["category"].as_str(),
        Some("trust/verification")
    );
    assert_eq!(
        value["error"]["reason_code"].as_str(),
        Some("execution-plan-signature-digest-mismatch")
    );
    assert_eq!(
        value["error"]["summary"].as_str(),
        Some("execution plan signature metadata did not match the execution plan bytes")
    );
    assert!(value["error"]["location"].is_null(), "{value}");
    assert!(
        value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .is_some_and(|step| step.contains("trust sign-plan"))),
        "{value}"
    );
}

#[test]
fn trust_verify_plan_unsupported_signature_format_surfaces_compatibility_guidance() {
    let temp = TempFixtureDir::new("guild-cli-plan-verify-format-skew");
    let registry_root = temp.path().join("registry");
    let identity_path = temp.path().join("publisher.json");
    let signed_plan_path = temp.path().join("signed-plan.json");
    let registry_root_display = registry_root.display().to_string();
    let identity = identity_path.display().to_string();
    let plan = draft_plan_path("zero-authority.admit.plan.json")
        .display()
        .to_string();
    let signed_plan = signed_plan_path.display().to_string();

    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "trust",
            "sign-plan",
            "--plan",
            &plan,
            "--identity-file",
            &identity,
            "--output",
            &signed_plan,
        ],
        None,
    );

    let mut signed_plan_json: Value =
        serde_json::from_str(&fs::read_to_string(&signed_plan_path).unwrap()).unwrap();
    signed_plan_json["plan_signature"]["format_version"] =
        Value::String("guild-plan-signature-v999".into());
    fs::write(
        &signed_plan_path,
        serde_json::to_vec_pretty(&signed_plan_json).unwrap(),
    )
    .unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_root_display,
            "trust",
            "verify-plan",
            "--plan",
            &signed_plan,
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "runtime/compatibility: execution plan signature format version is unsupported"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: execution-plan-signature-format-unsupported"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: confirm the target Guild build supports this signed plan format version, or rerun `guild --registry-root {} trust sign-plan --plan <plan.json> --identity-file <identity.json> --output <signed-plan.json>` with a compatible Guild version",
            registry_root.display()
        )),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn import_bundle_untrusted_publishers_surface_trust_guidance() {
    let temp = TempFixtureDir::new("guild-cli-import-untrusted");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "trust/verification: signed bundle publisher was not trusted by the target Guild root"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: bundle-publisher-untrusted"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root {} trust list`",
            registry_b.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "guild --registry-root {} trust add --identity-file <identity.json>",
            registry_b.display()
        )),
        "{stderr}"
    );
}

#[test]
fn json_failures_are_machine_readable_for_untrusted_bundle_imports() {
    let temp = TempFixtureDir::new("guild-cli-json-import-untrusted");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
            "--json",
        ],
        None,
    );
    let value = parse_failure_json_output(&output);
    assert_eq!(
        value["error"]["category"].as_str(),
        Some("trust/verification")
    );
    assert_eq!(
        value["error"]["reason_code"].as_str(),
        Some("bundle-publisher-untrusted")
    );
    assert_eq!(
        value["error"]["summary"].as_str(),
        Some("signed bundle publisher was not trusted by the target Guild root")
    );
    assert_eq!(value["error"]["location"].as_str(), Some("local.example"));
    assert!(
        value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .is_some_and(|step| step.contains("trust list"))),
        "{value}"
    );
    assert!(
        value["error"]["next_steps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step
                .as_str()
                .is_some_and(|step| step.contains("trust add --identity-file <identity.json>"))),
        "{value}"
    );
}

#[test]
fn import_bundle_tampered_content_surfaces_integrity_guidance() {
    let temp = TempFixtureDir::new("guild-cli-import-tampered");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    let installed = install_source_with_cli_json(&registry_a, &hello_source_dir());
    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let installed_manifest_path = installed["manifest_path"].as_str().unwrap();
    let installed_manifest: SkillManifest =
        serde_json::from_str(&fs::read_to_string(installed_manifest_path).unwrap()).unwrap();
    let digest_dir = installed["digest"].as_str().unwrap().replace(':', "-");
    let component_path = bundle_root
        .join("installed")
        .join(&installed_manifest.key.namespace)
        .join(&installed_manifest.key.name)
        .join(installed_manifest.version.to_string())
        .join(digest_dir)
        .join("component.wasm");
    fs::write(&component_path, b"tampered artifact").unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("trust/verification: artifact digest does not match manifest"),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: artifact-digest-mismatch"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "Next: confirm the signed bundle or OCI artifact was not modified after export, or fetch a fresh copy from the publisher before rerunning the import or pull"
        ),
        "{stderr}"
    );
}

#[test]
fn import_bundle_unsupported_format_surfaces_compatibility_guidance() {
    let temp = TempFixtureDir::new("guild-cli-import-unsupported-format");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let bundle_root = temp.path().join("bundle");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();
    let bundle = bundle_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "bundle",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &bundle,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let bundle_json_path = bundle_root.join("bundle.json");
    let mut bundle_json: Value =
        serde_json::from_str(&fs::read_to_string(&bundle_json_path).unwrap()).unwrap();
    bundle_json["format_version"] = Value::String("guild-installed-bundle-v999".into());
    fs::write(
        &bundle_json_path,
        serde_json::to_vec_pretty(&bundle_json).unwrap(),
    )
    .unwrap();

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "bundle",
            &bundle,
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "runtime/compatibility: installed skill bundle format version is unsupported"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: bundle-format-unsupported"),
        "{stderr}"
    );
    assert!(
        stderr.contains(
            "Next: confirm the target Guild build supports this bundle format version, or re-export with a compatible Guild version before rerunning the import or pull"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("trust/verification:"), "{stderr}");
}

#[test]
fn pull_untrusted_publishers_surface_trust_guidance() {
    let temp = TempFixtureDir::new("guild-cli-pull-untrusted");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let identity_path = temp.path().join("publisher.json");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);
    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = server.reference("guild-example-hello-inspect", "0.1.0");
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "push",
            "skill://example/hello-inspect@^0.1",
            "--reference",
            &reference,
            "--signer",
            &identity,
            "--allow-http",
        ],
        None,
    );

    let output = run_guild_failure_output(
        &[
            "--registry-root",
            &registry_b_root,
            "pull",
            &reference,
            "--allow-http",
        ],
        None,
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains(
            "trust/verification: signed bundle publisher was not trusted by the target Guild root"
        ),
        "{stderr}"
    );
    assert!(
        stderr.contains("reason: bundle-publisher-untrusted"),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Next: run `guild --registry-root {} trust list`",
            registry_b.display()
        )),
        "{stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "guild --registry-root {} trust add --identity-file <identity.json>",
            registry_b.display()
        )),
        "{stderr}"
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn export_import_layout_and_push_pull_commands_work_for_oci_transport() {
    let temp = TempFixtureDir::new("guild-cli-oci");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let registry_c = temp.path().join("registry-c");
    let identity_path = temp.path().join("publisher.json");
    let layout_root = temp.path().join("layout");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let registry_c_root = registry_c.display().to_string();
    let identity = identity_path.display().to_string();
    let layout = layout_root.display().to_string();

    install_with_cli(&registry_a);

    let _ = run_guild_success(
        &[
            "trust",
            "generate",
            "--publisher-id",
            "local.example",
            "--display-name",
            "Local Example",
            "--output",
            &identity,
        ],
        None,
    );

    let layout_output = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "oci-layout",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &layout,
            "--json",
        ],
        None,
    );
    let layout_value: Value = parse_json_stdout(&layout_output);
    assert_eq!(layout_value["format"].as_str(), Some("oci-layout"));

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    let import_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "oci-layout",
            &layout,
            "--json",
        ],
        None,
    );
    let imported: Value = parse_json_stdout(&import_output);
    assert_eq!(imported["format"].as_str(), Some("oci-layout"));
    assert_eq!(imported["installed"].as_array().unwrap().len(), 1);

    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = server.reference("guild-example-hello-inspect", "0.1.0");
    let push_output = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "push",
            "skill://example/hello-inspect@^0.1",
            "--reference",
            &reference,
            "--signer",
            &identity,
            "--allow-http",
            "--json",
        ],
        None,
    );
    let pushed: Value = parse_json_stdout(&push_output);
    assert_eq!(pushed["reference"].as_str(), Some(reference.as_str()));

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_c_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );
    let pull_output = run_guild_success(
        &[
            "--registry-root",
            &registry_c_root,
            "pull",
            &reference,
            "--allow-http",
            "--json",
        ],
        None,
    );
    let pulled: Value = parse_json_stdout(&pull_output);
    assert_eq!(pulled["format"].as_str(), Some("oci-registry"));
    assert_eq!(pulled["installed"].as_array().unwrap().len(), 1);
}

#[test]
#[allow(clippy::too_many_lines)]
fn import_layout_and_pull_render_human_trust_review_output() {
    let temp = TempFixtureDir::new("guild-cli-oci-human-review");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let registry_c = temp.path().join("registry-c");
    let identity_path = temp.path().join("publisher.json");
    let layout_root = temp.path().join("layout");
    let registry_store = temp.path().join("oci-registry-store");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let registry_c_root = registry_c.display().to_string();
    let identity = identity_path.display().to_string();
    let layout = layout_root.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "export",
            "oci-layout",
            "skill://example/hello-inspect@^0.1",
            "--signer",
            &identity,
            "--output",
            &layout,
        ],
        None,
    );
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let import_layout_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "import",
            "oci-layout",
            &layout,
        ],
        None,
    );
    assert_eq!(
        import_layout_output,
        expected_import_review_output(&registry_b, "oci-layout", &layout)
    );

    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = server.reference("guild-example-hello-inspect", "0.1.0");
    let push_output = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "push",
            "skill://example/hello-inspect@^0.1",
            "--reference",
            &reference,
            "--signer",
            &identity,
            "--allow-http",
        ],
        None,
    );
    assert!(
        push_output.contains("published installed state"),
        "{push_output}"
    );
    assert!(
        push_output.contains("transport: oci-registry"),
        "{push_output}"
    );
    assert!(
        push_output.contains("skill: skill://example/hello-inspect@0.1.0"),
        "{push_output}"
    );
    assert!(
        push_output.contains("publisher: local.example"),
        "{push_output}"
    );
    assert!(
        push_output.contains("contents: root skill only"),
        "{push_output}"
    );
    assert!(
        push_output.contains(&format!("reference: {reference}")),
        "{push_output}"
    );
    assert!(push_output.contains("manifest: sha256:"), "{push_output}");
    assert!(
        push_output.contains(&format!("Next: guild pull '{reference}' --allow-http")),
        "{push_output}"
    );

    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_c_root,
            "trust",
            "add",
            "--identity-file",
            &identity,
        ],
        None,
    );

    let pull_output = run_guild_success(
        &[
            "--registry-root",
            &registry_c_root,
            "pull",
            &reference,
            "--allow-http",
        ],
        None,
    );
    assert_eq!(
        pull_output,
        expected_import_review_output(&registry_c, "oci-registry", &reference)
    );
}

#[test]
fn pull_preview_renders_refusal_without_installing() {
    let temp = TempFixtureDir::new("guild-cli-pull-preview-refusal");
    let registry_a = temp.path().join("registry-a");
    let registry_b = temp.path().join("registry-b");
    let registry_store = temp.path().join("oci-registry-store");
    let identity_path = temp.path().join("publisher.json");
    let registry_a_root = registry_a.display().to_string();
    let registry_b_root = registry_b.display().to_string();
    let identity = identity_path.display().to_string();

    install_with_cli(&registry_a);
    generate_identity_with_cli(&identity_path);

    let server = oci_registry_test_server::OciRegistryTestServer::start(&registry_store);
    let reference = server.reference("guild-example-hello-inspect", "0.1.0");
    let _ = run_guild_success(
        &[
            "--registry-root",
            &registry_a_root,
            "push",
            "skill://example/hello-inspect@^0.1",
            "--reference",
            &reference,
            "--signer",
            &identity,
            "--allow-http",
        ],
        None,
    );
    let _ = run_guild_success(&["--registry-root", &registry_b_root, "init"], None);

    let preview_output = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "pull",
            &reference,
            "--allow-http",
            "--preview",
        ],
        None,
    );

    assert_import_preview_output(
        &preview_output,
        "oci-registry",
        &reference,
        "would-refuse",
        "untrusted",
    );
    assert!(
        preview_output.contains(
            "reason: bundle-publisher-untrusted: signed bundle publisher was not trusted by the target Guild root"
        ),
        "{preview_output}"
    );
    assert!(!preview_output.contains("Next:"), "{preview_output}");

    let installed = run_guild_success(
        &[
            "--registry-root",
            &registry_b_root,
            "ls",
            "skills",
            "--json",
        ],
        None,
    );
    let installed: Value = parse_json_stdout(&installed);
    assert_eq!(installed["installed_count"], json!(0));
}

struct McpHarness {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpHarness {
    fn spawn(registry_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_guild"))
            .current_dir(repo_root())
            .arg("--registry-root")
            .arg(registry_root)
            .arg("mcp")
            .arg("serve")
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        Self {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 1,
        }
    }

    fn initialize(&mut self) -> InitializeResult {
        let response = self.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION_2025_11_25,
                "capabilities": {},
                "clientInfo": {
                    "name": "guild-cli-test-client",
                    "version": "0.1.0"
                }
            }),
        );
        let initialized: InitializeResult =
            serde_json::from_value(response["result"].clone()).unwrap();
        self.notify("notifications/initialized", &json!({}));
        initialized
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request);
        self.read_message()
    }

    fn notify(&mut self, method: &str, params: &Value) {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&request);
    }

    fn write_message(&mut self, request: &Value) {
        serde_json::to_writer(&mut self.stdin, request).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    fn read_message(&mut self) -> Value {
        let mut line = String::new();
        let bytes = self.stdout.read_line(&mut line).unwrap();
        assert!(bytes > 0, "guild mcp stdio server exited before responding");
        serde_json::from_str(&line).unwrap()
    }
}

impl Drop for McpHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_stdio_launches_through_guild_cli() {
    let temp = TempFixtureDir::new("guild-cli-mcp");
    let registry_root = temp.path().join("registry");
    LocalSourceInstaller::new(&registry_root)
        .unwrap()
        .install(hello_source_dir())
        .unwrap();

    let mut harness = McpHarness::spawn(&registry_root);
    let initialized = harness.initialize();
    assert_eq!(initialized.server_info.name, "guild-mcp");
    assert_eq!(
        initialized
            .capabilities
            .tools
            .as_ref()
            .and_then(|tools| tools.list_changed),
        Some(false)
    );

    let tools_response = harness.request("tools/list", &json!({}));
    let tools: ListToolsResult = serde_json::from_value(tools_response["result"].clone()).unwrap();
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "guild.inspect");
    assert_eq!(tools.tools[0].title.as_deref(), Some("Guild Inspect"));
    assert_eq!(
        tools.tools[0]
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.title.as_deref()),
        Some("Guild Inspect")
    );
}

#[test]
fn codex_subcommand_print_config_honors_global_registry_root() {
    let temp = TempFixtureDir::new("guild-cli-codex-config");
    let registry_root = temp.path().join("registry");
    let registry_root_display = registry_root.display().to_string();

    let stdout = run_guild_success(
        &[
            "--registry-root",
            &registry_root_display,
            "codex",
            "print-config",
            "--json",
        ],
        None,
    );
    let config: Value = parse_json_stdout(&stdout);
    assert_eq!(config["command"].as_str(), Some("cargo"));
    assert_eq!(
        config["env"]["GUILD_REGISTRY_ROOT"].as_str(),
        Some(registry_root_display.as_str()),
    );
    assert_eq!(
        config["args"]
            .as_array()
            .unwrap()
            .last()
            .and_then(Value::as_str),
        Some("--stdio"),
    );
}

#[test]
fn codex_subcommand_help_is_available_through_guild_cli() {
    let stdout = run_guild_success(&["codex", "--help"], None);
    assert!(stdout.contains("usage: guild [--registry-root <path>] codex"));
    assert!(stdout.contains("<bootstrap|print-config|scenario|smoke>"));
    assert!(!stdout.contains("setup          "));
    assert!(stdout.contains("repo-local"));
    assert!(stdout.contains("scenario"));
    assert!(stdout.contains("smoke"));
    assert!(!stdout.contains("dogfood"));
}

#[test]
fn top_level_help_is_grouped_and_points_to_topic_help() {
    let stdout = run_guild_success(&["--help"], None);
    assert!(stdout.contains("Guild CLI"));
    assert!(stdout.contains("Run, inspect, and manage Guild skills locally."));
    assert!(stdout.contains("Daily use:"));
    assert!(stdout.contains("Install and publish:"));
    assert!(stdout.contains("Setup and integration:"));
    assert!(stdout.contains("grants    Print read-only grant templates"));
    assert!(stdout.contains("guild help refs"));
    assert!(stdout.contains("guild help trust"));
    assert!(stdout.contains("guild help roots"));
    assert!(stdout.contains("guild help doctor"));
    assert!(stdout.contains("guild help preview"));
    assert!(stdout.contains("guild help grants"));
    assert!(stdout.contains("guild <command> --help"));
    assert!(!stdout.contains("deferred:"));
    assert!(!stdout.contains("inspect path"));
    assert!(!stdout.contains("dogfood"));
}

#[test]
fn shared_help_topics_are_available() {
    let help = run_guild_success(&["help"], None);
    assert!(help.contains("Guild help topics"));
    assert!(help.contains("guild help [refs|trust|roots|doctor|preview|grants]"));

    let refs = run_guild_success(&["help", "refs"], None);
    assert!(refs.contains("Guild ref forms"));
    assert!(refs.contains("skill://<namespace>/<name>@<version-or-range>"));
    assert!(refs.contains("guild://..."));
    assert!(refs.contains("Identity layers:"));
    assert!(refs.contains("installed executable state"));
    assert!(refs.contains("resolved executable identity"));
    assert!(refs.contains("guild show -v skill://example/hello-inspect@^0.1"));
    assert!(refs.contains("guild show -vv skill://example/hello-inspect@^0.1"));

    let trust = run_guild_success(&["help", "trust"], None);
    assert!(trust.contains("Trust and verification"));
    assert!(trust.contains("Normal review loop:"));
    assert!(trust.contains("guild import ... --preview or guild pull ... --preview"));
    assert!(trust.contains("guild import ... or guild pull ..."));
    assert!(trust.contains("guild verify -v <skill-ref>"));
    assert!(trust.contains("guild verify <skill-ref>"));
    assert!(trust.contains("guild trust verify-plan"));
    assert!(trust.contains("Trust-store maintenance:"));
    assert!(trust.contains("guild trust add --record-file <record.json>"));
    assert!(trust.contains("guild trust show <publisher-id>"));
    assert!(trust.contains("guild trust remove <publisher-id>"));
    assert!(trust.contains("Plan signing review:"));
    assert!(trust.contains("local-source"));
    assert!(trust.contains("verified-import"));
    assert!(trust.contains("trusted-imported"));
    assert!(trust.contains("restricted"));
    assert!(trust.contains("`trust/verification` means Guild could not verify"));

    let trust_usage = run_guild_success(&["trust", "--help"], None);
    assert!(trust_usage.contains("Review loop:"));
    assert!(trust_usage.contains("trust list -> import/pull -> verify -v"));
    assert!(trust_usage.contains("Maintenance:"));
    assert!(trust_usage.contains("add/show/list/remove trusted publishers"));
    assert!(trust_usage.contains("Signing:"));
    assert!(trust_usage.contains("`sign-plan` writes a signed plan."));

    let trust_add_help = run_guild_success(&["trust", "add", "--help"], None);
    assert!(trust_add_help.contains("use `--identity-file`"));
    assert!(trust_add_help.contains("use `--record-file`"));

    let trust_show_help = run_guild_success(&["trust", "show", "--help"], None);
    assert!(trust_show_help.contains("Usage: guild trust show [OPTIONS] <publisher-id>"));
    assert!(trust_show_help.contains("shows one trusted publisher record"));

    let trust_remove_help = run_guild_success(&["trust", "remove", "--help"], None);
    assert!(trust_remove_help.contains("removes one trusted publisher record"));

    let trust_sign_help = run_guild_success(&["trust", "sign-plan", "--help"], None);
    assert!(trust_sign_help.contains("writes a signed execution plan"));

    let trust_verify_help = run_guild_success(&["trust", "verify-plan", "--help"], None);
    assert!(
        trust_verify_help
            .contains("verifies the signed plan against the selected local trust store")
    );
    assert!(trust_verify_help.contains("publisher, trust tier, and signed digest"));

    let roots = run_guild_success(&["help", "roots"], None);
    assert!(roots.contains("Guild root resolution"));
    assert!(roots.contains("GUILD_REGISTRY_ROOT"));
    assert!(roots.contains("There is no cwd-local .guild fallback."));
    assert!(roots.contains("`root/setup` means Guild could not open"));

    let doctor = run_guild_success(&["help", "doctor"], None);
    assert!(doctor.contains("Diagnostic command direction"));
    assert!(doctor.contains("guild doctor"));
    assert!(doctor.contains("not implemented yet"));
    assert!(doctor.contains("read-only Guild-scoped diagnostic command"));
    assert!(doctor.contains("selected Guild root resolution"));
    assert!(doctor.contains("local trust-store state relevant to guild verify and guild trust"));
    assert!(doctor.contains("no hidden bootstrap or repair side effects"));

    let preview = run_guild_success(&["help", "preview"], None);
    assert!(preview.contains("Preview direction for risky flows"));
    assert!(preview.contains("use `--preview` as the first preflight flag"));
    assert!(preview.contains("guild import bundle"));
    assert!(preview.contains("guild import oci-layout"));
    assert!(preview.contains("guild pull"));
    assert!(preview.contains(
        "publisher identity, combined verification result and trust tier, and bundle digest context"
    ));
    assert!(preview.contains("preview is now shipped for that first import-and-pull slice"));
    assert!(preview.contains("no preview contract for export or push in the first slice"));

    let grants = run_guild_success(&["help", "grants"], None);
    assert!(grants.contains("Grant authoring templates"));
    assert!(grants.contains("guild grants template"));
    assert!(grants.contains("read-resource"));
    assert!(grants.contains("invoke-skill"));
    assert!(grants.contains("http-request"));
    assert!(grants.contains("This helper is read-only."));
}

#[test]
fn invalid_help_topic_fails_closed() {
    let output = run_guild_failure_output(&["help", "unknown-topic"], None);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid value"));
    assert!(stderr.contains("unknown-topic"));
}

#[test]
fn show_help_points_to_ref_topics() {
    let stdout = run_guild_success(&["show", "--help"], None);
    assert!(stdout.contains("Show a skill, run, object, or evidence summary"));
    assert!(stdout.contains("Accepted refs:"));
    assert!(stdout.contains("does not run a skill"));
    assert!(stdout.contains("default output is a short human summary for reading, not parsing."));
    assert!(stdout.contains("low-noise `Next:` hints"));
    assert!(stdout.contains("Use -v with a skill ref"));
    assert!(stdout.contains("Use -vv with a skill ref"));
    assert!(stdout.contains("guild help refs"));
    assert!(stdout.contains("guild why --help"));
}

#[test]
fn run_help_uses_input_file_flag_and_ref_topic() {
    let stdout = run_guild_success(&["run", "--help"], None);
    assert!(stdout.contains("Run a skill locally"));
    assert!(stdout.contains("--input-file <PATH>"));
    assert!(!stdout.contains("input-file-path"));
    assert!(stdout.contains("Authority lifecycle:"));
    assert!(stdout.contains("declared authority comes from the installed manifest."));
    assert!(stdout.contains("requested authority comes from the caller-provided grants."));
    assert!(stdout.contains(
        "granted authority is the final capability slice the host policy allows for that run."
    ));
    assert!(stdout.contains(
        "effective at runtime is the authority the guest can actually exercise during execution."
    ));
    assert!(stdout.contains("Guild does not hand the guest ambient authority."));
    assert!(stdout.contains("in the default human mode, stdout carries the result payload."));
    assert!(
        stdout.contains(
            "with --json, stdout carries the machine-readable wrapper on success and a machine-readable `error` envelope on failure; stderr stays empty in either case."
        )
    );
    assert!(stdout.contains("low-noise `Next:` hints"));
    assert!(stdout.contains("guild help refs"));
    assert!(stdout.contains("guild why --help"));
}

#[test]
fn grants_help_and_templates_cover_active_families() {
    let top_help = run_guild_success(&["grants", "--help"], None);
    assert!(top_help.contains("Print read-only grant templates for active capability families"));
    assert!(top_help.contains("guild help grants"));
    assert!(top_help.contains("currently active executable capability families"));

    let template_help = run_guild_success(&["grants", "template", "--help"], None);
    assert!(template_help.contains("read-resource"));
    assert!(template_help.contains("invoke-skill"));
    assert!(template_help.contains("emit-evidence"));
    assert!(template_help.contains("log-write"));
    assert!(template_help.contains("http-request"));
    assert!(template_help.contains("omit the family to print a read-only per-family catalog"));

    let all_templates = run_guild_success(&["grants", "template"], None);
    let all_value: Value = serde_json::from_str(&all_templates).unwrap();
    let templates = all_value["templates"].as_object().unwrap();
    assert_eq!(templates.len(), 5, "{all_templates}");
    for family in [
        "read-resource",
        "invoke-skill",
        "emit-evidence",
        "log-write",
        "http-request",
    ] {
        let grants = templates[family]["grants"].as_array().unwrap();
        assert_eq!(grants.len(), 1, "{all_templates}");
        assert_eq!(grants[0]["id"], family, "{all_templates}");
    }

    let read_resource = run_guild_success(&["grants", "template", "read-resource"], None);
    let read_resource_value: Value = serde_json::from_str(&read_resource).unwrap();
    assert_eq!(read_resource_value["grants"][0]["id"], "read-resource");
    assert_eq!(
        read_resource_value["grants"][0]["constraints"]["uri_prefixes"][0],
        "guild://executions/"
    );
    assert_eq!(
        read_resource_value["grants"][0]["constraints"]["resource_kinds"][0],
        "execution"
    );

    let invoke_skill = run_guild_success(&["grants", "template", "invoke-skill"], None);
    let invoke_skill_value: Value = serde_json::from_str(&invoke_skill).unwrap();
    assert_eq!(invoke_skill_value["grants"][0]["id"], "invoke-skill");
    assert_eq!(
        invoke_skill_value["grants"][0]["constraints"]["aliases"][0],
        "<declared-alias>"
    );

    let http_request = run_guild_success(&["grants", "template", "http-request"], None);
    let http_request_value: Value = serde_json::from_str(&http_request).unwrap();
    assert_eq!(http_request_value["grants"][0]["id"], "http-request");
    assert_eq!(
        http_request_value["grants"][0]["constraints"]["allowed_hosts"][0],
        "api.example.com"
    );
    assert_eq!(
        http_request_value["grants"][0]["constraints"]["allowed_path_prefixes"][0],
        "/v1/"
    );
    assert_eq!(
        http_request_value["grants"][0]["constraints"]["follow_redirects"],
        false
    );
}

#[test]
fn ls_get_why_and_verify_help_call_out_scope() {
    let ls_help = run_guild_success(&["ls", "--help"], None);
    assert!(ls_help.contains("List skills, runs, objects, or evidence"));
    assert!(ls_help.contains("primary local-state listing command"));
    assert!(
        ls_help.contains("default output is a short local-state listing for reading, not parsing.")
    );
    assert!(ls_help.contains("Legacy alias:"));
    assert!(ls_help.contains("guild list ..."));
    assert!(ls_help.contains("guild show --help"));
    assert!(ls_help.contains("guild why --help"));

    let get_help = run_guild_success(&["get", "--help"], None);
    assert!(get_help.contains("Read a Guild resource"));
    assert!(get_help.contains("Accepted refs:"));
    assert!(get_help.contains("exec:<execution-id-prefix>"));
    assert!(get_help.contains("primary raw resource-read command"));
    assert!(get_help.contains("reads go to stdout by default."));
    assert!(
        get_help.contains(
            "with --json, stdout carries the machine-readable payload on success and a machine-readable `error` envelope on failure; stderr stays empty in either case."
        )
    );
    assert!(get_help.contains("use --porcelain for stable one-line machine reads."));
    assert!(get_help.contains("Legacy alias:"));
    assert!(get_help.contains("guild read ..."));
    assert!(get_help.contains("guild help refs"));
    assert!(get_help.contains("guild why --help"));

    let why_help = run_guild_success(&["why", "--help"], None);
    assert!(why_help.contains("Explain a persisted execution"));
    assert!(why_help.contains("primary persisted-execution explanation command"));
    assert!(
        why_help.contains("default output is a short human explanation for reading, not parsing.")
    );
    assert!(why_help.contains("--lineage"));
    assert!(why_help.contains("bounded read-only ancestor/descendant view"));
    assert!(why_help.contains("human-only"));
    assert!(why_help.contains("low-noise `Next:` hints"));
    assert!(why_help.contains("start with `guild why` and `guild why -v`"));
    assert!(
        why_help.contains("example skills may produce richer reusable authority or policy reports")
    );
    assert!(why_help.contains("persisted execution record"));
    assert!(why_help.contains("guild get --help"));

    let verify_help = run_guild_success(&["verify", "--help"], None);
    assert!(verify_help.contains("Show installed trust and verification status"));
    assert!(
        verify_help
            .contains("default output is a short human trust summary for reading, not parsing.")
    );
    assert!(verify_help.contains("low-noise `Next:` hints"));
    assert!(verify_help.contains("Verification details:"));
    assert!(verify_help.contains("use -v after import or pull"));
    assert!(verify_help.contains("guild trust verify-plan"));
    assert!(verify_help.contains("guild help trust"));
    assert!(verify_help.contains("guild show --help"));
}

#[test]
fn import_pull_and_transport_help_point_to_preview_direction() {
    let import_help = run_guild_success(&["import", "--help"], None);
    assert!(import_help.contains("Import a signed bundle or OCI layout into a Guild root"));
    assert!(import_help.contains("use `--preview` for a read-only preflight"));
    assert!(import_help.contains("guild help preview"));

    let import_bundle_help = run_guild_success(&["import", "bundle", "--help"], None);
    assert!(import_bundle_help.contains("Usage: guild import bundle [OPTIONS] <dir>"));
    assert!(import_bundle_help.contains("--preview"));
    assert!(import_bundle_help.contains("`--preview` stays read-only"));
    assert!(import_bundle_help.contains("same signed bundle and trust checks as import"));

    let import_oci_help = run_guild_success(&["import", "oci-layout", "--help"], None);
    assert!(import_oci_help.contains("Usage: guild import oci-layout [OPTIONS] <dir>"));
    assert!(import_oci_help.contains("--preview"));
    assert!(import_oci_help.contains("`--preview` stays read-only"));
    assert!(import_oci_help.contains("same signed bundle and trust checks as import"));

    let pull_help = run_guild_success(&["pull", "--help"], None);
    assert!(pull_help.contains("Pull and import installed state from an OCI registry"));
    assert!(pull_help.contains("Usage: guild pull [OPTIONS] <oci-ref>"));
    assert!(pull_help.contains("--allow-http"));
    assert!(pull_help.contains("--preview"));
    assert!(pull_help.contains("use `--preview` for a read-only preflight"));
    assert!(pull_help.contains("guild help preview"));

    let export_help = run_guild_success(&["export", "--help"], None);
    assert!(export_help.contains("no preview contract is chosen for export in the first slice"));

    let push_help = run_guild_success(&["push", "--help"], None);
    assert!(push_help.contains("no preview contract is chosen for push in the first slice"));
}

#[test]
fn install_command_maps_to_real_source_install_path() {
    let temp = TempFixtureDir::new("guild-cli-install");
    let registry_root = temp.path().join("registry");
    let root = registry_root.display().to_string();
    let source_root = hello_source_dir().display().to_string();
    let install_output = run_guild_success(
        &["--registry-root", &root, "install", &source_root, "--json"],
        None,
    );
    let install_value: Value = parse_json_stdout(&install_output);

    let installed_manifest_path = install_value["manifest_path"].as_str().unwrap();
    let manifest: SkillManifest =
        serde_json::from_str(&fs::read_to_string(installed_manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.key.name, "hello-inspect");
    assert!(Path::new(install_value["artifact_path"].as_str().unwrap()).exists());
    assert!(Path::new(install_value["root_dir"].as_str().unwrap()).exists());
}

#[test]
fn install_human_output_reports_digest_and_path_without_follow_up_hint() {
    let temp = TempFixtureDir::new("guild-cli-install-human");
    let registry_root = temp.path().join("registry");
    let root = registry_root.display().to_string();
    let source_root = hello_source_dir().display().to_string();
    let stdout = run_guild_success(&["--registry-root", &root, "install", &source_root], None);

    assert!(
        stdout.contains("installed skill://example/hello-inspect@0.1.0"),
        "{stdout}"
    );
    assert!(stdout.contains("digest: sha256:"), "{stdout}");
    assert!(stdout.contains("path: "), "{stdout}");
    assert!(!stdout.contains("Next:"), "{stdout}");
}

#[test]
fn install_json_output_stays_machine_only() {
    let temp = TempFixtureDir::new("guild-cli-install-json");
    let registry_root = temp.path().join("registry");
    let root = registry_root.display().to_string();
    let source_root = hello_source_dir().display().to_string();
    let stdout = run_guild_success(
        &["--registry-root", &root, "install", &source_root, "--json"],
        None,
    );
    let value: Value = parse_json_stdout(&stdout);

    assert_eq!(
        value["resolved_skill"].as_str(),
        Some("skill://example/hello-inspect@0.1.0")
    );
    assert!(!stdout.contains("Next:"), "{stdout}");
}

#[test]
fn show_verbose_output_traces_requested_to_resolved_identity() {
    let temp = TempFixtureDir::new("guild-cli-show-identity");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    let list_output = run_guild_success(&["list", "--json"], Some(&registry_root));
    let listed: Value = parse_json_stdout(&list_output);
    let digest = listed["installed"][0]["digest"]
        .as_str()
        .unwrap()
        .to_owned();

    let stdout = run_guild_success(&["show", "-v", "hello-inspect@^0.1"], Some(&registry_root));
    assert!(stdout.contains("requested: hello-inspect@^0.1"), "{stdout}");
    assert!(
        stdout.contains("resolved: skill://example/hello-inspect@0.1.0"),
        "{stdout}"
    );
    assert!(stdout.contains(&format!("digest: {digest}")), "{stdout}");
    assert!(stdout.contains("installed path:"), "{stdout}");
    assert!(!stdout.contains("\nsource:"), "{stdout}");
}

#[test]
fn show_very_verbose_output_explains_requested_ref_resolution() {
    let temp = TempFixtureDir::new("guild-cli-show-resolution");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let stdout = run_guild_success(&["show", "-vv", "hello-inspect@^0.1"], Some(&registry_root));
    assert!(stdout.contains("resolution:"), "{stdout}");
    assert!(
        stdout.contains(
            "short ref `hello-inspect@^0.1` resolved to `skill://example/hello-inspect@^0.1` because it was unambiguous across installed namespaces"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("matched installed versions satisfying `^0.1`: 0.1.0"),
        "{stdout}"
    );
    assert!(
        stdout.contains(
            "selected version `0.1.0` as the highest installed version satisfying the request"
        ),
        "{stdout}"
    );
    assert!(stdout.contains("selected digest `sha256:"), "{stdout}");
}

#[test]
fn why_human_output_suggests_get_next_step() {
    let temp = TempFixtureDir::new("guild-cli-why-next-step");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_id = inspect_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let execution_uri = inspect_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence_uri = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence_id = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    let evidence_ref = format!("evidence:{evidence_id}");

    let stdout = run_guild_success(
        &["why", &exec_prefix, "--color", "never"],
        Some(&registry_root),
    );
    assert!(stdout.contains("child executions: 0"), "{stdout}");
    assert!(stdout.contains("evidence records: 1"), "{stdout}");
    assert!(
        stdout.contains("authority: exercised(emit-evidence)"),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("nearby evidence: {evidence_ref}")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!(
            "Next: guild --registry-root {} get ",
            registry_root.display()
        )),
        "{stdout}"
    );
    assert!(stdout.contains(&execution_uri), "{stdout}");
    assert!(
        stdout.contains(&format!(
            "Next: guild --registry-root {} show {evidence_uri}",
            registry_root.display()
        )),
        "{stdout}"
    );
}

#[test]
fn why_human_output_summarizes_authority_observations() {
    let temp = TempFixtureDir::new("guild-cli-why-authority-summary");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let run_output = run_guild_success_output(
        &[
            "run",
            "hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada", "emit_log": true })),
            "--grants-json",
            &emit_evidence_and_log_write_grants_json(),
            "--json",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(run_output.stdout).unwrap();
    let stderr = String::from_utf8(run_output.stderr).unwrap();
    assert!(stderr.trim().is_empty(), "{stderr}");
    let run_value: Value = parse_json_stdout(&stdout);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);

    let why_output = run_guild_success(
        &["why", &exec_prefix, "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        why_output.contains("authority: exercised(emit-evidence, log-write)")
            || why_output.contains("authority: exercised(log-write, emit-evidence)"),
        "{why_output}"
    );

    let verbose_output = run_guild_success(
        &["why", &exec_prefix, "-v", "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        verbose_output.contains("authority observations:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains("- exercised log-write -> info"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains("- exercised emit-evidence -> evidence:"),
        "{verbose_output}"
    );
}

#[test]
fn why_human_output_reports_blocked_authority_observations() {
    let temp = TempFixtureDir::new("guild-cli-why-blocked-authority");
    let registry_root = temp.path().join("registry");
    install_source_with_cli(&registry_root, &http_source_dir());

    let output = run_guild_failure_output(
        &[
            "run",
            "skill://example/inspect-http-json@^0.1",
            "--input-json",
            &command_json(json!({ "url": "http://127.0.0.1/blocked.json" })),
            "--grants-json",
            &emit_http_path_denial_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stdout.trim().is_empty(), "{stdout}");
    let execution_uri = persisted_where_uri(&stderr);

    let why_output = run_guild_success(
        &["why", &execution_uri, "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        why_output.contains("authority: blocked(http-request)"),
        "{why_output}"
    );

    let verbose_output = run_guild_success(
        &["why", &execution_uri, "-v", "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        verbose_output.contains("authority observations:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains(
            "- blocked http-request -> http://127.0.0.1/blocked.json / http-request-path-not-granted"
        ),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains("request hints:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains(
            "- request an `http-request` grant whose `allowed_path_prefixes` covers `/blocked.json`"
        ),
        "{verbose_output}"
    );
}

#[test]
fn why_human_output_summarizes_requested_vs_granted_reduction() {
    let temp = TempFixtureDir::new("guild-cli-why-requested-vs-granted");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let run_output = run_guild_success_output(
        &[
            "run",
            "hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada", "emit_log": true })),
            "--grants-json",
            &emit_evidence_and_broad_log_write_grants_json(),
            "--json",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(run_output.stdout).unwrap();
    let stderr = String::from_utf8(run_output.stderr).unwrap();
    assert!(stderr.trim().is_empty(), "{stderr}");
    let run_value: Value = parse_json_stdout(&stdout);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);

    let why_output = run_guild_success(
        &["why", &exec_prefix, "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        why_output.contains("requested vs granted: reduced(log-write)"),
        "{why_output}"
    );

    let verbose_output = run_guild_success(
        &["why", &exec_prefix, "-v", "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        verbose_output.contains("requested vs granted:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains("- reduced log-write/write:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains("levels=info,warn"),
        "{verbose_output}"
    );
    assert!(verbose_output.contains("levels=info"), "{verbose_output}");
}

#[test]
fn successful_runs_suggest_verbose_why_when_requested_authority_is_reduced() {
    let temp = TempFixtureDir::new("guild-cli-run-why-verbose-hint");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let output = run_guild_success_output(
        &[
            "run",
            "hello-inspect@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada", "emit_log": true })),
            "--grants-json",
            &emit_evidence_and_broad_log_write_grants_json(),
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stderr.contains(&format!(
            "Next: guild --registry-root {} why -v guild://executions/",
            registry_root.display()
        )),
        "{stderr}"
    );
}

#[test]
fn why_verbose_output_lists_nearby_related_refs() {
    let temp = TempFixtureDir::new("guild-cli-why-verbose-nearby");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_id = inspect_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let evidence_id = inspect_value["record"]["emitted_evidence"][0]["uri"]
        .as_str()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    let evidence_ref = format!("evidence:{evidence_id}");

    let stdout = run_guild_success(
        &["why", &exec_prefix, "-v", "--color", "never"],
        Some(&registry_root),
    );
    assert!(stdout.contains("nearby evidence refs:"), "{stdout}");
    assert!(stdout.contains(&format!("- {evidence_ref}")), "{stdout}");
    assert!(!stdout.contains("nearby evidence: "), "{stdout}");
}

#[test]
fn why_human_output_prefers_child_execution_navigation_when_lineage_exists() {
    let temp = TempFixtureDir::new("guild-cli-why-child-navigation");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &composite_source_dir());

    let run_output = run_guild_success_output(
        &[
            "run",
            "skill://example/hello-composite@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &composite_invoke_and_emit_evidence_grants_json(),
            "--json",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(run_output.stdout).unwrap();
    let stderr = String::from_utf8(run_output.stderr).unwrap();
    assert!(stderr.trim().is_empty(), "{stderr}");
    let run_value: Value = parse_json_stdout(&stdout);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let execution_uri = run_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let child_execution_id = run_value["record"]["child_executions"][0]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let child_execution_uri = run_value["record"]["child_executions"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let child_execution_ref = format!("exec:{child_execution_id}");
    let exec_prefix = format!("exec:{}", &execution_id[..12]);

    let why_output = run_guild_success(
        &["why", &exec_prefix, "--color", "never"],
        Some(&registry_root),
    );
    assert!(why_output.contains("child executions: 1"), "{why_output}");
    assert!(why_output.contains("nearby child: exec:"), "{why_output}");
    assert!(
        why_output.contains(&format!("nearby child: {child_execution_ref}")),
        "{why_output}"
    );
    assert!(!why_output.contains("nearby evidence: "), "{why_output}");
    assert!(
        why_output.contains(&format!(
            "Next: guild --registry-root {} get {execution_uri}",
            registry_root.display()
        )),
        "{why_output}"
    );
    assert!(
        why_output.contains(&format!(
            "Next: guild --registry-root {} why {child_execution_uri}",
            registry_root.display()
        )),
        "{why_output}"
    );
    assert!(
        !why_output.contains(&format!(
            "Next: guild --registry-root {} show evidence:",
            registry_root.display()
        )),
        "{why_output}"
    );

    let verbose_output = run_guild_success(
        &["why", &exec_prefix, "-v", "--color", "never"],
        Some(&registry_root),
    );
    assert!(
        verbose_output.contains("nearby child refs:"),
        "{verbose_output}"
    );
    assert!(
        verbose_output.contains(&format!("- {child_execution_ref}")),
        "{verbose_output}"
    );
    assert!(
        !verbose_output.contains("nearby child: "),
        "{verbose_output}"
    );
}

#[test]
fn why_lineage_rejects_machine_output_modes() {
    let temp = TempFixtureDir::new("guild-cli-why-lineage-machine-modes");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);

    let inspect_value =
        inspect_hello_with_cli(&registry_root, "Ada", "skill://example/hello-inspect@^0.1");
    let execution_id = inspect_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);

    let json_output = run_guild_failure_output(
        &["why", &exec_prefix, "--lineage", "--json"],
        Some(&registry_root),
    );
    let json_value = parse_failure_json_output(&json_output);
    assert!(
        json_value["error"]["summary"]
            .as_str()
            .is_some_and(|summary| summary
                .contains("`guild why --lineage` does not support --json or --porcelain")),
        "{json_value}"
    );
    assert_eq!(json_value["error"]["category"].as_str(), Some("usage"));
    assert!(json_value["error"]["reason_code"].is_null(), "{json_value}");
    assert_eq!(
        json_value["error"]["next_steps"].as_array().map(Vec::len),
        Some(0),
        "{json_value}"
    );

    let porcelain_output = run_guild_failure_output(
        &["why", &exec_prefix, "--lineage", "--porcelain"],
        Some(&registry_root),
    );
    let porcelain_stderr = String::from_utf8(porcelain_output.stderr).unwrap();
    assert!(
        porcelain_stderr.contains("`guild why --lineage` does not support --json or --porcelain"),
        "{porcelain_stderr}"
    );
}

#[test]
fn why_lineage_human_output_renders_descendant_tree() {
    let temp = TempFixtureDir::new("guild-cli-why-lineage-descendants");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &composite_source_dir());

    let run_output = run_guild_success_output(
        &[
            "run",
            "skill://example/hello-composite@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &composite_invoke_and_emit_evidence_grants_json(),
            "--json",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(run_output.stdout).unwrap();
    let run_value: Value = parse_json_stdout(&stdout);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let child_execution_id = run_value["record"]["child_executions"][0]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);
    let child_execution_ref = format!("exec:{}", &child_execution_id[..12]);

    let why_output = run_guild_success(
        &["why", &exec_prefix, "--lineage", "--color", "never"],
        Some(&registry_root),
    );
    assert!(why_output.contains("lineage:"), "{why_output}");
    assert!(why_output.contains("ancestry: none"), "{why_output}");
    assert!(why_output.contains("descendants:"), "{why_output}");
    assert!(why_output.contains(&exec_prefix), "{why_output}");
    assert!(
        why_output.contains(&format!("alias hello  succeeded  {child_execution_ref}")),
        "{why_output}"
    );
    assert!(!why_output.contains("lineage warnings:"), "{why_output}");
}

#[test]
fn why_lineage_very_verbose_output_shows_full_execution_uris() {
    let temp = TempFixtureDir::new("guild-cli-why-lineage-very-verbose");
    let registry_root = temp.path().join("registry");
    install_with_cli(&registry_root);
    install_source_with_cli(&registry_root, &composite_source_dir());

    let run_output = run_guild_success_output(
        &[
            "run",
            "skill://example/hello-composite@^0.1",
            "--input-json",
            &command_json(json!({ "name": "Ada" })),
            "--grants-json",
            &composite_invoke_and_emit_evidence_grants_json(),
            "--json",
        ],
        Some(&registry_root),
    );
    let stdout = String::from_utf8(run_output.stdout).unwrap();
    let run_value: Value = parse_json_stdout(&stdout);
    let child_execution_uri = run_value["record"]["child_executions"][0]["uri"]
        .as_str()
        .unwrap()
        .to_owned();
    let execution_uri = run_value["record"]["receipt"]["uri"]
        .as_str()
        .unwrap()
        .to_owned();

    let why_output = run_guild_success(
        &[
            "why",
            &child_execution_uri,
            "--lineage",
            "-vv",
            "--color",
            "never",
        ],
        Some(&registry_root),
    );
    assert!(why_output.contains("ancestry:"), "{why_output}");
    assert!(
        why_output.contains(&format!("uri: {execution_uri}")),
        "{why_output}"
    );
    assert!(
        why_output.contains(&format!("uri: {child_execution_uri}")),
        "{why_output}"
    );
}

#[test]
fn user_facing_docs_use_installed_guild_cli_after_install() {
    let mut paths = vec![
        repo_root().join("README.md"),
        repo_root().join("docs/command-language.md"),
        repo_root().join("docs/how-guild-works.md"),
        repo_root().join("docs/mcp-agent-recipes.md"),
        repo_root().join("docs/adr/0019-thin-guild-cli.md"),
        repo_root().join("docs/testing.md"),
        repo_root().join("examples/README.md"),
    ];

    for entry in fs::read_dir(repo_root().join("examples/skills")).unwrap() {
        let entry = entry.unwrap();
        let readme = entry.path().join("README.md");
        if readme.is_file() {
            paths.push(readme);
        }
    }

    for path in paths {
        assert_markdown_uses_installed_guild_cli(&path);
        assert_markdown_keeps_legacy_alias_commands_out_of_examples(&path);
    }
}

#[test]
fn follow_on_program_tracking_stays_rebased() {
    let config = fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/config.yml")).unwrap();
    assert!(config.contains("Guild Follow-On Program"));
    assert!(config.contains("https://github.com/jkordish/Guild/issues/44"));
    assert!(!config.contains("/issues/15"));

    let epic_template =
        fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/ux-epic.md")).unwrap();
    let task_template =
        fs::read_to_string(repo_root().join(".github/ISSUE_TEMPLATE/ux-task.md")).unwrap();

    for phrase in [
        "Track one UX-hardening epic in the Guild day-to-day usability program",
        "No runtime-contract widening unless a separate contract issue says so",
        "No aspirational command names that the CLI does not already support honestly",
        "No repo-local planning file that duplicates the active GitHub issue tree",
    ] {
        assert!(
            epic_template.contains(phrase),
            "ux-epic.md is missing follow-on program guardrail wording: {phrase}"
        );
    }

    for forbidden in [
        "guild explain execution <id>",
        "guild check",
        "resources/list_changed",
    ] {
        assert!(
            !epic_template.contains(forbidden),
            "ux-epic.md reintroduced stale follow-on planning wording: {forbidden}"
        );
        assert!(
            !task_template.contains(forbidden),
            "ux-task.md reintroduced stale follow-on planning wording: {forbidden}"
        );
    }
}

#[test]
fn journey_docs_stay_centered_on_user_workflows() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(readme.contains("## User Journeys"));
    assert!(readme.contains("Install and run a skill"));
    assert!(readme.contains("Explain what happened"));
    assert!(readme.contains("Verify trust state and move installed state"));
    assert!(readme.contains("Debug failures and compare runs"));
    assert!(readme.contains("guild why -v"));
    assert!(readme.contains("guild why --lineage"));
    assert!(readme.contains("guild ls evidence --limit 5"));
    assert!(readme.contains("guild grants template"));
    assert!(readme.contains("examples/skills/explain-execution-tree/README.md"));
    assert!(readme.contains(
        "move to narrower authority and policy example skills only when `guild why -v` is no longer enough"
    ));

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    assert!(command_language.contains("### Journey Map"));
    assert!(command_language.contains("Install and run a skill"));
    assert!(command_language.contains("Explain what happened"));
    assert!(command_language.contains("Verify trust state and move installed state"));
    assert!(command_language.contains("Debug failures and compare runs"));
    assert!(command_language.contains("guild why -v"));
    assert!(command_language.contains("guild why --lineage"));
    assert!(command_language.contains("guild ls evidence --limit 5"));
    assert!(command_language.contains("guild grants template"));
    assert!(command_language.contains("../examples/skills/explain-execution-tree/README.md"));
    assert!(command_language.contains(
        "move to narrower authority and policy example skills only when `guild why -v` is no longer enough"
    ));

    let examples_index = fs::read_to_string(repo_root().join("examples/README.md")).unwrap();
    assert!(examples_index.contains("## User Journeys"));
    assert!(examples_index.contains("### Install and run a skill"));
    assert!(examples_index.contains("### Explain what happened"));
    assert!(examples_index.contains("### Verify trust state and move installed state"));
    assert!(examples_index.contains("### Debug failures and compare runs"));
    assert!(examples_index.contains("guild why -v"));
    assert!(examples_index.contains("guild why --lineage"));
    assert!(examples_index.contains("guild ls evidence --limit 5"));
    assert!(examples_index.contains("guild grants template"));
    assert!(examples_index.contains("Keep starting with the native CLI:"));
    assert!(
        examples_index
            .contains("For narrower authority and policy debugging after that native CLI path")
    );

    let hello_readme =
        fs::read_to_string(repo_root().join("examples/skills/hello-inspect/README.md")).unwrap();
    assert!(hello_readme.contains("User journey: install and run a skill locally."));
    assert!(hello_readme.contains("guild grants template emit-evidence"));
    assert!(hello_readme.contains(" show skill://example/hello-inspect@^0.1"));
    assert!(hello_readme.contains(" why exec:<execution-id-prefix>"));
    assert!(hello_readme.contains(" verify skill://example/hello-inspect@^0.1"));

    let explain_readme =
        fs::read_to_string(repo_root().join("examples/skills/explain-execution/README.md"))
            .unwrap();
    assert!(explain_readme.contains("Use `guild why` first"));
    assert!(explain_readme.contains("User journey: explain a stored execution."));

    let explain_tree_readme =
        fs::read_to_string(repo_root().join("examples/skills/explain-execution-tree/README.md"))
            .unwrap();
    assert!(explain_tree_readme.contains("guild why --lineage"));

    let explain_denial_readme =
        fs::read_to_string(repo_root().join("examples/skills/explain-capability-denial/README.md"))
            .unwrap();
    assert!(explain_denial_readme.contains("Use `guild why` first"));
    assert!(explain_denial_readme.contains("Use `guild why -v`"));
    assert!(explain_denial_readme.contains("richer reusable authority and"));
    assert!(explain_denial_readme.contains("policy report over that same stored execution"));

    let diff_authority_readme =
        fs::read_to_string(repo_root().join("examples/skills/diff-execution-authority/README.md"))
            .unwrap();
    assert!(diff_authority_readme.contains("Use `guild why` first"));
    assert!(diff_authority_readme.contains("Use `guild why -v`"));
    assert!(diff_authority_readme.contains("richer reusable authority comparison"));

    let explain_http_readme =
        fs::read_to_string(repo_root().join("examples/skills/explain-http-authority/README.md"))
            .unwrap();
    assert!(explain_http_readme.contains("Use `guild why` first"));
    assert!(explain_http_readme.contains("Use `guild why -v`"));
    assert!(explain_http_readme.contains("candidate HTTP request"));

    let how_it_works = fs::read_to_string(repo_root().join("docs/how-guild-works.md")).unwrap();
    assert!(how_it_works.contains("## Output Modes"));
    assert!(how_it_works.contains("## Trust Review"));
    assert!(how_it_works.contains("guild trust list"));
    assert!(how_it_works.contains("guild import ... --preview"));
    assert!(how_it_works.contains("guild import ...` or `guild pull ..."));
    assert!(how_it_works.contains("guild verify -v <skill-ref>"));
    assert!(how_it_works.contains("verified-import"));
    assert!(how_it_works.contains("guild trust add --record-file <record.json>"));
    assert!(how_it_works.contains("guild trust show <publisher-id>"));
    assert!(how_it_works.contains("guild trust remove <publisher-id>"));
    assert!(how_it_works.contains("Default human output is for reading, not parsing."));
    assert!(how_it_works.contains("low-noise follow-up hints such as `Next: ...`"));
    assert!(how_it_works.contains("use `--json` for structured machine-readable output"));
    assert!(how_it_works.contains("use `--porcelain` for stable one-line machine-readable output"));
    assert!(how_it_works.contains("guild help doctor"));
    assert!(how_it_works.contains("guild help preview"));
    assert!(how_it_works.contains("guild help grants"));
    assert!(how_it_works.contains("docs/mirroring-and-promotion.md"));
    assert!(how_it_works.contains("guild why -v"));
    assert!(how_it_works.contains("guild why --lineage"));
    assert!(how_it_works.contains("guild ls evidence --limit 5"));
    assert!(how_it_works.contains(
        "move to narrower authority and policy example skills only when that native CLI path is no longer enough"
    ));

    let incident_brief =
        fs::read_to_string(repo_root().join("examples/skills/incident-brief/README.md")).unwrap();
    assert!(incident_brief.contains("guild grants template read-resource"));
    assert!(incident_brief.contains("guild grants template invoke-skill"));

    let ops_pack =
        fs::read_to_string(repo_root().join("examples/skills/guild-ops-starter/README.md"))
            .unwrap();
    assert!(ops_pack.contains("## Journey 1: Explain One Stored Execution"));
    assert!(ops_pack.contains("## Journey 2: Compare Two Stored Executions"));
    assert!(ops_pack.contains("## Journey 3: Scan Recent Failures"));
    assert!(ops_pack.contains("## Journey 4: Discover And Inspect One Stored Evidence Record"));
    assert!(ops_pack.contains("guild why --lineage"));
    assert!(ops_pack.contains("guild ls evidence --limit 5"));
    assert!(ops_pack.contains("## Keep Going With The Normal CLI"));

    let mcp_recipes = fs::read_to_string(repo_root().join("docs/mcp-agent-recipes.md")).unwrap();
    assert!(mcp_recipes.contains("## Recipe 1: Inspect A Skill"));
    assert!(mcp_recipes.contains("## Recipe 2: Find An Execution"));
    assert!(mcp_recipes.contains("## Recipe 3: Fetch Evidence Safely"));
    assert!(mcp_recipes.contains("## Recipe 4: Explain A Failure"));
    assert!(mcp_recipes.contains("`tools/list`"));
    assert!(mcp_recipes.contains("`resources/list`"));
    assert!(mcp_recipes.contains("`resources/templates/list`"));
    assert!(mcp_recipes.contains("`resources/read`"));
    assert!(mcp_recipes.contains("`guild.inspect`"));
    assert!(mcp_recipes.contains("guild://queries/executions/failures/recent/10"));
    assert!(mcp_recipes.contains("guild://objects/records/<evidence-record-id>/metadata"));
}

#[test]
fn mirroring_and_promotion_docs_stay_linked_and_honest() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(readme.contains("docs/mirroring-and-promotion.md"));
    assert!(readme.contains("not silent copy or retag primitives"));

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    assert!(command_language.contains("docs/mirroring-and-promotion.md"));
    assert!(command_language.contains("not silent registry-copy or retag"));

    let guide = fs::read_to_string(repo_root().join("docs/mirroring-and-promotion.md")).unwrap();
    for phrase in [
        "`guild mirror`",
        "`guild promote`",
        "`guild import ... --preview`",
        "`guild pull ... --preview`",
        "`guild export ...` and `guild push ...`",
        "Treat them as new publication events",
        "registry-to-registry copy",
        "`guild verify -v <skill-ref>`",
        "cargo run -p guild-mcp --example export_import_local",
        "cargo run -p guild-mcp --example push_pull_oci_registry_local",
    ] {
        assert!(
            guide.contains(phrase),
            "docs/mirroring-and-promotion.md is missing operator guidance: {phrase}"
        );
    }
}

#[test]
fn testing_guide_tracks_preview_first_transport_proofs() {
    let testing = fs::read_to_string(repo_root().join("docs/testing.md")).unwrap();
    for phrase in [
        "Trust and signed-bundle smoke with preview:",
        "guild init --registry-root target/dev-local-registry/b-preview",
        "target/dev-local-registry/bundle \\",
        "--preview",
        "`would-import` after `guild trust add ...`",
        "OCI registry smoke with preview:",
        "guild init --registry-root target/dev-local-registry/c-preview",
        "[`mirroring-and-promotion.md`](mirroring-and-promotion.md)",
        "The primitive and composite success-path transport examples now preflight",
        "preview before the real import or pull step",
    ] {
        assert!(
            testing.contains(phrase),
            "docs/testing.md is missing preview-first transport guidance: {phrase}"
        );
    }
}

#[test]
fn mcp_contract_docs_match_the_discovery_catalog_surface() {
    let specs = fs::read_to_string(repo_root().join("SPECS.md")).unwrap();
    for phrase in [
        "`resources/list` as a bounded discovery catalog",
        "`resources/list` is a bounded discovery catalog",
        "the first listed URI is the canonical recent-executions query resource",
        "the second listed URI is the canonical recent-failures query resource",
        "bounded recent evidence-metadata slice",
    ] {
        assert!(
            specs.contains(phrase),
            "SPECS.md is missing MCP discovery-catalog wording: {phrase}"
        );
    }

    let architecture = fs::read_to_string(repo_root().join("ARCHITECTURE.md")).unwrap();
    for phrase in [
        "a bounded discovery-oriented `resources/list` catalog",
        "`resources/list` is a bounded discovery catalog",
        "starts with the canonical recent-executions and recent-failures query URIs",
        "recent evidence-metadata resources",
        "inspect-result links",
    ] {
        assert!(
            architecture.contains(phrase),
            "ARCHITECTURE.md is missing MCP discovery-catalog wording: {phrase}"
        );
    }

    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(readme.contains("start with `tools/list` and expect exactly one public tool"));
    assert!(readme.contains("`Tools: (none)`"));
    assert!(readme.contains(
        "`resources/list` is a bounded discovery catalog: the first entries are canonical recent-query URIs"
    ));

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    assert!(
        command_language
            .contains("`tools/list` to confirm the one current public tool, `guild.inspect`")
    );
    assert!(command_language.contains("`Tools: (none)`"));
    assert!(command_language.contains(
        "`resources/list` is a bounded discovery catalog: the first entries are canonical recent-query URIs"
    ));
}

#[test]
fn readme_command_language_and_testing_guide_document_failure_paths() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    for phrase in [
        "## Failure Paths",
        "`root/setup`",
        "`lookup/ambiguity`",
        "`resource/read`",
        "`authority denial`",
        "`runtime/compatibility`",
        "`trust/verification`",
        "use `guild why ...` after a rejected run when Guild persisted an execution receipt",
        "Wrong-world manifest drift and broader Guild component imports should surface as",
        "`runtime/compatibility`, not `authority denial`",
        "guild verify missing-skill@^0.1",
        "reason: bundle-publisher-untrusted",
    ] {
        assert!(
            readme.contains(phrase),
            "README.md is missing failure-path wording: {phrase}"
        );
    }

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    for phrase in [
        "## Failure Language",
        "`root/setup`",
        "`lookup/ambiguity`",
        "`resource/read`",
        "`authority denial`",
        "`runtime/compatibility`",
        "`trust/verification`",
        "use `guild show -v ...` before rerunning after authority or runtime failures",
        "Wrong-world manifest drift and broader Guild component imports should surface as",
        "`runtime/compatibility`, not `authority denial`",
        "guild verify missing-skill@^0.1",
        "reason: bundle-publisher-untrusted",
    ] {
        assert!(
            command_language.contains(phrase),
            "docs/command-language.md is missing failure-language wording: {phrase}"
        );
    }

    let testing = fs::read_to_string(repo_root().join("docs/testing.md")).unwrap();
    for phrase in [
        "Failure-oriented CLI smoke:",
        "Expect `root/setup`",
        "guild --registry-root target/dev-local-registry/cli-local verify missing-skill@^0.1",
        "Expect `trust/verification`",
        "signed bundle trust or integrity failures should say `trust/verification`",
        "wrong-world manifest drift and broader Guild component imports should say",
        "`runtime/compatibility`, not `authority denial`",
    ] {
        assert!(
            testing.contains(phrase),
            "docs/testing.md is missing failure-oriented CLI smoke wording: {phrase}"
        );
    }
}

#[test]
fn docs_describe_json_failure_machine_surface() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert!(
        readme.contains(
            "When a command supports `--json`, failure output stays machine-readable too: stdout carries a JSON `error` envelope, stderr stays empty, and the process exits nonzero."
        ),
        "README.md should describe the JSON failure surface",
    );

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    assert!(
        command_language.contains(
            "stdout carries a JSON `error` envelope, stderr stays empty, and the process"
        ),
        "docs/command-language.md should describe the JSON failure surface",
    );

    let how_it_works = fs::read_to_string(repo_root().join("docs/how-guild-works.md")).unwrap();
    assert!(
        how_it_works.contains(
            "stdout carries a JSON `error` envelope, stderr stays empty, and the process"
        ),
        "docs/how-guild-works.md should describe the JSON failure surface",
    );
}

#[test]
fn trust_review_terms_stay_canonical_across_help_and_docs() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    for phrase in [
        "The current trust review loop is:",
        "`guild trust list`",
        "`guild import ... --preview` or `guild pull ... --preview`",
        "`guild import ...` or `guild pull ...`",
        "`guild verify -v <skill-ref>`",
        "Use `guild verify -v <skill-ref>` as the first installed-state verification explanation path",
        "`local-source`",
        "`verified-import`",
        "`local-dev`",
        "`trusted-imported`",
        "`restricted`",
        "`guild trust add --record-file <record.json>`",
        "`guild trust show <publisher-id>`",
        "`guild trust remove <publisher-id>`",
        "Execution-plan signing stays on the same local trust model:",
    ] {
        assert!(
            readme.contains(phrase),
            "README.md is missing trust-review wording: {phrase}"
        );
    }

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    for phrase in [
        "The current trust review loop is:",
        "`guild trust list`",
        "`guild import ... --preview` or `guild pull ... --preview`",
        "`guild import ...` or `guild pull ...`",
        "`guild verify -v <skill-ref>`",
        "Use `guild verify -v <skill-ref>` as the first installed-state verification explanation path",
        "`local-source`",
        "`verified-import`",
        "`local-dev`",
        "`trusted-imported`",
        "`restricted`",
        "`guild trust add --record-file <record.json>`",
        "`guild trust show <publisher-id>`",
        "`guild trust remove <publisher-id>`",
    ] {
        assert!(
            command_language.contains(phrase),
            "docs/command-language.md is missing trust-review wording: {phrase}"
        );
    }
}

#[test]
fn authority_lifecycle_language_stays_canonical_across_docs_and_spec() {
    let readme = fs::read_to_string(repo_root().join("README.md")).unwrap();
    assert_contains_canonical_authority_lifecycle(&readme, "README.md");

    let command_language =
        fs::read_to_string(repo_root().join("docs/command-language.md")).unwrap();
    assert_contains_canonical_authority_lifecycle(&command_language, "docs/command-language.md");

    let how_it_works = fs::read_to_string(repo_root().join("docs/how-guild-works.md")).unwrap();
    assert_contains_canonical_authority_lifecycle(&how_it_works, "docs/how-guild-works.md");

    let hello_readme =
        fs::read_to_string(repo_root().join("examples/skills/hello-inspect/README.md")).unwrap();
    assert_contains_canonical_authority_lifecycle(
        &hello_readme,
        "examples/skills/hello-inspect/README.md",
    );
    assert!(
        hello_readme.contains(
            "In this example, `--grants-json` is the caller-requested grants input for `guild run`."
        ),
        "examples/skills/hello-inspect/README.md should tie --grants-json to requested authority",
    );

    let specs = fs::read_to_string(repo_root().join("SPECS.md")).unwrap();
    for phrase in [
        "described with one stable authority lifecycle vocabulary:",
        "- declared authority: capabilities declared by the installed manifest",
        "- requested authority: caller-requested grants for one run",
        "- granted authority: the final capability slice the host policy allows for that run",
        "- effective at runtime: the authority the guest can actually exercise during execution",
        "This explanatory vocabulary does not widen the normative model above; the",
    ] {
        assert!(
            specs.contains(phrase),
            "SPECS.md is missing authority-lifecycle bridge wording: {phrase}"
        );
    }
}
