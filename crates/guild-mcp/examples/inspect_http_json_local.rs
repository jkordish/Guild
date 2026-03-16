use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId, GrantedCapability,
    HttpMethod, HttpRequestConstraints, HttpScheme, RequestedSkillRef, SkillKey,
    VersionRequirement,
};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn example_source_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/inspect-http-json-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn requested_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "inspect-http-json".into(),
        },
        version_req: VersionRequirement::parse("^0.1").expect("example version requirement parses"),
    }
}

fn http_grant(host: &str, port: u16, path_prefix: &str, method: HttpMethod) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![method]),
            allowed_path_prefixes: Some(vec![path_prefix.to_owned()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(4_096),
        }),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = http_test_server::HttpTestServer::start();
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;
    let source_installer = LocalSourceInstaller::new(&registry_root)?;
    let installed_skill = source_installer.install(example_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let allow_response = facade.inspect(InspectRequest::new(
        requested_skill(),
        serde_json::json!({
            "url": server.json_url(),
            "method": "get",
            "json_pointers": ["/message", "/nested/count"],
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![http_grant(
                http_test_server::HttpTestServer::host(),
                server.port(),
                "/json",
                HttpMethod::Get,
            )],
        },
    ))?;

    println!("installed {}", installed_skill.resolved_ref.digest);
    println!("{}", allow_response.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&allow_response.structured_content)?
    );

    let stored_execution = facade.read_resource(&allow_response.structured_content.receipt.uri)?;
    println!("successful execution resource: {}", stored_execution.uri);
    println!("{}", String::from_utf8(stored_execution.bytes)?);

    let denied_error = facade
        .inspect(InspectRequest::new(
            requested_skill(),
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
                "json_pointers": ["/message"],
            }),
            "tenant-dev",
            "actor-dev",
            CapabilityGrantSet {
                grants: vec![http_grant(
                    "localhost",
                    server.port(),
                    "/json",
                    HttpMethod::Get,
                )],
            },
        ))
        .expect_err("mismatched host grant should fail closed");

    println!("denied: {} {}", denied_error.code, denied_error.message);
    let denied_receipt = denied_error
        .receipt
        .expect("HTTP denial persists a durable host-owned receipt");
    let denied_execution = facade.read_resource(&denied_receipt.uri)?;
    println!("denied execution resource: {}", denied_execution.uri);
    println!("{}", String::from_utf8(denied_execution.bytes)?);

    Ok(())
}
