use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use guild_mcp::protocol::{InitializeResult, PROTOCOL_VERSION_2025_11_25};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

pub struct McpStdioClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl McpStdioClient {
    pub fn spawn(
        command: impl AsRef<Path>,
        args: &[String],
        cwd: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = Command::new(command.as_ref())
            .args(args)
            .current_dir(cwd)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        Ok(Self {
            stdin: child.stdin.take().ok_or("missing child stdin")?,
            stdout: BufReader::new(child.stdout.take().ok_or("missing child stdout")?),
            child,
            next_id: 1,
        })
    }

    pub fn initialize(
        &mut self,
        client_name: &str,
    ) -> Result<InitializeResult, Box<dyn std::error::Error>> {
        let response = self.request(
            "initialize",
            &json!({
                "protocolVersion": PROTOCOL_VERSION_2025_11_25,
                "capabilities": {},
                "clientInfo": {
                    "name": client_name,
                    "version": "0.1.0"
                }
            }),
        )?;
        let initialized: InitializeResult = parse_result(&response)?;
        self.notify("notifications/initialized", &json!({}))?;
        Ok(initialized)
    }

    pub fn request(
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

    pub fn notify(
        &mut self,
        method: &str,
        params: &Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
            return Err("MCP server exited before responding".into());
        }
        Ok(serde_json::from_str(&line)?)
    }
}

impl Drop for McpStdioClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn parse_result<T: DeserializeOwned>(
    response: &Value,
) -> Result<T, Box<dyn std::error::Error>> {
    if let Some(error) = response.get("error") {
        return Err(format!("MCP error: {}", serde_json::to_string_pretty(error)?).into());
    }

    Ok(serde_json::from_value(response["result"].clone())?)
}
