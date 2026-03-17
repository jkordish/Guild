#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Execution boundary and runtime abstraction for Guild.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use guild_manifest::SkillManifest;
use guild_registry::{execution_resource_uri, InstalledSkill, RegistryError, SkillRegistry};
use guild_sdk_rust::GuildSkill;
use guild_types::{
    host_now_utc, mint_host_execution_id, CallerRequest, CapabilityAccess, CapabilityConstraints,
    CapabilityGrantSet, CapabilityId, CapabilityRequirement, ChildExecutionRecord, Diagnostic,
    Effect, EmitEvidenceConstraints, EvidenceAudience, EvidenceEmissionRequest, EvidenceRecord,
    EvidenceRef, ExecutionContext, ExecutionMetrics, ExecutionMode, ExecutionPhase,
    ExecutionReceipt, ExecutionRecord, ExecutionStatus, FilesystemConstraints, FilesystemOperation,
    FilesystemRoot, GrantedCapability, GuildResourceScope, GuildResourceUri, HttpMethod,
    HttpRequest, HttpRequestConstraints, HttpResponse, HttpScheme, InvokeDependencyConstraints,
    LocalPolicyConfig, LocalPolicyDefaultAction, LogConstraints, Mutability, PolicyDecision,
    PolicyDecisionOutcome, PolicyProfile, PolicyProfileBinding, PolicyReason, PolicyRule,
    PolicyRuleEffect, PolicyRuleTarget, Provenance, ReadResourceConstraints, RedactionClass,
    ResolvedExecutionEnvelope, ResolvedSkillRef, ResourceKind, ResourceReadResult, RuntimeKind,
    Severity, SkillError, SkillOutput, TerminationDetail,
};
use http::header::{CONTENT_TYPE, LOCATION};
use http::Request;
use http_body_util::{BodyExt, Empty};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};
use wasmtime_wasi_http::bindings::http::types::ErrorCode as WasiHttpErrorCode;
use wasmtime_wasi_http::body::HyperOutgoingBody;
use wasmtime_wasi_http::types::{default_send_request_handler, OutgoingRequestConfig};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../wit",
        world: "guild-skill",
        imports: { default: trappable },
    });
}

pub trait RuntimeAdapter: Send + Sync {
    fn kind(&self) -> RuntimeKind;

    /// Execute an installed skill with a host-issued context and host boundary.
    ///
    /// # Errors
    ///
    /// Returns a runtime failure when the runtime cannot load or execute the
    /// installed artifact successfully.
    fn execute(
        &self,
        installed: &InstalledSkill,
        context: &ExecutionContext,
        input: &Value,
        host: Arc<dyn RuntimeHost>,
    ) -> Result<RuntimeOutcome, RuntimeFailure>;
}

pub trait RuntimeHost: Send + Sync {
    /// Invoke a declared child dependency under the host's execution rules.
    ///
    /// # Errors
    ///
    /// Returns a child invocation error when the child cannot be invoked,
    /// authorized, or completed successfully.
    fn invoke_dependency(
        &self,
        parent: &InstalledSkill,
        context: &ExecutionContext,
        sequence: u16,
        alias: &str,
        input: &Value,
    ) -> Result<ChildInvocationOutcome, Box<ChildInvocationError>>;

    /// Persist guest-emitted evidence through the host-owned evidence store.
    ///
    /// # Errors
    ///
    /// Returns a skill error when the evidence cannot be accepted or persisted.
    fn emit_evidence(
        &self,
        execution_id: &str,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, SkillError>;

    /// Read a Guild resource through the host-owned resource backend.
    ///
    /// # Errors
    ///
    /// Returns a skill error when the URI is invalid, unauthorized, or missing.
    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, SkillError>;

    /// Execute a bounded outbound HTTP request through the host-owned runtime path.
    ///
    /// # Errors
    ///
    /// Returns a skill error when the request cannot be built, sent, or read
    /// within the host-enforced bounds.
    fn http_request(
        &self,
        request: &HttpRequest,
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<HostHttpResponse, SkillError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct ExecutionError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub phase: Option<ExecutionPhase>,
    pub detail: Option<Box<Value>>,
    pub receipt: Option<Box<ExecutionReceipt>>,
}

impl ExecutionError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
            phase: None,
            detail: None,
            receipt: None,
        }
    }

    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<Value>) -> Self {
        self.detail = Some(Box::new(detail.into()));
        self
    }

    #[must_use]
    pub fn with_phase(mut self, phase: ExecutionPhase) -> Self {
        self.phase = Some(phase);
        self
    }

    #[must_use]
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn with_receipt(mut self, receipt: ExecutionReceipt) -> Self {
        self.receipt = Some(Box::new(receipt));
        self
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ExecutionError {}

impl From<SkillError> for ExecutionError {
    fn from(value: SkillError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            retryable: value.retryable,
            phase: Some(ExecutionPhase::SkillDomain),
            detail: value.detail.map(Box::new),
            receipt: None,
        }
    }
}

impl From<RegistryError> for ExecutionError {
    fn from(value: RegistryError) -> Self {
        Self {
            code: value.code,
            message: value.message,
            retryable: false,
            phase: None,
            detail: value.detail.map(Box::new),
            receipt: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOutcome {
    pub output: SkillOutput,
    pub emitted_evidence: Vec<EvidenceRef>,
    pub child_executions: Vec<ChildExecutionRecord>,
    pub network_requests: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeFailure {
    pub error: Box<ExecutionError>,
    pub emitted_evidence: Vec<EvidenceRef>,
    pub child_executions: Vec<ChildExecutionRecord>,
    pub network_requests: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChildInvocationOutcome {
    pub output: SkillOutput,
    pub record: ChildExecutionRecord,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChildInvocationError {
    pub skill_error: SkillError,
    pub record: Option<Box<ChildExecutionRecord>>,
    denial: Option<CapabilityDenial>,
}

impl ChildInvocationError {
    fn without_record(skill_error: SkillError) -> Self {
        Self {
            skill_error,
            record: None,
            denial: None,
        }
    }

    fn denied(denial: CapabilityDenial) -> Self {
        Self {
            skill_error: denial.clone().into_skill_error(),
            record: None,
            denial: Some(denial),
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedHttpRequest {
    scheme: HttpScheme,
    host: String,
    port: u16,
    path: String,
    host_kind: ParsedHttpHost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParsedHttpHost {
    Domain { loopback_name: bool },
    IpLiteral(IpAddr),
}

impl ParsedHttpRequest {
    fn ip_literal(&self) -> Option<IpAddr> {
        match self.host_kind {
            ParsedHttpHost::Domain { .. } => None,
            ParsedHttpHost::IpLiteral(ip) => Some(ip),
        }
    }

    fn is_loopback_name(&self) -> bool {
        matches!(
            self.host_kind,
            ParsedHttpHost::Domain {
                loopback_name: true
            }
        )
    }
}

#[derive(Debug, Clone)]
struct PolicyEvaluationResult {
    granted_capabilities: CapabilityGrantSet,
    decision: PolicyDecision,
}

#[derive(Debug, Clone)]
struct CandidateGrant {
    grant: GrantedCapability,
    covers_required: bool,
}

#[derive(Debug, Clone, Copy)]
struct HttpExecutionPolicy {
    timeout: Duration,
    max_response_bytes: u64,
    follow_redirects: bool,
    max_redirects: u8,
}

#[derive(Debug, Clone)]
pub struct HostHttpResponse {
    response: HttpResponse,
    redirect_location: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingRedirect {
    from_url: String,
    status: u16,
    location: String,
}

#[derive(Debug, Clone, Copy)]
enum HttpGrantDenialKind {
    Method,
    Scheme,
    Host,
    Port,
    Path,
    Timeout,
    IpLiteral,
    Loopback,
    LinkLocal,
    PrivateNetwork,
    DestinationUnresolved,
}

const HTTP_DENIAL_METHOD: u16 = 1 << 0;
const HTTP_DENIAL_SCHEME: u16 = 1 << 1;
const HTTP_DENIAL_HOST: u16 = 1 << 2;
const HTTP_DENIAL_PORT: u16 = 1 << 3;
const HTTP_DENIAL_PATH: u16 = 1 << 4;
const HTTP_DENIAL_TIMEOUT: u16 = 1 << 5;
const HTTP_DENIAL_IP_LITERAL: u16 = 1 << 6;
const HTTP_DENIAL_LOOPBACK: u16 = 1 << 7;
const HTTP_DENIAL_LINK_LOCAL: u16 = 1 << 8;
const HTTP_DENIAL_PRIVATE_NETWORK: u16 = 1 << 9;
const HTTP_DENIAL_DESTINATION_UNRESOLVED: u16 = 1 << 10;
const HTTP_UNCONSTRAINED_MAX_REDIRECTS: u8 = 10;

#[derive(Debug, Default, Clone, Copy)]
struct HttpGrantState {
    authorized_timeout_ms: Option<u64>,
    authorized_response_bytes: Option<u64>,
    authorized_follow_redirects: bool,
    authorized_max_redirects: Option<u8>,
    denial_mask: u16,
}

impl HttpGrantState {
    fn authorize(
        &mut self,
        timeout_ms: u64,
        response_bytes: u64,
        follow_redirects: bool,
        max_redirects: Option<u8>,
    ) {
        self.authorized_timeout_ms = Some(
            self.authorized_timeout_ms
                .map_or(timeout_ms, |current| current.max(timeout_ms)),
        );
        self.authorized_response_bytes = Some(
            self.authorized_response_bytes
                .map_or(response_bytes, |current| current.max(response_bytes)),
        );
        if follow_redirects {
            self.authorized_follow_redirects = true;
            if let Some(max_redirects) = max_redirects {
                self.authorized_max_redirects = Some(
                    self.authorized_max_redirects
                        .map_or(max_redirects, |current| current.max(max_redirects)),
                );
            }
        }
    }

    fn note_denial(&mut self, denial: HttpGrantDenialKind) {
        self.denial_mask |= match denial {
            HttpGrantDenialKind::Method => HTTP_DENIAL_METHOD,
            HttpGrantDenialKind::Scheme => HTTP_DENIAL_SCHEME,
            HttpGrantDenialKind::Host => HTTP_DENIAL_HOST,
            HttpGrantDenialKind::Port => HTTP_DENIAL_PORT,
            HttpGrantDenialKind::Path => HTTP_DENIAL_PATH,
            HttpGrantDenialKind::Timeout => HTTP_DENIAL_TIMEOUT,
            HttpGrantDenialKind::IpLiteral => HTTP_DENIAL_IP_LITERAL,
            HttpGrantDenialKind::Loopback => HTTP_DENIAL_LOOPBACK,
            HttpGrantDenialKind::LinkLocal => HTTP_DENIAL_LINK_LOCAL,
            HttpGrantDenialKind::PrivateNetwork => HTTP_DENIAL_PRIVATE_NETWORK,
            HttpGrantDenialKind::DestinationUnresolved => HTTP_DENIAL_DESTINATION_UNRESOLVED,
        };
    }

    fn saw_denial(&self, denial_mask: u16) -> bool {
        self.denial_mask & denial_mask != 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
struct InProcessArtifact {
    implementation: String,
}

type DynSkill = dyn GuildSkill + Send + Sync;

#[derive(Clone, Default)]
pub struct InProcessRuntimeAdapter {
    implementations: HashMap<String, Arc<DynSkill>>,
}

impl InProcessRuntimeAdapter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<S>(&mut self, implementation: impl Into<String>, skill: S)
    where
        S: GuildSkill + Send + Sync + 'static,
    {
        self.register_arc(implementation, Arc::new(skill));
    }

    pub fn register_arc(&mut self, implementation: impl Into<String>, skill: Arc<DynSkill>) {
        self.implementations.insert(implementation.into(), skill);
    }
}

impl RuntimeAdapter for InProcessRuntimeAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::InProcess
    }

    fn execute(
        &self,
        installed: &InstalledSkill,
        context: &ExecutionContext,
        input: &Value,
        _host: Arc<dyn RuntimeHost>,
    ) -> Result<RuntimeOutcome, RuntimeFailure> {
        let artifact_bytes =
            fs::read(&installed.artifact_path).map_err(|error| RuntimeFailure {
                error: Box::new(
                    ExecutionError::new("artifact-read-failed", "failed to read artifact file")
                        .with_detail(error.to_string())
                        .with_phase(ExecutionPhase::RuntimeLoad),
                ),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            })?;

        let artifact: InProcessArtifact =
            serde_json::from_slice(&artifact_bytes).map_err(|error| RuntimeFailure {
                error: Box::new(
                    ExecutionError::new(
                        "artifact-parse-failed",
                        "failed to parse in-process artifact metadata",
                    )
                    .with_detail(error.to_string())
                    .with_phase(ExecutionPhase::RuntimeLoad),
                ),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            })?;

        if artifact.implementation != installed.manifest.runtime.entrypoint {
            return Err(RuntimeFailure {
                error: Box::new(
                    ExecutionError::new(
                        "artifact-entrypoint-mismatch",
                        "artifact implementation id does not match manifest entrypoint",
                    )
                    .with_detail(serde_json::json!({
                        "artifact_implementation": artifact.implementation,
                        "manifest_entrypoint": installed.manifest.runtime.entrypoint,
                    }))
                    .with_phase(ExecutionPhase::RuntimeLoad),
                ),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            });
        }

        let skill = self
            .implementations
            .get(&artifact.implementation)
            .ok_or_else(|| RuntimeFailure {
                error: Box::new(
                    ExecutionError::new(
                        "implementation-not-registered",
                        "no in-process skill implementation is registered for this artifact",
                    )
                    .with_detail(artifact.implementation.clone())
                    .with_phase(ExecutionPhase::RuntimeLoad),
                ),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            })?;

        skill
            .run(context.clone(), input.clone())
            .map(|output| RuntimeOutcome {
                output,
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            })
            .map_err(|error| RuntimeFailure {
                error: Box::new(ExecutionError::from(error)),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
                network_requests: 0,
            })
    }
}

#[derive(Clone)]
pub struct WasmtimeRuntimeAdapter {
    engine: Engine,
}

impl WasmtimeRuntimeAdapter {
    /// Construct the Wasmtime-backed Guild runtime adapter.
    ///
    /// # Errors
    ///
    /// Returns an error if the Wasmtime engine cannot be initialized.
    pub fn new() -> Result<Self, ExecutionError> {
        let mut config = Config::new();
        config.wasm_component_model(true);

        let engine = Engine::new(&config).map_err(|error| {
            ExecutionError::new(
                "wasmtime-engine-init-failed",
                "failed to initialize Wasmtime engine",
            )
            .with_detail(error.to_string())
        })?;

        Ok(Self { engine })
    }

    fn instantiate(
        &self,
        installed: &InstalledSkill,
        context: &ExecutionContext,
        host: Arc<dyn RuntimeHost>,
    ) -> Result<(Store<WasmStoreState>, bindings::GuildSkill), ExecutionError> {
        if installed.manifest.runtime.entrypoint != "guild-skill" {
            return Err(ExecutionError::new(
                "component-entrypoint-mismatch",
                "Wasm component runtime currently requires the `guild-skill` world entrypoint",
            )
            .with_detail(serde_json::json!({
                "manifest_entrypoint": installed.manifest.runtime.entrypoint,
            }))
            .with_phase(ExecutionPhase::RuntimeLoad));
        }

        let component =
            Component::from_file(&self.engine, &installed.artifact_path).map_err(|error| {
                ExecutionError::new(
                    "component-load-failed",
                    "failed to load Wasm component artifact",
                )
                .with_detail(error.to_string())
                .with_phase(ExecutionPhase::RuntimeLoad)
            })?;

        let mut linker = Linker::<WasmStoreState>::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker).map_err(|error| {
            ExecutionError::new("wasi-link-failed", "failed to attach WASI imports")
                .with_detail(error.to_string())
                .with_phase(ExecutionPhase::RuntimeLoad)
        })?;

        bindings::GuildSkill::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state).map_err(
            |error| {
                ExecutionError::new(
                    "host-link-failed",
                    "failed to attach Guild host imports to linker",
                )
                .with_detail(error.to_string())
                .with_phase(ExecutionPhase::RuntimeLoad)
            },
        )?;

        let mut store = Store::new(
            &self.engine,
            WasmStoreState::new(context.clone(), installed.clone(), host),
        );
        let instance = bindings::GuildSkill::instantiate(&mut store, &component, &linker).map_err(
            |error| {
                ExecutionError::new(
                    "component-instantiate-failed",
                    "failed to instantiate Wasm component",
                )
                .with_detail(error.to_string())
                .with_phase(ExecutionPhase::RuntimeLoad)
            },
        )?;

        Ok((store, instance))
    }
}

impl Default for WasmtimeRuntimeAdapter {
    fn default() -> Self {
        Self::new().expect("Wasmtime runtime initializes")
    }
}

impl RuntimeAdapter for WasmtimeRuntimeAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::WasmComponent
    }

    fn execute(
        &self,
        installed: &InstalledSkill,
        context: &ExecutionContext,
        input: &Value,
        host: Arc<dyn RuntimeHost>,
    ) -> Result<RuntimeOutcome, RuntimeFailure> {
        let wit_context = match to_wit_execution_context(context) {
            Ok(context) => context,
            Err(error) => {
                return Err(RuntimeFailure {
                    error: Box::new(error),
                    emitted_evidence: Vec::new(),
                    child_executions: Vec::new(),
                    network_requests: 0,
                });
            }
        };
        let (mut store, instance) =
            self.instantiate(installed, context, host)
                .map_err(|error| RuntimeFailure {
                    error: Box::new(error),
                    emitted_evidence: Vec::new(),
                    child_executions: Vec::new(),
                    network_requests: 0,
                })?;
        let wit_input = serde_json::to_string(input).expect("execution input serializes");

        let result = instance
            .guild_skill_skill()
            .call_run(&mut store, &wit_context, &wit_input)
            .map_err(|error: wasmtime::Error| RuntimeFailure {
                error: Box::new(parse_capability_denial_trap(&error).map_or_else(
                    || {
                        ExecutionError::new(
                            "component-call-failed",
                            "Wasm component execution trapped or failed",
                        )
                        .with_detail(error.to_string())
                        .with_phase(ExecutionPhase::RuntimeExec)
                    },
                    |denial| denial.into_execution_error(ExecutionPhase::Grant),
                )),
                emitted_evidence: store.data().emitted_evidence.clone(),
                child_executions: store.data().child_executions.clone(),
                network_requests: store.data().network_requests,
            })?;

        match result {
            Ok(output) => {
                let output = from_wit_skill_output(output).map_err(|error| RuntimeFailure {
                    error: Box::new(error),
                    emitted_evidence: store.data().emitted_evidence.clone(),
                    child_executions: store.data().child_executions.clone(),
                    network_requests: store.data().network_requests,
                })?;
                validate_emitted_evidence(&output, &store.data().emitted_evidence).map_err(
                    |error| RuntimeFailure {
                        error: Box::new(error),
                        emitted_evidence: store.data().emitted_evidence.clone(),
                        child_executions: store.data().child_executions.clone(),
                        network_requests: store.data().network_requests,
                    },
                )?;
                Ok(RuntimeOutcome {
                    output,
                    emitted_evidence: store.data().emitted_evidence.clone(),
                    child_executions: store.data().child_executions.clone(),
                    network_requests: store.data().network_requests,
                })
            }
            Err(error) => Err(RuntimeFailure {
                error: Box::new(from_wit_skill_error(error)),
                emitted_evidence: store.data().emitted_evidence.clone(),
                child_executions: store.data().child_executions.clone(),
                network_requests: store.data().network_requests,
            }),
        }
    }
}

struct WasmStoreState {
    execution: ExecutionContext,
    installed: InstalledSkill,
    host: Arc<dyn RuntimeHost>,
    child_executions: Vec<ChildExecutionRecord>,
    emitted_evidence: Vec<EvidenceRef>,
    network_requests: u32,
    wasi: WasiCtx,
    table: ResourceTable,
}

impl WasmStoreState {
    fn new(
        execution: ExecutionContext,
        installed: InstalledSkill,
        host: Arc<dyn RuntimeHost>,
    ) -> Self {
        Self {
            execution,
            installed,
            host,
            child_executions: Vec::new(),
            emitted_evidence: Vec::new(),
            network_requests: 0,
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
        }
    }

    fn grants(&self) -> &CapabilityGrantSet {
        &self.execution.granted_capabilities
    }
}

impl WasiView for WasmStoreState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl bindings::guild::skill::types::Host for WasmStoreState {}

impl bindings::guild::skill::host::Host for WasmStoreState {
    fn emit_evidence(
        &mut self,
        request: bindings::guild::skill::types::EvidenceEmissionRequest,
    ) -> wasmtime::Result<Result<bindings::guild::skill::types::EvidenceRef, String>> {
        let request = EvidenceEmissionRequest {
            payload: request.payload,
            mime_type: request.mime_type,
            title: request.title,
            audience: match request.audience {
                bindings::guild::skill::types::EvidenceAudience::User => EvidenceAudience::User,
                bindings::guild::skill::types::EvidenceAudience::Assistant => {
                    EvidenceAudience::Assistant
                }
                bindings::guild::skill::types::EvidenceAudience::Internal => {
                    EvidenceAudience::Internal
                }
            },
            redaction: match request.redaction {
                bindings::guild::skill::types::RedactionClass::None => RedactionClass::None,
                bindings::guild::skill::types::RedactionClass::SecretsRemoved => {
                    RedactionClass::SecretsRemoved
                }
                bindings::guild::skill::types::RedactionClass::PiiRemoved => {
                    RedactionClass::PiiRemoved
                }
                bindings::guild::skill::types::RedactionClass::TenantSensitive => {
                    RedactionClass::TenantSensitive
                }
            },
            freshness: request.freshness,
        };

        if let Err(denial) = CapabilityEvaluator::authorize(
            self.grants(),
            &CapabilityOperation::EmitEvidence { request: &request },
        ) {
            return Err(capability_denial_trap(&denial));
        }

        match self
            .host
            .emit_evidence(&self.execution.execution_id, &request)
        {
            Ok(evidence) => {
                self.emitted_evidence.push(evidence.clone());
                Ok(Ok(to_wit_evidence(&evidence)))
            }
            Err(error) => Ok(Err(format!("{}: {}", error.code, error.message))),
        }
    }

    fn log(
        &mut self,
        level: bindings::guild::skill::types::Severity,
        message: String,
    ) -> wasmtime::Result<()> {
        let level = from_wit_severity(level);
        if let Err(denial) =
            CapabilityEvaluator::authorize(self.grants(), &CapabilityOperation::Log { level })
        {
            return Err(capability_denial_trap(&denial));
        }

        let _ = message;
        Ok(())
    }

    fn read_resource(
        &mut self,
        uri: String,
    ) -> wasmtime::Result<Result<bindings::guild::skill::types::ResourceReadResult, String>> {
        let parsed_uri = match GuildResourceUri::parse(&uri) {
            Ok(parsed_uri) => parsed_uri,
            Err(error) => {
                let denial = CapabilityDenial {
                    code: "resource-uri-invalid".into(),
                    message: error.to_string(),
                    detail: serde_json::json!({
                        "uri": uri,
                    }),
                };
                return Err(capability_denial_trap(&denial));
            }
        };

        if let Err(denial) = CapabilityEvaluator::authorize(
            self.grants(),
            &CapabilityOperation::ReadResource {
                uri: &uri,
                parsed_uri: &parsed_uri,
            },
        ) {
            return Err(capability_denial_trap(&denial));
        }

        match self.host.read_resource(&uri) {
            Ok(result) => Ok(Ok(to_wit_resource_read_result(&result))),
            Err(error) => Ok(Err(format!("{}: {}", error.code, error.message))),
        }
    }

    fn cache_get(&mut self, _key: String) -> wasmtime::Result<Option<String>> {
        Err(wasmtime::Error::msg(
            "cache-get is not implemented in the Wasm inspect slice",
        ))
    }

    fn cache_put(
        &mut self,
        _key: String,
        _value: String,
        _ttl_seconds: u32,
    ) -> wasmtime::Result<()> {
        Err(wasmtime::Error::msg(
            "cache-put is not implemented in the Wasm inspect slice",
        ))
    }

    fn invoke_dependency(
        &mut self,
        request: bindings::guild::skill::types::DependencyInvocationRequest,
    ) -> wasmtime::Result<
        Result<
            bindings::guild::skill::types::SkillOutput,
            bindings::guild::skill::types::SkillError,
        >,
    > {
        let input = match serde_json::from_str::<Value>(&request.input) {
            Ok(input) => input,
            Err(error) => {
                return Ok(Err(bindings::guild::skill::types::SkillError {
                    code: "invalid-dependency-input".into(),
                    message: "dependency invocation input was not valid JSON".into(),
                    retryable: false,
                    detail: Some(serde_json::json!({ "error": error.to_string() }).to_string()),
                }));
            }
        };

        let sequence = next_child_sequence(self.child_executions.len())?;
        match self.host.invoke_dependency(
            &self.installed,
            &self.execution,
            sequence,
            &request.alias,
            &input,
        ) {
            Ok(outcome) => {
                self.child_executions.push(outcome.record);
                Ok(Ok(to_wit_skill_output(&outcome.output)))
            }
            Err(error) => {
                let error = *error;
                if let Some(denial) = error.denial {
                    return Err(capability_denial_trap(&denial));
                }
                if let Some(record) = error.record {
                    self.child_executions.push(*record);
                }

                Ok(Err(to_wit_skill_error(&error.skill_error)))
            }
        }
    }

    fn http_request(
        &mut self,
        request: bindings::guild::skill::types::HttpRequestMessage,
    ) -> wasmtime::Result<Result<bindings::guild::skill::types::HttpResponseMessage, String>> {
        let mut request = from_wit_http_request(request);
        let mut redirects_followed = 0_u8;
        let mut redirect_context: Option<PendingRedirect> = None;

        loop {
            let pending_redirect = redirect_context.take();
            let parsed_request = parse_http_request(&request).map_err(|denial| {
                let denial = pending_redirect.as_ref().map_or(denial.clone(), |pending| {
                    redirect_location_invalid_denial(
                        &pending.from_url,
                        pending.status,
                        &pending.location,
                        &denial,
                    )
                });
                capability_denial_trap(&denial)
            })?;
            let policy = CapabilityEvaluator::authorize_http_request(
                self.grants(),
                &self.execution.budget,
                self.network_requests,
                &request,
                &parsed_request,
            )
            .map_err(|denial| {
                let denial = pending_redirect.as_ref().map_or(denial.clone(), |pending| {
                    redirect_target_not_granted_denial(
                        &pending.from_url,
                        pending.status,
                        &pending.location,
                        &request.url,
                        &denial,
                    )
                });
                capability_denial_trap(&denial)
            })?;

            self.network_requests = self.network_requests.saturating_add(1);
            match self
                .host
                .http_request(&request, policy.timeout, policy.max_response_bytes)
            {
                Ok(response) => {
                    if !is_redirect_status(response.response.status) {
                        return Ok(Ok(to_wit_http_response(&response.response)));
                    }

                    let Some(location) = response.redirect_location else {
                        let denial = redirect_location_missing_denial(
                            &request.url,
                            response.response.status,
                        );
                        return Err(capability_denial_trap(&denial));
                    };

                    if !policy.follow_redirects {
                        let denial = redirect_not_allowed_denial(
                            &request.url,
                            response.response.status,
                            &location,
                        );
                        return Err(capability_denial_trap(&denial));
                    }

                    if redirects_followed >= policy.max_redirects {
                        let denial = redirect_hop_limit_denial(
                            &request.url,
                            response.response.status,
                            &location,
                            policy.max_redirects,
                        );
                        return Err(capability_denial_trap(&denial));
                    }

                    request = build_redirect_request(&request, &location)
                        .map_err(|denial| capability_denial_trap(&denial))?;
                    redirects_followed = redirects_followed.saturating_add(1);
                    redirect_context = Some(PendingRedirect {
                        from_url: response.response.url,
                        status: response.response.status,
                        location,
                    });
                }
                Err(error) => return Ok(Err(format!("{}: {}", error.code, error.message))),
            }
        }
    }

    fn get_secret(&mut self, _handle: String) -> wasmtime::Result<Result<Vec<u8>, String>> {
        Err(wasmtime::Error::msg(
            "get-secret is not implemented in the Wasm inspect slice",
        ))
    }

    fn monotonic_now(&mut self) -> wasmtime::Result<Result<u64, String>> {
        Err(wasmtime::Error::msg(
            "monotonic-now is not implemented in the Wasm inspect slice",
        ))
    }

    fn wall_clock_now(&mut self) -> wasmtime::Result<Result<String, String>> {
        Err(wasmtime::Error::msg(
            "wall-clock-now is not implemented in the Wasm inspect slice",
        ))
    }
}

#[derive(Clone)]
pub struct Runner<A> {
    runtime: A,
}

struct UnsuccessfulAttemptContext<'a> {
    installed: &'a InstalledSkill,
    envelope: &'a ResolvedExecutionEnvelope,
    execution_id: &'a str,
    started_at_utc: &'a str,
    duration_ms: u64,
}

impl<A> Runner<A>
where
    A: RuntimeAdapter,
{
    #[must_use]
    pub fn new(runtime: A) -> Self {
        Self { runtime }
    }

    #[must_use]
    pub fn runtime(&self) -> &A {
        &self.runtime
    }

    /// Resolve caller intent into a host-owned execution envelope using local policy.
    ///
    /// # Errors
    ///
    /// Returns an error if local policy cannot be loaded or requested capability
    /// input cannot be evaluated safely.
    pub fn authorize_execution<R>(
        &self,
        registry: &R,
        installed: &InstalledSkill,
        request: CallerRequest,
        parent_execution_id: Option<String>,
    ) -> Result<ResolvedExecutionEnvelope, ExecutionError>
    where
        R: SkillRegistry + ?Sized,
    {
        let policy = registry.load_policy_config().map_err(|error| {
            ExecutionError::from(error)
                .with_phase(ExecutionPhase::Grant)
                .with_detail(serde_json::json!({
                    "resolved_skill": installed.resolved_ref,
                }))
        })?;
        let declared_surface = declared_capability_surface(registry, installed)?;
        let evaluation = Self::evaluate_policy(installed, &request, &declared_surface, &policy);

        Ok(ResolvedExecutionEnvelope {
            request,
            resolved_skill: installed.resolved_ref.clone(),
            granted_capabilities: evaluation.granted_capabilities,
            policy_decision: evaluation.decision,
            parent_execution_id,
        })
    }

    /// Execute a resolved installed skill through the configured runtime adapter.
    ///
    /// # Errors
    ///
    /// Returns an execution error if validation, runtime execution, or durable
    /// persistence fails. Unsuccessful attempts that reached the host execution
    /// path still persist durable execution records before the error is returned.
    pub fn execute<R>(
        &self,
        registry: &R,
        installed: &InstalledSkill,
        envelope: &ResolvedExecutionEnvelope,
    ) -> Result<ExecutionRecord, ExecutionError>
    where
        A: Clone + 'static,
        R: SkillRegistry + Clone + Send + Sync + 'static,
    {
        let started = Instant::now();
        let execution_id = mint_host_execution_id();
        let started_at_utc = host_now_utc();
        let unsuccessful = |duration_ms| UnsuccessfulAttemptContext {
            installed,
            envelope,
            execution_id: &execution_id,
            started_at_utc: &started_at_utc,
            duration_ms,
        };

        if let Err(error) = self.validate_execution(installed, envelope) {
            return Err(Self::persist_unsuccessful_attempt(
                registry,
                &unsuccessful(duration_ms(started.elapsed())),
                error,
                Vec::new(),
                &[],
                0,
            ));
        }

        let context = Self::build_context(envelope, &execution_id, &started_at_utc);
        let runtime_host: Arc<dyn RuntimeHost> = Arc::new(RunnerRuntimeHost {
            runner: self.clone(),
            registry: registry.clone(),
        });
        let outcome =
            match self
                .runtime
                .execute(installed, &context, &envelope.request.input, runtime_host)
            {
                Ok(outcome) => outcome,
                Err(failure) => {
                    return Err(Self::persist_unsuccessful_attempt(
                        registry,
                        &unsuccessful(duration_ms(started.elapsed())),
                        *failure.error,
                        failure.child_executions,
                        &failure.emitted_evidence,
                        failure.network_requests,
                    ));
                }
            };
        let duration_ms = duration_ms(started.elapsed());
        let finished_at_utc = host_now_utc();
        let receipt = Self::build_receipt(
            &execution_id,
            &envelope.request.trace_id,
            ExecutionStatus::Succeeded,
        );

        let record = ExecutionRecord {
            receipt,
            request: envelope.request.clone(),
            policy_decision: envelope.policy_decision.clone(),
            resolved_skill: envelope.resolved_skill.clone(),
            parent_execution_id: envelope.parent_execution_id.clone(),
            status: ExecutionStatus::Succeeded,
            output: Some(outcome.output),
            termination: None,
            granted_capabilities: envelope.granted_capabilities.clone(),
            emitted_evidence: Self::load_evidence_records(registry, &outcome.emitted_evidence)?,
            metrics: ExecutionMetrics {
                duration_ms,
                network_requests: outcome.network_requests,
                child_executions: saturating_u16_len(outcome.child_executions.len()),
                ..ExecutionMetrics::default()
            },
            provenance: Self::build_provenance(
                installed,
                envelope,
                &started_at_utc,
                &finished_at_utc,
            ),
            child_executions: outcome.child_executions,
        };

        Self::persist_record(registry, &record)?;

        Ok(record)
    }

    fn validate_execution(
        &self,
        installed: &InstalledSkill,
        envelope: &ResolvedExecutionEnvelope,
    ) -> Result<(), ExecutionError> {
        Self::validate_manifest(&installed.manifest)?;
        Self::validate_request(&installed.manifest, envelope)?;
        self.validate_runtime(installed)?;
        self.validate_runtime_surface(installed, &envelope.granted_capabilities)?;
        Self::validate_resolved_ref(installed, envelope)?;
        Self::validate_policy_decision(envelope)?;
        Self::validate_grants(installed, &envelope.granted_capabilities)
    }

    #[must_use]
    pub fn build_context(
        envelope: &ResolvedExecutionEnvelope,
        execution_id: &str,
        now_utc: &str,
    ) -> ExecutionContext {
        ExecutionContext {
            execution_id: execution_id.to_owned(),
            trace_id: envelope.request.trace_id.clone(),
            tenant_id: envelope.request.tenant_id.clone(),
            skill: envelope.resolved_skill.clone(),
            mode: envelope.request.mode.clone(),
            input_sha256: hash_json(&envelope.request.input),
            now_utc: Some(now_utc.to_owned()),
            budget: envelope.request.budget.clone(),
            granted_capabilities: envelope.granted_capabilities.clone(),
        }
    }

    fn validate_policy_decision(
        envelope: &ResolvedExecutionEnvelope,
    ) -> Result<(), ExecutionError> {
        if envelope.policy_decision.outcome == PolicyDecisionOutcome::Rejected {
            return Err(ExecutionError::new(
                "policy-denied",
                envelope.policy_decision.summary.clone(),
            )
            .with_detail(serde_json::json!({
                "profile_name": envelope.policy_decision.profile_name,
                "trust_tier": envelope.policy_decision.trust_tier,
                "verification_state": envelope.policy_decision.verification_state,
                "reasons": envelope.policy_decision.reasons,
                "detail": envelope.policy_decision.detail,
            }))
            .with_phase(ExecutionPhase::Grant));
        }

        Ok(())
    }

    fn evaluate_policy(
        installed: &InstalledSkill,
        request: &CallerRequest,
        declared_surface: &[CapabilityRequirement],
        policy: &LocalPolicyConfig,
    ) -> PolicyEvaluationResult {
        let selection = match select_policy_profile(policy, request) {
            Ok(selection) => selection,
            Err(reason) => {
                let reasons = vec![reason.clone()];
                return rejected_policy_evaluation(
                    installed,
                    "ambiguous".into(),
                    "local policy profile selection was ambiguous",
                    reasons,
                    Some(serde_json::json!({
                        "actor_id": request.actor_id,
                        "tenant_id": request.tenant_id,
                    })),
                    CapabilityGrantSet::default(),
                );
            }
        };

        let invalid_requested = invalid_capability_grants(&request.requested_capabilities);
        if !invalid_requested.is_empty() {
            let reasons = vec![PolicyReason {
                code: "policy-requested-capability-invalid".into(),
                message: "caller requested invalid capability constraints".into(),
                detail: Some(serde_json::json!({ "invalid": invalid_requested })),
            }];
            return rejected_policy_evaluation(
                installed,
                selection.name.clone(),
                "local policy rejected invalid requested capabilities",
                reasons,
                Some(serde_json::json!({ "invalid": invalid_requested })),
                CapabilityGrantSet::default(),
            );
        }

        let mut reasons = Vec::new();
        let granted = match selection.profile.default_action {
            LocalPolicyDefaultAction::AllowRequestedDeclared => {
                clamp_requested_capabilities_to_manifest(
                    &request.requested_capabilities,
                    declared_surface,
                    &mut reasons,
                )
            }
        };

        let mut granted = mark_required_grants(installed, &granted);

        for rule in &selection.profile.rules {
            if policy_rule_matches(rule, installed) {
                granted =
                    apply_policy_rule(&selection.name, rule, &granted, installed, &mut reasons);
            }
        }

        let granted_set = candidate_grants_to_set(&granted);
        let missing = missing_required_capabilities(installed, &granted_set);
        if !missing.is_empty() {
            reasons.push(PolicyReason {
                code: "policy-required-capability-missing".into(),
                message: "local policy did not grant all required capabilities".into(),
                detail: Some(serde_json::json!({
                    "missing": missing,
                    "profile_name": selection.name,
                    "trust_tier": installed.trust.trust_tier,
                    "verification_state": installed.trust.verification_state,
                })),
            });
            return rejected_policy_evaluation(
                installed,
                selection.name,
                "local policy denied one or more required capabilities",
                reasons,
                Some(serde_json::json!({ "missing": missing })),
                granted_set,
            );
        }

        completed_policy_evaluation(installed, selection.name, granted_set, reasons)
    }

    /// Validate a resolved execution request against the installed manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest, requested mode, or global mode policy
    /// rejects the execution request.
    pub fn validate_request(
        manifest: &SkillManifest,
        envelope: &ResolvedExecutionEnvelope,
    ) -> Result<(), ExecutionError> {
        Self::validate_manifest(manifest)?;

        if !manifest.supports_mode(&envelope.request.mode) {
            return Err(ExecutionError::new(
                "unsupported-mode",
                format!(
                    "skill {}.{} does not support {:?} mode",
                    manifest.key.namespace, manifest.key.name, envelope.request.mode
                ),
            )
            .with_phase(ExecutionPhase::Mode));
        }

        if envelope.request.mode == ExecutionMode::Apply {
            return Err(ExecutionError::new(
                "apply-disabled",
                "apply mode remains globally gated until audit and approval paths exist",
            )
            .with_phase(ExecutionPhase::Mode));
        }

        Ok(())
    }

    fn validate_manifest(manifest: &SkillManifest) -> Result<(), ExecutionError> {
        manifest.validate().map_err(|errors| {
            ExecutionError::new("invalid-manifest", "manifest validation failed")
                .with_detail(serde_json::to_value(errors).expect("validation errors serialize"))
                .with_phase(ExecutionPhase::Validation)
        })
    }

    fn validate_runtime(&self, installed: &InstalledSkill) -> Result<(), ExecutionError> {
        if installed.manifest.runtime.kind != self.runtime.kind() {
            return Err(ExecutionError::new(
                "unsupported-runtime",
                "runner runtime adapter does not support the skill runtime kind",
            )
            .with_detail(serde_json::json!({
                "expected": format!("{:?}", self.runtime.kind()),
                "actual": format!("{:?}", installed.manifest.runtime.kind),
            }))
            .with_phase(ExecutionPhase::Validation));
        }

        Ok(())
    }

    fn validate_runtime_surface(
        &self,
        installed: &InstalledSkill,
        grants: &CapabilityGrantSet,
    ) -> Result<(), ExecutionError> {
        if self.runtime.kind() != RuntimeKind::WasmComponent {
            return Ok(());
        }

        let unsupported_manifest_capabilities: Vec<_> = installed
            .manifest
            .capabilities
            .iter()
            .filter(|requirement| {
                !is_supported_wasm_inspect_capability(&requirement.id, &requirement.access)
            })
            .map(|requirement| {
                serde_json::json!({
                    "id": requirement.id,
                    "access": requirement.access,
                    "constraints": requirement.constraints,
                    "required": requirement.required,
                })
            })
            .collect();

        let unsupported_grants: Vec<_> = grants
            .grants
            .iter()
            .filter(|grant| !is_supported_wasm_inspect_capability(&grant.id, &grant.access))
            .map(|grant| {
                serde_json::json!({
                    "id": grant.id,
                    "access": grant.access,
                    "constraints": grant.constraints,
                })
            })
            .collect();

        if unsupported_manifest_capabilities.is_empty() && unsupported_grants.is_empty() {
            return Ok(());
        }

        let includes_filesystem = unsupported_surface_includes_filesystem(
            &unsupported_manifest_capabilities,
            &unsupported_grants,
        );
        let (code, message) = if includes_filesystem {
            (
                "filesystem-runtime-not-supported",
                "filesystem capability contracts are not implemented in the active Wasm inspect slice",
            )
        } else {
            (
                "unsupported-runtime-surface",
                "Wasm inspect execution only supports the active capability allowlist",
            )
        };

        Err(ExecutionError::new(code, message)
            .with_detail(serde_json::json!({
                "runtime_kind": self.runtime.kind(),
                "supported_capabilities": supported_wasm_inspect_capabilities(),
                "unsupported_manifest_capabilities": unsupported_manifest_capabilities,
                "unsupported_grants": unsupported_grants,
                "deferred_filesystem_contract": includes_filesystem,
            }))
            .with_phase(ExecutionPhase::Validation))
    }

    fn validate_resolved_ref(
        installed: &InstalledSkill,
        envelope: &ResolvedExecutionEnvelope,
    ) -> Result<(), ExecutionError> {
        if installed.resolved_ref != envelope.resolved_skill {
            return Err(ExecutionError::new(
                "resolved-skill-mismatch",
                "execution request did not match the resolved installed skill",
            )
            .with_detail(serde_json::json!({
                "request": envelope.resolved_skill,
                "installed": installed.resolved_ref,
            }))
            .with_phase(ExecutionPhase::Validation));
        }

        Ok(())
    }

    fn validate_grants(
        installed: &InstalledSkill,
        grants: &CapabilityGrantSet,
    ) -> Result<(), ExecutionError> {
        let invalid_grants: Vec<_> = grants
            .grants
            .iter()
            .enumerate()
            .flat_map(|(index, grant)| {
                grant.validate().into_iter().map(move |message| {
                    serde_json::json!({
                        "index": index,
                        "id": grant.id,
                        "access": grant.access,
                        "constraints": grant.constraints,
                        "message": message,
                    })
                })
            })
            .collect();

        if !invalid_grants.is_empty() {
            return Err(ExecutionError::new(
                "capability-grant-invalid",
                "execution grants contained invalid capability constraints",
            )
            .with_detail(serde_json::json!({ "invalid": invalid_grants }))
            .with_phase(ExecutionPhase::Grant));
        }

        let missing: Vec<_> = installed
            .manifest
            .capabilities
            .iter()
            .filter(|requirement| requirement.required)
            .filter(|requirement| {
                !CapabilityEvaluator::grants_cover_requirement(grants, requirement)
            })
            .map(|requirement| {
                serde_json::json!({
                    "id": requirement.id,
                    "access": requirement.access,
                    "constraints": requirement.constraints,
                })
            })
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(ExecutionError::new(
                "capability-mismatch",
                "execution grants did not satisfy the skill's required capabilities",
            )
            .with_detail(serde_json::json!({ "missing": missing }))
            .with_phase(ExecutionPhase::Grant))
        }
    }

    fn build_provenance(
        installed: &InstalledSkill,
        envelope: &ResolvedExecutionEnvelope,
        started_at_utc: &str,
        finished_at_utc: &str,
    ) -> Provenance {
        Provenance {
            resolved_skill: envelope.resolved_skill.clone(),
            abi: installed.manifest.runtime.guest_abi_version.clone(),
            dependency_digests: installed
                .manifest
                .dependencies
                .iter()
                .map(|dependency| dependency.skill.digest.clone())
                .collect(),
            started_at_utc: Some(started_at_utc.to_owned()),
            finished_at_utc: Some(finished_at_utc.to_owned()),
        }
    }

    fn persist_record<R>(registry: &R, record: &ExecutionRecord) -> Result<(), ExecutionError>
    where
        R: SkillRegistry + ?Sized,
    {
        registry.persist_execution_record(record).map_err(|error| {
            ExecutionError::from(error)
                .with_phase(ExecutionPhase::Persistence)
                .with_detail(serde_json::json!({
                    "execution_id": record.receipt.execution_id,
                    "uri": record.receipt.uri,
                }))
        })
    }

    fn build_receipt(
        execution_id: &str,
        trace_id: &str,
        status: ExecutionStatus,
    ) -> ExecutionReceipt {
        ExecutionReceipt {
            execution_id: execution_id.to_owned(),
            uri: execution_resource_uri(execution_id),
            trace_id: trace_id.to_owned(),
            status,
        }
    }

    fn policy_decision_for_unsuccessful_attempt(
        envelope: &ResolvedExecutionEnvelope,
        error: &ExecutionError,
    ) -> PolicyDecision {
        let _ = error;
        envelope.policy_decision.clone()
    }

    fn load_evidence_records<R>(
        registry: &R,
        refs: &[EvidenceRef],
    ) -> Result<Vec<EvidenceRecord>, ExecutionError>
    where
        R: SkillRegistry + ?Sized,
    {
        refs.iter()
            .map(|evidence| {
                registry
                    .load_evidence_record(&evidence.uri)
                    .map_err(|error| {
                        ExecutionError::from(error)
                            .with_phase(ExecutionPhase::Persistence)
                            .with_detail(serde_json::json!({
                                "uri": evidence.uri,
                            }))
                    })
            })
            .collect()
    }

    fn persist_unsuccessful_attempt<R>(
        registry: &R,
        context: &UnsuccessfulAttemptContext<'_>,
        error: ExecutionError,
        child_executions: Vec<ChildExecutionRecord>,
        emitted_evidence: &[EvidenceRef],
        network_requests: u32,
    ) -> ExecutionError
    where
        R: SkillRegistry + ?Sized,
    {
        let status = status_from_error(&error);
        let finished_at_utc = host_now_utc();
        let receipt = Self::build_receipt(
            context.execution_id,
            &context.envelope.request.trace_id,
            status.clone(),
        );
        let record = ExecutionRecord {
            receipt,
            request: context.envelope.request.clone(),
            policy_decision: Self::policy_decision_for_unsuccessful_attempt(
                context.envelope,
                &error,
            ),
            resolved_skill: context.envelope.resolved_skill.clone(),
            parent_execution_id: context.envelope.parent_execution_id.clone(),
            status,
            output: None,
            termination: Some(termination_from_error(&error)),
            granted_capabilities: context.envelope.granted_capabilities.clone(),
            emitted_evidence: Self::load_evidence_records(registry, emitted_evidence)
                .unwrap_or_default(),
            metrics: ExecutionMetrics {
                duration_ms: context.duration_ms,
                network_requests,
                child_executions: saturating_u16_len(child_executions.len()),
                ..ExecutionMetrics::default()
            },
            provenance: Self::build_provenance(
                context.installed,
                context.envelope,
                context.started_at_utc,
                &finished_at_utc,
            ),
            child_executions,
        };

        match Self::persist_record(registry, &record) {
            Ok(()) => error.with_receipt(record.receipt.clone()),
            Err(persist_error) => persist_error.with_detail(serde_json::json!({
                "execution_id": record.receipt.execution_id,
                "uri": record.receipt.uri,
                "original_error": {
                    "code": error.code,
                    "message": error.message,
                    "retryable": error.retryable,
                    "phase": error.phase,
                    "detail": error.detail,
                }
            })),
        }
    }
}

struct RunnerRuntimeHost<A, R> {
    runner: Runner<A>,
    registry: R,
}

impl<A, R> RuntimeHost for RunnerRuntimeHost<A, R>
where
    A: RuntimeAdapter + Clone + 'static,
    R: SkillRegistry + Clone + Send + Sync + 'static,
{
    fn invoke_dependency(
        &self,
        parent: &InstalledSkill,
        context: &ExecutionContext,
        sequence: u16,
        alias: &str,
        input: &Value,
    ) -> Result<ChildInvocationOutcome, Box<ChildInvocationError>> {
        let dependency = find_declared_dependency(parent, alias)?;

        if let Err(denial) = CapabilityEvaluator::authorize(
            &context.granted_capabilities,
            &CapabilityOperation::InvokeDependency { alias },
        ) {
            return Err(Box::new(ChildInvocationError::denied(denial)));
        }

        if context.budget.max_child_executions == 0 {
            return Err(Box::new(ChildInvocationError::without_record(SkillError {
                code: "child-budget-exhausted".into(),
                message: "execution budget does not allow additional child invocations".into(),
                retryable: false,
                detail: Some(serde_json::json!({ "alias": alias })),
            })));
        }

        let child_installed = load_child_installed(&self.registry, dependency, alias)?;

        let child_grants = CapabilityEvaluator::derive_child_grants(
            &child_installed.manifest.capabilities,
            &context.granted_capabilities,
        )
        .map_err(|denial| Box::new(ChildInvocationError::denied(denial)))?;
        let child_request =
            build_child_caller_request(context, sequence, input, &child_installed, child_grants);
        let child_request = self
            .runner
            .authorize_execution(
                &self.registry,
                &child_installed,
                child_request,
                Some(context.execution_id.clone()),
            )
            .map_err(|error| {
                Box::new(ChildInvocationError::without_record(SkillError {
                    code: error.code,
                    message: error.message,
                    retryable: error.retryable,
                    detail: error.detail.map(|detail| *detail),
                }))
            })?;

        let record = match self
            .runner
            .execute(&self.registry, &child_installed, &child_request)
        {
            Ok(record) => record,
            Err(error) => {
                let child_record =
                    load_child_failure_record(&self.registry, alias, &context.execution_id, &error);

                return Err(Box::new(ChildInvocationError {
                    skill_error: child_execution_error_to_skill_error(alias, &error),
                    record: child_record,
                    denial: None,
                }));
            }
        };

        Ok(ChildInvocationOutcome {
            output: record
                .output
                .clone()
                .expect("successful child execution returns skill output"),
            record: child_execution_record_from_execution_record(
                alias,
                &context.execution_id,
                &record,
            ),
        })
    }

    fn emit_evidence(
        &self,
        execution_id: &str,
        request: &EvidenceEmissionRequest,
    ) -> Result<EvidenceRef, SkillError> {
        self.registry
            .store_evidence(execution_id, request)
            .map_err(|error| SkillError {
                code: "evidence-store-failed".into(),
                message: "failed to persist evidence in the local object store".into(),
                retryable: false,
                detail: Some(serde_json::json!({
                    "cause": {
                        "code": error.code,
                        "message": error.message,
                        "detail": error.detail,
                    }
                })),
            })
    }

    fn read_resource(&self, uri: &str) -> Result<ResourceReadResult, SkillError> {
        self.registry
            .read_resource(uri)
            .map_err(|error| SkillError {
                code: error.code,
                message: error.message,
                retryable: false,
                detail: Some(serde_json::json!({
                    "uri": uri,
                    "detail": error.detail,
                })),
            })
    }

    fn http_request(
        &self,
        request: &HttpRequest,
        timeout: Duration,
        max_response_bytes: u64,
    ) -> Result<HostHttpResponse, SkillError> {
        execute_http_request(request, timeout, max_response_bytes)
    }
}

fn find_declared_dependency<'a>(
    parent: &'a InstalledSkill,
    alias: &str,
) -> Result<&'a guild_manifest::InstalledDependencySpec, Box<ChildInvocationError>> {
    parent
        .manifest
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == alias)
        .ok_or_else(|| SkillError {
            code: "dependency-not-declared".into(),
            message: format!("dependency alias `{alias}` is not declared by the skill"),
            retryable: false,
            detail: Some(serde_json::json!({ "alias": alias })),
        })
        .map_err(|error| Box::new(ChildInvocationError::without_record(error)))
}

fn declared_capability_surface<R>(
    registry: &R,
    installed: &InstalledSkill,
) -> Result<Vec<CapabilityRequirement>, ExecutionError>
where
    R: SkillRegistry + ?Sized,
{
    let mut surface = installed.manifest.capabilities.clone();
    let mut visited = vec![installed.resolved_ref.digest.clone()];
    collect_declared_capability_surface(registry, installed, &mut visited, &mut surface)?;
    Ok(surface)
}

fn collect_declared_capability_surface<R>(
    registry: &R,
    installed: &InstalledSkill,
    visited: &mut Vec<String>,
    surface: &mut Vec<CapabilityRequirement>,
) -> Result<(), ExecutionError>
where
    R: SkillRegistry + ?Sized,
{
    for dependency in &installed.manifest.dependencies {
        if visited
            .iter()
            .any(|digest| digest == &dependency.skill.digest)
        {
            continue;
        }

        let child = registry.resolve_exact(&dependency.skill).map_err(|error| {
            ExecutionError::from(error)
                .with_phase(ExecutionPhase::Validation)
                .with_detail(serde_json::json!({
                    "alias": dependency.alias,
                    "dependency": dependency.skill,
                }))
        })?;

        visited.push(child.resolved_ref.digest.clone());
        surface.extend(child.manifest.capabilities.clone());
        collect_declared_capability_surface(registry, &child, visited, surface)?;
    }

    Ok(())
}

fn invalid_capability_grants(grants: &CapabilityGrantSet) -> Vec<Value> {
    grants
        .grants
        .iter()
        .enumerate()
        .flat_map(|(index, grant)| {
            grant.validate().into_iter().map(move |message| {
                serde_json::json!({
                    "index": index,
                    "id": grant.id,
                    "access": grant.access,
                    "constraints": grant.constraints,
                    "message": message,
                })
            })
        })
        .collect()
}

fn clamp_requested_capabilities_to_manifest(
    requested: &CapabilityGrantSet,
    declared: &[CapabilityRequirement],
    reasons: &mut Vec<PolicyReason>,
) -> CapabilityGrantSet {
    let mut grants = Vec::new();

    for grant in &requested.grants {
        let mut reduced = Vec::new();
        let mut matched = false;

        for requirement in declared
            .iter()
            .filter(|requirement| requirement.id == grant.id && requirement.access == grant.access)
        {
            matched = true;
            if let Some(reduced_grant) = reduce_grant_to_requirement(grant, requirement) {
                push_unique_grant(&mut reduced, reduced_grant);
            }
        }

        if reduced.is_empty() {
            let reason = if matched {
                PolicyReason {
                    code: "policy-requested-capability-outside-manifest".into(),
                    message: "caller requested capability constraints outside the skill manifest declaration".into(),
                    detail: Some(serde_json::json!({
                        "requested": grant,
                    })),
                }
            } else {
                PolicyReason {
                    code: "policy-requested-capability-undeclared".into(),
                    message: "caller requested a capability family the skill did not declare"
                        .into(),
                    detail: Some(serde_json::json!({
                        "requested": grant,
                    })),
                }
            };
            reasons.push(reason);
            continue;
        }

        if reduced.len() != 1 || reduced[0] != *grant {
            reasons.push(PolicyReason {
                code: "policy-requested-capability-reduced".into(),
                message:
                    "caller requested capability was reduced to the skill-declared manifest surface"
                        .into(),
                detail: Some(serde_json::json!({
                    "requested": grant,
                    "granted": reduced,
                })),
            });
        }

        for reduced_grant in reduced {
            push_unique_grant(&mut grants, reduced_grant);
        }
    }

    CapabilityGrantSet { grants }
}

#[derive(Debug, Clone)]
struct SelectedPolicyProfile<'a> {
    name: String,
    profile: &'a PolicyProfile,
}

fn select_policy_profile<'a>(
    policy: &'a LocalPolicyConfig,
    request: &CallerRequest,
) -> Result<SelectedPolicyProfile<'a>, PolicyReason> {
    let matching: Vec<&PolicyProfileBinding> = policy
        .bindings
        .iter()
        .filter(|binding| policy_profile_binding_matches(binding, request))
        .collect();

    if matching.len() > 1 {
        return Err(PolicyReason {
            code: "policy-profile-ambiguous".into(),
            message: "local policy matched multiple profile bindings for the same execution".into(),
            detail: Some(serde_json::json!({
                "actor_id": request.actor_id,
                "tenant_id": request.tenant_id,
                "matches": matching.iter().map(|binding| {
                    serde_json::json!({
                        "binding": binding.name,
                        "profile": binding.profile,
                    })
                }).collect::<Vec<_>>(),
            })),
        });
    }

    let profile_name = matching.first().map_or_else(
        || policy.default_profile.clone(),
        |binding| binding.profile.clone(),
    );
    let profile = policy
        .profiles
        .iter()
        .find(|profile| profile.name == profile_name)
        .ok_or_else(|| PolicyReason {
            code: "policy-profile-missing".into(),
            message: "local policy referenced a profile that was not declared".into(),
            detail: Some(serde_json::json!({ "profile": profile_name })),
        })?;

    Ok(SelectedPolicyProfile {
        name: profile.name.clone(),
        profile,
    })
}

fn rejected_policy_evaluation(
    installed: &InstalledSkill,
    profile_name: String,
    summary: &str,
    reasons: Vec<PolicyReason>,
    detail: Option<serde_json::Value>,
    granted_capabilities: CapabilityGrantSet,
) -> PolicyEvaluationResult {
    PolicyEvaluationResult {
        granted_capabilities,
        decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Rejected,
            summary: summary.into(),
            profile_name,
            trust_tier: installed.trust.trust_tier.clone(),
            verification_state: installed.trust.verification_state.clone(),
            reasons,
            detail,
        },
    }
}

fn completed_policy_evaluation(
    installed: &InstalledSkill,
    profile_name: String,
    granted_capabilities: CapabilityGrantSet,
    reasons: Vec<PolicyReason>,
) -> PolicyEvaluationResult {
    let outcome = if reasons.is_empty() {
        PolicyDecisionOutcome::Allowed
    } else {
        PolicyDecisionOutcome::Reduced
    };
    let summary = match outcome {
        PolicyDecisionOutcome::Allowed => "local policy granted requested capabilities",
        PolicyDecisionOutcome::Reduced => {
            "local policy reduced requested capabilities before execution"
        }
        PolicyDecisionOutcome::Rejected => "local policy denied execution before guest start",
    }
    .into();

    PolicyEvaluationResult {
        granted_capabilities,
        decision: PolicyDecision {
            outcome,
            summary,
            profile_name,
            trust_tier: installed.trust.trust_tier.clone(),
            verification_state: installed.trust.verification_state.clone(),
            reasons,
            detail: None,
        },
    }
}

fn policy_profile_binding_matches(binding: &PolicyProfileBinding, request: &CallerRequest) -> bool {
    if let Some(actor_ids) = &binding.actor_ids {
        if !actor_ids
            .iter()
            .any(|actor_id| actor_id == &request.actor_id)
        {
            return false;
        }
    }

    if let Some(tenant_ids) = &binding.tenant_ids {
        if !tenant_ids
            .iter()
            .any(|tenant_id| tenant_id == &request.tenant_id)
        {
            return false;
        }
    }

    true
}

fn mark_required_grants(
    installed: &InstalledSkill,
    grants: &CapabilityGrantSet,
) -> Vec<CandidateGrant> {
    let required: Vec<_> = installed
        .manifest
        .capabilities
        .iter()
        .filter(|requirement| requirement.required)
        .cloned()
        .collect();

    grants
        .grants
        .iter()
        .cloned()
        .map(|grant| CandidateGrant {
            covers_required: required.iter().any(|requirement| {
                CapabilityEvaluator::grant_covers_requirement(&grant, requirement)
            }),
            grant,
        })
        .collect()
}

fn candidate_grants_to_set(grants: &[CandidateGrant]) -> CapabilityGrantSet {
    let mut unique = Vec::new();
    for candidate in grants {
        push_unique_grant(&mut unique, candidate.grant.clone());
    }

    CapabilityGrantSet { grants: unique }
}

fn policy_rule_matches(rule: &PolicyRule, installed: &InstalledSkill) -> bool {
    if let Some(skills) = &rule.skills {
        if !skills.iter().any(|skill| skill == &installed.manifest.key) {
            return false;
        }
    }

    if let Some(publisher_ids) = &rule.publisher_ids {
        if !publisher_ids
            .iter()
            .any(|publisher_id| publisher_id == &installed.manifest.publisher.id)
        {
            return false;
        }
    }

    if let Some(trust_tiers) = &rule.trust_tiers {
        if !trust_tiers
            .iter()
            .any(|trust_tier| trust_tier == &installed.trust.trust_tier)
        {
            return false;
        }
    }

    if let Some(verification_states) = &rule.verification_states {
        if !verification_states
            .iter()
            .any(|state| state == &installed.trust.verification_state)
        {
            return false;
        }
    }

    true
}

fn apply_policy_rule(
    profile_name: &str,
    rule: &PolicyRule,
    grants: &[CandidateGrant],
    installed: &InstalledSkill,
    reasons: &mut Vec<PolicyReason>,
) -> Vec<CandidateGrant> {
    match rule.effect {
        PolicyRuleEffect::Deny => {
            apply_policy_deny_rule(profile_name, rule, grants, installed, reasons)
        }
        PolicyRuleEffect::Cap => {
            apply_policy_cap_rule(profile_name, rule, grants, installed, reasons)
        }
    }
}

fn policy_rule_target_matches(target: &PolicyRuleTarget, candidate: &CandidateGrant) -> bool {
    match target {
        PolicyRuleTarget::Any => true,
        PolicyRuleTarget::Requested => !candidate.covers_required,
        PolicyRuleTarget::Required => candidate.covers_required,
    }
}

fn apply_policy_deny_rule(
    profile_name: &str,
    rule: &PolicyRule,
    grants: &[CandidateGrant],
    installed: &InstalledSkill,
    reasons: &mut Vec<PolicyReason>,
) -> Vec<CandidateGrant> {
    let mut filtered = Vec::new();

    for candidate in grants {
        if !policy_rule_target_matches(&rule.applies_to, candidate) {
            filtered.push(candidate.clone());
            continue;
        }

        if let Some(rule_grant) = rule
            .capabilities
            .grants
            .iter()
            .find(|rule_grant| policy_grant_matches(rule_grant, &candidate.grant))
        {
            reasons.push(PolicyReason {
                code: "policy-profile-rule-deny".into(),
                message: "local policy profile denied a capability grant before execution".into(),
                detail: Some(serde_json::json!({
                    "profile_name": profile_name,
                    "rule": rule.name,
                    "grant": candidate.grant,
                    "rule_grant": rule_grant,
                    "trust_tier": installed.trust.trust_tier,
                    "verification_state": installed.trust.verification_state,
                    "applies_to": rule.applies_to,
                })),
            });
            continue;
        }

        filtered.push(candidate.clone());
    }

    filtered
}

fn apply_policy_cap_rule(
    profile_name: &str,
    rule: &PolicyRule,
    grants: &[CandidateGrant],
    installed: &InstalledSkill,
    reasons: &mut Vec<PolicyReason>,
) -> Vec<CandidateGrant> {
    let mut filtered = Vec::new();

    for candidate in grants {
        if !policy_rule_target_matches(&rule.applies_to, candidate) {
            filtered.push(candidate.clone());
            continue;
        }

        let matching: Vec<_> = rule
            .capabilities
            .grants
            .iter()
            .filter(|rule_grant| {
                rule_grant.id == candidate.grant.id && rule_grant.access == candidate.grant.access
            })
            .collect();

        if matching.is_empty() {
            filtered.push(candidate.clone());
            continue;
        }

        let mut reduced = Some(candidate.grant.clone());
        for rule_grant in matching {
            reduced = reduced.and_then(|current| reduce_grant_to_cap(rule_grant, &current));
        }

        match reduced {
            Some(reduced_grant) => {
                if reduced_grant != candidate.grant {
                    reasons.push(PolicyReason {
                        code: "policy-profile-rule-cap".into(),
                        message: "local policy profile reduced a capability grant before execution"
                            .into(),
                        detail: Some(serde_json::json!({
                            "profile_name": profile_name,
                            "rule": rule.name,
                            "requested": candidate.grant,
                            "granted": reduced_grant,
                            "trust_tier": installed.trust.trust_tier,
                            "verification_state": installed.trust.verification_state,
                            "applies_to": rule.applies_to,
                        })),
                    });
                }
                filtered.push(CandidateGrant {
                    grant: reduced_grant,
                    covers_required: candidate.covers_required,
                });
            }
            None => reasons.push(PolicyReason {
                code: "policy-profile-rule-cap".into(),
                message: "local policy profile removed a capability grant before execution".into(),
                detail: Some(serde_json::json!({
                    "profile_name": profile_name,
                    "rule": rule.name,
                    "requested": candidate.grant,
                    "trust_tier": installed.trust.trust_tier,
                    "verification_state": installed.trust.verification_state,
                    "applies_to": rule.applies_to,
                })),
            }),
        }
    }

    filtered
}

fn missing_required_capabilities(
    installed: &InstalledSkill,
    grants: &CapabilityGrantSet,
) -> Vec<Value> {
    installed
        .manifest
        .capabilities
        .iter()
        .filter(|requirement| requirement.required)
        .filter(|requirement| !CapabilityEvaluator::grants_cover_requirement(grants, requirement))
        .map(|requirement| {
            serde_json::json!({
                "id": requirement.id,
                "access": requirement.access,
                "constraints": requirement.constraints,
            })
        })
        .collect()
}

fn reduce_grant_to_requirement(
    grant: &GrantedCapability,
    requirement: &CapabilityRequirement,
) -> Option<GrantedCapability> {
    if grant.id != requirement.id || grant.access != requirement.access {
        return None;
    }

    let constraints = reduce_child_constraints(grant, requirement).ok()?;
    Some(GrantedCapability {
        id: grant.id.clone(),
        access: grant.access.clone(),
        constraints,
    })
}

fn reduce_grant_to_cap(
    cap: &GrantedCapability,
    grant: &GrantedCapability,
) -> Option<GrantedCapability> {
    if cap.id != grant.id || cap.access != grant.access {
        return None;
    }

    let constraints = match (&cap.constraints, &grant.constraints) {
        (CapabilityConstraints::None(_), _) | (_, CapabilityConstraints::None(_)) => {
            grant.constraints.clone()
        }
        (CapabilityConstraints::Filesystem(cap), CapabilityConstraints::Filesystem(grant)) => {
            CapabilityConstraints::Filesystem(reduce_cap_filesystem_constraints(cap, grant)?)
        }
        (CapabilityConstraints::HttpRequest(cap), CapabilityConstraints::HttpRequest(grant)) => {
            CapabilityConstraints::HttpRequest(reduce_cap_http_request_constraints(cap, grant)?)
        }
        (CapabilityConstraints::ReadResource(cap), CapabilityConstraints::ReadResource(grant)) => {
            CapabilityConstraints::ReadResource(
                reduce_child_read_resource_constraints(cap, grant).ok()?,
            )
        }
        (
            CapabilityConstraints::InvokeDependency(cap),
            CapabilityConstraints::InvokeDependency(grant),
        ) => CapabilityConstraints::InvokeDependency(
            reduce_child_invoke_dependency_constraints(cap, grant).ok()?,
        ),
        (CapabilityConstraints::EmitEvidence(cap), CapabilityConstraints::EmitEvidence(grant)) => {
            CapabilityConstraints::EmitEvidence(
                reduce_child_emit_evidence_constraints(cap, grant).ok()?,
            )
        }
        (CapabilityConstraints::Log(cap), CapabilityConstraints::Log(grant)) => {
            CapabilityConstraints::Log(reduce_child_log_constraints(cap, grant).ok()?)
        }
        _ => return None,
    };

    Some(GrantedCapability {
        id: grant.id.clone(),
        access: grant.access.clone(),
        constraints,
    })
}

fn policy_grant_matches(rule_grant: &GrantedCapability, grant: &GrantedCapability) -> bool {
    let requirement = CapabilityRequirement {
        id: grant.id.clone(),
        access: grant.access.clone(),
        constraints: grant.constraints.clone(),
        required: false,
    };

    CapabilityEvaluator::grant_covers_requirement(rule_grant, &requirement)
}

fn push_unique_grant(grants: &mut Vec<GrantedCapability>, grant: GrantedCapability) {
    if !grants.contains(&grant) {
        grants.push(grant);
    }
}

fn load_child_installed<R>(
    registry: &R,
    dependency: &guild_manifest::InstalledDependencySpec,
    alias: &str,
) -> Result<InstalledSkill, Box<ChildInvocationError>>
where
    R: SkillRegistry + ?Sized,
{
    registry
        .resolve_exact(&dependency.skill)
        .map_err(|error| SkillError {
            code: "dependency-resolution-failed".into(),
            message: "declared dependency could not be loaded as an installed executable skill"
                .into(),
            retryable: false,
            detail: Some(serde_json::json!({
                "alias": alias,
                "dependency": dependency.skill,
                "cause": {
                    "code": error.code,
                    "message": error.message,
                    "detail": error.detail,
                }
            })),
        })
        .map_err(|error| Box::new(ChildInvocationError::without_record(error)))
}

fn build_child_caller_request(
    context: &ExecutionContext,
    sequence: u16,
    input: &Value,
    child_installed: &InstalledSkill,
    child_grants: CapabilityGrantSet,
) -> CallerRequest {
    guild_types::CallerRequest {
        request_id: format!("{}:child:{sequence}", context.execution_id),
        skill: exact_requested_skill_ref(&child_installed.resolved_ref),
        tenant_id: context.tenant_id.clone(),
        actor_id: "skill".into(),
        mode: context.mode.clone(),
        input: input.clone(),
        budget: derive_child_budget(&context.budget),
        requested_capabilities: child_grants,
        idempotency_key: None,
        trace_id: context.trace_id.clone(),
    }
}

fn load_child_failure_record<R>(
    registry: &R,
    alias: &str,
    parent_execution_id: &str,
    error: &ExecutionError,
) -> Option<Box<ChildExecutionRecord>>
where
    R: SkillRegistry + ?Sized,
{
    error
        .receipt
        .as_ref()
        .and_then(|receipt| registry.load_execution_record(&receipt.execution_id).ok())
        .map(|record| {
            Box::new(child_execution_record_from_execution_record(
                alias,
                parent_execution_id,
                &record,
            ))
        })
}

fn derive_child_budget(parent: &guild_types::Budget) -> guild_types::Budget {
    let mut budget = parent.clone();
    budget.max_child_executions = budget.max_child_executions.saturating_sub(1);
    budget
}

fn next_child_sequence(current_children: usize) -> wasmtime::Result<u16> {
    let current = u16::try_from(current_children)
        .map_err(|_| wasmtime::Error::msg("child execution sequence exceeded u16 range"))?;
    current
        .checked_add(1)
        .ok_or_else(|| wasmtime::Error::msg("child execution sequence exceeded u16 range"))
}

fn saturating_u16_len(len: usize) -> u16 {
    u16::try_from(len).unwrap_or(u16::MAX)
}

fn duration_ms(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn status_from_error(error: &ExecutionError) -> ExecutionStatus {
    match error.phase.clone().unwrap_or(ExecutionPhase::RuntimeExec) {
        ExecutionPhase::Validation | ExecutionPhase::Grant | ExecutionPhase::Mode => {
            ExecutionStatus::Rejected
        }
        ExecutionPhase::RuntimeLoad
        | ExecutionPhase::RuntimeExec
        | ExecutionPhase::ChildInvocation
        | ExecutionPhase::Persistence
        | ExecutionPhase::SkillDomain => ExecutionStatus::Failed,
    }
}

fn termination_from_error(error: &ExecutionError) -> TerminationDetail {
    TerminationDetail {
        phase: error.phase.clone().unwrap_or(ExecutionPhase::RuntimeExec),
        code: error.code.clone(),
        message: error.message.clone(),
        retryable: error.retryable,
        detail: error.detail.as_deref().cloned(),
    }
}

fn child_execution_record_from_execution_record(
    alias: &str,
    parent_execution_id: &str,
    record: &ExecutionRecord,
) -> ChildExecutionRecord {
    ChildExecutionRecord {
        alias: alias.to_owned(),
        execution_id: record.receipt.execution_id.clone(),
        uri: record.receipt.uri.clone(),
        parent_execution_id: parent_execution_id.to_owned(),
        trace_id: record.receipt.trace_id.clone(),
        status: record.status.clone(),
        policy_decision: record.policy_decision.clone(),
        termination: record.termination.clone(),
        granted_capabilities: record.granted_capabilities.clone(),
        metrics: record.metrics.clone(),
        provenance: record.provenance.clone(),
    }
}

fn exact_requested_skill_ref(skill: &ResolvedSkillRef) -> guild_types::RequestedSkillRef {
    guild_types::RequestedSkillRef {
        key: skill.key.clone(),
        version_req: guild_types::VersionRequirement::parse(&format!("={}", skill.version))
            .expect("resolved skill versions render as valid exact semver requirements"),
    }
}

fn child_execution_error_to_skill_error(alias: &str, error: &ExecutionError) -> SkillError {
    let detail = serde_json::json!({
        "alias": alias,
        "cause": {
            "code": error.code,
            "message": error.message,
            "retryable": error.retryable,
            "detail": error.detail,
            "receipt": error.receipt,
        }
    });

    SkillError {
        code: "child-invocation-failed".into(),
        message: format!("dependency alias `{alias}` failed during child execution"),
        retryable: error.retryable,
        detail: Some(detail),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
struct CapabilityDenial {
    code: String,
    message: String,
    detail: Value,
}

impl CapabilityDenial {
    fn into_skill_error(self) -> SkillError {
        SkillError {
            code: self.code,
            message: self.message,
            retryable: false,
            detail: Some(self.detail),
        }
    }

    fn into_execution_error(self, phase: ExecutionPhase) -> ExecutionError {
        ExecutionError::new(self.code, self.message)
            .with_detail(self.detail)
            .with_phase(phase)
    }
}

fn parse_http_request(request: &HttpRequest) -> Result<ParsedHttpRequest, CapabilityDenial> {
    let url = Url::parse(&request.url).map_err(|error| CapabilityDenial {
        code: "http-request-url-invalid".into(),
        message: "http-request requires an absolute HTTP or HTTPS URL".into(),
        detail: serde_json::json!({
            "url": request.url,
            "error": error.to_string(),
        }),
    })?;

    if !url.username().is_empty() || url.password().is_some() {
        return Err(CapabilityDenial {
            code: "http-request-url-invalid".into(),
            message: "http-request URLs must not embed credentials".into(),
            detail: serde_json::json!({
                "url": request.url,
            }),
        });
    }

    let scheme = match url.scheme() {
        "http" => HttpScheme::Http,
        "https" => HttpScheme::Https,
        other => {
            return Err(CapabilityDenial {
                code: "http-request-url-invalid".into(),
                message: "http-request only supports HTTP and HTTPS URLs".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "scheme": other,
                }),
            });
        }
    };

    let (host, host_kind) = match url.host() {
        Some(url::Host::Domain(domain)) => {
            let host = domain.to_ascii_lowercase();
            let loopback_name = host == "localhost" || host.ends_with(".localhost");
            (host, ParsedHttpHost::Domain { loopback_name })
        }
        Some(url::Host::Ipv4(address)) => (
            address.to_string(),
            ParsedHttpHost::IpLiteral(IpAddr::V4(address)),
        ),
        Some(url::Host::Ipv6(address)) => (
            address.to_string(),
            ParsedHttpHost::IpLiteral(IpAddr::V6(address)),
        ),
        None => {
            return Err(CapabilityDenial {
                code: "http-request-url-invalid".into(),
                message: "http-request URL must include a host".into(),
                detail: serde_json::json!({
                    "url": request.url,
                }),
            });
        }
    };
    let port = url
        .port_or_known_default()
        .ok_or_else(|| CapabilityDenial {
            code: "http-request-url-invalid".into(),
            message: "http-request URL must resolve to an explicit or default port".into(),
            detail: serde_json::json!({
                "url": request.url,
            }),
        })?;
    let path = if url.path().is_empty() {
        "/".to_owned()
    } else {
        url.path().to_owned()
    };

    Ok(ParsedHttpRequest {
        scheme,
        host,
        port,
        path,
        host_kind,
    })
}

fn execute_http_request(
    request: &HttpRequest,
    timeout: Duration,
    max_response_bytes: u64,
) -> Result<HostHttpResponse, SkillError> {
    let parsed_request = parse_http_request(request).map_err(CapabilityDenial::into_skill_error)?;
    let http_request = Request::builder()
        .method(http_method(&request.method))
        .uri(request.url.as_str())
        .body(empty_http_body())
        .map_err(|error| SkillError {
            code: "http-request-build-failed".into(),
            message: "host could not build the outbound HTTP request".into(),
            retryable: false,
            detail: Some(serde_json::json!({
                "url": request.url,
                "error": error.to_string(),
            })),
        })?;
    let response = wasmtime_wasi::runtime::in_tokio(default_send_request_handler(
        http_request,
        OutgoingRequestConfig {
            use_tls: matches!(parsed_request.scheme, HttpScheme::Https),
            connect_timeout: timeout,
            first_byte_timeout: timeout,
            between_bytes_timeout: timeout,
        },
    ))
    .map_err(|error| skill_error_from_wasi_http_error(&error, request, max_response_bytes))?;

    let between_bytes_timeout = response.between_bytes_timeout;
    let worker = response.worker;
    let resp = response.resp;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let redirect_location = resp
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    if is_redirect_status(status) {
        return Ok(HostHttpResponse {
            response: HttpResponse {
                url: request.url.clone(),
                status,
                content_type,
                body: Vec::new(),
            },
            redirect_location,
        });
    }
    let mut body = resp.into_body();
    let body = wasmtime_wasi::runtime::in_tokio(async move {
        let _worker = worker;
        let mut bytes = Vec::new();

        loop {
            let frame = tokio::time::timeout(between_bytes_timeout, body.frame())
                .await
                .map_err(|_| WasiHttpErrorCode::HttpResponseTimeout)?;
            let Some(frame) = frame else {
                break;
            };
            let frame = frame?;
            let Ok(data) = frame.into_data() else {
                continue;
            };

            let next_len = u64::try_from(bytes.len())
                .unwrap_or(u64::MAX)
                .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
            if next_len > max_response_bytes {
                return Err(WasiHttpErrorCode::HttpResponseBodySize(Some(
                    max_response_bytes,
                )));
            }

            bytes.extend_from_slice(&data);
        }

        Ok::<Vec<u8>, WasiHttpErrorCode>(bytes)
    })
    .map_err(|error| skill_error_from_wasi_http_error(&error, request, max_response_bytes))?;

    Ok(HostHttpResponse {
        response: HttpResponse {
            url: request.url.clone(),
            status,
            content_type,
            body,
        },
        redirect_location,
    })
}

fn empty_http_body() -> HyperOutgoingBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed_unsync()
}

fn http_method(method: &HttpMethod) -> http::Method {
    match method {
        HttpMethod::Get => http::Method::GET,
        HttpMethod::Head => http::Method::HEAD,
    }
}

fn skill_error_from_wasi_http_error(
    error: &WasiHttpErrorCode,
    request: &HttpRequest,
    max_response_bytes: u64,
) -> SkillError {
    let (code, message, retryable) = match error {
        WasiHttpErrorCode::ConnectionTimeout
        | WasiHttpErrorCode::ConnectionReadTimeout
        | WasiHttpErrorCode::HttpResponseTimeout => (
            "http-request-timeout",
            "outbound HTTP request exceeded the host timeout",
            true,
        ),
        WasiHttpErrorCode::HttpResponseBodySize(_) => (
            "http-request-response-too-large",
            "outbound HTTP response exceeded the configured max_response_bytes limit",
            false,
        ),
        WasiHttpErrorCode::HttpRequestUriInvalid | WasiHttpErrorCode::HttpProtocolError => (
            "http-request-build-failed",
            "host could not construct a valid outbound HTTP request",
            false,
        ),
        _ => (
            "http-request-failed",
            "host failed to complete the outbound HTTP request",
            false,
        ),
    };

    SkillError {
        code: code.into(),
        message: message.into(),
        retryable,
        detail: Some(serde_json::json!({
            "url": request.url,
            "method": request.method,
            "max_response_bytes": max_response_bytes,
            "error": format!("{error:?}"),
        })),
    }
}

fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn build_redirect_request(
    request: &HttpRequest,
    location: &str,
) -> Result<HttpRequest, CapabilityDenial> {
    let current = Url::parse(&request.url).map_err(|error| CapabilityDenial {
        code: "http-request-redirect-location-invalid".into(),
        message: "http-request redirect source URL could not be reparsed safely".into(),
        detail: serde_json::json!({
            "url": request.url,
            "location": location,
            "error": error.to_string(),
        }),
    })?;
    let redirected = current.join(location).map_err(|error| CapabilityDenial {
        code: "http-request-redirect-location-invalid".into(),
        message: "http-request redirect location was invalid".into(),
        detail: serde_json::json!({
            "url": request.url,
            "location": location,
            "error": error.to_string(),
        }),
    })?;

    Ok(HttpRequest {
        method: request.method.clone(),
        url: redirected.to_string(),
        timeout_ms: request.timeout_ms,
    })
}

enum CapabilityOperation<'a> {
    ReadResource {
        uri: &'a str,
        parsed_uri: &'a GuildResourceUri,
    },
    InvokeDependency {
        alias: &'a str,
    },
    EmitEvidence {
        request: &'a EvidenceEmissionRequest,
    },
    Log {
        level: Severity,
    },
}

struct CapabilityEvaluator;

impl CapabilityEvaluator {
    fn grants_cover_requirement(
        grants: &CapabilityGrantSet,
        requirement: &CapabilityRequirement,
    ) -> bool {
        grants
            .grants
            .iter()
            .any(|grant| Self::grant_covers_requirement(grant, requirement))
    }

    fn grant_covers_requirement(
        grant: &GrantedCapability,
        requirement: &CapabilityRequirement,
    ) -> bool {
        if grant.id != requirement.id || grant.access != requirement.access {
            return false;
        }

        match (&grant.constraints, &requirement.constraints) {
            (CapabilityConstraints::None(_), _) | (_, CapabilityConstraints::None(_)) => true,
            (
                CapabilityConstraints::Filesystem(grant),
                CapabilityConstraints::Filesystem(required),
            ) => filesystem_covers(grant, required),
            (
                CapabilityConstraints::HttpRequest(grant),
                CapabilityConstraints::HttpRequest(required),
            ) => http_request_covers(grant, required),
            (
                CapabilityConstraints::ReadResource(grant),
                CapabilityConstraints::ReadResource(required),
            ) => read_resource_covers(grant, required),
            (
                CapabilityConstraints::InvokeDependency(grant),
                CapabilityConstraints::InvokeDependency(required),
            ) => invoke_dependency_covers(grant, required),
            (
                CapabilityConstraints::EmitEvidence(grant),
                CapabilityConstraints::EmitEvidence(required),
            ) => emit_evidence_covers(grant, required),
            (CapabilityConstraints::Log(grant), CapabilityConstraints::Log(required)) => {
                log_covers(grant, required)
            }
            _ => false,
        }
    }

    fn authorize(
        grants: &CapabilityGrantSet,
        operation: &CapabilityOperation<'_>,
    ) -> Result<(), CapabilityDenial> {
        match operation {
            CapabilityOperation::ReadResource { uri, parsed_uri } => {
                Self::authorize_read_resource(grants, uri, parsed_uri)
            }
            CapabilityOperation::InvokeDependency { alias } => {
                Self::authorize_invoke_dependency(grants, alias)
            }
            CapabilityOperation::EmitEvidence { request } => {
                Self::authorize_emit_evidence(grants, request)
            }
            CapabilityOperation::Log { level } => Self::authorize_log(grants, level),
        }
    }

    fn authorize_http_request(
        grants: &CapabilityGrantSet,
        budget: &guild_types::Budget,
        used_network_requests: u32,
        request: &HttpRequest,
        parsed_request: &ParsedHttpRequest,
    ) -> Result<HttpExecutionPolicy, CapabilityDenial> {
        let matching =
            Self::matching_grants(grants, &CapabilityId::HttpRequest, &CapabilityAccess::Read);

        if matching.is_empty() {
            return Err(http_request_not_granted_denial(request));
        }

        if used_network_requests >= budget.max_network_requests {
            return Err(http_request_budget_denial(
                request,
                used_network_requests,
                budget.max_network_requests,
            ));
        }

        let mut state = HttpGrantState::default();

        for grant in matching {
            let CapabilityConstraints::HttpRequest(constraints) = &grant.constraints else {
                if matches!(grant.constraints, CapabilityConstraints::None(_)) {
                    authorize_unconstrained_http_grant(&mut state, request, budget);
                    continue;
                }
                continue;
            };

            evaluate_http_request_constraints(
                &mut state,
                constraints,
                request,
                parsed_request,
                budget,
            );
        }

        finalize_http_request_authorization(state, request, parsed_request)
    }

    fn matching_grants<'a>(
        grants: &'a CapabilityGrantSet,
        id: &CapabilityId,
        access: &CapabilityAccess,
    ) -> Vec<&'a GrantedCapability> {
        grants
            .grants
            .iter()
            .filter(|grant| grant.id == *id && grant.access == *access)
            .collect()
    }

    fn authorize_read_resource(
        grants: &CapabilityGrantSet,
        uri: &str,
        parsed_uri: &GuildResourceUri,
    ) -> Result<(), CapabilityDenial> {
        let kind = parsed_uri.kind();
        let matching =
            Self::matching_grants(grants, &CapabilityId::ReadResource, &CapabilityAccess::Read);

        if matching.is_empty() {
            return Err(CapabilityDenial {
                code: "read-resource-not-granted".into(),
                message: format!("resource URI `{uri}` was not granted for read access"),
                detail: serde_json::json!({
                    "uri": uri,
                    "resource_kind": kind,
                }),
            });
        }

        let kind_allowed = matching.iter().any(|grant| match &grant.constraints {
            CapabilityConstraints::None(_) => true,
            CapabilityConstraints::ReadResource(constraints) => constraints
                .resource_kinds
                .as_ref()
                .is_none_or(|kinds| kinds.contains(&kind)),
            _ => false,
        });

        if !kind_allowed {
            return Err(CapabilityDenial {
                code: "read-resource-kind-denied".into(),
                message: format!(
                    "resource kind `{}` was not granted for read access",
                    resource_kind_label(&kind)
                ),
                detail: serde_json::json!({
                    "uri": uri,
                    "resource_kind": kind,
                }),
            });
        }

        let scope_allowed = matching.iter().any(|grant| match &grant.constraints {
            CapabilityConstraints::None(_) => true,
            CapabilityConstraints::ReadResource(constraints) => {
                constraints.uri_prefixes.as_ref().is_none_or(|prefixes| {
                    prefixes.iter().any(|prefix| {
                        GuildResourceScope::parse(prefix)
                            .is_ok_and(|scope| scope.matches(parsed_uri))
                    })
                })
            }
            _ => false,
        });

        if scope_allowed {
            Ok(())
        } else {
            Err(CapabilityDenial {
                code: "read-resource-not-granted".into(),
                message: format!("resource URI `{uri}` was not granted for read access"),
                detail: serde_json::json!({
                    "uri": uri,
                    "resource_kind": kind,
                }),
            })
        }
    }

    fn authorize_invoke_dependency(
        grants: &CapabilityGrantSet,
        alias: &str,
    ) -> Result<(), CapabilityDenial> {
        let matching = Self::matching_grants(
            grants,
            &CapabilityId::InvokeSkill,
            &CapabilityAccess::Invoke,
        );

        if matching.is_empty() {
            return Err(CapabilityDenial {
                code: "dependency-invoke-not-granted".into(),
                message: format!("dependency alias `{alias}` was not granted for invocation"),
                detail: serde_json::json!({ "alias": alias }),
            });
        }

        let allowed = matching.iter().any(|grant| match &grant.constraints {
            CapabilityConstraints::None(_) => true,
            CapabilityConstraints::InvokeDependency(constraints) => constraints
                .aliases
                .as_ref()
                .is_none_or(|aliases| aliases.iter().any(|candidate| candidate == alias)),
            _ => false,
        });

        if allowed {
            Ok(())
        } else {
            Err(CapabilityDenial {
                code: "dependency-invoke-not-granted".into(),
                message: format!("dependency alias `{alias}` was not granted for invocation"),
                detail: serde_json::json!({ "alias": alias }),
            })
        }
    }

    fn authorize_emit_evidence(
        grants: &CapabilityGrantSet,
        request: &EvidenceEmissionRequest,
    ) -> Result<(), CapabilityDenial> {
        let matching = Self::matching_grants(
            grants,
            &CapabilityId::EmitEvidence,
            &CapabilityAccess::Write,
        );
        let payload_bytes = u64::try_from(request.payload.len()).unwrap_or(u64::MAX);

        if matching.is_empty() {
            return Err(Self::emit_evidence_not_granted(request, payload_bytes));
        }

        let mut saw_size_denial = false;
        let mut saw_audience_denial = false;
        let mut saw_redaction_denial = false;

        for grant in matching {
            let CapabilityConstraints::EmitEvidence(constraints) = &grant.constraints else {
                if matches!(grant.constraints, CapabilityConstraints::None(_)) {
                    return Ok(());
                }
                continue;
            };

            if let Some(max_bytes) = constraints.max_bytes {
                if payload_bytes > max_bytes {
                    saw_size_denial = true;
                    continue;
                }
            }

            if let Some(audiences) = &constraints.audiences {
                if !audiences.contains(&request.audience) {
                    saw_audience_denial = true;
                    continue;
                }
            }

            if let Some(redactions) = &constraints.redactions {
                if !redactions.contains(&request.redaction) {
                    saw_redaction_denial = true;
                    continue;
                }
            }

            return Ok(());
        }

        if saw_size_denial {
            Err(CapabilityDenial {
                code: "emit-evidence-too-large".into(),
                message: "evidence payload exceeded the granted max_bytes limit".into(),
                detail: serde_json::json!({
                    "payload_bytes": payload_bytes,
                }),
            })
        } else if saw_audience_denial {
            Err(CapabilityDenial {
                code: "emit-evidence-audience-not-granted".into(),
                message: "evidence audience was not granted for this execution".into(),
                detail: serde_json::json!({
                    "audience": request.audience,
                }),
            })
        } else if saw_redaction_denial {
            Err(CapabilityDenial {
                code: "emit-evidence-redaction-not-granted".into(),
                message: "evidence redaction class was not granted for this execution".into(),
                detail: serde_json::json!({
                    "redaction": request.redaction,
                }),
            })
        } else {
            Err(Self::emit_evidence_not_granted(request, payload_bytes))
        }
    }

    fn emit_evidence_not_granted(
        request: &EvidenceEmissionRequest,
        payload_bytes: u64,
    ) -> CapabilityDenial {
        CapabilityDenial {
            code: "emit-evidence-not-granted".into(),
            message: "evidence emission was not granted for this execution".into(),
            detail: serde_json::json!({
                "mime_type": request.mime_type,
                "payload_bytes": payload_bytes,
                "audience": request.audience,
                "redaction": request.redaction,
            }),
        }
    }

    fn authorize_log(
        grants: &CapabilityGrantSet,
        level: &Severity,
    ) -> Result<(), CapabilityDenial> {
        let matching =
            Self::matching_grants(grants, &CapabilityId::LogWrite, &CapabilityAccess::Write);

        if matching.is_empty() {
            return Err(CapabilityDenial {
                code: "log-write-not-granted".into(),
                message: "log-write was not granted for this execution".into(),
                detail: serde_json::json!({ "level": level }),
            });
        }

        let allowed = matching.iter().any(|grant| match &grant.constraints {
            CapabilityConstraints::None(_) => true,
            CapabilityConstraints::Log(constraints) => constraints
                .levels
                .as_ref()
                .is_none_or(|levels| levels.contains(level)),
            _ => false,
        });

        if allowed {
            Ok(())
        } else {
            Err(CapabilityDenial {
                code: "log-level-not-granted".into(),
                message: format!(
                    "log level `{}` was not granted for this execution",
                    severity_label(level)
                ),
                detail: serde_json::json!({ "level": level }),
            })
        }
    }

    fn derive_child_grants(
        child_capabilities: &[CapabilityRequirement],
        parent_grants: &CapabilityGrantSet,
    ) -> Result<CapabilityGrantSet, CapabilityDenial> {
        let mut grants = Vec::new();

        for capability in child_capabilities {
            if let Some(parent_grant) = parent_grants
                .grants
                .iter()
                .find(|grant| Self::grant_covers_requirement(grant, capability))
            {
                let constraints = reduce_child_constraints(parent_grant, capability)?;
                grants.push(GrantedCapability {
                    id: capability.id.clone(),
                    access: capability.access.clone(),
                    constraints,
                });
            } else if capability.required {
                return Err(CapabilityDenial {
                    code: "child-capability-mismatch".into(),
                    message:
                        "child invocation required capabilities that were not granted to the parent"
                            .into(),
                    detail: serde_json::json!({
                        "id": capability.id,
                        "access": capability.access,
                        "constraints": capability.constraints,
                    }),
                });
            }
        }

        Ok(CapabilityGrantSet { grants })
    }
}

fn reduce_child_constraints(
    parent_grant: &GrantedCapability,
    requirement: &CapabilityRequirement,
) -> Result<CapabilityConstraints, CapabilityDenial> {
    match (&parent_grant.constraints, &requirement.constraints) {
        (CapabilityConstraints::None(_), required) => Ok(required.clone()),
        (_, CapabilityConstraints::None(_)) => Ok(parent_grant.constraints.clone()),
        (
            CapabilityConstraints::Filesystem(parent),
            CapabilityConstraints::Filesystem(required),
        ) => Ok(CapabilityConstraints::Filesystem(
            reduce_child_filesystem_constraints(parent, required)?,
        )),
        (
            CapabilityConstraints::HttpRequest(parent),
            CapabilityConstraints::HttpRequest(required),
        ) => Ok(CapabilityConstraints::HttpRequest(
            reduce_child_http_request_constraints(parent, required)?,
        )),
        (
            CapabilityConstraints::ReadResource(parent),
            CapabilityConstraints::ReadResource(required),
        ) => Ok(CapabilityConstraints::ReadResource(
            reduce_child_read_resource_constraints(parent, required)?,
        )),
        (
            CapabilityConstraints::InvokeDependency(parent),
            CapabilityConstraints::InvokeDependency(required),
        ) => Ok(CapabilityConstraints::InvokeDependency(
            reduce_child_invoke_dependency_constraints(parent, required)?,
        )),
        (
            CapabilityConstraints::EmitEvidence(parent),
            CapabilityConstraints::EmitEvidence(required),
        ) => Ok(CapabilityConstraints::EmitEvidence(
            reduce_child_emit_evidence_constraints(parent, required)?,
        )),
        (CapabilityConstraints::Log(parent), CapabilityConstraints::Log(required)) => Ok(
            CapabilityConstraints::Log(reduce_child_log_constraints(parent, required)?),
        ),
        _ => Err(CapabilityDenial {
            code: "child-capability-mismatch".into(),
            message: "child capability constraints could not be reduced from the parent grant"
                .into(),
            detail: serde_json::json!({
                "id": requirement.id,
                "access": requirement.access,
                "parent_constraints": parent_grant.constraints,
                "required_constraints": requirement.constraints,
            }),
        }),
    }
}

fn filesystem_covers(grant: &FilesystemConstraints, required: &FilesystemConstraints) -> bool {
    required.preopened_roots.iter().all(|required_root| {
        grant
            .preopened_roots
            .iter()
            .any(|grant_root| filesystem_root_covers(grant_root, required_root))
    })
}

fn filesystem_root_covers(grant: &FilesystemRoot, required: &FilesystemRoot) -> bool {
    filesystem_root_identity_matches(grant, required)
        && required
            .operations
            .iter()
            .all(|operation| grant.operations.contains(operation))
}

fn filesystem_root_identity_matches(left: &FilesystemRoot, right: &FilesystemRoot) -> bool {
    left.name == right.name
        && left.guest_path_prefix == right.guest_path_prefix
        && left.host_path == right.host_path
}

fn reduce_child_filesystem_constraints(
    parent: &FilesystemConstraints,
    required: &FilesystemConstraints,
) -> Result<FilesystemConstraints, CapabilityDenial> {
    let mut roots = Vec::new();

    for required_root in &required.preopened_roots {
        let Some(parent_root) = parent
            .preopened_roots
            .iter()
            .find(|candidate| filesystem_root_covers(candidate, required_root))
        else {
            return Err(CapabilityDenial {
                code: "child-capability-mismatch".into(),
                message: "child filesystem capability exceeded the parent grant".into(),
                detail: serde_json::json!({
                    "required_root": required_root,
                    "parent_constraints": parent,
                }),
            });
        };

        let operations =
            intersect_filesystem_operations(&parent_root.operations, &required_root.operations);
        if operations.is_empty() {
            return Err(CapabilityDenial {
                code: "child-capability-mismatch".into(),
                message: "child filesystem capability exceeded the parent grant".into(),
                detail: serde_json::json!({
                    "required_root": required_root,
                    "parent_root": parent_root,
                }),
            });
        }

        roots.push(FilesystemRoot {
            name: required_root.name.clone(),
            guest_path_prefix: required_root.guest_path_prefix.clone(),
            host_path: required_root.host_path.clone(),
            operations,
        });
    }

    Ok(FilesystemConstraints {
        preopened_roots: roots,
    })
}

fn reduce_cap_filesystem_constraints(
    cap: &FilesystemConstraints,
    grant: &FilesystemConstraints,
) -> Option<FilesystemConstraints> {
    let preopened_roots: Vec<_> = grant
        .preopened_roots
        .iter()
        .filter_map(|grant_root| {
            cap.preopened_roots
                .iter()
                .find(|cap_root| filesystem_root_identity_matches(cap_root, grant_root))
                .and_then(|cap_root| {
                    let operations = intersect_filesystem_operations(
                        &cap_root.operations,
                        &grant_root.operations,
                    );
                    if operations.is_empty() {
                        None
                    } else {
                        Some(FilesystemRoot {
                            name: grant_root.name.clone(),
                            guest_path_prefix: grant_root.guest_path_prefix.clone(),
                            host_path: grant_root.host_path.clone(),
                            operations,
                        })
                    }
                })
        })
        .collect();

    if preopened_roots.is_empty() {
        None
    } else {
        Some(FilesystemConstraints { preopened_roots })
    }
}

fn intersect_filesystem_operations(
    left: &[FilesystemOperation],
    right: &[FilesystemOperation],
) -> Vec<FilesystemOperation> {
    let mut operations = Vec::new();

    for operation in left {
        if right.contains(operation) && !operations.contains(operation) {
            operations.push(operation.clone());
        }
    }

    operations
}

fn reduce_child_http_request_constraints(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
) -> Result<HttpRequestConstraints, CapabilityDenial> {
    let core = reduce_child_http_core_constraints(parent, required)?;
    let redirects = reduce_child_http_redirect_constraints(parent, required)?;
    let destinations = reduce_child_http_destination_constraints(parent, required)?;

    Ok(HttpRequestConstraints {
        allowed_schemes: core.allowed_schemes,
        allowed_hosts: core.host_scope.hosts,
        allowed_host_suffixes: core.host_scope.suffixes,
        allowed_ports: core.allowed_ports,
        allowed_methods: core.allowed_methods,
        allowed_path_prefixes: core.allowed_path_prefixes,
        max_timeout_ms: core.max_timeout_ms,
        max_response_bytes: core.max_response_bytes,
        follow_redirects: redirects.follow_redirects,
        max_redirects: redirects.max_redirects,
        allow_loopback: destinations.loopback,
        allow_link_local: destinations.link_local,
        allow_private_networks: destinations.private_networks,
        allow_ip_literals: destinations.ip_literals,
    })
}

fn reduce_cap_http_request_constraints(
    cap: &HttpRequestConstraints,
    grant: &HttpRequestConstraints,
) -> Option<HttpRequestConstraints> {
    let host_scope = reduce_required_host_scope(
        cap.allowed_hosts.as_ref(),
        cap.allowed_host_suffixes.as_ref(),
        grant.allowed_hosts.as_ref(),
        grant.allowed_host_suffixes.as_ref(),
    )?;
    let follow_redirects = reduce_cap_allow_flag(cap.follow_redirects, grant.follow_redirects);
    let max_redirects = if allow_flag_enabled(follow_redirects) {
        reduce_required_max_redirects(cap.max_redirects, grant.max_redirects)?.into_option()
    } else {
        None
    };

    Some(HttpRequestConstraints {
        allowed_schemes: reduce_required_enum_scope(
            cap.allowed_schemes.as_ref(),
            grant.allowed_schemes.as_ref(),
        )?
        .into_option(),
        allowed_hosts: host_scope.hosts,
        allowed_host_suffixes: host_scope.suffixes,
        allowed_ports: reduce_required_enum_scope(
            cap.allowed_ports.as_ref(),
            grant.allowed_ports.as_ref(),
        )?
        .into_option(),
        allowed_methods: reduce_required_enum_scope(
            cap.allowed_methods.as_ref(),
            grant.allowed_methods.as_ref(),
        )?
        .into_option(),
        allowed_path_prefixes: reduce_required_path_prefix_scope(
            cap.allowed_path_prefixes.as_ref(),
            grant.allowed_path_prefixes.as_ref(),
        )?
        .into_option(),
        max_timeout_ms: reduce_required_max_bytes(cap.max_timeout_ms, grant.max_timeout_ms)?
            .into_option(),
        max_response_bytes: reduce_required_max_bytes(
            cap.max_response_bytes,
            grant.max_response_bytes,
        )?
        .into_option(),
        follow_redirects,
        max_redirects,
        allow_loopback: reduce_cap_allow_flag(cap.allow_loopback, grant.allow_loopback),
        allow_link_local: reduce_cap_allow_flag(cap.allow_link_local, grant.allow_link_local),
        allow_private_networks: reduce_cap_allow_flag(
            cap.allow_private_networks,
            grant.allow_private_networks,
        ),
        allow_ip_literals: reduce_cap_allow_flag(cap.allow_ip_literals, grant.allow_ip_literals),
    })
}

fn reduce_child_http_core_constraints(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
) -> Result<ReducedChildHttpCore, CapabilityDenial> {
    let allowed_schemes = reduce_child_http_constraint(
        parent,
        required,
        "allowed_schemes",
        "schemes",
        reduce_required_enum_scope(
            parent.allowed_schemes.as_ref(),
            required.allowed_schemes.as_ref(),
        ),
    )?;
    let host_scope = reduce_required_host_scope(
        parent.allowed_hosts.as_ref(),
        parent.allowed_host_suffixes.as_ref(),
        required.allowed_hosts.as_ref(),
        required.allowed_host_suffixes.as_ref(),
    )
    .ok_or_else(|| {
        child_http_constraint_mismatch(parent, required, "allowed_hosts", "host scope")
    })?;
    let allowed_ports = reduce_child_http_constraint(
        parent,
        required,
        "allowed_ports",
        "ports",
        reduce_required_enum_scope(
            parent.allowed_ports.as_ref(),
            required.allowed_ports.as_ref(),
        ),
    )?;
    let allowed_methods = reduce_child_http_constraint(
        parent,
        required,
        "allowed_methods",
        "methods",
        reduce_required_enum_scope(
            parent.allowed_methods.as_ref(),
            required.allowed_methods.as_ref(),
        ),
    )?;
    let allowed_path_prefixes = reduce_child_http_constraint(
        parent,
        required,
        "allowed_path_prefixes",
        "paths",
        reduce_required_path_prefix_scope(
            parent.allowed_path_prefixes.as_ref(),
            required.allowed_path_prefixes.as_ref(),
        ),
    )?;
    let max_timeout_ms = reduce_child_http_constraint(
        parent,
        required,
        "max_timeout_ms",
        "max_timeout_ms",
        reduce_required_max_bytes(parent.max_timeout_ms, required.max_timeout_ms),
    )?;
    let max_response_bytes = reduce_child_http_constraint(
        parent,
        required,
        "max_response_bytes",
        "max_response_bytes",
        reduce_required_max_bytes(parent.max_response_bytes, required.max_response_bytes),
    )?;

    Ok(ReducedChildHttpCore {
        allowed_schemes,
        host_scope,
        allowed_ports,
        allowed_methods,
        allowed_path_prefixes,
        max_timeout_ms,
        max_response_bytes,
    })
}

fn reduce_child_http_redirect_constraints(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
) -> Result<ReducedChildHttpRedirects, CapabilityDenial> {
    let follow_redirects = reduce_child_http_allow_flag(
        parent,
        required,
        parent.follow_redirects,
        required.follow_redirects,
        "follow_redirects",
        "redirect-following",
    )?;
    let max_redirects = if allow_flag_enabled(follow_redirects) {
        reduce_child_http_constraint(
            parent,
            required,
            "max_redirects",
            "max_redirects",
            reduce_required_max_redirects(parent.max_redirects, required.max_redirects),
        )?
    } else {
        None
    };

    Ok(ReducedChildHttpRedirects {
        follow_redirects,
        max_redirects,
    })
}

fn reduce_child_http_destination_constraints(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
) -> Result<ReducedChildHttpDestinationFlags, CapabilityDenial> {
    let allow_loopback = reduce_child_http_allow_flag(
        parent,
        required,
        parent.allow_loopback,
        required.allow_loopback,
        "allow_loopback",
        "loopback destinations",
    )?;
    let allow_link_local = reduce_child_http_allow_flag(
        parent,
        required,
        parent.allow_link_local,
        required.allow_link_local,
        "allow_link_local",
        "link-local destinations",
    )?;
    let allow_private_networks = reduce_child_http_allow_flag(
        parent,
        required,
        parent.allow_private_networks,
        required.allow_private_networks,
        "allow_private_networks",
        "private-network destinations",
    )?;
    let allow_ip_literals = reduce_child_http_allow_flag(
        parent,
        required,
        parent.allow_ip_literals,
        required.allow_ip_literals,
        "allow_ip_literals",
        "IP-literal destinations",
    )?;

    Ok(ReducedChildHttpDestinationFlags {
        loopback: allow_loopback,
        link_local: allow_link_local,
        private_networks: allow_private_networks,
        ip_literals: allow_ip_literals,
    })
}

fn reduce_child_read_resource_constraints(
    parent: &ReadResourceConstraints,
    required: &ReadResourceConstraints,
) -> Result<ReadResourceConstraints, CapabilityDenial> {
    let uri_prefixes = reduce_required_resource_scope(
        parent.uri_prefixes.as_ref(),
        required.uri_prefixes.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child resource-read URI scope could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let resource_kinds = reduce_required_enum_scope(
        parent.resource_kinds.as_ref(),
        required.resource_kinds.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child resource-read kinds could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();

    Ok(ReadResourceConstraints {
        uri_prefixes,
        resource_kinds,
    })
}

fn reduce_child_invoke_dependency_constraints(
    parent: &InvokeDependencyConstraints,
    required: &InvokeDependencyConstraints,
) -> Result<InvokeDependencyConstraints, CapabilityDenial> {
    let aliases =
        reduce_required_exact_string_scope(parent.aliases.as_ref(), required.aliases.as_ref())
            .ok_or_else(|| CapabilityDenial {
                code: "child-capability-mismatch".into(),
                message: "child invocation aliases could not be reduced from the parent grant"
                    .into(),
                detail: serde_json::json!({
                    "parent_constraints": parent,
                    "required_constraints": required,
                }),
            })?
            .into_option();

    Ok(InvokeDependencyConstraints { aliases })
}

fn reduce_child_emit_evidence_constraints(
    parent: &EmitEvidenceConstraints,
    required: &EmitEvidenceConstraints,
) -> Result<EmitEvidenceConstraints, CapabilityDenial> {
    let audiences =
        reduce_required_enum_scope(parent.audiences.as_ref(), required.audiences.as_ref())
            .ok_or_else(|| CapabilityDenial {
                code: "child-capability-mismatch".into(),
                message: "child evidence audiences could not be reduced from the parent grant"
                    .into(),
                detail: serde_json::json!({
                    "parent_constraints": parent,
                    "required_constraints": required,
                }),
            })?
            .into_option();
    let redactions =
        reduce_required_enum_scope(parent.redactions.as_ref(), required.redactions.as_ref())
            .ok_or_else(|| CapabilityDenial {
                code: "child-capability-mismatch".into(),
                message: "child evidence redactions could not be reduced from the parent grant"
                    .into(),
                detail: serde_json::json!({
                    "parent_constraints": parent,
                    "required_constraints": required,
                }),
            })?
            .into_option();
    let max_bytes = reduce_required_max_bytes(parent.max_bytes, required.max_bytes)
        .ok_or_else(|| CapabilityDenial {
            code: "child-capability-mismatch".into(),
            message: "child evidence max_bytes could not be reduced from the parent grant".into(),
            detail: serde_json::json!({
                "parent_constraints": parent,
                "required_constraints": required,
            }),
        })?
        .into_option();

    Ok(EmitEvidenceConstraints {
        max_bytes,
        audiences,
        redactions,
    })
}

fn reduce_child_log_constraints(
    parent: &LogConstraints,
    required: &LogConstraints,
) -> Result<LogConstraints, CapabilityDenial> {
    let levels = reduce_required_enum_scope(parent.levels.as_ref(), required.levels.as_ref())
        .ok_or_else(|| CapabilityDenial {
            code: "child-capability-mismatch".into(),
            message: "child log levels could not be reduced from the parent grant".into(),
            detail: serde_json::json!({
                "parent_constraints": parent,
                "required_constraints": required,
            }),
        })?
        .into_option();

    Ok(LogConstraints { levels })
}

fn read_resource_covers(
    grant: &ReadResourceConstraints,
    required: &ReadResourceConstraints,
) -> bool {
    resource_scope_covers(grant.uri_prefixes.as_ref(), required.uri_prefixes.as_ref())
        && enum_scope_covers(
            grant.resource_kinds.as_ref(),
            required.resource_kinds.as_ref(),
        )
}

fn invoke_dependency_covers(
    grant: &InvokeDependencyConstraints,
    required: &InvokeDependencyConstraints,
) -> bool {
    string_scope_covers_exact(grant.aliases.as_ref(), required.aliases.as_ref())
}

fn http_request_covers(grant: &HttpRequestConstraints, required: &HttpRequestConstraints) -> bool {
    enum_scope_covers(
        grant.allowed_schemes.as_ref(),
        required.allowed_schemes.as_ref(),
    ) && http_host_scope_covers(
        grant.allowed_hosts.as_ref(),
        grant.allowed_host_suffixes.as_ref(),
        required.allowed_hosts.as_ref(),
        required.allowed_host_suffixes.as_ref(),
    ) && enum_scope_covers(
        grant.allowed_ports.as_ref(),
        required.allowed_ports.as_ref(),
    ) && enum_scope_covers(
        grant.allowed_methods.as_ref(),
        required.allowed_methods.as_ref(),
    ) && path_prefix_scope_covers(
        grant.allowed_path_prefixes.as_ref(),
        required.allowed_path_prefixes.as_ref(),
    ) && max_bytes_covers(grant.max_timeout_ms, required.max_timeout_ms)
        && max_bytes_covers(grant.max_response_bytes, required.max_response_bytes)
        && allow_flag_covers(grant.follow_redirects, required.follow_redirects)
        && max_redirects_covers(
            if allow_flag_enabled(grant.follow_redirects) {
                grant.max_redirects
            } else {
                None
            },
            if allow_flag_enabled(required.follow_redirects) {
                required.max_redirects
            } else {
                None
            },
        )
        && allow_flag_covers(grant.allow_loopback, required.allow_loopback)
        && allow_flag_covers(grant.allow_link_local, required.allow_link_local)
        && allow_flag_covers(
            grant.allow_private_networks,
            required.allow_private_networks,
        )
        && allow_flag_covers(grant.allow_ip_literals, required.allow_ip_literals)
}

fn emit_evidence_covers(
    grant: &EmitEvidenceConstraints,
    required: &EmitEvidenceConstraints,
) -> bool {
    max_bytes_covers(grant.max_bytes, required.max_bytes)
        && enum_scope_covers(grant.audiences.as_ref(), required.audiences.as_ref())
        && enum_scope_covers(grant.redactions.as_ref(), required.redactions.as_ref())
}

fn log_covers(grant: &LogConstraints, required: &LogConstraints) -> bool {
    enum_scope_covers(grant.levels.as_ref(), required.levels.as_ref())
}

fn string_scope_covers_exact(
    granted: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => required
            .iter()
            .all(|value| granted.iter().any(|candidate| candidate == value)),
    }
}

fn path_prefix_scope_covers(granted: Option<&Vec<String>>, required: Option<&Vec<String>>) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => required
            .iter()
            .all(|value| granted.iter().any(|prefix| value.starts_with(prefix))),
    }
}

fn resource_scope_covers(granted: Option<&Vec<String>>, required: Option<&Vec<String>>) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => {
            let Some(granted) = parse_resource_scopes(granted) else {
                return false;
            };
            let Some(required) = parse_resource_scopes(required) else {
                return false;
            };

            required
                .iter()
                .all(|required_scope| granted.contains(required_scope))
        }
    }
}

fn enum_scope_covers<T: PartialEq>(granted: Option<&Vec<T>>, required: Option<&Vec<T>>) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => required
            .iter()
            .all(|value| granted.iter().any(|candidate| candidate == value)),
    }
}

fn max_bytes_covers(granted: Option<u64>, required: Option<u64>) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => granted >= required,
    }
}

fn max_redirects_covers(granted: Option<u8>, required: Option<u8>) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => granted >= required,
    }
}

fn allow_flag_enabled(value: Option<bool>) -> bool {
    value == Some(true)
}

fn allow_flag_covers(granted: Option<bool>, required: Option<bool>) -> bool {
    !allow_flag_enabled(required) || allow_flag_enabled(granted)
}

fn reduce_optional_allow_flag(
    parent: Option<bool>,
    required: Option<bool>,
) -> Option<ReducedConstraint<bool>> {
    match required {
        Some(false) => Some(ReducedConstraint::Restricted(false)),
        Some(true) => {
            if allow_flag_enabled(parent) {
                Some(ReducedConstraint::Restricted(true))
            } else {
                None
            }
        }
        None => Some(match parent {
            Some(value) => ReducedConstraint::Restricted(value),
            None => ReducedConstraint::Unbounded,
        }),
    }
}

fn reduce_cap_allow_flag(cap: Option<bool>, grant: Option<bool>) -> Option<bool> {
    match cap {
        Some(false) => Some(false),
        Some(true) | None => grant,
    }
}

fn http_host_scope_covers(
    granted_hosts: Option<&Vec<String>>,
    granted_suffixes: Option<&Vec<String>>,
    required_hosts: Option<&Vec<String>>,
    required_suffixes: Option<&Vec<String>>,
) -> bool {
    let granted_hosts = granted_hosts.map_or_else(Vec::new, |hosts| canonicalize_host_scope(hosts));
    let granted_suffixes = granted_suffixes.map_or_else(Vec::new, |suffixes| {
        canonicalize_host_suffix_scope(suffixes)
    });

    let required_hosts_ok = required_hosts.is_none_or(|hosts| {
        canonicalize_host_scope(hosts).iter().all(|host| {
            granted_hosts.iter().any(|candidate| candidate == host)
                || (!is_ip_literal_host(host)
                    && granted_suffixes
                        .iter()
                        .any(|suffix| domain_suffix_matches(host, suffix)))
        })
    });
    let required_suffixes_ok = required_suffixes.is_none_or(|suffixes| {
        canonicalize_host_suffix_scope(suffixes)
            .iter()
            .all(|suffix| {
                granted_suffixes
                    .iter()
                    .any(|candidate| domain_suffix_matches(suffix, candidate))
            })
    });

    required_hosts_ok && required_suffixes_ok
}

enum ReducedConstraint<T> {
    Unbounded,
    Restricted(T),
}

struct ReducedHostScope {
    hosts: Option<Vec<String>>,
    suffixes: Option<Vec<String>>,
}

struct ReducedChildHttpCore {
    allowed_schemes: Option<Vec<HttpScheme>>,
    host_scope: ReducedHostScope,
    allowed_ports: Option<Vec<u16>>,
    allowed_methods: Option<Vec<HttpMethod>>,
    allowed_path_prefixes: Option<Vec<String>>,
    max_timeout_ms: Option<u64>,
    max_response_bytes: Option<u64>,
}

struct ReducedChildHttpRedirects {
    follow_redirects: Option<bool>,
    max_redirects: Option<u8>,
}

struct ReducedChildHttpDestinationFlags {
    loopback: Option<bool>,
    link_local: Option<bool>,
    private_networks: Option<bool>,
    ip_literals: Option<bool>,
}

impl<T> ReducedConstraint<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Self::Unbounded => None,
            Self::Restricted(value) => Some(value),
        }
    }
}

fn reduce_required_exact_string_scope(
    parent: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> Option<ReducedConstraint<Vec<String>>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(parent.clone())),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(required.clone())),
        (Some(parent), Some(required)) => {
            let reduced = required
                .iter()
                .filter(|candidate| parent.iter().any(|prefix| candidate.starts_with(prefix)))
                .cloned()
                .collect::<Vec<_>>();
            if reduced.is_empty() {
                None
            } else {
                Some(ReducedConstraint::Restricted(reduced))
            }
        }
    }
}

fn reduce_required_host_scope(
    parent_hosts: Option<&Vec<String>>,
    parent_suffixes: Option<&Vec<String>>,
    required_hosts: Option<&Vec<String>>,
    required_suffixes: Option<&Vec<String>>,
) -> Option<ReducedHostScope> {
    let hosts = reduce_required_http_hosts(parent_hosts, parent_suffixes, required_hosts).ok()?;
    let suffixes = reduce_required_http_host_suffixes(parent_suffixes, required_suffixes).ok()?;
    Some(ReducedHostScope { hosts, suffixes })
}

fn reduce_required_path_prefix_scope(
    parent: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> Option<ReducedConstraint<Vec<String>>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(parent.clone())),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(required.clone())),
        (Some(parent), Some(required)) => {
            let reduced = required
                .iter()
                .filter(|candidate| parent.iter().any(|prefix| candidate.starts_with(prefix)))
                .cloned()
                .collect::<Vec<_>>();
            if reduced.is_empty() {
                None
            } else {
                Some(ReducedConstraint::Restricted(reduced))
            }
        }
    }
}

fn canonicalize_host_scope(hosts: &[String]) -> Vec<String> {
    let mut canonical = Vec::with_capacity(hosts.len());

    for host in hosts {
        let host = canonicalize_http_host(host);
        if !canonical.contains(&host) {
            canonical.push(host);
        }
    }

    canonical
}

fn canonicalize_host_suffix_scope(hosts: &[String]) -> Vec<String> {
    let mut canonical = Vec::with_capacity(hosts.len());

    for host in hosts {
        let host = canonicalize_http_host_suffix(host);
        if !canonical.contains(&host) {
            canonical.push(host);
        }
    }

    canonical
}

fn reduce_required_resource_scope(
    parent: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> Option<ReducedConstraint<Vec<String>>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => {
            canonicalize_resource_scopes(parent).map(ReducedConstraint::Restricted)
        }
        (None, Some(required)) => {
            canonicalize_resource_scopes(required).map(ReducedConstraint::Restricted)
        }
        (Some(parent), Some(required)) => {
            let parent = parse_resource_scopes(parent)?;
            let required = parse_resource_scopes(required)?;
            let reduced = required
                .iter()
                .filter(|candidate| parent.contains(candidate))
                .map(|scope| scope.canonical_prefix().to_owned())
                .collect::<Vec<_>>();
            if reduced.is_empty() {
                None
            } else {
                Some(ReducedConstraint::Restricted(reduced))
            }
        }
    }
}

fn canonicalize_resource_scopes(scopes: &[String]) -> Option<Vec<String>> {
    parse_resource_scopes(scopes).map(|scopes| {
        scopes
            .into_iter()
            .map(|scope| scope.canonical_prefix().to_owned())
            .collect()
    })
}

fn parse_resource_scopes(scopes: &[String]) -> Option<Vec<GuildResourceScope>> {
    let mut parsed = Vec::with_capacity(scopes.len());

    for scope in scopes {
        let scope = GuildResourceScope::parse(scope).ok()?;
        if !parsed.contains(&scope) {
            parsed.push(scope);
        }
    }

    Some(parsed)
}

fn reduce_required_enum_scope<T: Clone + PartialEq>(
    parent: Option<&Vec<T>>,
    required: Option<&Vec<T>>,
) -> Option<ReducedConstraint<Vec<T>>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(parent.clone())),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(required.clone())),
        (Some(parent), Some(required)) => {
            if required
                .iter()
                .all(|value| parent.iter().any(|candidate| candidate == value))
            {
                Some(ReducedConstraint::Restricted(required.clone()))
            } else {
                None
            }
        }
    }
}

fn reduce_required_max_bytes(
    parent: Option<u64>,
    required: Option<u64>,
) -> Option<ReducedConstraint<u64>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(parent)),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(required)),
        (Some(parent), Some(required)) if parent >= required => {
            Some(ReducedConstraint::Restricted(required))
        }
        _ => None,
    }
}

fn reduce_required_max_redirects(
    parent: Option<u8>,
    required: Option<u8>,
) -> Option<ReducedConstraint<u8>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(parent)),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(required)),
        (Some(parent), Some(required)) if parent >= required => {
            Some(ReducedConstraint::Restricted(required))
        }
        _ => None,
    }
}

fn reduce_required_http_hosts(
    parent_hosts: Option<&Vec<String>>,
    parent_suffixes: Option<&Vec<String>>,
    required_hosts: Option<&Vec<String>>,
) -> Result<Option<Vec<String>>, ()> {
    let Some(required_hosts) = required_hosts else {
        return Ok(parent_hosts.map(|hosts| canonicalize_host_scope(hosts)));
    };

    let required_hosts = canonicalize_host_scope(required_hosts);
    if parent_hosts.is_none() && parent_suffixes.is_none() {
        return Ok(Some(required_hosts));
    }

    let parent_hosts = parent_hosts.map_or_else(Vec::new, |hosts| canonicalize_host_scope(hosts));
    let parent_suffixes = parent_suffixes.map_or_else(Vec::new, |suffixes| {
        canonicalize_host_suffix_scope(suffixes)
    });
    let reduced = required_hosts
        .into_iter()
        .filter(|candidate| {
            parent_hosts.iter().any(|host| host == candidate)
                || (!is_ip_literal_host(candidate)
                    && parent_suffixes
                        .iter()
                        .any(|suffix| domain_suffix_matches(candidate, suffix)))
        })
        .collect::<Vec<_>>();
    if reduced.is_empty() {
        Err(())
    } else {
        Ok(Some(reduced))
    }
}

fn reduce_required_http_host_suffixes(
    parent_suffixes: Option<&Vec<String>>,
    required_suffixes: Option<&Vec<String>>,
) -> Result<Option<Vec<String>>, ()> {
    let Some(required_suffixes) = required_suffixes else {
        return Ok(parent_suffixes.map(|suffixes| canonicalize_host_suffix_scope(suffixes)));
    };

    let required_suffixes = canonicalize_host_suffix_scope(required_suffixes);
    let Some(parent_suffixes) = parent_suffixes else {
        return Ok(Some(required_suffixes));
    };

    let parent_suffixes = canonicalize_host_suffix_scope(parent_suffixes);
    let reduced = required_suffixes
        .into_iter()
        .filter(|candidate| {
            parent_suffixes
                .iter()
                .any(|suffix| domain_suffix_matches(candidate, suffix))
        })
        .collect::<Vec<_>>();
    if reduced.is_empty() {
        Err(())
    } else {
        Ok(Some(reduced))
    }
}

fn reduce_child_http_constraint<T>(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
    field: &str,
    label: &str,
    reduced: Option<ReducedConstraint<T>>,
) -> Result<Option<T>, CapabilityDenial> {
    reduced
        .ok_or_else(|| child_http_constraint_mismatch(parent, required, field, label))
        .map(ReducedConstraint::into_option)
}

fn reduce_child_http_allow_flag(
    parent_constraints: &HttpRequestConstraints,
    required_constraints: &HttpRequestConstraints,
    parent: Option<bool>,
    required: Option<bool>,
    field: &str,
    label: &str,
) -> Result<Option<bool>, CapabilityDenial> {
    reduce_optional_allow_flag(parent, required)
        .ok_or_else(|| {
            child_http_constraint_mismatch(parent_constraints, required_constraints, field, label)
        })
        .map(ReducedConstraint::into_option)
}

fn child_http_constraint_mismatch(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
    field: &str,
    label: &str,
) -> CapabilityDenial {
    CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: format!("child http-request {label} could not be reduced from the parent grant"),
        detail: serde_json::json!({
            "field": field,
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    }
}

fn is_supported_wasm_inspect_capability(id: &CapabilityId, access: &CapabilityAccess) -> bool {
    matches!(
        (id, access),
        (
            CapabilityId::HttpRequest | CapabilityId::ReadResource,
            CapabilityAccess::Read
        ) | (CapabilityId::InvokeSkill, CapabilityAccess::Invoke)
            | (
                CapabilityId::EmitEvidence | CapabilityId::LogWrite,
                CapabilityAccess::Write
            )
    )
}

fn supported_wasm_inspect_capabilities() -> Vec<Value> {
    vec![
        serde_json::json!({ "id": CapabilityId::HttpRequest, "access": CapabilityAccess::Read }),
        serde_json::json!({ "id": CapabilityId::ReadResource, "access": CapabilityAccess::Read }),
        serde_json::json!({ "id": CapabilityId::InvokeSkill, "access": CapabilityAccess::Invoke }),
        serde_json::json!({ "id": CapabilityId::EmitEvidence, "access": CapabilityAccess::Write }),
        serde_json::json!({ "id": CapabilityId::LogWrite, "access": CapabilityAccess::Write }),
    ]
}

fn unsupported_surface_includes_filesystem(
    unsupported_manifest_capabilities: &[Value],
    unsupported_grants: &[Value],
) -> bool {
    unsupported_manifest_capabilities
        .iter()
        .chain(unsupported_grants.iter())
        .any(|entry| {
            entry
                .get("id")
                .is_some_and(|id| id == &serde_json::json!(CapabilityId::Filesystem))
        })
}

const CAPABILITY_DENIAL_TRAP_PREFIX: &str = "guild-capability-denial:";

fn capability_denial_trap(denial: &CapabilityDenial) -> wasmtime::Error {
    let payload = serde_json::to_string(&denial).expect("capability denial serializes");
    wasmtime::Error::msg(format!("{CAPABILITY_DENIAL_TRAP_PREFIX}{payload}"))
}

fn parse_capability_denial_payload(message: &str) -> Option<CapabilityDenial> {
    let start = message.find(CAPABILITY_DENIAL_TRAP_PREFIX)?;
    let payload = &message[start + CAPABILITY_DENIAL_TRAP_PREFIX.len()..];
    let mut stream = serde_json::Deserializer::from_str(payload).into_iter::<CapabilityDenial>();
    stream.next().and_then(Result::ok)
}

fn parse_capability_denial_trap(error: &wasmtime::Error) -> Option<CapabilityDenial> {
    parse_capability_denial_payload(&error.to_string())
        .or_else(|| parse_capability_denial_payload(&format!("{error:#}")))
        .or_else(|| {
            error
                .chain()
                .find_map(|cause| parse_capability_denial_payload(&cause.to_string()))
        })
}

fn hash_json(value: &Value) -> String {
    let mut buffer = String::new();
    write_canonical_json(value, &mut buffer);
    format!("sha256:{:x}", Sha256::digest(buffer.as_bytes()))
}

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            output.push_str(&serde_json::to_string(value).expect("primitive JSON serializes"));
        }
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(item, output);
            }
            output.push(']');
        }
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));

            output.push('{');
            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key).expect("object keys serialize as strings"),
                );
                output.push(':');
                write_canonical_json(item, output);
            }
            output.push('}');
        }
    }
}

fn to_wit_execution_context(
    context: &ExecutionContext,
) -> Result<bindings::guild::skill::types::ExecutionContext, ExecutionError> {
    Ok(bindings::guild::skill::types::ExecutionContext {
        execution_id: context.execution_id.clone(),
        trace_id: context.trace_id.clone(),
        tenant_id: context.tenant_id.clone(),
        skill: to_wit_resolved_skill_ref(&context.skill),
        mode: to_wit_execution_mode(&context.mode),
        input_sha256: context.input_sha256.clone(),
        now_utc: context.now_utc.clone(),
        budget: bindings::guild::skill::types::Budget {
            max_millis: context.budget.max_millis,
            max_memory_bytes: context.budget.max_memory_bytes,
            max_output_bytes: context.budget.max_output_bytes,
            max_network_requests: context.budget.max_network_requests,
            max_child_executions: context.budget.max_child_executions,
        },
        granted_capabilities: context
            .granted_capabilities
            .grants
            .iter()
            .map(to_wit_granted_capability)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn to_wit_resolved_skill_ref(
    skill: &ResolvedSkillRef,
) -> bindings::guild::skill::types::ResolvedSkillRef {
    bindings::guild::skill::types::ResolvedSkillRef {
        key: bindings::guild::skill::types::SkillKey {
            namespace: skill.key.namespace.clone(),
            name: skill.key.name.clone(),
        },
        version: skill.version.to_string(),
        digest: skill.digest.clone(),
    }
}

fn to_wit_granted_capability(
    grant: &GrantedCapability,
) -> Result<bindings::guild::skill::types::GrantedCapability, ExecutionError> {
    Ok(bindings::guild::skill::types::GrantedCapability {
        id: to_wit_capability_id(&grant.id)?,
        access: to_wit_capability_access(&grant.access),
        constraints: to_wit_capability_constraints(&grant.id, &grant.constraints)?,
    })
}

fn to_wit_execution_mode(mode: &ExecutionMode) -> bindings::guild::skill::types::ExecutionMode {
    match mode {
        ExecutionMode::Inspect => bindings::guild::skill::types::ExecutionMode::Inspect,
        ExecutionMode::Plan => bindings::guild::skill::types::ExecutionMode::Plan,
        ExecutionMode::Apply => bindings::guild::skill::types::ExecutionMode::Apply,
    }
}

fn to_wit_capability_id(
    id: &CapabilityId,
) -> Result<bindings::guild::skill::types::CapabilityId, ExecutionError> {
    match id {
        CapabilityId::HttpRequest => Ok(bindings::guild::skill::types::CapabilityId::HttpRequest),
        CapabilityId::ReadResource => Ok(bindings::guild::skill::types::CapabilityId::ReadResource),
        CapabilityId::InvokeSkill => Ok(bindings::guild::skill::types::CapabilityId::InvokeSkill),
        CapabilityId::EmitEvidence => Ok(bindings::guild::skill::types::CapabilityId::EmitEvidence),
        CapabilityId::GetSecret => Ok(bindings::guild::skill::types::CapabilityId::GetSecret),
        CapabilityId::CacheRead => Ok(bindings::guild::skill::types::CapabilityId::CacheRead),
        CapabilityId::CacheWrite => Ok(bindings::guild::skill::types::CapabilityId::CacheWrite),
        CapabilityId::LogWrite => Ok(bindings::guild::skill::types::CapabilityId::LogWrite),
        CapabilityId::Filesystem => Err(ExecutionError::new(
            "filesystem-runtime-not-supported",
            "filesystem capability contracts are not implemented in the active Wasm inspect slice",
        )
        .with_detail(serde_json::json!({
            "id": id,
        }))
        .with_phase(ExecutionPhase::Validation)),
        CapabilityId::MonotonicClock => {
            Ok(bindings::guild::skill::types::CapabilityId::MonotonicClock)
        }
        CapabilityId::WallClock => Ok(bindings::guild::skill::types::CapabilityId::WallClock),
    }
}

fn to_wit_capability_access(
    access: &CapabilityAccess,
) -> bindings::guild::skill::types::CapabilityAccess {
    match access {
        CapabilityAccess::Read => bindings::guild::skill::types::CapabilityAccess::Read,
        CapabilityAccess::Write => bindings::guild::skill::types::CapabilityAccess::Write,
        CapabilityAccess::Invoke => bindings::guild::skill::types::CapabilityAccess::Invoke,
    }
}

fn to_wit_capability_constraints(
    id: &CapabilityId,
    constraints: &CapabilityConstraints,
) -> Result<bindings::guild::skill::types::CapabilityConstraints, ExecutionError> {
    match constraints {
        CapabilityConstraints::None(_) => {
            Ok(bindings::guild::skill::types::CapabilityConstraints::None)
        }
        CapabilityConstraints::Filesystem(value) => Err(ExecutionError::new(
            "filesystem-runtime-not-supported",
            "filesystem capability contracts are not implemented in the active Wasm inspect slice",
        )
        .with_detail(serde_json::json!({
            "id": id,
            "constraints": value,
        }))
        .with_phase(ExecutionPhase::Validation)),
        CapabilityConstraints::HttpRequest(value) => Ok(
            bindings::guild::skill::types::CapabilityConstraints::HttpRequest(
                bindings::guild::skill::types::HttpRequestConstraints {
                    allowed_schemes: value
                        .allowed_schemes
                        .as_ref()
                        .map(|schemes| schemes.iter().map(to_wit_http_scheme).collect()),
                    allowed_hosts: value
                        .allowed_hosts
                        .as_ref()
                        .map(|hosts| canonicalize_host_scope(hosts)),
                    allowed_ports: value.allowed_ports.clone(),
                    allowed_methods: value
                        .allowed_methods
                        .as_ref()
                        .map(|methods| methods.iter().map(to_wit_http_method).collect()),
                    allowed_path_prefixes: value.allowed_path_prefixes.clone(),
                    max_timeout_ms: value.max_timeout_ms,
                    max_response_bytes: value.max_response_bytes,
                },
            ),
        ),
        CapabilityConstraints::ReadResource(value) => Ok(
            bindings::guild::skill::types::CapabilityConstraints::ReadResource(
                bindings::guild::skill::types::ReadResourceConstraints {
                    uri_prefixes: value.uri_prefixes.clone(),
                    resource_kinds: value
                        .resource_kinds
                        .as_ref()
                        .map(|kinds| kinds.iter().map(to_wit_resource_kind).collect()),
                },
            ),
        ),
        CapabilityConstraints::InvokeDependency(value) => Ok(
            bindings::guild::skill::types::CapabilityConstraints::InvokeDependency(
                bindings::guild::skill::types::InvokeDependencyConstraints {
                    aliases: value.aliases.clone(),
                },
            ),
        ),
        CapabilityConstraints::EmitEvidence(value) => {
            Ok(
                bindings::guild::skill::types::CapabilityConstraints::EmitEvidence(
                    bindings::guild::skill::types::EmitEvidenceConstraints {
                        max_bytes: value.max_bytes,
                        audiences: value.audiences.as_ref().map(|audiences| {
                            audiences.iter().map(to_wit_evidence_audience).collect()
                        }),
                        redactions: value.redactions.as_ref().map(|redactions| {
                            redactions.iter().map(to_wit_redaction_class).collect()
                        }),
                    },
                ),
            )
        }
        CapabilityConstraints::Log(value) => {
            Ok(bindings::guild::skill::types::CapabilityConstraints::Log(
                bindings::guild::skill::types::LogConstraints {
                    levels: value
                        .levels
                        .as_ref()
                        .map(|levels| levels.iter().map(to_wit_severity).collect()),
                },
            ))
        }
    }
}

fn from_wit_skill_output(
    output: bindings::guild::skill::types::SkillOutput,
) -> Result<SkillOutput, ExecutionError> {
    Ok(SkillOutput {
        summary: output.summary,
        structured: parse_json_payload("structured", &output.structured)?,
        diagnostics: output
            .diagnostics
            .into_iter()
            .map(from_wit_diagnostic)
            .collect::<Result<Vec<_>, _>>()?,
        effects: output.effects.into_iter().map(from_wit_effect).collect(),
        evidence: output.evidence.into_iter().map(from_wit_evidence).collect(),
    })
}

fn to_wit_skill_output(output: &SkillOutput) -> bindings::guild::skill::types::SkillOutput {
    bindings::guild::skill::types::SkillOutput {
        summary: output.summary.clone(),
        structured: serde_json::to_string(&output.structured)
            .expect("structured output serializes"),
        diagnostics: output.diagnostics.iter().map(to_wit_diagnostic).collect(),
        effects: output.effects.iter().map(to_wit_effect).collect(),
        evidence: output.evidence.iter().map(to_wit_evidence).collect(),
    }
}

fn to_wit_skill_error(error: &SkillError) -> bindings::guild::skill::types::SkillError {
    bindings::guild::skill::types::SkillError {
        code: error.code.clone(),
        message: error.message.clone(),
        retryable: error.retryable,
        detail: error
            .detail
            .as_ref()
            .map(|detail| serde_json::to_string(detail).expect("skill error detail serializes")),
    }
}

fn from_wit_http_request(
    request: bindings::guild::skill::types::HttpRequestMessage,
) -> HttpRequest {
    HttpRequest {
        method: from_wit_http_method(request.method),
        url: request.url,
        timeout_ms: request.timeout_ms,
    }
}

fn to_wit_http_response(
    response: &HttpResponse,
) -> bindings::guild::skill::types::HttpResponseMessage {
    bindings::guild::skill::types::HttpResponseMessage {
        url: response.url.clone(),
        status: response.status,
        content_type: response.content_type.clone(),
        body: response.body.clone(),
    }
}

fn from_wit_http_method(method: bindings::guild::skill::types::HttpMethod) -> HttpMethod {
    match method {
        bindings::guild::skill::types::HttpMethod::Get => HttpMethod::Get,
        bindings::guild::skill::types::HttpMethod::Head => HttpMethod::Head,
    }
}

fn to_wit_http_method(method: &HttpMethod) -> bindings::guild::skill::types::HttpMethod {
    match method {
        HttpMethod::Get => bindings::guild::skill::types::HttpMethod::Get,
        HttpMethod::Head => bindings::guild::skill::types::HttpMethod::Head,
    }
}

fn to_wit_http_scheme(scheme: &HttpScheme) -> bindings::guild::skill::types::HttpScheme {
    match scheme {
        HttpScheme::Http => bindings::guild::skill::types::HttpScheme::Http,
        HttpScheme::Https => bindings::guild::skill::types::HttpScheme::Https,
    }
}

fn validate_emitted_evidence(
    output: &SkillOutput,
    emitted_evidence: &[EvidenceRef],
) -> Result<(), ExecutionError> {
    if output.evidence == emitted_evidence {
        return Ok(());
    }

    Err(ExecutionError::new(
        "invalid-evidence-output",
        "skill output evidence did not match the host-issued evidence refs emitted during execution",
    )
    .with_detail(serde_json::json!({
        "expected": emitted_evidence,
        "actual": output.evidence,
    }))
    .with_phase(ExecutionPhase::RuntimeExec))
}

fn from_wit_skill_error(error: bindings::guild::skill::types::SkillError) -> ExecutionError {
    let phase = phase_for_skill_error_code(&error.code);
    ExecutionError {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
        phase: Some(phase),
        detail: error
            .detail
            .and_then(|payload| {
                serde_json::from_str(&payload)
                    .ok()
                    .or(Some(Value::String(payload)))
            })
            .map(Box::new),
        receipt: None,
    }
}

fn from_wit_severity(severity: bindings::guild::skill::types::Severity) -> Severity {
    match severity {
        bindings::guild::skill::types::Severity::Info => Severity::Info,
        bindings::guild::skill::types::Severity::Warn => Severity::Warn,
        bindings::guild::skill::types::Severity::Error => Severity::Error,
    }
}

fn phase_for_skill_error_code(code: &str) -> ExecutionPhase {
    if code.starts_with("dependency-") || code.starts_with("child-") {
        ExecutionPhase::ChildInvocation
    } else {
        ExecutionPhase::SkillDomain
    }
}

fn to_wit_diagnostic(diagnostic: &Diagnostic) -> bindings::guild::skill::types::Diagnostic {
    bindings::guild::skill::types::Diagnostic {
        severity: to_wit_severity(&diagnostic.severity),
        code: diagnostic.code.clone(),
        message: diagnostic.message.clone(),
        retryable: diagnostic.retryable,
        detail: diagnostic
            .detail
            .as_ref()
            .map(|detail| serde_json::to_string(detail).expect("diagnostic detail serializes")),
    }
}

fn from_wit_diagnostic(
    diagnostic: bindings::guild::skill::types::Diagnostic,
) -> Result<Diagnostic, ExecutionError> {
    Ok(Diagnostic {
        severity: from_wit_severity(diagnostic.severity),
        code: diagnostic.code,
        message: diagnostic.message,
        retryable: diagnostic.retryable,
        detail: diagnostic
            .detail
            .map(|payload| parse_json_payload("diagnostic.detail", &payload))
            .transpose()?,
    })
}

fn from_wit_effect(effect: bindings::guild::skill::types::Effect) -> Effect {
    Effect {
        kind: match effect.kind {
            bindings::guild::skill::types::Mutability::ReadOnly => Mutability::ReadOnly,
            bindings::guild::skill::types::Mutability::Additive => Mutability::Additive,
            bindings::guild::skill::types::Mutability::Destructive => Mutability::Destructive,
        },
        target: effect.target,
        summary: effect.summary,
    }
}

fn to_wit_effect(effect: &Effect) -> bindings::guild::skill::types::Effect {
    bindings::guild::skill::types::Effect {
        kind: match effect.kind {
            Mutability::ReadOnly => bindings::guild::skill::types::Mutability::ReadOnly,
            Mutability::Additive => bindings::guild::skill::types::Mutability::Additive,
            Mutability::Destructive => bindings::guild::skill::types::Mutability::Destructive,
        },
        target: effect.target.clone(),
        summary: effect.summary.clone(),
    }
}

fn from_wit_evidence(evidence: bindings::guild::skill::types::EvidenceRef) -> EvidenceRef {
    EvidenceRef {
        uri: evidence.uri,
        title: evidence.title,
        mime_type: evidence.mime_type,
        sha256: evidence.sha256,
        audience: match evidence.audience {
            bindings::guild::skill::types::EvidenceAudience::User => EvidenceAudience::User,
            bindings::guild::skill::types::EvidenceAudience::Assistant => {
                EvidenceAudience::Assistant
            }
            bindings::guild::skill::types::EvidenceAudience::Internal => EvidenceAudience::Internal,
        },
        redaction: match evidence.redaction {
            bindings::guild::skill::types::RedactionClass::None => RedactionClass::None,
            bindings::guild::skill::types::RedactionClass::SecretsRemoved => {
                RedactionClass::SecretsRemoved
            }
            bindings::guild::skill::types::RedactionClass::PiiRemoved => RedactionClass::PiiRemoved,
            bindings::guild::skill::types::RedactionClass::TenantSensitive => {
                RedactionClass::TenantSensitive
            }
        },
        freshness: evidence.freshness,
    }
}

fn to_wit_evidence(evidence: &EvidenceRef) -> bindings::guild::skill::types::EvidenceRef {
    bindings::guild::skill::types::EvidenceRef {
        uri: evidence.uri.clone(),
        mime_type: evidence.mime_type.clone(),
        sha256: evidence.sha256.clone(),
        title: evidence.title.clone(),
        audience: to_wit_evidence_audience(&evidence.audience),
        redaction: to_wit_redaction_class(&evidence.redaction),
        freshness: evidence.freshness.clone(),
    }
}

fn to_wit_resource_kind(kind: &ResourceKind) -> bindings::guild::skill::types::ResourceKind {
    match kind {
        ResourceKind::Execution => bindings::guild::skill::types::ResourceKind::Execution,
        ResourceKind::Object => bindings::guild::skill::types::ResourceKind::Object,
        ResourceKind::Query => bindings::guild::skill::types::ResourceKind::Query,
    }
}

fn to_wit_evidence_audience(
    audience: &EvidenceAudience,
) -> bindings::guild::skill::types::EvidenceAudience {
    match audience {
        EvidenceAudience::User => bindings::guild::skill::types::EvidenceAudience::User,
        EvidenceAudience::Assistant => bindings::guild::skill::types::EvidenceAudience::Assistant,
        EvidenceAudience::Internal => bindings::guild::skill::types::EvidenceAudience::Internal,
    }
}

fn to_wit_redaction_class(
    redaction: &RedactionClass,
) -> bindings::guild::skill::types::RedactionClass {
    match redaction {
        RedactionClass::None => bindings::guild::skill::types::RedactionClass::None,
        RedactionClass::SecretsRemoved => {
            bindings::guild::skill::types::RedactionClass::SecretsRemoved
        }
        RedactionClass::PiiRemoved => bindings::guild::skill::types::RedactionClass::PiiRemoved,
        RedactionClass::TenantSensitive => {
            bindings::guild::skill::types::RedactionClass::TenantSensitive
        }
    }
}

fn to_wit_severity(severity: &Severity) -> bindings::guild::skill::types::Severity {
    match severity {
        Severity::Info => bindings::guild::skill::types::Severity::Info,
        Severity::Warn => bindings::guild::skill::types::Severity::Warn,
        Severity::Error => bindings::guild::skill::types::Severity::Error,
    }
}

fn to_wit_resource_read_result(
    result: &ResourceReadResult,
) -> bindings::guild::skill::types::ResourceReadResult {
    bindings::guild::skill::types::ResourceReadResult {
        uri: result.uri.clone(),
        mime_type: result.mime_type.clone(),
        bytes: result.bytes.clone(),
        sha256: result.sha256.clone(),
    }
}

fn parse_json_payload(field: &str, payload: &str) -> Result<Value, ExecutionError> {
    serde_json::from_str(payload).map_err(|error| {
        ExecutionError::new(
            "invalid-json-payload",
            format!("component returned invalid JSON for `{field}`"),
        )
        .with_detail(error.to_string())
        .with_phase(ExecutionPhase::RuntimeExec)
    })
}

fn resource_kind_label(kind: &ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Execution => "execution",
        ResourceKind::Object => "object",
        ResourceKind::Query => "query",
    }
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn http_request_not_granted_denial(request: &HttpRequest) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-not-granted".into(),
        message: "http-request was not granted for this execution".into(),
        detail: serde_json::json!({
            "url": request.url,
            "method": request.method,
        }),
    }
}

fn http_request_budget_denial(
    request: &HttpRequest,
    used_network_requests: u32,
    max_network_requests: u32,
) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-budget-exhausted".into(),
        message: "execution budget does not allow additional outbound HTTP requests".into(),
        detail: serde_json::json!({
            "url": request.url,
            "used_network_requests": used_network_requests,
            "max_network_requests": max_network_requests,
        }),
    }
}

fn authorize_unconstrained_http_grant(
    state: &mut HttpGrantState,
    request: &HttpRequest,
    budget: &guild_types::Budget,
) {
    state.authorize(
        effective_timeout_ms(request.timeout_ms, None, budget.max_millis),
        effective_response_bytes(None, budget.max_output_bytes),
        true,
        Some(HTTP_UNCONSTRAINED_MAX_REDIRECTS),
    );
}

fn evaluate_http_request_constraints(
    state: &mut HttpGrantState,
    constraints: &HttpRequestConstraints,
    request: &HttpRequest,
    parsed_request: &ParsedHttpRequest,
    budget: &guild_types::Budget,
) {
    if !matches_http_method(constraints.allowed_methods.as_ref(), &request.method) {
        state.note_denial(HttpGrantDenialKind::Method);
        return;
    }

    if !matches_http_scheme(constraints.allowed_schemes.as_ref(), &parsed_request.scheme) {
        state.note_denial(HttpGrantDenialKind::Scheme);
        return;
    }

    if !matches_http_host(
        constraints.allowed_hosts.as_ref(),
        constraints.allowed_host_suffixes.as_ref(),
        &parsed_request.host,
        parsed_request.ip_literal().is_some(),
    ) {
        state.note_denial(HttpGrantDenialKind::Host);
        return;
    }

    if !matches_http_port(constraints.allowed_ports.as_ref(), parsed_request.port) {
        state.note_denial(HttpGrantDenialKind::Port);
        return;
    }

    if !matches_http_path(
        constraints.allowed_path_prefixes.as_ref(),
        &parsed_request.path,
    ) {
        state.note_denial(HttpGrantDenialKind::Path);
        return;
    }

    if request.timeout_ms.is_some_and(|request_timeout_ms| {
        constraints
            .max_timeout_ms
            .is_some_and(|grant_timeout_ms| request_timeout_ms > grant_timeout_ms)
    }) {
        state.note_denial(HttpGrantDenialKind::Timeout);
        return;
    }

    if parsed_request.ip_literal().is_some() && !allow_flag_enabled(constraints.allow_ip_literals) {
        state.note_denial(HttpGrantDenialKind::IpLiteral);
        return;
    }

    if parsed_request.is_loopback_name() && !allow_flag_enabled(constraints.allow_loopback) {
        state.note_denial(HttpGrantDenialKind::Loopback);
        return;
    }

    match authorize_http_destination(parsed_request, constraints) {
        Ok(()) => {}
        Err(kind) => {
            state.note_denial(kind);
            return;
        }
    }

    state.authorize(
        effective_timeout_ms(
            request.timeout_ms,
            constraints.max_timeout_ms,
            budget.max_millis,
        ),
        effective_response_bytes(constraints.max_response_bytes, budget.max_output_bytes),
        allow_flag_enabled(constraints.follow_redirects),
        if allow_flag_enabled(constraints.follow_redirects) {
            constraints.max_redirects
        } else {
            None
        },
    );
}

fn finalize_http_request_authorization(
    state: HttpGrantState,
    request: &HttpRequest,
    parsed_request: &ParsedHttpRequest,
) -> Result<HttpExecutionPolicy, CapabilityDenial> {
    match (state.authorized_timeout_ms, state.authorized_response_bytes) {
        (Some(timeout_ms), Some(max_response_bytes)) => Ok(HttpExecutionPolicy {
            timeout: Duration::from_millis(timeout_ms),
            max_response_bytes,
            follow_redirects: state.authorized_follow_redirects,
            max_redirects: state.authorized_max_redirects.unwrap_or(0),
        }),
        _ if state.saw_denial(HTTP_DENIAL_TIMEOUT) => Err(CapabilityDenial {
            code: "http-request-timeout-not-granted".into(),
            message: "http-request timeout exceeded the granted max_timeout_ms limit".into(),
            detail: serde_json::json!({
                "url": request.url,
                "requested_timeout_ms": request.timeout_ms,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_PRIVATE_NETWORK) => Err(CapabilityDenial {
            code: "http-request-private-network-not-granted".into(),
            message: "http-request destination resolved to a private-network target that was not granted".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_LINK_LOCAL) => Err(CapabilityDenial {
            code: "http-request-link-local-not-granted".into(),
            message: "http-request destination resolved to a link-local target that was not granted".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_LOOPBACK) => Err(CapabilityDenial {
            code: "http-request-loopback-not-granted".into(),
            message: "http-request destination resolved to a loopback target that was not granted".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_IP_LITERAL) => Err(CapabilityDenial {
            code: "http-request-ip-literal-not-granted".into(),
            message: "http-request IP-literal destinations were not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_DESTINATION_UNRESOLVED) => Err(CapabilityDenial {
            code: "http-request-destination-unresolved".into(),
            message: "http-request destination could not be resolved safely under the current host policy".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
                "port": parsed_request.port,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_PATH) => Err(CapabilityDenial {
            code: "http-request-path-not-granted".into(),
            message: "http-request path was not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "path": parsed_request.path,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_PORT) => Err(CapabilityDenial {
            code: "http-request-port-not-granted".into(),
            message: "http-request port was not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "port": parsed_request.port,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_HOST) => Err(CapabilityDenial {
            code: "http-request-host-not-granted".into(),
            message: "http-request host was not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "host": parsed_request.host,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_SCHEME) => Err(CapabilityDenial {
            code: "http-request-scheme-not-granted".into(),
            message: "http-request scheme was not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "scheme": parsed_request.scheme,
            }),
        }),
        _ if state.saw_denial(HTTP_DENIAL_METHOD) => Err(CapabilityDenial {
            code: "http-request-method-not-granted".into(),
            message: "http-request method was not granted for this execution".into(),
            detail: serde_json::json!({
                "url": request.url,
                "method": request.method,
            }),
        }),
        _ => Err(http_request_not_granted_denial(request)),
    }
}

fn authorize_http_destination(
    parsed_request: &ParsedHttpRequest,
    constraints: &HttpRequestConstraints,
) -> Result<(), HttpGrantDenialKind> {
    if parsed_request.ip_literal().is_none()
        && allow_flag_enabled(constraints.allow_loopback)
        && allow_flag_enabled(constraints.allow_link_local)
        && allow_flag_enabled(constraints.allow_private_networks)
    {
        return Ok(());
    }

    let addresses = resolve_http_destination_addresses(parsed_request)?;
    if addresses.iter().any(|address| {
        matches!(
            classify_destination_ip(*address),
            HttpDestinationClass::Loopback
        )
    }) && !allow_flag_enabled(constraints.allow_loopback)
    {
        return Err(HttpGrantDenialKind::Loopback);
    }
    if addresses.iter().any(|address| {
        matches!(
            classify_destination_ip(*address),
            HttpDestinationClass::LinkLocal
        )
    }) && !allow_flag_enabled(constraints.allow_link_local)
    {
        return Err(HttpGrantDenialKind::LinkLocal);
    }
    if addresses.iter().any(|address| {
        matches!(
            classify_destination_ip(*address),
            HttpDestinationClass::PrivateNetwork
        )
    }) && !allow_flag_enabled(constraints.allow_private_networks)
    {
        return Err(HttpGrantDenialKind::PrivateNetwork);
    }

    Ok(())
}

fn resolve_http_destination_addresses(
    parsed_request: &ParsedHttpRequest,
) -> Result<Vec<IpAddr>, HttpGrantDenialKind> {
    if let Some(ip) = parsed_request.ip_literal() {
        return Ok(vec![ip]);
    }

    let resolved = (parsed_request.host.as_str(), parsed_request.port)
        .to_socket_addrs()
        .map_err(|_| HttpGrantDenialKind::DestinationUnresolved)?
        .map(|address: SocketAddr| address.ip())
        .collect::<Vec<_>>();
    if resolved.is_empty() {
        return Err(HttpGrantDenialKind::DestinationUnresolved);
    }

    Ok(resolved)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpDestinationClass {
    Loopback,
    LinkLocal,
    PrivateNetwork,
    Other,
}

fn classify_destination_ip(address: IpAddr) -> HttpDestinationClass {
    match address {
        IpAddr::V4(address) if address.is_loopback() => HttpDestinationClass::Loopback,
        IpAddr::V6(address) if address.is_loopback() => HttpDestinationClass::Loopback,
        IpAddr::V4(address) if address.is_link_local() => HttpDestinationClass::LinkLocal,
        IpAddr::V6(address) if address.is_unicast_link_local() => HttpDestinationClass::LinkLocal,
        IpAddr::V4(address) if address.is_private() => HttpDestinationClass::PrivateNetwork,
        IpAddr::V6(address) if address.is_unique_local() => HttpDestinationClass::PrivateNetwork,
        _ => HttpDestinationClass::Other,
    }
}

fn matches_http_method(allowed: Option<&Vec<HttpMethod>>, method: &HttpMethod) -> bool {
    allowed.is_none_or(|methods| methods.iter().any(|candidate| candidate == method))
}

fn matches_http_scheme(allowed: Option<&Vec<HttpScheme>>, scheme: &HttpScheme) -> bool {
    allowed.is_none_or(|schemes| schemes.iter().any(|candidate| candidate == scheme))
}

fn matches_http_host(
    allowed_hosts: Option<&Vec<String>>,
    allowed_suffixes: Option<&Vec<String>>,
    host: &str,
    is_ip_literal: bool,
) -> bool {
    if allowed_hosts.is_none() && allowed_suffixes.is_none() {
        return true;
    }

    let canonical_host = canonicalize_http_host(host);
    let exact_allowed = allowed_hosts.is_some_and(|hosts| {
        canonicalize_host_scope(hosts)
            .iter()
            .any(|candidate| candidate == &canonical_host)
    });
    let suffix_allowed = !is_ip_literal
        && allowed_suffixes.is_some_and(|suffixes| {
            canonicalize_host_suffix_scope(suffixes)
                .iter()
                .any(|suffix| domain_suffix_matches(&canonical_host, suffix))
        });

    exact_allowed || suffix_allowed
}

fn matches_http_port(allowed: Option<&Vec<u16>>, port: u16) -> bool {
    allowed.is_none_or(|ports| ports.contains(&port))
}

fn matches_http_path(allowed: Option<&Vec<String>>, path: &str) -> bool {
    allowed.is_none_or(|prefixes| prefixes.iter().any(|prefix| path.starts_with(prefix)))
}

fn canonicalize_http_host(host: &str) -> String {
    host.parse::<IpAddr>()
        .map_or_else(|_| host.to_ascii_lowercase(), |address| address.to_string())
}

fn canonicalize_http_host_suffix(host: &str) -> String {
    host.to_ascii_lowercase()
}

fn is_ip_literal_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

fn domain_suffix_matches(host: &str, suffix: &str) -> bool {
    host == suffix
        || host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn redirect_not_allowed_denial(from_url: &str, status: u16, location: &str) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-redirect-not-allowed".into(),
        message: "http-request received a redirect but follow_redirects was not granted".into(),
        detail: serde_json::json!({
            "url": from_url,
            "status": status,
            "location": location,
        }),
    }
}

fn redirect_hop_limit_denial(
    from_url: &str,
    status: u16,
    location: &str,
    max_redirects: u8,
) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-redirect-hop-limit-exceeded".into(),
        message: "http-request redirect chain exceeded the granted max_redirects limit".into(),
        detail: serde_json::json!({
            "url": from_url,
            "status": status,
            "location": location,
            "max_redirects": max_redirects,
        }),
    }
}

fn redirect_location_missing_denial(from_url: &str, status: u16) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-redirect-location-invalid".into(),
        message: "http-request redirect response did not include a valid Location header".into(),
        detail: serde_json::json!({
            "url": from_url,
            "status": status,
        }),
    }
}

fn redirect_location_invalid_denial(
    from_url: &str,
    status: u16,
    location: &str,
    cause: &CapabilityDenial,
) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-redirect-location-invalid".into(),
        message: "http-request redirect location was invalid".into(),
        detail: serde_json::json!({
            "url": from_url,
            "status": status,
            "location": location,
            "cause": {
                "code": cause.code,
                "message": cause.message,
                "detail": cause.detail,
            }
        }),
    }
}

fn redirect_target_not_granted_denial(
    from_url: &str,
    status: u16,
    location: &str,
    redirected_url: &str,
    cause: &CapabilityDenial,
) -> CapabilityDenial {
    CapabilityDenial {
        code: "http-request-redirect-target-not-granted".into(),
        message: "http-request redirect target was outside the granted HTTP authority".into(),
        detail: serde_json::json!({
            "url": from_url,
            "status": status,
            "location": location,
            "redirected_url": redirected_url,
            "cause": {
                "code": cause.code,
                "message": cause.message,
                "detail": cause.detail,
            }
        }),
    }
}

fn effective_timeout_ms(
    request_timeout_ms: Option<u64>,
    grant_timeout_ms: Option<u64>,
    budget_timeout_ms: u64,
) -> u64 {
    request_timeout_ms
        .unwrap_or(u64::MAX)
        .min(grant_timeout_ms.unwrap_or(u64::MAX))
        .min(budget_timeout_ms)
}

fn effective_response_bytes(
    grant_max_response_bytes: Option<u64>,
    budget_max_output_bytes: u64,
) -> u64 {
    grant_max_response_bytes
        .unwrap_or(u64::MAX)
        .min(budget_max_output_bytes)
}
