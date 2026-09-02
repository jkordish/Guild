//! Immutable authority policy, enrollment, warrant, approval, and revocation bodies.

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    body::{
        AuthorityPolicyRef, BodyError, BodySpec, InstallationEnrollmentRef,
        InstallationEnrollmentTag, PublicationApprovalTag, PublicationRevocationTag,
        PublicationWarrantRef, PublicationWarrantTag, SeparationApprovalTag,
        SeparationRevocationTag, SeparationWarrantRef, SeparationWarrantTag, SortedUnique,
        StaticArtifactPublishInputRef, StaticArtifactPublishPreconditionRef,
        StaticArtifactSeparationInputRef, StaticArtifactSeparationPreconditionRef, TypedEdge,
        ValidatedBody,
    },
    lease::{AdmissionError, BudgetClass},
    scalar::{
        Hex256, IdempotencyKey, Identifier, IncarnationId, ResourceKey, SafeUInt, U64Decimal,
        UnixNanoseconds, ValidationError,
    },
};

pub type PrincipalId = Identifier;
pub type WitnessId = Identifier;
pub type BudgetKey = Identifier;
pub type PolicyId = Identifier;
pub type InstallationId = Identifier;
pub type PolicyGeneration = U64Decimal;
pub type Fence = U64Decimal;
pub type CustodyGeneration = U64Decimal;

/// A nonzero budget quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct BudgetAmount(SafeUInt);

impl BudgetAmount {
    /// Constructs a nonzero protocol budget quantity.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::Zero`] when the amount is zero.
    pub const fn new(value: SafeUInt) -> Result<Self, ValidationError> {
        if value.get() == 0 {
            return Err(ValidationError::Zero {
                scalar: "BudgetAmount",
            });
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl<'de> Deserialize<'de> for BudgetAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(SafeUInt::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetCapacity {
    key: BudgetKey,
    capacity: SafeUInt,
}

impl BudgetCapacity {
    #[must_use]
    pub const fn new(key: BudgetKey, capacity: SafeUInt) -> Self {
        Self { key, capacity }
    }

    #[must_use]
    pub const fn key(&self) -> &BudgetKey {
        &self.key
    }

    #[must_use]
    pub const fn capacity(&self) -> SafeUInt {
        self.capacity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetClaim {
    key: BudgetKey,
    amount: BudgetAmount,
}

impl BudgetClaim {
    #[must_use]
    pub const fn new(key: BudgetKey, amount: BudgetAmount) -> Self {
        Self { key, amount }
    }

    #[must_use]
    pub const fn key(&self) -> &BudgetKey {
        &self.key
    }

    #[must_use]
    pub const fn amount(&self) -> BudgetAmount {
        self.amount
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    StaticArtifactPublish,
    StaticArtifactSeparation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorityPolicy {
    policy_id: PolicyId,
    generation: PolicyGeneration,
    proposer_ids: SortedUnique<PrincipalId>,
    approver_ids: SortedUnique<PrincipalId>,
    revoker_ids: SortedUnique<PrincipalId>,
    witness_ids: SortedUnique<WitnessId>,
    require_distinct_approval_principal: bool,
    reservation_budgets: SortedUnique<BudgetCapacity>,
    start_budgets: SortedUnique<BudgetCapacity>,
    trusted_clock_id: Identifier,
    trusted_store_id: Identifier,
}

impl AuthorityPolicy {
    #[allow(clippy::too_many_arguments)]
    /// Constructs the immutable generation-zero policy for one event chain.
    ///
    /// # Errors
    ///
    /// Rejects a nonzero generation, empty or oversized role, a policy that permits the same
    /// proposal and approval principal, or duplicate budget keys within one budget namespace.
    pub fn new(
        policy_id: PolicyId,
        generation: PolicyGeneration,
        proposer_ids: SortedUnique<PrincipalId>,
        approver_ids: SortedUnique<PrincipalId>,
        revoker_ids: SortedUnique<PrincipalId>,
        witness_ids: SortedUnique<WitnessId>,
        require_distinct_approval_principal: bool,
        reservation_budgets: SortedUnique<BudgetCapacity>,
        start_budgets: SortedUnique<BudgetCapacity>,
        trusted_clock_id: Identifier,
        trusted_store_id: Identifier,
    ) -> Result<Self, BodyError> {
        let policy = Self {
            policy_id,
            generation,
            proposer_ids,
            approver_ids,
            revoker_ids,
            witness_ids,
            require_distinct_approval_principal,
            reservation_budgets,
            start_budgets,
            trusted_clock_id,
            trusted_store_id,
        };
        policy.validate_local()?;
        Ok(policy)
    }

    pub(crate) fn validate_local(&self) -> Result<(), BodyError> {
        if self.generation.get() != 0 {
            return Err(BodyError::Local(
                "authority policy generation must equal zero".to_owned(),
            ));
        }
        if !self.require_distinct_approval_principal {
            return Err(BodyError::Local(
                "authority policy must require a distinct approval principal".to_owned(),
            ));
        }
        for (name, values) in [
            ("proposerIds", &self.proposer_ids),
            ("approverIds", &self.approver_ids),
            ("revokerIds", &self.revoker_ids),
            ("witnessIds", &self.witness_ids),
        ] {
            if values.is_empty() || values.len() > 1_024 {
                return Err(BodyError::Local(format!(
                    "{name} must contain 1..=1024 principals"
                )));
            }
        }
        validate_budget_keys(&self.reservation_budgets, "reservationBudgets")?;
        validate_budget_keys(&self.start_budgets, "startBudgets")?;
        Ok(())
    }

    #[must_use]
    pub const fn policy_id(&self) -> &PolicyId {
        &self.policy_id
    }

    #[must_use]
    pub const fn generation(&self) -> PolicyGeneration {
        self.generation
    }

    #[must_use]
    pub const fn proposer_ids(&self) -> &SortedUnique<PrincipalId> {
        &self.proposer_ids
    }

    #[must_use]
    pub const fn approver_ids(&self) -> &SortedUnique<PrincipalId> {
        &self.approver_ids
    }

    #[must_use]
    pub const fn revoker_ids(&self) -> &SortedUnique<PrincipalId> {
        &self.revoker_ids
    }

    #[must_use]
    pub const fn witness_ids(&self) -> &SortedUnique<WitnessId> {
        &self.witness_ids
    }

    #[must_use]
    pub const fn require_distinct_approval_principal(&self) -> bool {
        self.require_distinct_approval_principal
    }

    #[must_use]
    pub const fn reservation_budgets(&self) -> &SortedUnique<BudgetCapacity> {
        &self.reservation_budgets
    }

    #[must_use]
    pub const fn start_budgets(&self) -> &SortedUnique<BudgetCapacity> {
        &self.start_budgets
    }

    #[must_use]
    pub const fn trusted_clock_id(&self) -> &Identifier {
        &self.trusted_clock_id
    }

    #[must_use]
    pub const fn trusted_store_id(&self) -> &Identifier {
        &self.trusted_store_id
    }

    #[must_use]
    pub fn contains_proposer(&self, principal: &PrincipalId) -> bool {
        self.proposer_ids.as_slice().contains(principal)
    }

    #[must_use]
    pub fn contains_approver(&self, principal: &PrincipalId) -> bool {
        self.approver_ids.as_slice().contains(principal)
    }

    #[must_use]
    pub fn contains_revoker(&self, principal: &PrincipalId) -> bool {
        self.revoker_ids.as_slice().contains(principal)
    }

    #[must_use]
    pub fn budget_capacity(&self, class: BudgetClass, key: &BudgetKey) -> Option<SafeUInt> {
        let budgets = match class {
            BudgetClass::Reservation => &self.reservation_budgets,
            BudgetClass::Start => &self.start_budgets,
        };
        budgets
            .as_slice()
            .iter()
            .find(|budget| budget.key() == key)
            .map(BudgetCapacity::capacity)
    }
}

fn validate_budget_keys(
    budgets: &SortedUnique<BudgetCapacity>,
    name: &str,
) -> Result<(), BodyError> {
    if budgets.len() > 1_024 {
        return Err(BodyError::Local(format!(
            "{name} exceeds the protocol maximum 1024"
        )));
    }
    let mut keys = std::collections::BTreeSet::new();
    if budgets
        .as_slice()
        .iter()
        .any(|budget| !keys.insert(budget.key()))
    {
        return Err(BodyError::Local(format!(
            "{name} contains a duplicate budget key"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallationEnrollment {
    installation_id: InstallationId,
    incarnation: IncarnationId,
    policy_digest: AuthorityPolicyRef,
    enrolled_at: UnixNanoseconds,
}

impl InstallationEnrollment {
    /// Constructs an enrollment naming one exact immutable policy.
    ///
    /// # Errors
    ///
    /// This fallible boundary is retained for closed local validation.
    pub const fn new(
        installation_id: InstallationId,
        incarnation: IncarnationId,
        policy_digest: AuthorityPolicyRef,
        enrolled_at: UnixNanoseconds,
    ) -> Result<Self, BodyError> {
        Ok(Self {
            installation_id,
            incarnation,
            policy_digest,
            enrolled_at,
        })
    }

    #[must_use]
    pub const fn installation_id(&self) -> &InstallationId {
        &self.installation_id
    }

    #[must_use]
    pub const fn incarnation(&self) -> &IncarnationId {
        &self.incarnation
    }

    #[must_use]
    pub const fn policy_digest(&self) -> &AuthorityPolicyRef {
        &self.policy_digest
    }

    #[must_use]
    pub const fn enrolled_at(&self) -> UnixNanoseconds {
        self.enrolled_at
    }
}

macro_rules! warrant_type {
    (
        $name:ident, $tag:ty, $effect:expr, $input:ty, $precondition:ty
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name {
            installation_digest: InstallationEnrollmentRef,
            policy_digest: AuthorityPolicyRef,
            policy_generation: PolicyGeneration,
            effect_kind: EffectKind,
            proposer_id: PrincipalId,
            input_digest: $input,
            precondition_digest: $precondition,
            idempotency_key: IdempotencyKey,
            resource_keys: [ResourceKey; 2],
            reservation_budget: BudgetClaim,
            start_budget: BudgetClaim,
            issued_at: UnixNanoseconds,
            expires_at: UnixNanoseconds,
            nonce: Hex256,
        }

        impl $name {
            #[allow(clippy::too_many_arguments)]
            /// Constructs a one-shot warrant bound to one immutable enrollment and policy.
            ///
            /// # Errors
            ///
            /// Rejects policy mismatch, unenrolled proposal authority, unavailable budget,
            /// nonzero generation, the wrong effect family, invalid times, or resource keys.
            pub fn new(
                enrollment: &ValidatedBody<InstallationEnrollment>,
                policy: &ValidatedBody<AuthorityPolicy>,
                proposer_id: PrincipalId,
                input_digest: $input,
                precondition_digest: $precondition,
                idempotency_key: IdempotencyKey,
                resource_keys: [ResourceKey; 2],
                reservation_budget: BudgetClaim,
                start_budget: BudgetClaim,
                issued_at: UnixNanoseconds,
                expires_at: UnixNanoseconds,
                nonce: Hex256,
            ) -> Result<Self, AdmissionError> {
                if enrollment.payload().policy_digest() != policy.reference()
                    || !policy.payload().contains_proposer(&proposer_id)
                    || policy.payload().generation().get() != 0
                    || policy
                        .payload()
                        .budget_capacity(BudgetClass::Reservation, reservation_budget.key())
                        .is_none_or(|capacity| capacity.get() < reservation_budget.amount().get())
                    || policy
                        .payload()
                        .budget_capacity(BudgetClass::Start, start_budget.key())
                        .is_none_or(|capacity| capacity.get() < start_budget.amount().get())
                {
                    return Err(AdmissionError::AuthorityRefused);
                }
                let warrant = Self {
                    installation_digest: enrollment.reference().clone(),
                    policy_digest: policy.reference().clone(),
                    policy_generation: policy.payload().generation(),
                    effect_kind: $effect,
                    proposer_id,
                    input_digest,
                    precondition_digest,
                    idempotency_key,
                    resource_keys,
                    reservation_budget,
                    start_budget,
                    issued_at,
                    expires_at,
                    nonce,
                };
                warrant.validate_local().map_err(AdmissionError::from)?;
                Ok(warrant)
            }

            pub(crate) fn validate_local(&self) -> Result<(), BodyError> {
                if self.effect_kind != $effect {
                    return Err(BodyError::Local(
                        "warrant effect kind does not match its body family".to_owned(),
                    ));
                }
                if self.policy_generation.get() != 0 {
                    return Err(BodyError::Local(
                        "warrant policy generation must equal zero".to_owned(),
                    ));
                }
                if self.issued_at >= self.expires_at {
                    return Err(BodyError::Local(
                        "warrant issuedAt must be strictly before expiresAt".to_owned(),
                    ));
                }
                if self.resource_keys[0] >= self.resource_keys[1] {
                    return Err(BodyError::Local(
                        "warrant resource keys must be two distinct sorted keys".to_owned(),
                    ));
                }
                Ok(())
            }

            #[must_use]
            pub const fn installation_digest(&self) -> &InstallationEnrollmentRef {
                &self.installation_digest
            }

            #[must_use]
            pub const fn policy_digest(&self) -> &AuthorityPolicyRef {
                &self.policy_digest
            }

            #[must_use]
            pub const fn policy_generation(&self) -> PolicyGeneration {
                self.policy_generation
            }

            #[must_use]
            pub const fn effect_kind(&self) -> EffectKind {
                self.effect_kind
            }

            #[must_use]
            pub const fn proposer_id(&self) -> &PrincipalId {
                &self.proposer_id
            }

            #[must_use]
            pub const fn input_digest(&self) -> &$input {
                &self.input_digest
            }

            #[must_use]
            pub const fn precondition_digest(&self) -> &$precondition {
                &self.precondition_digest
            }

            #[must_use]
            pub const fn idempotency_key(&self) -> &IdempotencyKey {
                &self.idempotency_key
            }

            #[must_use]
            pub const fn resource_keys(&self) -> &[ResourceKey; 2] {
                &self.resource_keys
            }

            #[must_use]
            pub const fn reservation_budget(&self) -> &BudgetClaim {
                &self.reservation_budget
            }

            #[must_use]
            pub const fn start_budget(&self) -> &BudgetClaim {
                &self.start_budget
            }

            #[must_use]
            pub const fn issued_at(&self) -> UnixNanoseconds {
                self.issued_at
            }

            #[must_use]
            pub const fn expires_at(&self) -> UnixNanoseconds {
                self.expires_at
            }

            #[must_use]
            pub const fn nonce(&self) -> &Hex256 {
                &self.nonce
            }

            #[must_use]
            pub fn is_live_at(&self, now: UnixNanoseconds) -> bool {
                now < self.expires_at
            }
        }
    };
}

warrant_type!(
    PublicationWarrant,
    PublicationWarrantTag,
    EffectKind::StaticArtifactPublish,
    StaticArtifactPublishInputRef,
    StaticArtifactPublishPreconditionRef
);
warrant_type!(
    SeparationWarrant,
    SeparationWarrantTag,
    EffectKind::StaticArtifactSeparation,
    StaticArtifactSeparationInputRef,
    StaticArtifactSeparationPreconditionRef
);

macro_rules! approval_type {
    ($name:ident, $tag:ty, $warrant:ty, $warrant_ref:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name {
            warrant_digest: $warrant_ref,
            approver_id: PrincipalId,
            approved_at: UnixNanoseconds,
        }

        impl $name {
            /// Approves one exact warrant digest under its immutable policy.
            ///
            /// # Errors
            ///
            /// Rejects policy mismatch, an unenrolled or same proposal principal, or a time
            /// outside the warrant's half-open validity interval.
            pub fn new(
                warrant: &ValidatedBody<$warrant>,
                policy: &ValidatedBody<AuthorityPolicy>,
                approver_id: PrincipalId,
                approved_at: UnixNanoseconds,
            ) -> Result<Self, AdmissionError> {
                if warrant.payload().policy_digest() != policy.reference()
                    || !policy.payload().contains_approver(&approver_id)
                    || (policy.payload().require_distinct_approval_principal()
                        && warrant.payload().proposer_id() == &approver_id)
                    || approved_at < warrant.payload().issued_at()
                    || approved_at >= warrant.payload().expires_at()
                {
                    return Err(AdmissionError::AuthorityRefused);
                }
                Ok(Self {
                    warrant_digest: warrant.reference().clone(),
                    approver_id,
                    approved_at,
                })
            }

            #[must_use]
            pub const fn warrant_digest(&self) -> &$warrant_ref {
                &self.warrant_digest
            }

            #[must_use]
            pub const fn approver_id(&self) -> &PrincipalId {
                &self.approver_id
            }

            #[must_use]
            pub const fn approved_at(&self) -> UnixNanoseconds {
                self.approved_at
            }
        }
    };
}

approval_type!(
    PublicationApproval,
    PublicationApprovalTag,
    PublicationWarrant,
    PublicationWarrantRef
);
approval_type!(
    SeparationApproval,
    SeparationApprovalTag,
    SeparationWarrant,
    SeparationWarrantRef
);

macro_rules! revocation_type {
    ($name:ident, $tag:ty, $warrant:ty, $warrant_ref:ty, $approval:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        pub struct $name {
            warrant_digest: $warrant_ref,
            revoker_id: PrincipalId,
            revoked_at: UnixNanoseconds,
            reason: Identifier,
        }

        impl $name {
            /// Revokes one previously approved warrant as an enrolled revoker.
            ///
            /// # Errors
            ///
            /// Rejects policy/warrant mismatch, unenrolled revocation authority, or a revocation
            /// time preceding the approval.
            pub fn new(
                warrant: &ValidatedBody<$warrant>,
                approval: &ValidatedBody<$approval>,
                policy: &ValidatedBody<AuthorityPolicy>,
                revoker_id: PrincipalId,
                revoked_at: UnixNanoseconds,
                reason: Identifier,
            ) -> Result<Self, AdmissionError> {
                if warrant.payload().policy_digest() != policy.reference()
                    || approval.payload().warrant_digest() != warrant.reference()
                    || !policy.payload().contains_revoker(&revoker_id)
                    || revoked_at < approval.payload().approved_at()
                {
                    return Err(AdmissionError::AuthorityRefused);
                }
                Ok(Self {
                    warrant_digest: warrant.reference().clone(),
                    revoker_id,
                    revoked_at,
                    reason,
                })
            }

            #[must_use]
            pub const fn warrant_digest(&self) -> &$warrant_ref {
                &self.warrant_digest
            }

            #[must_use]
            pub const fn revoker_id(&self) -> &PrincipalId {
                &self.revoker_id
            }

            #[must_use]
            pub const fn revoked_at(&self) -> UnixNanoseconds {
                self.revoked_at
            }

            #[must_use]
            pub const fn reason(&self) -> &Identifier {
                &self.reason
            }

            #[must_use]
            pub fn is_effective_at(&self, now: UnixNanoseconds) -> bool {
                now >= self.revoked_at
            }
        }
    };
}

revocation_type!(
    PublicationRevocation,
    PublicationRevocationTag,
    PublicationWarrant,
    PublicationWarrantRef,
    PublicationApproval
);
revocation_type!(
    SeparationRevocation,
    SeparationRevocationTag,
    SeparationWarrant,
    SeparationWarrantRef,
    SeparationApproval
);

/// Enforces nondecreasing event time against the current anchored head.
///
/// # Errors
///
/// Returns [`AdmissionError::TimeRegression`] when `current` precedes `previous`.
pub fn ensure_nondecreasing_time(
    previous: Option<UnixNanoseconds>,
    current: UnixNanoseconds,
) -> Result<(), AdmissionError> {
    if previous.is_some_and(|previous| current < previous) {
        return Err(AdmissionError::TimeRegression);
    }
    Ok(())
}

impl crate::body::sealed::BodySpec for AuthorityPolicy {}
impl BodySpec for AuthorityPolicy {
    type Tag = crate::body::AuthorityPolicyTag;

    fn edges(&self) -> Vec<TypedEdge> {
        Vec::new()
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Self::validate_local(self)
    }
}

impl crate::body::sealed::BodySpec for InstallationEnrollment {}
impl BodySpec for InstallationEnrollment {
    type Tag = InstallationEnrollmentTag;

    fn edges(&self) -> Vec<TypedEdge> {
        vec![TypedEdge::new(&self.policy_digest)]
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Ok(())
    }
}

macro_rules! warrant_body_spec {
    ($warrant:ty, $tag:ty) => {
        impl crate::body::sealed::BodySpec for $warrant {}
        impl BodySpec for $warrant {
            type Tag = $tag;

            fn edges(&self) -> Vec<TypedEdge> {
                vec![
                    TypedEdge::new(&self.installation_digest),
                    TypedEdge::new(&self.policy_digest),
                    TypedEdge::new(&self.input_digest),
                    TypedEdge::new(&self.precondition_digest),
                ]
            }

            fn validate_local(&self) -> Result<(), BodyError> {
                Self::validate_local(self)
            }
        }
    };
}

warrant_body_spec!(PublicationWarrant, PublicationWarrantTag);
warrant_body_spec!(SeparationWarrant, SeparationWarrantTag);

macro_rules! authority_leaf_body_spec {
    ($payload:ty, $tag:ty) => {
        impl crate::body::sealed::BodySpec for $payload {}
        impl BodySpec for $payload {
            type Tag = $tag;

            fn edges(&self) -> Vec<TypedEdge> {
                vec![TypedEdge::new(&self.warrant_digest)]
            }

            fn validate_local(&self) -> Result<(), BodyError> {
                Ok(())
            }
        }
    };
}

authority_leaf_body_spec!(PublicationApproval, PublicationApprovalTag);
authority_leaf_body_spec!(PublicationRevocation, PublicationRevocationTag);
authority_leaf_body_spec!(SeparationApproval, SeparationApprovalTag);
authority_leaf_body_spec!(SeparationRevocation, SeparationRevocationTag);
