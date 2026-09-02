//! Deterministic effect identity, permanent bindings, budgets, fences, locks, and leases.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    authority::{
        AuthorityPolicy, BudgetAmount, BudgetClaim, BudgetKey, EffectKind, Fence,
        InstallationEnrollment, PublicationApproval, PublicationWarrant, SeparationApproval,
        SeparationWarrant, ensure_nondecreasing_time,
    },
    body::{
        BodyError, BodyGraph, BodyKind, BodySpec, EffectLeaseTag, IdempotencyBindingRef,
        IdempotencyBindingTag, OptionalValue, ProtocolRef, PublicationWarrantRef,
        SeparationBindingRef, SeparationBindingTag, SeparationLeaseTag, SeparationWarrantRef,
        SortedUnique, TypedEdge, ValidatedBody, validated_body,
    },
    canonical::{CanonicalError, canonical_digest},
    scalar::{
        Digest, EffectId, IdempotencyKey, LogicalAddress, ResourceKey, U64Decimal, UnixNanoseconds,
    },
};

const LEASE_DURATION_NANOS: u64 = 5_000_000_000;
const PUBLICATION_TERMINAL_SLOTS: u64 = 3;
const SEPARATION_TERMINAL_SLOTS: u64 = 2;

/// Closed admission failures shared by authority and lease transitions.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionError {
    #[error("authority refused")]
    AuthorityRefused,
    #[error("warrant expired")]
    WarrantExpired,
    #[error("warrant revoked")]
    WarrantRevoked,
    #[error("warrant already spent")]
    WarrantSpent,
    #[error("idempotency binding conflicts with the requested effect")]
    IdempotencyConflict,
    #[error("resource is locked or claimed")]
    ResourceConflict,
    #[error("budget is unavailable")]
    BudgetUnavailable,
    #[error("counter exhausted")]
    CounterExhausted,
    #[error("event sequence exhausted")]
    SequenceExhausted,
    #[error("precondition refused")]
    PreconditionRefused,
    #[error("transition time moved backwards")]
    TimeRegression,
    #[error("body graph is invalid: {0}")]
    Body(#[from] BodyError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetClass {
    Reservation,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    Available,
    Held,
    Consumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreStartEffectState {
    Reserved,
    Prepared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreStartResult {
    NotAttempted,
    PreparedOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreStartReason {
    RequestDisconnected,
    ReservationDeadline,
    AuthorizationIneligible,
    PeerIdentityChanged,
    PreconditionChanged,
    RecoveryOrphaned,
    BudgetUnavailable,
    SeparationPreconditionRefused,
}

impl PreStartReason {
    #[must_use]
    pub const fn is_cancellation_reason(self) -> bool {
        matches!(
            self,
            Self::RequestDisconnected
                | Self::ReservationDeadline
                | Self::AuthorizationIneligible
                | Self::PeerIdentityChanged
                | Self::PreconditionChanged
                | Self::RecoveryOrphaned
        )
    }
}

pub type BindingRef = ProtocolRef<IdempotencyBindingTag, SeparationBindingTag>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreStartOutcome {
    result: PreStartResult,
    reason: PreStartReason,
    binding_digest: OptionalValue<BindingRef>,
}

impl PreStartOutcome {
    #[must_use]
    pub const fn result(&self) -> PreStartResult {
        self.result
    }

    #[must_use]
    pub const fn reason(&self) -> PreStartReason {
        self.reason
    }

    #[must_use]
    pub const fn binding_digest(&self) -> &OptionalValue<BindingRef> {
        &self.binding_digest
    }

    #[must_use]
    pub const fn refused(reason: PreStartReason) -> Self {
        Self {
            result: PreStartResult::NotAttempted,
            reason,
            binding_digest: OptionalValue::Absent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceFence {
    resource_key: ResourceKey,
    fence: Fence,
}

impl ResourceFence {
    #[must_use]
    pub const fn resource_key(&self) -> &ResourceKey {
        &self.resource_key
    }

    #[must_use]
    pub const fn fence(&self) -> Fence {
        self.fence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetHold {
    key: BudgetKey,
    amount: BudgetAmount,
}

impl BudgetHold {
    #[must_use]
    pub const fn key(&self) -> &BudgetKey {
        &self.key
    }

    #[must_use]
    pub const fn amount(&self) -> BudgetAmount {
        self.amount
    }
}

/// Permanent publication idempotency binding. Only lawful reservation can construct one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdempotencyBinding {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: PublicationWarrantRef,
}

impl IdempotencyBinding {
    #[must_use]
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn warrant_digest(&self) -> &PublicationWarrantRef {
        &self.warrant_digest
    }
}

/// Five-second publication reservation lease. Only lawful reservation can construct one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectLease {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}

impl EffectLease {
    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn binding_digest(&self) -> &IdempotencyBindingRef {
        &self.binding_digest
    }

    #[must_use]
    pub const fn resource_fences(&self) -> &[ResourceFence; 2] {
        &self.resource_fences
    }

    #[must_use]
    pub const fn reservation_budget_hold(&self) -> &BudgetHold {
        &self.reservation_budget_hold
    }

    #[must_use]
    pub const fn start_budget_hold(&self) -> &BudgetHold {
        &self.start_budget_hold
    }

    #[must_use]
    pub const fn reserved_at(&self) -> UnixNanoseconds {
        self.reserved_at
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixNanoseconds {
        self.expires_at
    }

    #[must_use]
    pub fn is_live_at(&self, now: UnixNanoseconds) -> bool {
        now >= self.reserved_at && now < self.expires_at
    }

    pub(crate) fn validate_local(&self) -> Result<(), BodyError> {
        validate_lease_fields(&self.resource_fences, self.reserved_at, self.expires_at)
    }
}

/// Permanent separation idempotency binding. Only lawful reservation can construct one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeparationBinding {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: SeparationWarrantRef,
}

impl SeparationBinding {
    #[must_use]
    pub const fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn warrant_digest(&self) -> &SeparationWarrantRef {
        &self.warrant_digest
    }
}

/// Five-second separation reservation lease. Only lawful reservation can construct one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeparationLease {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}

impl SeparationLease {
    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn binding_digest(&self) -> &SeparationBindingRef {
        &self.binding_digest
    }

    #[must_use]
    pub const fn resource_fences(&self) -> &[ResourceFence; 2] {
        &self.resource_fences
    }

    #[must_use]
    pub const fn reservation_budget_hold(&self) -> &BudgetHold {
        &self.reservation_budget_hold
    }

    #[must_use]
    pub const fn start_budget_hold(&self) -> &BudgetHold {
        &self.start_budget_hold
    }

    #[must_use]
    pub const fn reserved_at(&self) -> UnixNanoseconds {
        self.reserved_at
    }

    #[must_use]
    pub const fn expires_at(&self) -> UnixNanoseconds {
        self.expires_at
    }

    #[must_use]
    pub fn is_live_at(&self, now: UnixNanoseconds) -> bool {
        now >= self.reserved_at && now < self.expires_at
    }

    pub(crate) fn validate_local(&self) -> Result<(), BodyError> {
        validate_lease_fields(&self.resource_fences, self.reserved_at, self.expires_at)
    }
}

fn validate_lease_fields(
    resource_fences: &[ResourceFence; 2],
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
) -> Result<(), BodyError> {
    if resource_fences[0].resource_key >= resource_fences[1].resource_key
        || resource_fences.iter().any(|fence| fence.fence.get() == 0)
    {
        return Err(BodyError::Local(
            "lease resource fences must be two distinct sorted nonzero fences".to_owned(),
        ));
    }
    let expected = reserved_at
        .checked_add(LEASE_DURATION_NANOS)
        .map_err(|_| BodyError::Local("lease expiration arithmetic overflowed".to_owned()))?;
    if expires_at != expected {
        return Err(BodyError::Local(
            "lease expiry must be exactly five seconds after reservation".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceIdentityPreimage<'a> {
    effect_family: &'static str,
    logical_address: &'a LogicalAddress,
}

/// Derives the sole static-artifact resource identity from an opaque canonical address.
///
/// # Errors
///
/// Returns canonical encoding or digest validation failure.
pub fn derive_resource_key(
    logical_address: &LogicalAddress,
) -> Result<ResourceKey, CanonicalError> {
    let digest = canonical_digest(&ResourceIdentityPreimage {
        effect_family: "static_artifact",
        logical_address,
    })?;
    ResourceKey::parse(digest.as_str()).map_err(CanonicalError::Digest)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EffectIdentityPreimage<'a> {
    installation_digest: &'a Digest,
    warrant_digest: &'a Digest,
    effect_kind: EffectKind,
    resource_keys: &'a SortedUnique<ResourceKey>,
    input_digest: &'a Digest,
    precondition_digest: &'a Digest,
}

/// Derives an effect identity from exactly the protocol §8.2 preimage.
///
/// # Errors
///
/// Returns canonical encoding or digest validation failure.
pub fn derive_effect_id(
    installation_digest: &Digest,
    warrant_digest: &Digest,
    effect_kind: EffectKind,
    resource_keys: &SortedUnique<ResourceKey>,
    input_digest: &Digest,
    precondition_digest: &Digest,
) -> Result<EffectId, CanonicalError> {
    let digest = canonical_digest(&EffectIdentityPreimage {
        installation_digest,
        warrant_digest,
        effect_kind,
        resource_keys,
        input_digest,
        precondition_digest,
    })?;
    EffectId::parse(digest.as_str()).map_err(CanonicalError::Digest)
}

/// Opaque deterministic state delta carried beside a proposed binding and lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseDelta {
    changed: bool,
}

impl LeaseDelta {
    const fn changed() -> Self {
        Self { changed: true }
    }

    const fn unchanged() -> Self {
        Self { changed: false }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.changed
    }
}

/// Bodies and identity minted by one lawful reservation proposal.
#[derive(Clone)]
pub struct ReservationMaterial<B: BodySpec, L: BodySpec> {
    binding: ValidatedBody<B>,
    lease: ValidatedBody<L>,
    effect_id: EffectId,
    delta: LeaseDelta,
}

impl<B, L> std::fmt::Debug for ReservationMaterial<B, L>
where
    B: BodySpec + std::fmt::Debug,
    L: BodySpec + std::fmt::Debug,
    B::Tag: std::fmt::Debug,
    L::Tag: std::fmt::Debug,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ReservationMaterial")
            .field("binding", &self.binding)
            .field("lease", &self.lease)
            .field("effect_id", &self.effect_id)
            .field("delta", &self.delta)
            .finish()
    }
}

impl<B: BodySpec, L: BodySpec> ReservationMaterial<B, L> {
    #[must_use]
    pub const fn binding(&self) -> &ValidatedBody<B> {
        &self.binding
    }

    #[must_use]
    pub const fn lease(&self) -> &ValidatedBody<L> {
        &self.lease
    }

    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn delta(&self) -> &LeaseDelta {
        &self.delta
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetBalance {
    capacity: u64,
    available: u64,
    held: u64,
    consumed: u64,
}

impl BudgetBalance {
    #[must_use]
    pub const fn capacity(self) -> u64 {
        self.capacity
    }

    #[must_use]
    pub const fn available(self) -> u64 {
        self.available
    }

    #[must_use]
    pub const fn held(self) -> u64 {
        self.held
    }

    #[must_use]
    pub const fn consumed(self) -> u64 {
        self.consumed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLock {
    effect_id: EffectId,
    fence: Fence,
}

impl ResourceLock {
    #[must_use]
    pub const fn effect_id(&self) -> &EffectId {
        &self.effect_id
    }

    #[must_use]
    pub const fn fence(&self) -> Fence {
        self.fence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermanentBinding {
    effect_id: EffectId,
    warrant_digest: Digest,
    binding_digest: BindingRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EffectProtocol {
    Publication,
    Separation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Reserved,
    Prepared,
    Started,
    Cancelled,
    Terminal,
}

#[derive(Debug, Clone)]
enum StoredReservation {
    Publication {
        binding: ValidatedBody<IdempotencyBinding>,
        lease: ValidatedBody<EffectLease>,
    },
    Separation {
        binding: ValidatedBody<SeparationBinding>,
        lease: ValidatedBody<SeparationLease>,
    },
}

#[derive(Debug, Clone)]
struct EffectRecord {
    protocol: EffectProtocol,
    lifecycle: Lifecycle,
    material: StoredReservation,
    resource_fences: [ResourceFence; 2],
    reservation_hold: BudgetHold,
    start_hold: BudgetHold,
    warrant_digest: Digest,
    warrant_expires_at: UnixNanoseconds,
    separation_generation: Option<U64Decimal>,
}

/// Deterministic lease projection with private, atomically-updated protocol maps.
#[derive(Debug, Clone)]
pub struct LeaseProjection {
    installation_digest: Digest,
    policy_digest: Digest,
    bindings: BTreeMap<IdempotencyKey, PermanentBinding>,
    spent_warrants: BTreeSet<Digest>,
    budget_accounts: BTreeMap<(BudgetClass, BudgetKey), BudgetBalance>,
    resource_fences: BTreeMap<ResourceKey, Fence>,
    resource_locks: BTreeMap<ResourceKey, ResourceLock>,
    effects: BTreeMap<EffectId, EffectRecord>,
    terminal_sequence_reserve: U64Decimal,
    head_sequence: U64Decimal,
    last_transition_time: Option<UnixNanoseconds>,
}

impl LeaseProjection {
    /// Initializes namespaced budget accounts from the immutable policy.
    ///
    /// # Errors
    ///
    /// Returns a body error if the supplied policy is structurally invalid.
    pub fn new(
        graph: &BodyGraph,
        enrollment: &ValidatedBody<InstallationEnrollment>,
        policy: &ValidatedBody<AuthorityPolicy>,
    ) -> Result<Self, AdmissionError> {
        Self::with_head_sequence(graph, enrollment, policy, U64Decimal::from_u64(0))
    }

    /// Initializes a replay projection at an explicit anchored sequence.
    ///
    /// # Errors
    ///
    /// Returns a body error if the supplied policy is structurally invalid.
    pub fn with_head_sequence(
        graph: &BodyGraph,
        enrollment: &ValidatedBody<InstallationEnrollment>,
        policy: &ValidatedBody<AuthorityPolicy>,
        head_sequence: U64Decimal,
    ) -> Result<Self, AdmissionError> {
        require_graph_body(graph, enrollment)?;
        require_graph_body(graph, policy)?;
        if enrollment.payload().policy_digest() != policy.reference() {
            return Err(AdmissionError::AuthorityRefused);
        }
        let mut budget_accounts = BTreeMap::new();
        for (class, budgets) in [
            (
                BudgetClass::Reservation,
                policy.payload().reservation_budgets(),
            ),
            (BudgetClass::Start, policy.payload().start_budgets()),
        ] {
            for budget in budgets.as_slice() {
                let capacity = budget.capacity().get();
                budget_accounts.insert(
                    (class, budget.key().clone()),
                    BudgetBalance {
                        capacity,
                        available: capacity,
                        held: 0,
                        consumed: 0,
                    },
                );
            }
        }
        Ok(Self {
            installation_digest: enrollment.reference().digest().clone(),
            policy_digest: policy.reference().digest().clone(),
            bindings: BTreeMap::new(),
            spent_warrants: BTreeSet::new(),
            budget_accounts,
            resource_fences: BTreeMap::new(),
            resource_locks: BTreeMap::new(),
            effects: BTreeMap::new(),
            terminal_sequence_reserve: U64Decimal::from_u64(0),
            head_sequence,
            last_transition_time: None,
        })
    }

    #[must_use]
    pub fn binding_effect(&self, key: &IdempotencyKey) -> Option<&EffectId> {
        self.bindings.get(key).map(|binding| &binding.effect_id)
    }

    #[must_use]
    pub fn is_warrant_spent(&self, warrant: &Digest) -> bool {
        self.spent_warrants.contains(warrant)
    }

    #[must_use]
    pub fn budget_balance(&self, class: BudgetClass, key: &BudgetKey) -> Option<BudgetBalance> {
        self.budget_accounts.get(&(class, key.clone())).copied()
    }

    #[must_use]
    pub fn resource_fence(&self, key: &ResourceKey) -> Option<Fence> {
        self.resource_fences.get(key).copied()
    }

    #[must_use]
    pub fn resource_lock(&self, key: &ResourceKey) -> Option<&ResourceLock> {
        self.resource_locks.get(key)
    }

    #[must_use]
    pub const fn terminal_sequence_reserve(&self) -> U64Decimal {
        self.terminal_sequence_reserve
    }

    #[must_use]
    pub const fn head_sequence(&self) -> U64Decimal {
        self.head_sequence
    }

    #[must_use]
    pub fn effect_state(&self, effect_id: &EffectId) -> Option<PreStartEffectState> {
        match self.effects.get(effect_id)?.lifecycle {
            Lifecycle::Reserved => Some(PreStartEffectState::Reserved),
            Lifecycle::Prepared => Some(PreStartEffectState::Prepared),
            Lifecycle::Started | Lifecycle::Cancelled | Lifecycle::Terminal => None,
        }
    }

    /// Atomically reserves a publication or returns the existing identical binding.
    ///
    /// # Errors
    ///
    /// Fails closed for ineligible authority, identity conflict, expiry/revocation, budget,
    /// resource, fence, time, sequence, or canonical body failure. No map changes on failure.
    pub fn reserve_publication(
        &mut self,
        graph: &BodyGraph,
        policy: &ValidatedBody<AuthorityPolicy>,
        warrant: &ValidatedBody<PublicationWarrant>,
        approval: &ValidatedBody<PublicationApproval>,
        reserved_at: UnixNanoseconds,
    ) -> Result<ReservationMaterial<IdempotencyBinding, EffectLease>, AdmissionError> {
        require_graph_body(graph, policy)?;
        require_graph_body(graph, warrant)?;
        require_graph_body(graph, approval)?;
        let revoked_at = graph.publication_revoked_at(warrant.reference().digest())?;
        self.reserve_publication_resolved(policy, warrant, approval, reserved_at, revoked_at)
    }

    fn reserve_publication_resolved(
        &mut self,
        policy: &ValidatedBody<AuthorityPolicy>,
        warrant: &ValidatedBody<PublicationWarrant>,
        approval: &ValidatedBody<PublicationApproval>,
        reserved_at: UnixNanoseconds,
        revoked_at: Option<UnixNanoseconds>,
    ) -> Result<ReservationMaterial<IdempotencyBinding, EffectLease>, AdmissionError> {
        let resources = SortedUnique::new(warrant.payload().resource_keys().to_vec())?;
        let effect_id = derive_effect_id(
            warrant.payload().installation_digest().digest(),
            warrant.reference().digest(),
            EffectKind::StaticArtifactPublish,
            &resources,
            warrant.payload().input_digest().digest(),
            warrant.payload().precondition_digest().digest(),
        )
        .map_err(|error| AdmissionError::Body(BodyError::Canonical(error)))?;

        if let Some(existing) = self.bindings.get(warrant.payload().idempotency_key()) {
            if existing.effect_id != effect_id
                || existing.warrant_digest != *warrant.reference().digest()
            {
                return Err(AdmissionError::IdempotencyConflict);
            }
            let record = self
                .effects
                .get(&effect_id)
                .ok_or(AdmissionError::AuthorityRefused)?;
            let StoredReservation::Publication { binding, lease } = &record.material else {
                return Err(AdmissionError::IdempotencyConflict);
            };
            return Ok(ReservationMaterial {
                binding: binding.clone(),
                lease: lease.clone(),
                effect_id,
                delta: LeaseDelta::unchanged(),
            });
        }
        if self.spent_warrants.contains(warrant.reference().digest()) {
            return Err(AdmissionError::WarrantSpent);
        }
        validate_publication_authority(self, policy, warrant, approval, revoked_at, reserved_at)?;

        let mut candidate = self.clone();
        candidate.admit_ordinary_events_internal(1, reserved_at)?;
        let fences = candidate.plan_resource_fences(warrant.payload().resource_keys())?;
        candidate.hold(
            BudgetClass::Reservation,
            warrant.payload().reservation_budget(),
        )?;
        candidate.hold(BudgetClass::Start, warrant.payload().start_budget())?;
        let expires_at = reserved_at
            .checked_add(LEASE_DURATION_NANOS)
            .map_err(|_| AdmissionError::CounterExhausted)?;
        let binding = validated_body(IdempotencyBinding {
            idempotency_key: warrant.payload().idempotency_key().clone(),
            effect_id: effect_id.clone(),
            warrant_digest: warrant.reference().clone(),
        })?;
        let lease = validated_body(EffectLease {
            effect_id: effect_id.clone(),
            binding_digest: binding.reference().clone(),
            resource_fences: fences.clone(),
            reservation_budget_hold: claim_to_hold(warrant.payload().reservation_budget()),
            start_budget_hold: claim_to_hold(warrant.payload().start_budget()),
            reserved_at,
            expires_at,
        })?;
        candidate.install_publication_reservation(
            warrant,
            &binding,
            &lease,
            effect_id.clone(),
            fences,
        );
        *self = candidate;
        Ok(ReservationMaterial {
            binding,
            lease,
            effect_id,
            delta: LeaseDelta::changed(),
        })
    }

    /// Atomically reserves a separation or returns the existing identical binding.
    ///
    /// # Errors
    ///
    /// Uses the same fail-closed rules as publication with the separation namespace.
    pub fn reserve_separation(
        &mut self,
        graph: &BodyGraph,
        policy: &ValidatedBody<AuthorityPolicy>,
        warrant: &ValidatedBody<SeparationWarrant>,
        approval: &ValidatedBody<SeparationApproval>,
        current_custody_generation: U64Decimal,
        reserved_at: UnixNanoseconds,
    ) -> Result<ReservationMaterial<SeparationBinding, SeparationLease>, AdmissionError> {
        require_graph_body(graph, policy)?;
        require_graph_body(graph, warrant)?;
        require_graph_body(graph, approval)?;
        let revoked_at = graph.separation_revoked_at(warrant.reference().digest())?;
        self.reserve_separation_resolved(
            policy,
            warrant,
            approval,
            current_custody_generation,
            reserved_at,
            revoked_at,
        )
    }

    fn reserve_separation_resolved(
        &mut self,
        policy: &ValidatedBody<AuthorityPolicy>,
        warrant: &ValidatedBody<SeparationWarrant>,
        approval: &ValidatedBody<SeparationApproval>,
        current_custody_generation: U64Decimal,
        reserved_at: UnixNanoseconds,
        revoked_at: Option<UnixNanoseconds>,
    ) -> Result<ReservationMaterial<SeparationBinding, SeparationLease>, AdmissionError> {
        let resources = SortedUnique::new(warrant.payload().resource_keys().to_vec())?;
        let effect_id = derive_effect_id(
            warrant.payload().installation_digest().digest(),
            warrant.reference().digest(),
            EffectKind::StaticArtifactSeparation,
            &resources,
            warrant.payload().input_digest().digest(),
            warrant.payload().precondition_digest().digest(),
        )
        .map_err(|error| AdmissionError::Body(BodyError::Canonical(error)))?;

        if let Some(existing) = self.bindings.get(warrant.payload().idempotency_key()) {
            if existing.effect_id != effect_id
                || existing.warrant_digest != *warrant.reference().digest()
            {
                return Err(AdmissionError::IdempotencyConflict);
            }
            let record = self
                .effects
                .get(&effect_id)
                .ok_or(AdmissionError::AuthorityRefused)?;
            let StoredReservation::Separation { binding, lease } = &record.material else {
                return Err(AdmissionError::IdempotencyConflict);
            };
            return Ok(ReservationMaterial {
                binding: binding.clone(),
                lease: lease.clone(),
                effect_id,
                delta: LeaseDelta::unchanged(),
            });
        }
        if self.spent_warrants.contains(warrant.reference().digest()) {
            return Err(AdmissionError::WarrantSpent);
        }
        validate_separation_authority(self, policy, warrant, approval, revoked_at, reserved_at)?;
        checked_next_generation(Some(current_custody_generation))?;

        let mut candidate = self.clone();
        candidate.admit_ordinary_events_internal(1, reserved_at)?;
        let fences = candidate.plan_resource_fences(warrant.payload().resource_keys())?;
        candidate.hold(
            BudgetClass::Reservation,
            warrant.payload().reservation_budget(),
        )?;
        candidate.hold(BudgetClass::Start, warrant.payload().start_budget())?;
        let expires_at = reserved_at
            .checked_add(LEASE_DURATION_NANOS)
            .map_err(|_| AdmissionError::CounterExhausted)?;
        let binding = validated_body(SeparationBinding {
            idempotency_key: warrant.payload().idempotency_key().clone(),
            effect_id: effect_id.clone(),
            warrant_digest: warrant.reference().clone(),
        })?;
        let lease = validated_body(SeparationLease {
            effect_id: effect_id.clone(),
            binding_digest: binding.reference().clone(),
            resource_fences: fences.clone(),
            reservation_budget_hold: claim_to_hold(warrant.payload().reservation_budget()),
            start_budget_hold: claim_to_hold(warrant.payload().start_budget()),
            reserved_at,
            expires_at,
        })?;
        candidate.install_separation_reservation(
            warrant,
            &binding,
            &lease,
            effect_id.clone(),
            fences,
            current_custody_generation,
        );
        *self = candidate;
        Ok(ReservationMaterial {
            binding,
            lease,
            effect_id,
            delta: LeaseDelta::changed(),
        })
    }

    /// Records the publication preparation event without releasing locks or holds.
    ///
    /// # Errors
    ///
    /// Rejects the wrong protocol/state, time regression, or unavailable sequence space.
    pub fn mark_prepared(
        &mut self,
        effect_id: &EffectId,
        prepared_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let mut candidate = self.clone();
        let record = candidate
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?;
        if record.protocol != EffectProtocol::Publication || record.lifecycle != Lifecycle::Reserved
        {
            return Err(AdmissionError::AuthorityRefused);
        }
        if !record.lease_is_live_at(prepared_at) || prepared_at >= record.warrant_expires_at {
            return Err(AdmissionError::WarrantExpired);
        }
        candidate.admit_ordinary_events_internal(1, prepared_at)?;
        candidate
            .effects
            .get_mut(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .lifecycle = Lifecycle::Prepared;
        *self = candidate;
        Ok(())
    }

    /// Consumes both held budgets and reserves terminal event capacity while retaining locks.
    ///
    /// # Errors
    ///
    /// Rejects an expired lease, duplicate start, wrong state, time regression, budget mismatch,
    /// or sequence exhaustion. No state changes on failure.
    pub fn start(
        &mut self,
        graph: &BodyGraph,
        effect_id: &EffectId,
        start_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let record = self
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?;
        let revoked_at = Self::resolve_start_authority(graph, record)?;
        self.start_publication_resolved(effect_id, start_at, revoked_at)
    }

    fn start_publication_resolved(
        &mut self,
        effect_id: &EffectId,
        start_at: UnixNanoseconds,
        revoked_at: Option<UnixNanoseconds>,
    ) -> Result<(), AdmissionError> {
        let mut candidate = self.clone();
        ensure_nondecreasing_time(candidate.last_transition_time, start_at)?;
        let record = candidate
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .clone();
        if record.protocol != EffectProtocol::Publication {
            return Err(AdmissionError::AuthorityRefused);
        }
        if record.lifecycle != Lifecycle::Prepared {
            return Err(AdmissionError::PreconditionRefused);
        }
        Self::ensure_start_authorized(&record, start_at, revoked_at)?;
        candidate.apply_start(effect_id, &record, start_at)?;
        *self = candidate;
        Ok(())
    }

    /// Starts a separation only after rechecking the reserved custody generation successor.
    ///
    /// # Errors
    ///
    /// Rejects a changed/exhausted generation, expired lease, duplicate start, time regression,
    /// budget mismatch, or sequence exhaustion without changing state.
    pub fn start_separation(
        &mut self,
        graph: &BodyGraph,
        effect_id: &EffectId,
        current_custody_generation: U64Decimal,
        start_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let record = self
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?;
        let revoked_at = Self::resolve_start_authority(graph, record)?;
        self.start_separation_resolved(effect_id, current_custody_generation, start_at, revoked_at)
    }

    fn start_separation_resolved(
        &mut self,
        effect_id: &EffectId,
        current_custody_generation: U64Decimal,
        start_at: UnixNanoseconds,
        revoked_at: Option<UnixNanoseconds>,
    ) -> Result<(), AdmissionError> {
        let mut candidate = self.clone();
        ensure_nondecreasing_time(candidate.last_transition_time, start_at)?;
        let record = candidate
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .clone();
        if record.protocol != EffectProtocol::Separation || record.lifecycle != Lifecycle::Reserved
        {
            return Err(AdmissionError::PreconditionRefused);
        }
        checked_next_generation(Some(current_custody_generation))?;
        if record.separation_generation != Some(current_custody_generation) {
            return Err(AdmissionError::PreconditionRefused);
        }
        Self::ensure_start_authorized(&record, start_at, revoked_at)?;
        candidate.apply_start(effect_id, &record, start_at)?;
        *self = candidate;
        Ok(())
    }

    /// Cancels one unstarted effect and releases held budgets and locks permanently.
    ///
    /// # Errors
    ///
    /// Rejects non-cancellation vocabulary, a started/terminal effect, deadline cancellation
    /// before lease expiry, time regression, or invalid held state.
    pub fn cancel(
        &mut self,
        effect_id: &EffectId,
        reason: PreStartReason,
        cancelled_at: UnixNanoseconds,
    ) -> Result<PreStartOutcome, AdmissionError> {
        if !reason.is_cancellation_reason() {
            return Err(AdmissionError::AuthorityRefused);
        }
        let mut candidate = self.clone();
        let record = candidate
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .clone();
        let result = match record.lifecycle {
            Lifecycle::Reserved => PreStartResult::NotAttempted,
            Lifecycle::Prepared => PreStartResult::PreparedOnly,
            Lifecycle::Started | Lifecycle::Cancelled | Lifecycle::Terminal => {
                return Err(AdmissionError::AuthorityRefused);
            }
        };
        if reason == PreStartReason::ReservationDeadline && record.lease_is_live_at(cancelled_at) {
            return Err(AdmissionError::AuthorityRefused);
        }
        candidate.admit_ordinary_events_internal(1, cancelled_at)?;
        candidate.release(BudgetClass::Reservation, &record.reservation_hold)?;
        candidate.release(BudgetClass::Start, &record.start_hold)?;
        candidate.release_locks(effect_id, &record.resource_fences)?;
        let binding_digest = record.binding_ref();
        candidate
            .effects
            .get_mut(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .lifecycle = Lifecycle::Cancelled;
        *self = candidate;
        Ok(PreStartOutcome {
            result,
            reason,
            binding_digest: OptionalValue::Present {
                value: binding_digest,
            },
        })
    }

    /// Releases a started effect's locks and its entire family reserve in one terminal bundle.
    ///
    /// # Errors
    ///
    /// Rejects duplicate/non-started terminalization, illegal family event counts, time
    /// regression, or arithmetic exhaustion. One call can terminalize only this one effect.
    pub fn terminalize(
        &mut self,
        effect_id: &EffectId,
        event_count: u64,
        terminal_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let mut candidate = self.clone();
        ensure_nondecreasing_time(candidate.last_transition_time, terminal_at)?;
        let record = candidate
            .effects
            .get(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .clone();
        if record.lifecycle != Lifecycle::Started || !record.valid_terminal_count(event_count) {
            return Err(AdmissionError::AuthorityRefused);
        }
        let slots = record.terminal_slots();
        if candidate.terminal_sequence_reserve.get() < slots {
            return Err(AdmissionError::SequenceExhausted);
        }
        candidate.head_sequence = candidate
            .head_sequence
            .checked_add(event_count)
            .map_err(|_| AdmissionError::SequenceExhausted)?;
        candidate.terminal_sequence_reserve = candidate
            .terminal_sequence_reserve
            .checked_sub(slots)
            .map_err(|_| AdmissionError::SequenceExhausted)?;
        candidate.release_locks(effect_id, &record.resource_fences)?;
        candidate.last_transition_time = Some(terminal_at);
        candidate
            .effects
            .get_mut(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .lifecycle = Lifecycle::Terminal;
        *self = candidate;
        Ok(())
    }

    /// Admits unrelated events without consuming terminal slots reserved for started effects.
    ///
    /// # Errors
    ///
    /// Returns sequence exhaustion or time regression without changing the projection.
    pub fn admit_ordinary_events(
        &mut self,
        event_count: u64,
        transition_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let mut candidate = self.clone();
        candidate.admit_ordinary_events_internal(event_count, transition_at)?;
        *self = candidate;
        Ok(())
    }

    fn admit_ordinary_events_internal(
        &mut self,
        event_count: u64,
        transition_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        if event_count == 0 {
            return Err(AdmissionError::AuthorityRefused);
        }
        ensure_nondecreasing_time(self.last_transition_time, transition_at)?;
        let needed = self
            .terminal_sequence_reserve
            .get()
            .checked_add(event_count)
            .ok_or(AdmissionError::SequenceExhausted)?;
        let remaining = U64Decimal::MAX - self.head_sequence.get();
        if remaining < needed {
            return Err(AdmissionError::SequenceExhausted);
        }
        self.head_sequence = self
            .head_sequence
            .checked_add(event_count)
            .map_err(|_| AdmissionError::SequenceExhausted)?;
        self.last_transition_time = Some(transition_at);
        Ok(())
    }

    fn ensure_start_capacity(&self, slots: u64) -> Result<(), AdmissionError> {
        let needed = self
            .terminal_sequence_reserve
            .get()
            .checked_add(1)
            .and_then(|value| value.checked_add(slots))
            .ok_or(AdmissionError::SequenceExhausted)?;
        let remaining = U64Decimal::MAX - self.head_sequence.get();
        if remaining < needed {
            return Err(AdmissionError::SequenceExhausted);
        }
        Ok(())
    }

    fn resolve_start_authority(
        graph: &BodyGraph,
        record: &EffectRecord,
    ) -> Result<Option<UnixNanoseconds>, AdmissionError> {
        let (kind, revoked_at) = match record.protocol {
            EffectProtocol::Publication => (
                BodyKind::PublicationWarrant,
                graph.publication_revoked_at(&record.warrant_digest)?,
            ),
            EffectProtocol::Separation => (
                BodyKind::SeparationWarrant,
                graph.separation_revoked_at(&record.warrant_digest)?,
            ),
        };
        graph
            .require_kind(&record.warrant_digest, kind)
            .map_err(|_| AdmissionError::AuthorityRefused)?;
        Ok(revoked_at)
    }

    fn ensure_start_authorized(
        record: &EffectRecord,
        start_at: UnixNanoseconds,
        revoked_at: Option<UnixNanoseconds>,
    ) -> Result<(), AdmissionError> {
        if !record.lease_is_live_at(start_at) || start_at >= record.warrant_expires_at {
            return Err(AdmissionError::WarrantExpired);
        }
        if revoked_at.is_some_and(|revoked_at| start_at >= revoked_at) {
            return Err(AdmissionError::WarrantRevoked);
        }
        Ok(())
    }

    fn apply_start(
        &mut self,
        effect_id: &EffectId,
        record: &EffectRecord,
        start_at: UnixNanoseconds,
    ) -> Result<(), AdmissionError> {
        let slots = record.terminal_slots();
        self.ensure_start_capacity(slots)?;
        self.consume(BudgetClass::Reservation, &record.reservation_hold)?;
        self.consume(BudgetClass::Start, &record.start_hold)?;
        self.head_sequence = self
            .head_sequence
            .checked_add(1)
            .map_err(|_| AdmissionError::SequenceExhausted)?;
        self.terminal_sequence_reserve = self
            .terminal_sequence_reserve
            .checked_add(slots)
            .map_err(|_| AdmissionError::SequenceExhausted)?;
        self.last_transition_time = Some(start_at);
        self.effects
            .get_mut(effect_id)
            .ok_or(AdmissionError::AuthorityRefused)?
            .lifecycle = Lifecycle::Started;
        Ok(())
    }

    fn plan_resource_fences(
        &mut self,
        keys: &[ResourceKey; 2],
    ) -> Result<[ResourceFence; 2], AdmissionError> {
        if keys.iter().any(|key| self.resource_locks.contains_key(key)) {
            return Err(AdmissionError::ResourceConflict);
        }
        let fences = [
            ResourceFence {
                resource_key: keys[0].clone(),
                fence: checked_next_fence(self.resource_fences.get(&keys[0]).copied())?,
            },
            ResourceFence {
                resource_key: keys[1].clone(),
                fence: checked_next_fence(self.resource_fences.get(&keys[1]).copied())?,
            },
        ];
        for resource_fence in &fences {
            self.resource_fences
                .insert(resource_fence.resource_key.clone(), resource_fence.fence);
        }
        Ok(fences)
    }

    fn hold(&mut self, class: BudgetClass, claim: &BudgetClaim) -> Result<(), AdmissionError> {
        let account = self
            .budget_accounts
            .get_mut(&(class, claim.key().clone()))
            .ok_or(AdmissionError::BudgetUnavailable)?;
        let amount = claim.amount().get();
        if account.available < amount {
            return Err(AdmissionError::BudgetUnavailable);
        }
        account.available -= amount;
        account.held = account
            .held
            .checked_add(amount)
            .ok_or(AdmissionError::BudgetUnavailable)?;
        Ok(())
    }

    fn consume(&mut self, class: BudgetClass, hold: &BudgetHold) -> Result<(), AdmissionError> {
        let account = self
            .budget_accounts
            .get_mut(&(class, hold.key.clone()))
            .ok_or(AdmissionError::BudgetUnavailable)?;
        let amount = hold.amount.get();
        if account.held < amount {
            return Err(AdmissionError::BudgetUnavailable);
        }
        account.held -= amount;
        account.consumed = account
            .consumed
            .checked_add(amount)
            .ok_or(AdmissionError::BudgetUnavailable)?;
        Ok(())
    }

    fn release(&mut self, class: BudgetClass, hold: &BudgetHold) -> Result<(), AdmissionError> {
        let account = self
            .budget_accounts
            .get_mut(&(class, hold.key.clone()))
            .ok_or(AdmissionError::BudgetUnavailable)?;
        let amount = hold.amount.get();
        if account.held < amount {
            return Err(AdmissionError::BudgetUnavailable);
        }
        account.held -= amount;
        account.available = account
            .available
            .checked_add(amount)
            .filter(|available| *available <= account.capacity)
            .ok_or(AdmissionError::BudgetUnavailable)?;
        Ok(())
    }

    fn release_locks(
        &mut self,
        effect_id: &EffectId,
        fences: &[ResourceFence; 2],
    ) -> Result<(), AdmissionError> {
        if fences.iter().any(|fence| {
            self.resource_locks
                .get(&fence.resource_key)
                .is_none_or(|lock| lock.effect_id != *effect_id || lock.fence != fence.fence)
        }) {
            return Err(AdmissionError::ResourceConflict);
        }
        for fence in fences {
            self.resource_locks.remove(&fence.resource_key);
        }
        Ok(())
    }

    fn install_publication_reservation(
        &mut self,
        warrant: &ValidatedBody<PublicationWarrant>,
        binding: &ValidatedBody<IdempotencyBinding>,
        lease: &ValidatedBody<EffectLease>,
        effect_id: EffectId,
        fences: [ResourceFence; 2],
    ) {
        let binding_ref = BindingRef::publication(binding.reference().clone());
        self.install_common(
            warrant.payload().idempotency_key().clone(),
            warrant.reference().digest().clone(),
            &effect_id,
            binding_ref,
            &fences,
        );
        self.effects.insert(
            effect_id,
            EffectRecord {
                protocol: EffectProtocol::Publication,
                lifecycle: Lifecycle::Reserved,
                material: StoredReservation::Publication {
                    binding: binding.clone(),
                    lease: lease.clone(),
                },
                resource_fences: fences,
                reservation_hold: lease.payload().reservation_budget_hold.clone(),
                start_hold: lease.payload().start_budget_hold.clone(),
                warrant_digest: warrant.reference().digest().clone(),
                warrant_expires_at: warrant.payload().expires_at(),
                separation_generation: None,
            },
        );
    }

    fn install_separation_reservation(
        &mut self,
        warrant: &ValidatedBody<SeparationWarrant>,
        binding: &ValidatedBody<SeparationBinding>,
        lease: &ValidatedBody<SeparationLease>,
        effect_id: EffectId,
        fences: [ResourceFence; 2],
        current_custody_generation: U64Decimal,
    ) {
        let binding_ref = BindingRef::separation(binding.reference().clone());
        self.install_common(
            warrant.payload().idempotency_key().clone(),
            warrant.reference().digest().clone(),
            &effect_id,
            binding_ref,
            &fences,
        );
        self.effects.insert(
            effect_id,
            EffectRecord {
                protocol: EffectProtocol::Separation,
                lifecycle: Lifecycle::Reserved,
                material: StoredReservation::Separation {
                    binding: binding.clone(),
                    lease: lease.clone(),
                },
                resource_fences: fences,
                reservation_hold: lease.payload().reservation_budget_hold.clone(),
                start_hold: lease.payload().start_budget_hold.clone(),
                warrant_digest: warrant.reference().digest().clone(),
                warrant_expires_at: warrant.payload().expires_at(),
                separation_generation: Some(current_custody_generation),
            },
        );
    }

    fn install_common(
        &mut self,
        idempotency_key: IdempotencyKey,
        warrant_digest: Digest,
        effect_id: &EffectId,
        binding_digest: BindingRef,
        fences: &[ResourceFence; 2],
    ) {
        self.spent_warrants.insert(warrant_digest.clone());
        self.bindings.insert(
            idempotency_key,
            PermanentBinding {
                effect_id: effect_id.clone(),
                warrant_digest,
                binding_digest,
            },
        );
        for resource_fence in fences {
            self.resource_locks.insert(
                resource_fence.resource_key.clone(),
                ResourceLock {
                    effect_id: effect_id.clone(),
                    fence: resource_fence.fence,
                },
            );
        }
    }
}

impl EffectRecord {
    fn lease_is_live_at(&self, now: UnixNanoseconds) -> bool {
        match &self.material {
            StoredReservation::Publication { lease, .. } => lease.payload().is_live_at(now),
            StoredReservation::Separation { lease, .. } => lease.payload().is_live_at(now),
        }
    }

    const fn terminal_slots(&self) -> u64 {
        match self.protocol {
            EffectProtocol::Publication => PUBLICATION_TERMINAL_SLOTS,
            EffectProtocol::Separation => SEPARATION_TERMINAL_SLOTS,
        }
    }

    const fn valid_terminal_count(&self, count: u64) -> bool {
        match self.protocol {
            EffectProtocol::Publication => count == 2 || count == 3,
            EffectProtocol::Separation => count == 1 || count == 2,
        }
    }

    fn binding_ref(&self) -> BindingRef {
        match &self.material {
            StoredReservation::Publication { binding, .. } => {
                BindingRef::publication(binding.reference().clone())
            }
            StoredReservation::Separation { binding, .. } => {
                BindingRef::separation(binding.reference().clone())
            }
        }
    }
}

fn validate_publication_authority(
    projection: &LeaseProjection,
    policy: &ValidatedBody<AuthorityPolicy>,
    warrant: &ValidatedBody<PublicationWarrant>,
    approval: &ValidatedBody<PublicationApproval>,
    revoked_at: Option<UnixNanoseconds>,
    now: UnixNanoseconds,
) -> Result<(), AdmissionError> {
    if projection.policy_digest != *policy.reference().digest()
        || projection.installation_digest != *warrant.payload().installation_digest().digest()
        || warrant.payload().policy_digest() != policy.reference()
        || approval.payload().warrant_digest() != warrant.reference()
        || !policy
            .payload()
            .contains_approver(approval.payload().approver_id())
        || approval.payload().approved_at() < warrant.payload().issued_at()
        || approval.payload().approved_at() >= warrant.payload().expires_at()
        || now < approval.payload().approved_at()
    {
        return Err(AdmissionError::AuthorityRefused);
    }
    if policy.payload().require_distinct_approval_principal()
        && approval.payload().approver_id() == warrant.payload().proposer_id()
    {
        return Err(AdmissionError::AuthorityRefused);
    }
    if now >= warrant.payload().expires_at() {
        return Err(AdmissionError::WarrantExpired);
    }
    if revoked_at.is_some_and(|revoked_at| now >= revoked_at) {
        return Err(AdmissionError::WarrantRevoked);
    }
    ensure_nondecreasing_time(projection.last_transition_time, now)
}

fn validate_separation_authority(
    projection: &LeaseProjection,
    policy: &ValidatedBody<AuthorityPolicy>,
    warrant: &ValidatedBody<SeparationWarrant>,
    approval: &ValidatedBody<SeparationApproval>,
    revoked_at: Option<UnixNanoseconds>,
    now: UnixNanoseconds,
) -> Result<(), AdmissionError> {
    if projection.policy_digest != *policy.reference().digest()
        || projection.installation_digest != *warrant.payload().installation_digest().digest()
        || warrant.payload().policy_digest() != policy.reference()
        || approval.payload().warrant_digest() != warrant.reference()
        || !policy
            .payload()
            .contains_approver(approval.payload().approver_id())
        || approval.payload().approved_at() < warrant.payload().issued_at()
        || approval.payload().approved_at() >= warrant.payload().expires_at()
        || now < approval.payload().approved_at()
    {
        return Err(AdmissionError::AuthorityRefused);
    }
    if policy.payload().require_distinct_approval_principal()
        && approval.payload().approver_id() == warrant.payload().proposer_id()
    {
        return Err(AdmissionError::AuthorityRefused);
    }
    if now >= warrant.payload().expires_at() {
        return Err(AdmissionError::WarrantExpired);
    }
    if revoked_at.is_some_and(|revoked_at| now >= revoked_at) {
        return Err(AdmissionError::WarrantRevoked);
    }
    ensure_nondecreasing_time(projection.last_transition_time, now)
}

fn claim_to_hold(claim: &BudgetClaim) -> BudgetHold {
    BudgetHold {
        key: claim.key().clone(),
        amount: claim.amount(),
    }
}

fn require_graph_body<P: BodySpec>(
    graph: &BodyGraph,
    body: &ValidatedBody<P>,
) -> Result<(), AdmissionError> {
    graph
        .require_validated_body(body)
        .map_err(|_| AdmissionError::AuthorityRefused)
}

fn checked_next_fence(current: Option<Fence>) -> Result<Fence, AdmissionError> {
    current.map_or_else(
        || Ok(U64Decimal::from_u64(1)),
        |value| {
            value
                .checked_add(1)
                .map_err(|_| AdmissionError::CounterExhausted)
        },
    )
}

/// Computes the next custody generation without wrapping.
///
/// # Errors
///
/// Returns counter exhaustion at the maximum generation.
pub fn checked_next_generation(current: Option<U64Decimal>) -> Result<U64Decimal, AdmissionError> {
    current.map_or_else(
        || Ok(U64Decimal::from_u64(0)),
        |value| {
            value
                .checked_add(1)
                .map_err(|_| AdmissionError::CounterExhausted)
        },
    )
}

impl crate::body::sealed::BodySpec for IdempotencyBinding {}
impl BodySpec for IdempotencyBinding {
    type Tag = IdempotencyBindingTag;

    fn edges(&self) -> Vec<TypedEdge> {
        vec![TypedEdge::new(&self.warrant_digest)]
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Ok(())
    }
}

impl crate::body::sealed::BodySpec for EffectLease {}
impl BodySpec for EffectLease {
    type Tag = EffectLeaseTag;

    fn edges(&self) -> Vec<TypedEdge> {
        vec![TypedEdge::new(&self.binding_digest)]
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Self::validate_local(self)
    }
}

impl crate::body::sealed::BodySpec for SeparationBinding {}
impl BodySpec for SeparationBinding {
    type Tag = SeparationBindingTag;

    fn edges(&self) -> Vec<TypedEdge> {
        vec![TypedEdge::new(&self.warrant_digest)]
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Ok(())
    }
}

impl crate::body::sealed::BodySpec for SeparationLease {}
impl BodySpec for SeparationLease {
    type Tag = SeparationLeaseTag;

    fn edges(&self) -> Vec<TypedEdge> {
        vec![TypedEdge::new(&self.binding_digest)]
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Self::validate_local(self)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdempotencyBindingWire {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: PublicationWarrantRef,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EffectLeaseWire {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeparationBindingWire {
    idempotency_key: IdempotencyKey,
    effect_id: EffectId,
    warrant_digest: SeparationWarrantRef,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SeparationLeaseWire {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    resource_fences: [ResourceFence; 2],
    reservation_budget_hold: BudgetHold,
    start_budget_hold: BudgetHold,
    reserved_at: UnixNanoseconds,
    expires_at: UnixNanoseconds,
}

pub(crate) fn decode_idempotency_binding(body: Value) -> Result<IdempotencyBinding, BodyError> {
    let wire: IdempotencyBindingWire =
        serde_json::from_value(body).map_err(CanonicalError::from)?;
    Ok(IdempotencyBinding {
        idempotency_key: wire.idempotency_key,
        effect_id: wire.effect_id,
        warrant_digest: wire.warrant_digest,
    })
}

pub(crate) fn decode_effect_lease(body: Value) -> Result<EffectLease, BodyError> {
    let wire: EffectLeaseWire = serde_json::from_value(body).map_err(CanonicalError::from)?;
    Ok(EffectLease {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        resource_fences: wire.resource_fences,
        reservation_budget_hold: wire.reservation_budget_hold,
        start_budget_hold: wire.start_budget_hold,
        reserved_at: wire.reserved_at,
        expires_at: wire.expires_at,
    })
}

pub(crate) fn decode_separation_binding(body: Value) -> Result<SeparationBinding, BodyError> {
    let wire: SeparationBindingWire = serde_json::from_value(body).map_err(CanonicalError::from)?;
    Ok(SeparationBinding {
        idempotency_key: wire.idempotency_key,
        effect_id: wire.effect_id,
        warrant_digest: wire.warrant_digest,
    })
}

pub(crate) fn decode_separation_lease(body: Value) -> Result<SeparationLease, BodyError> {
    let wire: SeparationLeaseWire = serde_json::from_value(body).map_err(CanonicalError::from)?;
    Ok(SeparationLease {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        resource_fences: wire.resource_fences,
        reservation_budget_hold: wire.reservation_budget_hold,
        start_budget_hold: wire.start_budget_hold,
        reserved_at: wire.reserved_at,
        expires_at: wire.expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        authority::BudgetCapacity,
        body::{
            StaticArtifactPublishInputRef, StaticArtifactPublishPreconditionRef,
            StaticArtifactSeparationInputRef, StaticArtifactSeparationPreconditionRef,
        },
        scalar::{Hex256, Identifier, IncarnationId, SafeUInt},
    };

    fn test_digest(byte: char) -> Digest {
        Digest::parse(&format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
    }

    fn test_resource(byte: char) -> ResourceKey {
        ResourceKey::parse(test_digest(byte).as_str()).unwrap()
    }

    fn blank_projection() -> LeaseProjection {
        LeaseProjection {
            installation_digest: test_digest('e'),
            policy_digest: test_digest('f'),
            bindings: BTreeMap::new(),
            spent_warrants: BTreeSet::new(),
            budget_accounts: BTreeMap::new(),
            resource_fences: BTreeMap::new(),
            resource_locks: BTreeMap::new(),
            effects: BTreeMap::new(),
            terminal_sequence_reserve: U64Decimal::from_u64(0),
            head_sequence: U64Decimal::from_u64(0),
            last_transition_time: None,
        }
    }

    fn authority_projection() -> (
        LeaseProjection,
        ValidatedBody<AuthorityPolicy>,
        ValidatedBody<InstallationEnrollment>,
    ) {
        let budget_key = Identifier::parse("shared-budget").unwrap();
        let policy = validated_body(
            AuthorityPolicy::new(
                Identifier::parse("unit-policy").unwrap(),
                U64Decimal::from_u64(0),
                SortedUnique::new(vec![Identifier::parse("proposer").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("approver").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("revoker").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("witness").unwrap()]).unwrap(),
                true,
                SortedUnique::new(vec![BudgetCapacity::new(
                    budget_key.clone(),
                    SafeUInt::new(8).unwrap(),
                )])
                .unwrap(),
                SortedUnique::new(vec![BudgetCapacity::new(
                    budget_key.clone(),
                    SafeUInt::new(8).unwrap(),
                )])
                .unwrap(),
                Identifier::parse("trusted-clock").unwrap(),
                Identifier::parse("trusted-store").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let enrollment = validated_body(
            InstallationEnrollment::new(
                Identifier::parse("installation").unwrap(),
                IncarnationId::parse(test_digest('a').as_str()).unwrap(),
                policy.reference().clone(),
                UnixNanoseconds::parse("0").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut projection = blank_projection();
        projection.installation_digest = enrollment.reference().digest().clone();
        projection.policy_digest = policy.reference().digest().clone();
        for class in [BudgetClass::Reservation, BudgetClass::Start] {
            projection.budget_accounts.insert(
                (class, budget_key.clone()),
                BudgetBalance {
                    capacity: 8,
                    available: 8,
                    held: 0,
                    consumed: 0,
                },
            );
        }
        (projection, policy, enrollment)
    }

    fn budget_claim() -> BudgetClaim {
        BudgetClaim::new(
            Identifier::parse("shared-budget").unwrap(),
            BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
        )
    }

    fn publication_authority(
        policy: &ValidatedBody<AuthorityPolicy>,
        enrollment: &ValidatedBody<InstallationEnrollment>,
        resources: [ResourceKey; 2],
    ) -> (
        ValidatedBody<PublicationWarrant>,
        ValidatedBody<PublicationApproval>,
    ) {
        let warrant = validated_body(
            PublicationWarrant::new(
                enrollment,
                policy,
                Identifier::parse("proposer").unwrap(),
                StaticArtifactPublishInputRef::from_digest(test_digest('b')),
                StaticArtifactPublishPreconditionRef::from_digest(test_digest('c')),
                IdempotencyKey::parse("publication-unit-0001").unwrap(),
                resources,
                budget_claim(),
                budget_claim(),
                UnixNanoseconds::parse("0").unwrap(),
                UnixNanoseconds::parse("10000000000").unwrap(),
                Hex256::parse(&"d".repeat(64)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let approval = validated_body(
            PublicationApproval::new(
                &warrant,
                policy,
                Identifier::parse("approver").unwrap(),
                UnixNanoseconds::parse("500000000").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        (warrant, approval)
    }

    fn separation_authority(
        policy: &ValidatedBody<AuthorityPolicy>,
        enrollment: &ValidatedBody<InstallationEnrollment>,
        resources: [ResourceKey; 2],
        expires_at: &str,
    ) -> (
        ValidatedBody<SeparationWarrant>,
        ValidatedBody<SeparationApproval>,
    ) {
        let warrant = validated_body(
            SeparationWarrant::new(
                enrollment,
                policy,
                Identifier::parse("proposer").unwrap(),
                StaticArtifactSeparationInputRef::from_digest(test_digest('e')),
                StaticArtifactSeparationPreconditionRef::from_digest(test_digest('f')),
                IdempotencyKey::parse("separation-unit-0001").unwrap(),
                resources,
                budget_claim(),
                budget_claim(),
                UnixNanoseconds::parse("0").unwrap(),
                UnixNanoseconds::parse(expires_at).unwrap(),
                Hex256::parse(&"1".repeat(64)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let approval = validated_body(
            SeparationApproval::new(
                &warrant,
                policy,
                Identifier::parse("approver").unwrap(),
                UnixNanoseconds::parse("500000000").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        (warrant, approval)
    }

    #[test]
    fn first_fence_is_one_and_exhaustion_is_closed() {
        assert_eq!(checked_next_fence(None).unwrap().get(), 1);
        assert_eq!(
            checked_next_fence(Some(U64Decimal::from_u64(u64::MAX))),
            Err(AdmissionError::CounterExhausted),
        );
    }

    #[test]
    fn two_fence_plan_rolls_back_when_the_second_counter_is_exhausted() {
        let first = test_resource('1');
        let second = test_resource('2');
        let mut projection = blank_projection();
        projection
            .resource_fences
            .insert(second.clone(), U64Decimal::from_u64(u64::MAX));
        let before = projection.resource_fences.clone();
        assert_eq!(
            projection.plan_resource_fences(&[first, second]),
            Err(AdmissionError::CounterExhausted)
        );
        assert_eq!(projection.resource_fences, before);
    }

    #[test]
    fn lease_expiry_overflow_is_rejected() {
        let fences = [
            ResourceFence {
                resource_key: test_resource('1'),
                fence: U64Decimal::from_u64(1),
            },
            ResourceFence {
                resource_key: test_resource('2'),
                fence: U64Decimal::from_u64(1),
            },
        ];
        assert!(
            validate_lease_fields(
                &fences,
                UnixNanoseconds::parse("18446744073709551614").unwrap(),
                UnixNanoseconds::parse("18446744073709551615").unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn separation_rechecks_generation_expiry_and_revocation_at_start() {
        let (mut projection, policy, enrollment) = authority_projection();
        let resources = [test_resource('3'), test_resource('4')];
        let (warrant, approval) =
            separation_authority(&policy, &enrollment, resources, "3000000000");

        assert_eq!(
            projection
                .reserve_separation_resolved(
                    &policy,
                    &warrant,
                    &approval,
                    U64Decimal::from_u64(u64::MAX),
                    UnixNanoseconds::parse("1000000000").unwrap(),
                    None,
                )
                .unwrap_err(),
            AdmissionError::CounterExhausted,
        );
        assert!(!projection.is_warrant_spent(warrant.reference().digest()));
        assert!(projection.resource_fence(&test_resource('3')).is_none());

        let material = projection
            .reserve_separation_resolved(
                &policy,
                &warrant,
                &approval,
                U64Decimal::from_u64(7),
                UnixNanoseconds::parse("1000000000").unwrap(),
                None,
            )
            .unwrap();
        let before = (
            projection.head_sequence(),
            projection.terminal_sequence_reserve(),
        );
        assert_eq!(
            projection
                .start_separation_resolved(
                    material.effect_id(),
                    U64Decimal::from_u64(8),
                    UnixNanoseconds::parse("1500000000").unwrap(),
                    None,
                )
                .unwrap_err(),
            AdmissionError::PreconditionRefused,
        );
        assert_eq!(
            (
                projection.head_sequence(),
                projection.terminal_sequence_reserve(),
            ),
            before,
        );
        assert_eq!(
            projection
                .start_separation_resolved(
                    material.effect_id(),
                    U64Decimal::from_u64(7),
                    UnixNanoseconds::parse("3000000000").unwrap(),
                    None,
                )
                .unwrap_err(),
            AdmissionError::WarrantExpired,
        );
        assert_eq!(
            projection
                .start_separation_resolved(
                    material.effect_id(),
                    U64Decimal::from_u64(7),
                    UnixNanoseconds::parse("2000000000").unwrap(),
                    Some(UnixNanoseconds::parse("2000000000").unwrap()),
                )
                .unwrap_err(),
            AdmissionError::WarrantRevoked,
        );
        assert_eq!(
            (
                projection.head_sequence(),
                projection.terminal_sequence_reserve(),
            ),
            before,
        );
    }

    #[test]
    fn ordinary_events_interleave_with_both_started_families_and_release_unused_slots() {
        let (mut projection, policy, enrollment) = authority_projection();
        let publication_resources = [test_resource('1'), test_resource('2')];
        let separation_resources = [test_resource('3'), test_resource('4')];
        let (publication_warrant, publication_approval) =
            publication_authority(&policy, &enrollment, publication_resources.clone());
        let (separation_warrant, separation_approval) = separation_authority(
            &policy,
            &enrollment,
            separation_resources.clone(),
            "10000000000",
        );
        let publication = projection
            .reserve_publication_resolved(
                &policy,
                &publication_warrant,
                &publication_approval,
                UnixNanoseconds::parse("1000000000").unwrap(),
                None,
            )
            .unwrap();
        let separation = projection
            .reserve_separation_resolved(
                &policy,
                &separation_warrant,
                &separation_approval,
                U64Decimal::from_u64(7),
                UnixNanoseconds::parse("1100000000").unwrap(),
                None,
            )
            .unwrap();
        projection
            .mark_prepared(
                publication.effect_id(),
                UnixNanoseconds::parse("1200000000").unwrap(),
            )
            .unwrap();
        projection
            .start_publication_resolved(
                publication.effect_id(),
                UnixNanoseconds::parse("1300000000").unwrap(),
                None,
            )
            .unwrap();
        projection
            .start_separation_resolved(
                separation.effect_id(),
                U64Decimal::from_u64(7),
                UnixNanoseconds::parse("1400000000").unwrap(),
                None,
            )
            .unwrap();
        assert_eq!(projection.terminal_sequence_reserve().get(), 5);

        projection
            .admit_ordinary_events(2, UnixNanoseconds::parse("1500000000").unwrap())
            .unwrap();
        assert_eq!(projection.terminal_sequence_reserve().get(), 5);
        projection
            .terminalize(
                publication.effect_id(),
                2,
                UnixNanoseconds::parse("1600000000").unwrap(),
            )
            .unwrap();
        assert_eq!(projection.terminal_sequence_reserve().get(), 2);
        assert!(
            publication_resources
                .iter()
                .all(|key| projection.resource_lock(key).is_none())
        );
        assert!(
            separation_resources
                .iter()
                .all(|key| projection.resource_lock(key).is_some())
        );

        projection
            .admit_ordinary_events(2, UnixNanoseconds::parse("1700000000").unwrap())
            .unwrap();
        projection
            .terminalize(
                separation.effect_id(),
                1,
                UnixNanoseconds::parse("1800000000").unwrap(),
            )
            .unwrap();
        assert_eq!(projection.terminal_sequence_reserve().get(), 0);
        assert!(
            separation_resources
                .iter()
                .all(|key| projection.resource_lock(key).is_none())
        );
        assert_eq!(projection.head_sequence().get(), 12);
    }
}
