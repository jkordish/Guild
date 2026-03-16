#![warn(clippy::all, clippy::pedantic, clippy::cargo, clippy::perf)]
#![allow(clippy::multiple_crate_versions)]

//! Execution boundary and runtime abstraction for Guild.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::sync::Arc;
use std::time::Instant;

use guild_manifest::SkillManifest;
use guild_registry::{execution_resource_uri, InstalledSkill, RegistryError, SkillRegistry};
use guild_sdk_rust::GuildSkill;
use guild_types::{
    host_now_utc, mint_host_execution_id, CapabilityAccess, CapabilityConstraints,
    CapabilityGrantSet, CapabilityId, CapabilityRequirement, ChildExecutionRecord, Diagnostic,
    Effect, EmitEvidenceConstraints, EvidenceAudience, EvidenceEmissionRequest, EvidenceRecord,
    EvidenceRef, ExecutionContext, ExecutionMetrics, ExecutionMode, ExecutionPhase,
    ExecutionReceipt, ExecutionRecord, ExecutionStatus, GrantedCapability, GuildResourceScope,
    GuildResourceUri, InvokeDependencyConstraints, LogConstraints, Mutability, PolicyDecision,
    PolicyDecisionOutcome, Provenance, ReadResourceConstraints, RedactionClass,
    ResolvedExecutionEnvelope, ResolvedSkillRef, ResourceKind, ResourceReadResult, RuntimeKind,
    Severity, SkillError, SkillOutput, TerminationDetail,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

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
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeFailure {
    pub error: Box<ExecutionError>,
    pub emitted_evidence: Vec<EvidenceRef>,
    pub child_executions: Vec<ChildExecutionRecord>,
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
            })?;

        skill
            .run(context.clone(), input.clone())
            .map(|output| RuntimeOutcome {
                output,
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
            })
            .map_err(|error| RuntimeFailure {
                error: Box::new(ExecutionError::from(error)),
                emitted_evidence: Vec::new(),
                child_executions: Vec::new(),
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
            })?;

        match result {
            Ok(output) => {
                let output = from_wit_skill_output(output).map_err(|error| RuntimeFailure {
                    error: Box::new(error),
                    emitted_evidence: store.data().emitted_evidence.clone(),
                    child_executions: store.data().child_executions.clone(),
                })?;
                validate_emitted_evidence(&output, &store.data().emitted_evidence).map_err(
                    |error| RuntimeFailure {
                        error: Box::new(error),
                        emitted_evidence: store.data().emitted_evidence.clone(),
                        child_executions: store.data().child_executions.clone(),
                    },
                )?;
                Ok(RuntimeOutcome {
                    output,
                    emitted_evidence: store.data().emitted_evidence.clone(),
                    child_executions: store.data().child_executions.clone(),
                })
            }
            Err(error) => Err(RuntimeFailure {
                error: Box::new(from_wit_skill_error(error)),
                emitted_evidence: store.data().emitted_evidence.clone(),
                child_executions: store.data().child_executions.clone(),
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

    fn http_request(&mut self, _request: String) -> wasmtime::Result<Result<String, String>> {
        Err(wasmtime::Error::msg(
            "http-request is not implemented in the Wasm inspect slice",
        ))
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
        (CapabilityId::ReadResource, CapabilityAccess::Read)
            | (CapabilityId::InvokeSkill, CapabilityAccess::Invoke)
            | (
                CapabilityId::EmitEvidence | CapabilityId::LogWrite,
                CapabilityAccess::Write
            )
    )
}

fn supported_wasm_inspect_capabilities() -> Vec<Value> {
    vec![
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
