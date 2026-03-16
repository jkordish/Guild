#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Execution boundary and runtime abstraction for Guild.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use guild_manifest::SkillManifest;
use guild_registry::{execution_resource_uri, InstalledSkill, RegistryError, SkillRegistry};
use guild_sdk_rust::GuildSkill;
use guild_types::{
    host_now_utc, mint_host_execution_id, CapabilityAccess, CapabilityConstraints,
    CapabilityGrantSet, CapabilityId, CapabilityRequirement, ChildExecutionRecord, Diagnostic,
    Effect, EmitEvidenceConstraints, EvidenceAudience, EvidenceEmissionRequest, EvidenceRecord,
    EvidenceRef, ExecutionContext, ExecutionMetrics, ExecutionMode, ExecutionPhase,
    ExecutionReceipt, ExecutionRecord, ExecutionStatus, GrantedCapability, GuildResourceScope,
    GuildResourceUri, HttpMethod, HttpRequest, HttpRequestConstraints, HttpResponse, HttpScheme,
    InvokeDependencyConstraints, LogConstraints, Mutability, PolicyDecision, PolicyDecisionOutcome,
    Provenance, ReadResourceConstraints, RedactionClass, ResolvedExecutionEnvelope,
    ResolvedSkillRef, ResourceKind, ResourceReadResult, RuntimeKind, Severity, SkillError,
    SkillOutput, TerminationDetail,
};
use http::header::CONTENT_TYPE;
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
    ) -> Result<HttpResponse, SkillError>;
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
}

#[derive(Debug, Clone, Copy)]
struct HttpExecutionPolicy {
    timeout: Duration,
    max_response_bytes: u64,
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
        let (mut store, instance) =
            self.instantiate(installed, context, host)
                .map_err(|error| RuntimeFailure {
                    error: Box::new(error),
                    emitted_evidence: Vec::new(),
                    child_executions: Vec::new(),
                    network_requests: 0,
                })?;
        let wit_context = to_wit_execution_context(context);
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
        let request = from_wit_http_request(request);
        let parsed_request =
            parse_http_request(&request).map_err(|denial| capability_denial_trap(&denial))?;
        let policy = CapabilityEvaluator::authorize_http_request(
            self.grants(),
            &self.execution.budget,
            self.network_requests,
            &request,
            &parsed_request,
        )
        .map_err(|denial| capability_denial_trap(&denial))?;

        self.network_requests = self.network_requests.saturating_add(1);
        match self
            .host
            .http_request(&request, policy.timeout, policy.max_response_bytes)
        {
            Ok(response) => Ok(Ok(to_wit_http_response(&response))),
            Err(error) => Ok(Err(format!("{}: {}", error.code, error.message))),
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

        Err(ExecutionError::new(
            "unsupported-runtime-surface",
            "Wasm inspect execution only supports the active capability allowlist",
        )
        .with_detail(serde_json::json!({
            "runtime_kind": self.runtime.kind(),
            "supported_capabilities": supported_wasm_inspect_capabilities(),
            "unsupported_manifest_capabilities": unsupported_manifest_capabilities,
            "unsupported_grants": unsupported_grants,
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
        match status_from_error(error) {
            ExecutionStatus::Rejected => PolicyDecision {
                outcome: PolicyDecisionOutcome::Rejected,
                summary: error.message.clone(),
                detail: error.detail.as_deref().cloned(),
            },
            _ => envelope.policy_decision.clone(),
        }
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
        let child_request = build_child_request(
            context,
            sequence,
            alias,
            input,
            &child_installed,
            child_grants,
        );

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
    ) -> Result<HttpResponse, SkillError> {
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

fn build_child_request(
    context: &ExecutionContext,
    sequence: u16,
    alias: &str,
    input: &Value,
    child_installed: &InstalledSkill,
    child_grants: CapabilityGrantSet,
) -> ResolvedExecutionEnvelope {
    ResolvedExecutionEnvelope {
        request: guild_types::CallerRequest {
            request_id: format!("{}:child:{sequence}", context.execution_id),
            skill: exact_requested_skill_ref(&child_installed.resolved_ref),
            tenant_id: context.tenant_id.clone(),
            actor_id: "skill".into(),
            mode: context.mode.clone(),
            input: input.clone(),
            budget: derive_child_budget(&context.budget),
            requested_capabilities: child_grants.clone(),
            idempotency_key: None,
            trace_id: context.trace_id.clone(),
        },
        resolved_skill: child_installed.resolved_ref.clone(),
        granted_capabilities: child_grants,
        policy_decision: PolicyDecision {
            outcome: PolicyDecisionOutcome::Allowed,
            summary: "dependency invocation allowed".into(),
            detail: Some(serde_json::json!({ "alias": alias })),
        },
        parent_execution_id: Some(context.execution_id.clone()),
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

    let host = url.host_str().ok_or_else(|| CapabilityDenial {
        code: "http-request-url-invalid".into(),
        message: "http-request URL must include a host".into(),
        detail: serde_json::json!({
            "url": request.url,
        }),
    })?;
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
        host: host.to_ascii_lowercase(),
        port,
        path,
    })
}

fn execute_http_request(
    request: &HttpRequest,
    timeout: Duration,
    max_response_bytes: u64,
) -> Result<HttpResponse, SkillError> {
    let parsed_request = parse_http_request(request).map_err(CapabilityDenial::into_skill_error)?;
    let http_request = Request::builder()
        .method(http_method(request.method.clone()))
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
    .map_err(|error| skill_error_from_wasi_http_error(error, request, max_response_bytes))?;

    let between_bytes_timeout = response.between_bytes_timeout;
    let worker = response.worker;
    let resp = response.resp;
    let status = resp.status().as_u16();
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
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
    .map_err(|error| skill_error_from_wasi_http_error(error, request, max_response_bytes))?;

    Ok(HttpResponse {
        url: request.url.clone(),
        status,
        content_type,
        body,
    })
}

fn empty_http_body() -> HyperOutgoingBody {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed_unsync()
}

fn http_method(method: HttpMethod) -> http::Method {
    match method {
        HttpMethod::Get => http::Method::GET,
        HttpMethod::Head => http::Method::HEAD,
    }
}

fn skill_error_from_wasi_http_error(
    error: WasiHttpErrorCode,
    request: &HttpRequest,
    max_response_bytes: u64,
) -> SkillError {
    let (code, message, retryable) = match &error {
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
            return Err(CapabilityDenial {
                code: "http-request-not-granted".into(),
                message: "http-request was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "method": request.method,
                }),
            });
        }

        if used_network_requests >= budget.max_network_requests {
            return Err(CapabilityDenial {
                code: "http-request-budget-exhausted".into(),
                message: "execution budget does not allow additional outbound HTTP requests".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "used_network_requests": used_network_requests,
                    "max_network_requests": budget.max_network_requests,
                }),
            });
        }

        let mut saw_method_denial = false;
        let mut saw_scheme_denial = false;
        let mut saw_host_denial = false;
        let mut saw_port_denial = false;
        let mut saw_path_denial = false;
        let mut saw_timeout_denial = false;
        let mut authorized_timeout_ms: Option<u64> = None;
        let mut authorized_response_bytes: Option<u64> = None;

        for grant in matching {
            let CapabilityConstraints::HttpRequest(constraints) = &grant.constraints else {
                if matches!(grant.constraints, CapabilityConstraints::None(_)) {
                    let timeout_ms =
                        effective_timeout_ms(request.timeout_ms, None, budget.max_millis);
                    let response_bytes = effective_response_bytes(None, budget.max_output_bytes);
                    authorized_timeout_ms = Some(
                        authorized_timeout_ms.map_or(timeout_ms, |current| current.max(timeout_ms)),
                    );
                    authorized_response_bytes = Some(
                        authorized_response_bytes
                            .map_or(response_bytes, |current| current.max(response_bytes)),
                    );
                    continue;
                }
                continue;
            };

            if !matches_http_method(constraints.allowed_methods.as_ref(), &request.method) {
                saw_method_denial = true;
                continue;
            }

            if !matches_http_scheme(constraints.allowed_schemes.as_ref(), &parsed_request.scheme) {
                saw_scheme_denial = true;
                continue;
            }

            if !matches_http_host(constraints.allowed_hosts.as_ref(), &parsed_request.host) {
                saw_host_denial = true;
                continue;
            }

            if !matches_http_port(constraints.allowed_ports.as_ref(), parsed_request.port) {
                saw_port_denial = true;
                continue;
            }

            if !matches_http_path(
                constraints.allowed_path_prefixes.as_ref(),
                &parsed_request.path,
            ) {
                saw_path_denial = true;
                continue;
            }

            if let Some(request_timeout_ms) = request.timeout_ms {
                if let Some(grant_timeout_ms) = constraints.max_timeout_ms {
                    if request_timeout_ms > grant_timeout_ms {
                        saw_timeout_denial = true;
                        continue;
                    }
                }
            }

            let timeout_ms = effective_timeout_ms(
                request.timeout_ms,
                constraints.max_timeout_ms,
                budget.max_millis,
            );
            let response_bytes =
                effective_response_bytes(constraints.max_response_bytes, budget.max_output_bytes);
            authorized_timeout_ms =
                Some(authorized_timeout_ms.map_or(timeout_ms, |current| current.max(timeout_ms)));
            authorized_response_bytes = Some(
                authorized_response_bytes
                    .map_or(response_bytes, |current| current.max(response_bytes)),
            );
        }

        match (authorized_timeout_ms, authorized_response_bytes) {
            (Some(timeout_ms), Some(max_response_bytes)) => Ok(HttpExecutionPolicy {
                timeout: Duration::from_millis(timeout_ms),
                max_response_bytes,
            }),
            _ if saw_timeout_denial => Err(CapabilityDenial {
                code: "http-request-timeout-not-granted".into(),
                message: "http-request timeout exceeded the granted max_timeout_ms limit".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "requested_timeout_ms": request.timeout_ms,
                }),
            }),
            _ if saw_path_denial => Err(CapabilityDenial {
                code: "http-request-path-not-granted".into(),
                message: "http-request path was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "path": parsed_request.path,
                }),
            }),
            _ if saw_port_denial => Err(CapabilityDenial {
                code: "http-request-port-not-granted".into(),
                message: "http-request port was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "port": parsed_request.port,
                }),
            }),
            _ if saw_host_denial => Err(CapabilityDenial {
                code: "http-request-host-not-granted".into(),
                message: "http-request host was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "host": parsed_request.host,
                }),
            }),
            _ if saw_scheme_denial => Err(CapabilityDenial {
                code: "http-request-scheme-not-granted".into(),
                message: "http-request scheme was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "scheme": parsed_request.scheme,
                }),
            }),
            _ if saw_method_denial => Err(CapabilityDenial {
                code: "http-request-method-not-granted".into(),
                message: "http-request method was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "method": request.method,
                }),
            }),
            _ => Err(CapabilityDenial {
                code: "http-request-not-granted".into(),
                message: "http-request was not granted for this execution".into(),
                detail: serde_json::json!({
                    "url": request.url,
                    "method": request.method,
                }),
            }),
        }
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

fn reduce_child_http_request_constraints(
    parent: &HttpRequestConstraints,
    required: &HttpRequestConstraints,
) -> Result<HttpRequestConstraints, CapabilityDenial> {
    let allowed_schemes = reduce_required_enum_scope(
        parent.allowed_schemes.as_ref(),
        required.allowed_schemes.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request schemes could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let allowed_hosts = reduce_required_host_scope(
        parent.allowed_hosts.as_ref(),
        required.allowed_hosts.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request hosts could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let allowed_ports = reduce_required_enum_scope(
        parent.allowed_ports.as_ref(),
        required.allowed_ports.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request ports could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let allowed_methods = reduce_required_enum_scope(
        parent.allowed_methods.as_ref(),
        required.allowed_methods.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request methods could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let allowed_path_prefixes = reduce_required_path_prefix_scope(
        parent.allowed_path_prefixes.as_ref(),
        required.allowed_path_prefixes.as_ref(),
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request paths could not be reduced from the parent grant".into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();
    let max_timeout_ms = reduce_required_max_bytes(parent.max_timeout_ms, required.max_timeout_ms)
        .ok_or_else(|| CapabilityDenial {
            code: "child-capability-mismatch".into(),
            message: "child http-request max_timeout_ms could not be reduced from the parent grant"
                .into(),
            detail: serde_json::json!({
                "parent_constraints": parent,
                "required_constraints": required,
            }),
        })?
        .into_option();
    let max_response_bytes = reduce_required_max_bytes(
        parent.max_response_bytes,
        required.max_response_bytes,
    )
    .ok_or_else(|| CapabilityDenial {
        code: "child-capability-mismatch".into(),
        message: "child http-request max_response_bytes could not be reduced from the parent grant"
            .into(),
        detail: serde_json::json!({
            "parent_constraints": parent,
            "required_constraints": required,
        }),
    })?
    .into_option();

    Ok(HttpRequestConstraints {
        allowed_schemes,
        allowed_hosts,
        allowed_ports,
        allowed_methods,
        allowed_path_prefixes,
        max_timeout_ms,
        max_response_bytes,
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
    ) && string_scope_covers_casefold_exact(
        grant.allowed_hosts.as_ref(),
        required.allowed_hosts.as_ref(),
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

fn string_scope_covers_casefold_exact(
    granted: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> bool {
    match (granted, required) {
        (_, None) | (None, Some(_)) => true,
        (Some(granted), Some(required)) => {
            let granted = canonicalize_host_scope(granted);
            required.iter().all(|value| {
                let value = value.to_ascii_lowercase();
                granted.iter().any(|candidate| candidate == &value)
            })
        }
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

enum ReducedConstraint<T> {
    Unbounded,
    Restricted(T),
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
    parent: Option<&Vec<String>>,
    required: Option<&Vec<String>>,
) -> Option<ReducedConstraint<Vec<String>>> {
    match (parent, required) {
        (None, None) => Some(ReducedConstraint::Unbounded),
        (Some(parent), None) => Some(ReducedConstraint::Restricted(canonicalize_host_scope(
            parent,
        ))),
        (None, Some(required)) => Some(ReducedConstraint::Restricted(canonicalize_host_scope(
            required,
        ))),
        (Some(parent), Some(required)) => {
            let parent = canonicalize_host_scope(parent);
            let reduced = canonicalize_host_scope(required)
                .into_iter()
                .filter(|candidate| parent.iter().any(|host| host == candidate))
                .collect::<Vec<_>>();
            if reduced.is_empty() {
                None
            } else {
                Some(ReducedConstraint::Restricted(reduced))
            }
        }
    }
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
        let host = host.to_ascii_lowercase();
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

fn is_supported_wasm_inspect_capability(id: &CapabilityId, access: &CapabilityAccess) -> bool {
    matches!(
        (id, access),
        (CapabilityId::HttpRequest, CapabilityAccess::Read)
            | (CapabilityId::ReadResource, CapabilityAccess::Read)
            | (CapabilityId::InvokeSkill, CapabilityAccess::Invoke)
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
) -> bindings::guild::skill::types::ExecutionContext {
    bindings::guild::skill::types::ExecutionContext {
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
            .collect(),
    }
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
) -> bindings::guild::skill::types::GrantedCapability {
    bindings::guild::skill::types::GrantedCapability {
        id: to_wit_capability_id(&grant.id),
        access: to_wit_capability_access(&grant.access),
        constraints: to_wit_capability_constraints(&grant.constraints),
    }
}

fn to_wit_execution_mode(mode: &ExecutionMode) -> bindings::guild::skill::types::ExecutionMode {
    match mode {
        ExecutionMode::Inspect => bindings::guild::skill::types::ExecutionMode::Inspect,
        ExecutionMode::Plan => bindings::guild::skill::types::ExecutionMode::Plan,
        ExecutionMode::Apply => bindings::guild::skill::types::ExecutionMode::Apply,
    }
}

fn to_wit_capability_id(id: &CapabilityId) -> bindings::guild::skill::types::CapabilityId {
    match id {
        CapabilityId::HttpRequest => bindings::guild::skill::types::CapabilityId::HttpRequest,
        CapabilityId::ReadResource => bindings::guild::skill::types::CapabilityId::ReadResource,
        CapabilityId::InvokeSkill => bindings::guild::skill::types::CapabilityId::InvokeSkill,
        CapabilityId::EmitEvidence => bindings::guild::skill::types::CapabilityId::EmitEvidence,
        CapabilityId::GetSecret => bindings::guild::skill::types::CapabilityId::GetSecret,
        CapabilityId::CacheRead => bindings::guild::skill::types::CapabilityId::CacheRead,
        CapabilityId::CacheWrite => bindings::guild::skill::types::CapabilityId::CacheWrite,
        CapabilityId::LogWrite => bindings::guild::skill::types::CapabilityId::LogWrite,
        CapabilityId::MonotonicClock => bindings::guild::skill::types::CapabilityId::MonotonicClock,
        CapabilityId::WallClock => bindings::guild::skill::types::CapabilityId::WallClock,
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
    constraints: &CapabilityConstraints,
) -> bindings::guild::skill::types::CapabilityConstraints {
    match constraints {
        CapabilityConstraints::None(_) => {
            bindings::guild::skill::types::CapabilityConstraints::None
        }
        CapabilityConstraints::HttpRequest(value) => {
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
            )
        }
        CapabilityConstraints::ReadResource(value) => {
            bindings::guild::skill::types::CapabilityConstraints::ReadResource(
                bindings::guild::skill::types::ReadResourceConstraints {
                    uri_prefixes: value.uri_prefixes.clone(),
                    resource_kinds: value
                        .resource_kinds
                        .as_ref()
                        .map(|kinds| kinds.iter().map(to_wit_resource_kind).collect()),
                },
            )
        }
        CapabilityConstraints::InvokeDependency(value) => {
            bindings::guild::skill::types::CapabilityConstraints::InvokeDependency(
                bindings::guild::skill::types::InvokeDependencyConstraints {
                    aliases: value.aliases.clone(),
                },
            )
        }
        CapabilityConstraints::EmitEvidence(value) => {
            bindings::guild::skill::types::CapabilityConstraints::EmitEvidence(
                bindings::guild::skill::types::EmitEvidenceConstraints {
                    max_bytes: value.max_bytes,
                    audiences: value
                        .audiences
                        .as_ref()
                        .map(|audiences| audiences.iter().map(to_wit_evidence_audience).collect()),
                    redactions: value
                        .redactions
                        .as_ref()
                        .map(|redactions| redactions.iter().map(to_wit_redaction_class).collect()),
                },
            )
        }
        CapabilityConstraints::Log(value) => {
            bindings::guild::skill::types::CapabilityConstraints::Log(
                bindings::guild::skill::types::LogConstraints {
                    levels: value
                        .levels
                        .as_ref()
                        .map(|levels| levels.iter().map(to_wit_severity).collect()),
                },
            )
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
    }
}

fn severity_label(severity: &Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn matches_http_method(allowed: Option<&Vec<HttpMethod>>, method: &HttpMethod) -> bool {
    allowed.is_none_or(|methods| methods.iter().any(|candidate| candidate == method))
}

fn matches_http_scheme(allowed: Option<&Vec<HttpScheme>>, scheme: &HttpScheme) -> bool {
    allowed.is_none_or(|schemes| schemes.iter().any(|candidate| candidate == scheme))
}

fn matches_http_host(allowed: Option<&Vec<String>>, host: &str) -> bool {
    allowed.is_none_or(|hosts| {
        hosts
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(host))
    })
}

fn matches_http_port(allowed: Option<&Vec<u16>>, port: u16) -> bool {
    allowed.is_none_or(|ports| ports.contains(&port))
}

fn matches_http_path(allowed: Option<&Vec<String>>, path: &str) -> bool {
    allowed.is_none_or(|prefixes| prefixes.iter().any(|prefix| path.starts_with(prefix)))
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
