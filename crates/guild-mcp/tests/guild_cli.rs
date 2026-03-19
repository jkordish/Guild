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
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, RedactionClass,
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

fn guild_command(env_registry_root: Option<&Path>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_guild"));
    command.current_dir(repo_root());
    if let Some(root) = env_registry_root {
        command.env("GUILD_REGISTRY_ROOT", root);
    } else {
        command.env_remove("GUILD_REGISTRY_ROOT");
    }
    command
}

fn run_guild(args: &[&str], env_registry_root: Option<&Path>) -> Output {
    guild_command(env_registry_root)
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

fn parse_json_stdout<T: DeserializeOwned>(stdout: &str) -> T {
    serde_json::from_str(stdout).unwrap()
}

fn install_with_cli(registry_root: &Path) {
    let source_dir = hello_source_dir().display().to_string();
    let root = registry_root.display().to_string();
    let _ = run_guild_success(&["--registry-root", &root, "install", &source_dir], None);
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
fn missing_registry_root_fails_with_explicit_guidance() {
    let output = run_guild(&["inspect", "skill://example/hello-inspect@^0.1"], None);
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("pass `--registry-root <path>` or set `GUILD_REGISTRY_ROOT`"));
    assert!(
        stderr.contains("there is no implicit `.guild/` or `target/dev-local-registry/...` root")
    );
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
