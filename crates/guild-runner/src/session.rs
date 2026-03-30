//! Future session and admission seams for Guild's session-substrate evolution.
//!
//! These traits are intentionally interface-only in this pass. They document
//! where future session-aware admission and wake logic should live without
//! changing the current skill-first runtime path.

use std::fmt;

use guild_types::{
    CapabilityGrantSet, RehydratePolicy, ResumePolicy, SessionId, SessionMaterializationMode,
    SessionState,
};

/// Host-owned disposition returned by future session-aware admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionDisposition {
    Allow,
    Deny,
    AskHuman,
    ElevateIsolation,
}

/// Minimal request envelope for future session-aware admission decisions.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionAdmissionRequest {
    pub session_id: Option<SessionId>,
    pub requested_capabilities: CapabilityGrantSet,
    pub resume_policy: ResumePolicy,
    pub rehydrate_policy: RehydratePolicy,
}

/// Minimal admission result for future session-aware execution or wake paths.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionAdmission {
    pub disposition: AdmissionDisposition,
    pub granted_capabilities: CapabilityGrantSet,
    pub reason: Option<String>,
}

/// Trait for future host-owned session admission decisions.
pub trait AdmissionController: Send + Sync {
    fn admit(&self, request: &SessionAdmissionRequest) -> SessionAdmission;
}

/// Request to materialize or wake a durable session.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionWakeRequest {
    pub session_id: SessionId,
    pub resume_policy: ResumePolicy,
    pub rehydrate_policy: RehydratePolicy,
}

/// Host-selected outcome for waking or materializing a session.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionWakeOutcome {
    pub session_id: SessionId,
    pub state: SessionState,
    pub materialization_mode: SessionMaterializationMode,
}

/// Host-owned error for future session broker operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBrokerError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for SessionBrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SessionBrokerError {}

/// Trait for future host-owned session wake, resume, and rehydrate decisions.
pub trait SessionBroker: Send + Sync {
    /// Wake an existing session or select a safe materialization strategy.
    ///
    /// # Errors
    ///
    /// Returns an error when the broker cannot determine a safe outcome.
    fn wake_session(
        &self,
        request: &SessionWakeRequest,
    ) -> Result<SessionWakeOutcome, SessionBrokerError>;
}
