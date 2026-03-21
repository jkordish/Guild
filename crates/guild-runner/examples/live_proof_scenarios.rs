use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use guild_registry::{LocalRegistry, LocalSourceInstaller, SkillRegistry, execution_resource_uri};
use guild_runner::{LiveProofComparatorProfile, Runner, WasmtimeRuntimeAdapter};
use guild_types::{
    Budget, CallerRequest, CapabilityAccess, CapabilityConstraints, CapabilityGrantSet,
    CapabilityId, EmitEvidenceConstraints, EvidenceAudience, ExecutionMode, GrantedCapability,
    HttpMethod, HttpRequestConstraints, HttpScheme, PolicyDecision, PolicyDecisionOutcome,
    ReadResourceConstraints, RedactionClass, RequestedSkillRef, ResolvedExecutionEnvelope,
    ResourceKind, Severity, SkillKey, VersionRequirement,
};
use serde_json::json;

#[path = "../../../test-support/http_test_server.rs"]
mod http_test_server;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn requested_skill(name: &str) -> RequestedSkillRef {
    RequestedSkillRef {
        key: SkillKey {
            namespace: "example".into(),
            name: name.into(),
        },
        version_req: VersionRequirement::parse("^0.1").unwrap(),
    }
}

fn unique_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{prefix}-{nanos}")
}

fn build_runner() -> Runner<WasmtimeRuntimeAdapter> {
    Runner::new(WasmtimeRuntimeAdapter::new().unwrap())
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

fn read_resource_grant(prefixes: &[&str]) -> GrantedCapability {
    let resource_kinds = prefixes
        .iter()
        .filter_map(|prefix| ResourceKind::from_uri_prefix(prefix))
        .fold(Vec::new(), |mut kinds, kind| {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
            kinds
        });
    GrantedCapability {
        id: CapabilityId::ReadResource,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::ReadResource(ReadResourceConstraints {
            uri_prefixes: Some(prefixes.iter().map(|value| (*value).to_owned()).collect()),
            resource_kinds: Some(resource_kinds),
        }),
    }
}

fn log_grant(levels: &[Severity]) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::LogWrite,
        access: CapabilityAccess::Write,
        constraints: CapabilityConstraints::Log(guild_types::LogConstraints {
            levels: Some(levels.to_vec()),
        }),
    }
}

fn http_grant(host: &str, port: u16, path_prefix: &str) -> GrantedCapability {
    GrantedCapability {
        id: CapabilityId::HttpRequest,
        access: CapabilityAccess::Read,
        constraints: CapabilityConstraints::HttpRequest(HttpRequestConstraints {
            allowed_schemes: Some(vec![HttpScheme::Http]),
            allowed_hosts: Some(vec![host.to_owned()]),
            allowed_host_suffixes: None,
            allowed_ports: Some(vec![port]),
            allowed_methods: Some(vec![HttpMethod::Get]),
            allowed_path_prefixes: Some(vec![path_prefix.to_owned()]),
            max_timeout_ms: Some(2_000),
            max_response_bytes: Some(16_384),
            follow_redirects: Some(false),
            max_redirects: None,
            allow_loopback: Some(true),
            allow_link_local: Some(false),
            allow_private_networks: Some(false),
            allow_ip_literals: Some(true),
        }),
    }
}

fn envelope_for(
    installed: &guild_registry::InstalledSkill,
    input: serde_json::Value,
    grants: CapabilityGrantSet,
) -> ResolvedExecutionEnvelope {
    ResolvedExecutionEnvelope {
        request: CallerRequest {
            request_id: unique_id("proof-request"),
            skill: requested_skill(&installed.resolved_ref.key.name),
            tenant_id: "tenant-1".into(),
            actor_id: "actor-1".into(),
            mode: ExecutionMode::Inspect,
            input,
            budget: Budget::default(),
            requested_capabilities: grants.clone(),
            idempotency_key: None,
            trace_id: unique_id("trace"),
        },
        resolved_skill: installed.resolved_ref.clone(),
        granted_capabilities: grants,
        policy_decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Allowed,
            summary: "local policy granted requested capabilities".into(),
            profile_name: "default".into(),
            trust_tier: guild_types::LocalTrustTier::LocalDev,
            verification_state: guild_types::InstalledVerificationState::LocalSource,
            reasons: Vec::new(),
            detail: None,
        },
        parent_execution_id: None,
    }
}

struct TempRegistry {
    root: PathBuf,
}

impl TempRegistry {
    fn new() -> Self {
        let root = std::env::temp_dir().join(unique_id("guild-live-proof-example"));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn install(&self, skill_dir: impl AsRef<Path>) {
        LocalSourceInstaller::new(&self.root)
            .unwrap()
            .install(skill_dir)
            .unwrap();
    }

    fn load(&self) -> LocalRegistry {
        LocalRegistry::load(&self.root).unwrap()
    }
}

impl Drop for TempRegistry {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn run_read_resource_bounded() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    temp.install(repo_root().join("examples/skills/explain-execution"));
    let registry = temp.load();
    let runner = build_runner();

    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();
    let baseline_target = runner
        .execute(
            &registry,
            &hello,
            &envelope_for(
                &hello,
                json!({ "name": "Ada" }),
                CapabilityGrantSet {
                    grants: vec![emit_evidence_grant()],
                },
            ),
        )
        .unwrap();

    let explain = registry.resolve(&requested_skill("explain-execution")).unwrap();
    let result = runner
        .prove_live_authority(
            &registry,
            &explain,
            &envelope_for(
                &explain,
                json!({
                    "execution_uri": execution_resource_uri(&baseline_target.receipt.execution_id),
                    "include_first_evidence": false,
                }),
                CapabilityGrantSet {
                    grants: vec![read_resource_grant(&[
                        "guild://executions/",
                        "guild://objects/records/",
                    ])],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    json!({
        "scenario": "read-resource-bounded",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_http_request_not_proven() -> serde_json::Value {
    let server = http_test_server::HttpTestServer::start();
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/inspect-http-json"));
    let registry = temp.load();
    let runner = build_runner();
    let http_skill = registry.resolve(&requested_skill("inspect-http-json")).unwrap();

    let result = runner
        .prove_live_authority(
            &registry,
            &http_skill,
            &envelope_for(
                &http_skill,
                json!({
                    "url": server.json_url(),
                    "method": "get",
                    "json_pointers": ["/message"],
                }),
                CapabilityGrantSet {
                    grants: vec![http_grant(
                        http_test_server::HttpTestServer::host(),
                        server.port(),
                        "/json",
                    )],
                },
            ),
            LiveProofComparatorProfile::ExactOutput,
        )
        .unwrap();

    json!({
        "scenario": "http-request-not-proven",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn run_log_write_reduced() -> serde_json::Value {
    let temp = TempRegistry::new();
    temp.install(repo_root().join("examples/skills/hello-inspect"));
    let registry = temp.load();
    let runner = build_runner();
    let hello = registry.resolve(&requested_skill("hello-inspect")).unwrap();

    let result = runner
        .prove_live_authority(
            &registry,
            &hello,
            &envelope_for(
                &hello,
                json!({ "name": "Ada", "emit_log": true }),
                CapabilityGrantSet {
                    grants: vec![
                        emit_evidence_grant(),
                        log_grant(&[Severity::Info, Severity::Error]),
                    ],
                },
            ),
            LiveProofComparatorProfile::NormalizedInspectOutputV1,
        )
        .unwrap();

    json!({
        "scenario": "log-write-reduced",
        "baseline_execution_record": result.baseline_execution_record,
        "proof": result.proof,
    })
}

fn main() {
    let scenario = env::args()
        .nth(1)
        .unwrap_or_else(|| "read-resource-bounded".into());
    let output = match scenario.as_str() {
        "read-resource-bounded" => run_read_resource_bounded(),
        "http-request-not-proven" => run_http_request_not_proven(),
        "log-write-reduced" => run_log_write_reduced(),
        other => {
            eprintln!("unknown live proof scenario: {other}");
            std::process::exit(2);
        }
    };
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
