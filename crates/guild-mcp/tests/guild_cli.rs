use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use guild_manifest::SkillManifest;
use guild_mcp::protocol::{InitializeResult, ListToolsResult, PROTOCOL_VERSION_2025_11_25};
use guild_registry::LocalSourceInstaller;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    CapabilityRequirement, EmitEvidenceConstraints, EvidenceAudience, FilesystemConstraints,
    FilesystemOperation, FilesystemRoot, GrantedCapability, InvokeDependencyConstraints,
    ReadResourceConstraints, RedactionClass, ResourceKind,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

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

fn command_json(value: Value) -> String {
    serde_json::to_string(&value).unwrap()
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

fn install_with_cli(registry_root: &Path) {
    install_source_with_cli(registry_root, &hello_source_dir());
}

fn install_source_with_cli(registry_root: &Path, source_dir: &Path) {
    let source_dir = source_dir.display().to_string();
    let root = registry_root.display().to_string();
    let _ = run_guild_success(&["--registry-root", &root, "install", &source_dir], None);
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
        concat!(
            "example/hello-inspect@0.1.0  Hello Inspect\n",
            "status: local-source / local-dev\n",
            "support: proof-backed(log-write) not_proven(emit-evidence)\n",
            "runtime: wasm-component / guild-skill-inspect-v1\n",
            "caps: emit-evidence(write,required) log-write(write)\n",
        )
    );

    let verify_output = run_guild_success(
        &["verify", "hello-inspect@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_output,
        concat!(
            "example/hello-inspect@0.1.0\n",
            "verification: local-source\n",
            "trust: local-dev\n",
        )
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

    let verify_output = run_guild_success(
        &["verify", "incident-brief@^0.1", "--color", "never"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_output,
        concat!(
            "example/incident-brief@0.1.0\n",
            "verification: local-source\n",
            "trust: local-dev\n",
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
        stderr.contains("filesystem-runtime-not-supported"),
        "{stderr}"
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

    let show_porcelain = run_guild_success(
        &["show", "hello-inspect@^0.1", "--porcelain"],
        Some(&registry_root),
    );
    assert_eq!(
        show_porcelain,
        "skill\texample/hello-inspect@0.1.0\tlocal-source\tlocal-dev\tnot_proven\n"
    );

    let verify_porcelain = run_guild_success(
        &["verify", "hello-inspect@^0.1", "--porcelain"],
        Some(&registry_root),
    );
    assert_eq!(
        verify_porcelain,
        "verify\texample/hello-inspect@0.1.0\tlocal-source\tlocal-dev\n"
    );

    let grants_json = emit_evidence_grants_json();
    let run_json = run_guild_success(
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
    let run_value: Value = parse_json_stdout(&run_json);
    let execution_id = run_value["record"]["receipt"]["execution_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let exec_prefix = format!("exec:{}", &execution_id[..12]);

    let why_json = run_guild_success(&["why", &exec_prefix, "--json"], Some(&registry_root));
    let why_value: Value = parse_json_stdout(&why_json);
    assert_eq!(why_value["summary"]["plan"].as_str(), Some("upper-bound"));
    assert_eq!(why_value["summary"]["proof"].as_str(), Some("not_proven"));
    assert_eq!(why_value["summary"]["token"].as_str(), Some("upper-bound"));
    assert_eq!(why_value["summary"]["witness"].as_str(), Some("unlinked"));

    let why_porcelain =
        run_guild_success(&["why", &exec_prefix, "--porcelain"], Some(&registry_root));
    assert!(
        why_porcelain.starts_with(&format!(
            "why\t{}\tupper-bound\tnot_proven\tupper-bound\tunlinked\t",
            execution_id
        )),
        "{why_porcelain}"
    );
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
    assert_eq!(
        get_execution_value["uri"].as_str(),
        Some(execution_uri.as_str())
    );

    let show_execution = run_guild_success(&["show", &exec_prefix, "--json"], Some(&registry_root));
    let show_execution_value: Value = parse_json_stdout(&show_execution);
    assert_eq!(
        show_execution_value["record"]["receipt"]["uri"].as_str(),
        Some(execution_uri.as_str())
    );

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

    let show_object = run_guild_success(&["show", &object_prefix, "--json"], Some(&registry_root));
    let show_object_value: Value = parse_json_stdout(&show_object);
    assert_eq!(
        show_object_value["record"]["uri"].as_str(),
        Some(blob_uri.as_str())
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
        show_stderr.contains(
            "short skill ref `hello-inspect@^0.1` was ambiguous across namespaces: example, other-example"
        ) || show_stderr.contains(
            "short skill ref `hello-inspect@^0.1` was ambiguous across namespaces: other-example, example"
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
        why_stderr.contains("execution ref `exec:0` was ambiguous"),
        "{why_stderr}"
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

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("read-only commands do not initialize a new root"),
        "{stderr}"
    );
    assert!(
        stderr.contains(default_root.to_string_lossy().as_ref()),
        "{stderr}"
    );
    assert!(!default_root.exists());
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
    assert!(stderr.contains("execution-plan-signature-digest-mismatch"));
}

#[test]
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

    let tools_response = harness.request("tools/list", &json!({}));
    let tools: ListToolsResult = serde_json::from_value(tools_response["result"].clone()).unwrap();
    assert_eq!(tools.tools.len(), 1);
    assert_eq!(tools.tools[0].name, "guild.inspect");
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
    assert!(stdout.contains("guild help refs"));
    assert!(stdout.contains("guild help trust"));
    assert!(stdout.contains("guild help roots"));
    assert!(stdout.contains("guild <command> --help"));
    assert!(!stdout.contains("deferred:"));
    assert!(!stdout.contains("inspect path"));
    assert!(!stdout.contains("dogfood"));
}

#[test]
fn shared_help_topics_are_available() {
    let help = run_guild_success(&["help"], None);
    assert!(help.contains("Guild help topics"));
    assert!(help.contains("guild help [refs|trust|roots]"));

    let refs = run_guild_success(&["help", "refs"], None);
    assert!(refs.contains("Guild ref forms"));
    assert!(refs.contains("skill://<namespace>/<name>@<version-or-range>"));
    assert!(refs.contains("guild://..."));

    let trust = run_guild_success(&["help", "trust"], None);
    assert!(trust.contains("Trust and verification"));
    assert!(trust.contains("guild verify <skill-ref>"));
    assert!(trust.contains("guild trust verify-plan"));

    let roots = run_guild_success(&["help", "roots"], None);
    assert!(roots.contains("Guild root resolution"));
    assert!(roots.contains("GUILD_REGISTRY_ROOT"));
    assert!(roots.contains("There is no cwd-local .guild fallback."));
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
    assert!(stdout.contains("guild help refs"));
}

#[test]
fn run_help_uses_input_file_flag_and_ref_topic() {
    let stdout = run_guild_success(&["run", "--help"], None);
    assert!(stdout.contains("Run a skill locally"));
    assert!(stdout.contains("--input-file <PATH>"));
    assert!(!stdout.contains("input-file-path"));
    assert!(stdout.contains("stdout carries the result payload."));
    assert!(stdout.contains("guild help refs"));
}

#[test]
fn why_and_verify_help_call_out_scope() {
    let why_help = run_guild_success(&["why", "--help"], None);
    assert!(why_help.contains("Explain a persisted execution"));
    assert!(why_help.contains("persisted execution record"));

    let verify_help = run_guild_success(&["verify", "--help"], None);
    assert!(verify_help.contains("Show installed trust and verification status"));
    assert!(verify_help.contains("guild trust verify-plan"));
    assert!(verify_help.contains("guild help trust"));
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
