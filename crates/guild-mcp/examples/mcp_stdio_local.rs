use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use guild_mcp::protocol::{
    CallToolResult, InitializeResult, ListToolsResult, PROTOCOL_VERSION_2025_11_25,
    ReadResourceResult,
};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    EmitEvidenceConstraints, EvidenceAudience, GrantedCapability, RedactionClass,
    RequestedSkillRef, SkillKey, VersionRequirement,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/hello-inspect")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/mcp-stdio-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

fn ensure_server_binary() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = repo_root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .arg("build")
        .arg("-p")
        .arg("guild-mcp")
        .arg("--bin")
        .arg("guild-mcp-server")
        .current_dir(&root)
        .status()?;

    if !status.success() {
        return Err("failed to build guild-mcp-server".into());
    }

    Ok(root.join("target/debug/guild-mcp-server"))
}

struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(
        server_binary: &Path,
        registry_root: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = Command::new(server_binary)
            .arg("--registry-root")
            .arg(registry_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        Ok(Self {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
            next_id: 1,
        })
    }

    fn initialize(&mut self) -> Result<InitializeResult, Box<dyn std::error::Error>> {
        let response = self.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION_2025_11_25,
                "capabilities": {},
                "clientInfo": {
                    "name": "guild-mcp-example-client",
                    "version": "0.1.0"
                }
            }),
        )?;
        let initialized: InitializeResult = parse_result(&response)?;
        self.notify("notifications/initialized", &json!({}))?;
        Ok(initialized)
    }

    fn request(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        self.write_message(&request)?;
        self.read_message()
    }

    fn notify(&mut self, method: &str, params: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_message(&request)
    }

    fn write_message(&mut self, message: &Value) -> Result<(), Box<dyn std::error::Error>> {
        serde_json::to_writer(&mut self.stdin, message)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn read_message(&mut self) -> Result<Value, Box<dyn std::error::Error>> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line)?;
        if read == 0 {
            return Err("guild-mcp-server exited before responding".into());
        }
        Ok(serde_json::from_str(&line)?)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_result<T: DeserializeOwned>(response: &Value) -> Result<T, Box<dyn std::error::Error>> {
    if let Some(error) = response.get("error") {
        return Err(format!("MCP error: {}", serde_json::to_string_pretty(error)?).into());
    }

    Ok(serde_json::from_value(response["result"].clone())?)
}

fn emit_evidence_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::EmitEvidence,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::EmitEvidence(EmitEvidenceConstraints {
            max_bytes: Some(65_536),
            audiences: Some(vec![EvidenceAudience::User]),
            redactions: Some(vec![RedactionClass::None]),
        }),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;

    LocalSourceInstaller::new(&registry_root)?.install(example_source_dir())?;
    let _registry = LocalRegistry::load(&registry_root)?;

    let server_binary = ensure_server_binary()?;
    let mut client = McpClient::spawn(&server_binary, &registry_root)?;
    let initialized = client.initialize()?;
    println!(
        "initialized Guild MCP server {} {}",
        initialized.server_info.name, initialized.protocol_version
    );

    let tools: ListToolsResult = parse_result(&client.request("tools/list", &json!({}))?)?;
    println!(
        "tools: {}",
        tools
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let inspect_result: CallToolResult = parse_result(&client.request(
        "tools/call",
        &json!({
            "name": "guild.inspect",
            "arguments": {
                "skill": RequestedSkillRef {
                    key: SkillKey {
                        namespace: "example".into(),
                        name: "hello-inspect".into(),
                    },
                    version_req: VersionRequirement::parse("^0.1")?,
                },
                "input": {
                    "name": "Ada"
                },
                "grants": CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                }
            }
        }),
    )?)?;
    let record: guild_types::ExecutionRecord = serde_json::from_value(
        inspect_result
            .structured_content
            .clone()
            .expect("inspect returns structured content"),
    )?;
    println!("execution uri: {}", record.receipt.uri);
    if let Some(first) = record.emitted_evidence.first() {
        println!("evidence uri: {}", first.uri);
    }

    let execution_resource: ReadResourceResult =
        parse_result(&client.request("resources/read", &json!({ "uri": record.receipt.uri }))?)?;
    println!(
        "execution resource contents: {} item(s)",
        execution_resource.contents.len()
    );

    if let Some(first) = record.emitted_evidence.first() {
        let evidence_resource: ReadResourceResult =
            parse_result(&client.request("resources/read", &json!({ "uri": first.uri }))?)?;
        println!(
            "evidence resource contents: {} item(s)",
            evidence_resource.contents.len()
        );
    }

    Ok(())
}
