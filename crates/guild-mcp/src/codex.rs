use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use guild_registry::{InstalledSkill, LocalSourceInstaller, RegistryError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::SERVER_BINARY_NAME;

pub const CODEX_WORKFLOW_BINARY_NAME: &str = "guild-codex";
pub const DEFAULT_CODEX_SERVER_NAME: &str = "guild-local";
const DEFAULT_CODEX_REGISTRY_ROOT: &str = "target/dev-local-registry/codex-local";
const DEFAULT_CODEX_SKILLS: [&str; 4] = [
    "hello-inspect",
    "hello-composite",
    "explain-execution",
    "explain-execution-tree",
];

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BootstrappedSkill {
    pub namespace: String,
    pub name: String,
    pub version: String,
    pub digest: String,
    pub source_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexBootstrapSummary {
    pub repo_root: PathBuf,
    pub registry_root: PathBuf,
    pub skills: Vec<BootstrappedSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexServerConfig {
    pub name: String,
    pub cwd: PathBuf,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct CodexBootstrapOutput {
    pub bootstrap: CodexBootstrapSummary,
    pub config: CodexServerConfig,
    pub recommended_proof_commands: Vec<String>,
}

/// Resolve the repository root used by the local Codex workflow helper.
///
/// # Panics
///
/// Panics if the workspace root cannot be resolved from the current crate
/// location.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

#[must_use]
pub fn default_registry_root() -> PathBuf {
    repo_root().join(DEFAULT_CODEX_REGISTRY_ROOT)
}

#[must_use]
pub fn recommended_proof_commands() -> Vec<String> {
    vec![
        "cargo run -p guild-mcp --example codex_explain_execution_local".into(),
        "cargo run -p guild-mcp --example codex_explain_execution_tree_local".into(),
    ]
}

/// Build the default local Codex dogfood root by installing the example skills
/// used by the documented Codex flows.
///
/// # Errors
///
/// Returns an error if the registry root cannot be prepared or an example skill
/// cannot be installed.
pub fn bootstrap_codex_registry(
    registry_root: impl AsRef<Path>,
    reset: bool,
) -> Result<CodexBootstrapSummary, RegistryError> {
    let repo_root = repo_root();
    let registry_root = prepare_registry_root(registry_root, reset)?;
    let installer = LocalSourceInstaller::new(&registry_root)?;

    let mut skills = Vec::with_capacity(DEFAULT_CODEX_SKILLS.len());
    for skill_dir in DEFAULT_CODEX_SKILLS {
        let installed_skill =
            installer.install(repo_root.join("examples/skills").join(skill_dir))?;
        skills.push(summarize_installed_skill(skill_dir, installed_skill));
    }

    Ok(CodexBootstrapSummary {
        repo_root,
        registry_root,
        skills,
    })
}

#[must_use]
pub fn codex_server_config(
    registry_root: impl AsRef<Path>,
    name: impl Into<String>,
) -> CodexServerConfig {
    let registry_root = absolute_path(registry_root);
    let mut env = BTreeMap::new();
    env.insert(
        "GUILD_REGISTRY_ROOT".into(),
        registry_root.to_string_lossy().into_owned(),
    );

    CodexServerConfig {
        name: name.into(),
        cwd: repo_root(),
        command: "cargo".into(),
        args: vec![
            "run".into(),
            "-q".into(),
            "-p".into(),
            "guild-mcp".into(),
            "--bin".into(),
            SERVER_BINARY_NAME.into(),
            "--".into(),
        ],
        env,
    }
}

impl CodexServerConfig {
    /// Render the exact local command used to launch the Guild stdio MCP server
    /// outside Codex.
    ///
    /// # Panics
    ///
    /// Panics if the config does not carry `GUILD_REGISTRY_ROOT`, which is a
    /// required invariant for instances built through `codex_server_config`.
    #[must_use]
    pub fn manual_server_command(&self) -> String {
        let registry_root = self.registry_root_env();
        format!(
            "cargo run -q -p guild-mcp --bin {SERVER_BINARY_NAME} -- --registry-root {}",
            shell_quote(registry_root)
        )
    }

    /// Render the `codex mcp add` command matching this local stdio server
    /// configuration.
    ///
    /// # Panics
    ///
    /// Panics if the config does not carry `GUILD_REGISTRY_ROOT`, which is a
    /// required invariant for instances built through `codex_server_config`.
    #[must_use]
    pub fn codex_mcp_add_command(&self) -> String {
        let registry_root = self.registry_root_env();
        let mut command = format!(
            "codex mcp add {} --env GUILD_REGISTRY_ROOT={} -- {}",
            shell_quote(&self.name),
            shell_quote(registry_root),
            shell_quote(&self.command),
        );

        for arg in &self.args {
            let _ = write!(command, " {}", shell_quote(arg));
        }

        command
    }

    #[must_use]
    pub fn config_toml(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|arg| toml_string(arg))
            .collect::<Vec<_>>()
            .join(", ");
        let env = self
            .env
            .iter()
            .map(|(key, value)| format!("{key} = {}", toml_string(value)))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "[mcp_servers.{}]\ncwd = {}\ncommand = {}\nargs = [{}]\nenv = {{ {} }}",
            self.name,
            toml_string(&self.cwd.to_string_lossy()),
            toml_string(&self.command),
            args,
            env
        )
    }

    fn registry_root_env(&self) -> &str {
        self.env
            .get("GUILD_REGISTRY_ROOT")
            .map(String::as_str)
            .expect("Codex server config always carries GUILD_REGISTRY_ROOT")
    }
}

fn prepare_registry_root(
    registry_root: impl AsRef<Path>,
    reset: bool,
) -> Result<PathBuf, RegistryError> {
    let registry_root = absolute_path(registry_root);

    if reset && registry_root.exists() {
        fs::remove_dir_all(&registry_root).map_err(|error| {
            io_registry_error(
                "codex-bootstrap-reset-failed",
                "failed to reset the requested Codex registry root",
                &registry_root,
                &error,
            )
        })?;
    } else if registry_root.exists() && !directory_is_empty(&registry_root)? {
        return Err(RegistryError::new(
            "codex-bootstrap-root-not-empty",
            "registry root already exists and is not empty; pass --reset to rebuild it",
        )
        .with_detail(serde_json::json!({
            "registry_root": registry_root,
        })));
    }

    fs::create_dir_all(&registry_root).map_err(|error| {
        io_registry_error(
            "codex-bootstrap-create-failed",
            "failed to create the requested Codex registry root",
            &registry_root,
            &error,
        )
    })?;

    registry_root.canonicalize().map_err(|error| {
        io_registry_error(
            "codex-bootstrap-canonicalize-failed",
            "failed to canonicalize the requested Codex registry root",
            &registry_root,
            &error,
        )
    })
}

fn directory_is_empty(path: &Path) -> Result<bool, RegistryError> {
    let mut entries = fs::read_dir(path).map_err(|error| {
        io_registry_error(
            "codex-bootstrap-read-dir-failed",
            "failed to inspect the requested Codex registry root",
            path,
            &error,
        )
    })?;
    Ok(entries.next().is_none())
}

fn summarize_installed_skill(source_dir: &str, installed: InstalledSkill) -> BootstrappedSkill {
    BootstrappedSkill {
        namespace: installed.manifest.key.namespace,
        name: installed.manifest.key.name,
        version: installed.manifest.version.to_string(),
        digest: installed.resolved_ref.digest,
        source_dir: source_dir.into(),
    }
}

fn absolute_path(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current directory available")
            .join(path)
    }
}

fn io_registry_error(
    code: &str,
    message: &str,
    path: &Path,
    error: &std::io::Error,
) -> RegistryError {
    RegistryError::new(code, message).with_detail(serde_json::json!({
        "path": path,
        "io_error": error.to_string(),
    }))
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".into();
    }

    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return value.into();
    }

    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("JSON string escaping also works for TOML basic strings")
}
