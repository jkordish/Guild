use std::path::{Path, PathBuf};

use guild_mcp::{GuildMcpFacade, InspectRequest};
use guild_registry::{execution_query_resource_uri, LocalRegistry, LocalSourceInstaller};
use guild_runner::WasmtimeRuntimeAdapter;
use guild_types::{
    CapabilityAccess, CapabilityConstraints, CapabilityGrantSet, CapabilityId,
    ExecutionQueryResource, GrantedCapability, HttpMethod, HttpRequestConstraints, HttpScheme,
    ReadResourceConstraints, RequestedSkillRef, ResourceKind, SkillKey, VersionRequirement,
};

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root exists")
}

fn inspect_http_source_dir() -> PathBuf {
    repo_root().join("examples/skills/inspect-http-json")
}

fn summarize_query_source_dir() -> PathBuf {
    repo_root().join("examples/skills/summarize-execution-query")
}

fn local_registry_root() -> PathBuf {
    repo_root().join("target/dev-local-registry/explain-recent-failures-local")
}

fn reset_registry_root(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn inspect_http_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "inspect-http-json".into(),
        },
        version_req: VersionRequirement::parse("^0.1").expect("example version requirement parses"),
    }
}

fn summarize_query_skill() -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: "summarize-execution-query".into(),
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

fn query_grant() -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(vec!["guild://queries/executions/".into()]),
            resource_kinds: Some(vec![ResourceKind::Query]),
        }),
    }
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = http_test_server::HttpTestServer::start();
    let registry_root = local_registry_root();
    reset_registry_root(&registry_root)?;

    let installer = LocalSourceInstaller::new(&registry_root)?;
    let inspect_http = installer.install(inspect_http_source_dir())?;

    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);

    let succeeded = facade.inspect(InspectRequest::new(
        inspect_http_skill(),
        serde_json::json!({
            "url": server.json_url(),
            "method": "get",
            "json_pointers": ["/message"],
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

    let failed = facade
        .inspect(InspectRequest::new(
            inspect_http_skill(),
            serde_json::json!({
                "url": server.json_url(),
                "method": "post",
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
        ))
        .expect_err("invalid method should persist a failed execution");

    let rejected = facade
        .inspect(InspectRequest::new(
            inspect_http_skill(),
            serde_json::json!({
                "url": server.json_url(),
                "method": "get",
            }),
            "tenant-dev",
            "actor-dev",
            CapabilityGrantSet::default(),
        ))
        .expect_err("missing HTTP grant should persist a rejected execution");

    let query_uri =
        execution_query_resource_uri(&ExecutionQueryResource::FailuresRecent { limit: 10 });
    let query_resource = facade.read_resource(&query_uri)?;

    let summarize_query = installer.install(summarize_query_source_dir())?;
    let registry = LocalRegistry::load(&registry_root)?;
    let facade = GuildMcpFacade::new(registry, WasmtimeRuntimeAdapter::new()?);
    let summary = facade.inspect(InspectRequest::new(
        summarize_query_skill(),
        serde_json::json!({
            "query_uri": query_uri,
        }),
        "tenant-dev",
        "actor-dev",
        CapabilityGrantSet {
            grants: vec![query_grant()],
        },
    ))?;

    println!(
        "installed inspect-http-json {}",
        inspect_http.resolved_ref.digest
    );
    println!(
        "installed summarize-execution-query {}",
        summarize_query.resolved_ref.digest
    );
    println!(
        "successful execution URI: {}",
        succeeded.structured_content.receipt.uri
    );
    println!(
        "failed execution URI: {}",
        failed
            .receipt
            .as_ref()
            .expect("failed execution returns a persisted receipt")
            .uri
    );
    println!(
        "rejected execution URI: {}",
        rejected
            .receipt
            .as_ref()
            .expect("rejected execution returns a persisted receipt")
            .uri
    );
    println!("query resource URI: {}", query_resource.uri);
    println!("{}", String::from_utf8(query_resource.bytes)?);
    println!("{}", summary.summary);
    println!(
        "{}",
        serde_json::to_string_pretty(&summary.structured_content)?
    );

    Ok(())
}
