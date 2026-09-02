//! Closed evidence vocabulary and deterministic receipt, deed, and custody derivation.

#![allow(
    dead_code,
    reason = "Task 7 installs crate-private classifier primitives consumed by Tasks 11-13"
)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    authority::{CustodyGeneration, WitnessId},
    body::{
        BodyError, BodySpec, CausalityAssessmentRef, CausalityAssessmentTag, CustodyRecordTag,
        EffectReceiptRef, EffectReceiptTag, IdempotencyBindingRef, LocalFileObservation,
        LocalFileObservationRef, OptionalValue, PreparedArtifactRef, ProtocolRef,
        PublicationEvidenceRef, PublicationEvidenceTag, RecoveryAssessmentTag, ResourceDeedRef,
        ResourceDeedTag, SeparationBindingRef, SeparationEvidenceRef, SeparationEvidenceTag,
        SeparationReceiptTag, SortedUnique, TypedEdge, ValidatedBody, XattrValueRef,
        validated_body,
    },
    canonical::{CanonicalError, canonical_bytes},
    lease::derive_resource_key,
    scalar::{
        ArtifactName, ByteLength, EffectId, IncarnationId, LogicalAddress, RawDigest, ResourceKey,
        UnixNanoseconds,
    },
};

macro_rules! closed_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $($variant),+
        }
    };
}

closed_enum!(EvidenceLimitation {
    WitnessUnavailable,
    UnsupportedIdentity,
    NonAtomicExternalOperation,
    StaleObservation,
    ConflictingObservation,
});

closed_enum!(CommandReport {
    ReportedSuccess,
    ReportedNoEffect,
    ReportedUncertain,
    NotAvailable,
});

closed_enum!(PublicationPostcondition {
    ExactRequested,
    AuthoritativeAbsence,
    PriorStateUnchanged,
    ContentMismatch,
    Ambiguous,
});

closed_enum!(SeparationPostcondition {
    ExactQuarantine,
    NoMove,
    Ambiguous,
});

closed_enum!(CausalityOutcome {
    ExactPreparedIncarnation,
    DifferentIncarnation,
    DuplicateIncarnation,
    Ambiguous,
    Unsupported,
});

closed_enum!(ReceiptState {
    Verified,
    Failed,
    Indeterminate,
});

closed_enum!(MutationMode {
    Conditional,
    Unconditional,
});

closed_enum!(WitnessStatus {
    AuthenticatedEnrolled,
    Unauthenticated,
    Unenrolled,
});

closed_enum!(OperationResult {
    NotAttempted,
    PreparedOnly,
    PublishReportedSuccess,
    PublishReportedNoEffect,
    PublishReportedUncertain,
    PublishRecovered,
    QuarantineReportedSuccess,
    QuarantineReportedNoEffect,
    QuarantineReportedUncertain,
    QuarantineRecovered,
});

closed_enum!(ReceiptReason {
    ArtifactVerified,
    SeparationVerified,
    SourceChanged,
    SourceInvalidAfterStart,
    DigestMismatchAfterStart,
    PublicationNoEffect,
    AuthoritativeAbsence,
    SeparationPreconditionRefused,
    SeparationNoMove,
    WitnessUnavailable,
    PublicationAmbiguous,
    IncarnationAmbiguous,
    DuplicateIncarnation,
    SeparationAmbiguous,
    UnsupportedIdentity,
});

closed_enum!(CustodyState {
    Owned,
    Quarantined,
    Absent,
    Disputed,
});

/// Probe facts accepted by a future public transition API.
///
/// This deliberately has no caller-selected classification, receipt, deed, generation, or
/// custody fields.
#[allow(
    clippy::large_enum_variant,
    reason = "protocol §7.1 fixes the public attempt fields and forbids boxing the observation"
)]
#[derive(Debug, Clone)]
pub enum ObservationAttempt {
    Observed {
        observation: ValidatedBody<LocalFileObservation>,
        witness: WitnessStatus,
    },
    Unavailable {
        logical_address: LogicalAddress,
        witness_id: WitnessId,
        attempted_at: UnixNanoseconds,
    },
    Unsupported {
        logical_address: LogicalAddress,
        witness_id: WitnessId,
        attempted_at: UnixNanoseconds,
    },
    Conflicting {
        observations: SortedUnique<ValidatedBody<LocalFileObservation>>,
        witness: WitnessStatus,
        attempted_at: UnixNanoseconds,
    },
}

/// The exact persisted after-observation union from protocol §7.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObservationEvidence {
    Observed {
        digest: LocalFileObservationRef,
    },
    Unavailable {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
    },
    Unsupported {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
    },
    Conflicting {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
        #[serde(rename = "observationDigests")]
        observation_digests: SortedUnique<LocalFileObservationRef>,
    },
}

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ObservationEvidenceWire {
    Observed {
        digest: LocalFileObservationRef,
    },
    Unavailable {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
    },
    Unsupported {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
    },
    Conflicting {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: WitnessId,
        #[serde(rename = "attemptedAt")]
        attempted_at: UnixNanoseconds,
        #[serde(rename = "observationDigests")]
        observation_digests: SortedUnique<LocalFileObservationRef>,
    },
}

impl<'de> Deserialize<'de> for ObservationEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ObservationEvidenceWire::deserialize(deserializer)?;
        let value = match wire {
            ObservationEvidenceWire::Observed { digest } => Self::Observed { digest },
            ObservationEvidenceWire::Unavailable {
                logical_address,
                witness_id,
                attempted_at,
            } => Self::Unavailable {
                logical_address,
                witness_id,
                attempted_at,
            },
            ObservationEvidenceWire::Unsupported {
                logical_address,
                witness_id,
                attempted_at,
            } => Self::Unsupported {
                logical_address,
                witness_id,
                attempted_at,
            },
            ObservationEvidenceWire::Conflicting {
                logical_address,
                witness_id,
                attempted_at,
                observation_digests,
            } => Self::Conflicting {
                logical_address,
                witness_id,
                attempted_at,
                observation_digests,
            },
        };
        value.validate_local().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}

impl ObservationEvidence {
    #[must_use]
    pub const fn logical_address(&self) -> Option<&LogicalAddress> {
        match self {
            Self::Observed { .. } => None,
            Self::Unavailable {
                logical_address, ..
            }
            | Self::Unsupported {
                logical_address, ..
            }
            | Self::Conflicting {
                logical_address, ..
            } => Some(logical_address),
        }
    }

    #[must_use]
    pub const fn witness_id(&self) -> Option<&WitnessId> {
        match self {
            Self::Observed { .. } => None,
            Self::Unavailable { witness_id, .. }
            | Self::Unsupported { witness_id, .. }
            | Self::Conflicting { witness_id, .. } => Some(witness_id),
        }
    }

    #[must_use]
    pub const fn attempted_at(&self) -> Option<UnixNanoseconds> {
        match self {
            Self::Observed { .. } => None,
            Self::Unavailable { attempted_at, .. }
            | Self::Unsupported { attempted_at, .. }
            | Self::Conflicting { attempted_at, .. } => Some(*attempted_at),
        }
    }

    #[must_use]
    pub const fn observed_digest(&self) -> Option<&LocalFileObservationRef> {
        match self {
            Self::Observed { digest } => Some(digest),
            _ => None,
        }
    }

    #[must_use]
    pub const fn conflicting_digests(&self) -> Option<&SortedUnique<LocalFileObservationRef>> {
        match self {
            Self::Conflicting {
                observation_digests,
                ..
            } => Some(observation_digests),
            _ => None,
        }
    }

    fn edges(&self) -> Vec<TypedEdge> {
        match self {
            Self::Observed { digest } => vec![TypedEdge::new(digest)],
            Self::Conflicting {
                observation_digests,
                ..
            } => observation_digests
                .as_slice()
                .iter()
                .map(TypedEdge::new)
                .collect(),
            Self::Unavailable { .. } | Self::Unsupported { .. } => Vec::new(),
        }
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        if let Self::Conflicting {
            observation_digests,
            ..
        } = self
            && !(2..=1_024).contains(&observation_digests.len())
        {
            return Err(BodyError::Local(
                "conflicting observation evidence requires 2..=1024 digests".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Publication evidence body. Classification fields are private and derived by this module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicationEvidence {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    prepared_artifact_digest: PreparedArtifactRef,
    command_report: CommandReport,
    source_before_observation_digest: LocalFileObservationRef,
    target_before_observation_digest: LocalFileObservationRef,
    source_after: ObservationEvidence,
    target_after: ObservationEvidence,
    postcondition: PublicationPostcondition,
    limitations: SortedUnique<EvidenceLimitation>,
    assessed_at: UnixNanoseconds,
}

/// Independent publication causality body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CausalityAssessment {
    effect_id: EffectId,
    evidence_digest: PublicationEvidenceRef,
    outcome: CausalityOutcome,
}

/// Terminal publication receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EffectReceipt {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    evidence_digest: PublicationEvidenceRef,
    causality_digest: CausalityAssessmentRef,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
    terminal_at: UnixNanoseconds,
}

/// Deed-backed custody proof. Only the private successful-classification proof can mint one.
///
/// ```compile_fail
/// use guild_effect_kernel::evidence::ResourceDeed;
/// let _forged = ResourceDeed::new();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceDeed {
    resource_key: ResourceKey,
    logical_address: LogicalAddress,
    artifact_name: ArtifactName,
    content_digest: RawDigest,
    byte_length: ByteLength,
    incarnation: IncarnationId,
    publication_receipt_digest: EffectReceiptRef,
    custody_generation: CustodyGeneration,
}

/// Separation evidence body. Classification fields are private and derived by this module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeparationEvidence {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    deed_digest: ResourceDeedRef,
    active_before_observation_digest: LocalFileObservationRef,
    quarantine_before_observation_digest: LocalFileObservationRef,
    active_after: ObservationEvidence,
    quarantine_after: ObservationEvidence,
    command_report: CommandReport,
    postcondition: SeparationPostcondition,
    limitations: SortedUnique<EvidenceLimitation>,
    assessed_at: UnixNanoseconds,
}

/// Terminal separation receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SeparationReceipt {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    evidence_digest: SeparationEvidenceRef,
    deed_digest: ResourceDeedRef,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
    terminal_at: UnixNanoseconds,
    next_custody_generation: CustodyGeneration,
}

pub type TerminalReceiptRef = ProtocolRef<EffectReceiptTag, SeparationReceiptTag>;
pub type TerminalBindingRef =
    ProtocolRef<crate::body::IdempotencyBindingTag, crate::body::SeparationBindingTag>;
pub type TerminalEvidenceRef = ProtocolRef<PublicationEvidenceTag, SeparationEvidenceTag>;

/// One immutable custody projection record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustodyRecord {
    resource_key: ResourceKey,
    deed_digest: OptionalValue<ResourceDeedRef>,
    custody_generation: CustodyGeneration,
    state: CustodyState,
    terminal_receipt: TerminalReceiptRef,
    active_address: LogicalAddress,
    quarantine_address: OptionalValue<LogicalAddress>,
}

/// A recovery audit body that binds one matching protocol family end to end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecoveryAssessment {
    effect_id: EffectId,
    binding_digest: TerminalBindingRef,
    evidence_digest: TerminalEvidenceRef,
    receipt_digest: TerminalReceiptRef,
    recovered_at: UnixNanoseconds,
    state: ReceiptState,
    reason: ReceiptReason,
}

macro_rules! getter {
    ($name:ident, $field:ident, $ty:ty) => {
        #[must_use]
        pub const fn $field(&self) -> &$ty {
            &self.$field
        }
    };
}

impl PublicationEvidence {
    getter!(PublicationEvidence, effect_id, EffectId);
    getter!(PublicationEvidence, binding_digest, IdempotencyBindingRef);
    getter!(
        PublicationEvidence,
        prepared_artifact_digest,
        PreparedArtifactRef
    );
    getter!(
        PublicationEvidence,
        source_before_observation_digest,
        LocalFileObservationRef
    );
    getter!(
        PublicationEvidence,
        target_before_observation_digest,
        LocalFileObservationRef
    );

    #[must_use]
    pub const fn command_report(&self) -> CommandReport {
        self.command_report
    }

    #[must_use]
    pub const fn source_after(&self) -> &ObservationEvidence {
        &self.source_after
    }

    #[must_use]
    pub const fn target_after(&self) -> &ObservationEvidence {
        &self.target_after
    }

    #[must_use]
    pub const fn postcondition(&self) -> PublicationPostcondition {
        self.postcondition
    }

    #[must_use]
    pub const fn limitations(&self) -> &SortedUnique<EvidenceLimitation> {
        &self.limitations
    }

    #[must_use]
    pub const fn assessed_at(&self) -> UnixNanoseconds {
        self.assessed_at
    }
}

impl CausalityAssessment {
    getter!(CausalityAssessment, effect_id, EffectId);
    getter!(CausalityAssessment, evidence_digest, PublicationEvidenceRef);

    #[must_use]
    pub const fn outcome(&self) -> CausalityOutcome {
        self.outcome
    }
}

impl EffectReceipt {
    getter!(EffectReceipt, effect_id, EffectId);
    getter!(EffectReceipt, binding_digest, IdempotencyBindingRef);
    getter!(EffectReceipt, evidence_digest, PublicationEvidenceRef);
    getter!(EffectReceipt, causality_digest, CausalityAssessmentRef);

    #[must_use]
    pub const fn state(&self) -> ReceiptState {
        self.state
    }

    #[must_use]
    pub const fn result(&self) -> OperationResult {
        self.result
    }

    #[must_use]
    pub const fn reason(&self) -> ReceiptReason {
        self.reason
    }

    #[must_use]
    pub const fn terminal_at(&self) -> UnixNanoseconds {
        self.terminal_at
    }
}

impl ResourceDeed {
    getter!(ResourceDeed, resource_key, ResourceKey);
    getter!(ResourceDeed, logical_address, LogicalAddress);
    getter!(ResourceDeed, artifact_name, ArtifactName);
    getter!(ResourceDeed, content_digest, RawDigest);
    getter!(ResourceDeed, incarnation, IncarnationId);
    getter!(ResourceDeed, publication_receipt_digest, EffectReceiptRef);

    #[must_use]
    pub const fn byte_length(&self) -> ByteLength {
        self.byte_length
    }

    #[must_use]
    pub const fn custody_generation(&self) -> CustodyGeneration {
        self.custody_generation
    }
}

impl SeparationEvidence {
    getter!(SeparationEvidence, effect_id, EffectId);
    getter!(SeparationEvidence, binding_digest, SeparationBindingRef);
    getter!(SeparationEvidence, deed_digest, ResourceDeedRef);
    getter!(
        SeparationEvidence,
        active_before_observation_digest,
        LocalFileObservationRef
    );
    getter!(
        SeparationEvidence,
        quarantine_before_observation_digest,
        LocalFileObservationRef
    );

    #[must_use]
    pub const fn active_after(&self) -> &ObservationEvidence {
        &self.active_after
    }

    #[must_use]
    pub const fn quarantine_after(&self) -> &ObservationEvidence {
        &self.quarantine_after
    }

    #[must_use]
    pub const fn command_report(&self) -> CommandReport {
        self.command_report
    }

    #[must_use]
    pub const fn postcondition(&self) -> SeparationPostcondition {
        self.postcondition
    }

    #[must_use]
    pub const fn limitations(&self) -> &SortedUnique<EvidenceLimitation> {
        &self.limitations
    }

    #[must_use]
    pub const fn assessed_at(&self) -> UnixNanoseconds {
        self.assessed_at
    }
}

impl SeparationReceipt {
    getter!(SeparationReceipt, effect_id, EffectId);
    getter!(SeparationReceipt, binding_digest, SeparationBindingRef);
    getter!(SeparationReceipt, evidence_digest, SeparationEvidenceRef);
    getter!(SeparationReceipt, deed_digest, ResourceDeedRef);

    #[must_use]
    pub const fn state(&self) -> ReceiptState {
        self.state
    }

    #[must_use]
    pub const fn result(&self) -> OperationResult {
        self.result
    }

    #[must_use]
    pub const fn reason(&self) -> ReceiptReason {
        self.reason
    }

    #[must_use]
    pub const fn terminal_at(&self) -> UnixNanoseconds {
        self.terminal_at
    }

    #[must_use]
    pub const fn next_custody_generation(&self) -> CustodyGeneration {
        self.next_custody_generation
    }
}

impl CustodyRecord {
    getter!(CustodyRecord, resource_key, ResourceKey);

    #[must_use]
    pub const fn deed_digest(&self) -> &OptionalValue<ResourceDeedRef> {
        &self.deed_digest
    }

    #[must_use]
    pub const fn custody_generation(&self) -> CustodyGeneration {
        self.custody_generation
    }

    #[must_use]
    pub const fn state(&self) -> CustodyState {
        self.state
    }

    #[must_use]
    pub const fn terminal_receipt(&self) -> &TerminalReceiptRef {
        &self.terminal_receipt
    }

    getter!(CustodyRecord, active_address, LogicalAddress);

    #[must_use]
    pub const fn quarantine_address(&self) -> &OptionalValue<LogicalAddress> {
        &self.quarantine_address
    }
}

impl RecoveryAssessment {
    getter!(RecoveryAssessment, effect_id, EffectId);

    #[must_use]
    pub const fn binding_digest(&self) -> &TerminalBindingRef {
        &self.binding_digest
    }

    #[must_use]
    pub const fn evidence_digest(&self) -> &TerminalEvidenceRef {
        &self.evidence_digest
    }

    #[must_use]
    pub const fn receipt_digest(&self) -> &TerminalReceiptRef {
        &self.receipt_digest
    }

    #[must_use]
    pub const fn recovered_at(&self) -> UnixNanoseconds {
        self.recovered_at
    }

    #[must_use]
    pub const fn state(&self) -> ReceiptState {
        self.state
    }

    #[must_use]
    pub const fn reason(&self) -> ReceiptReason {
        self.reason
    }
}

fn evidence_edges(values: [&ObservationEvidence; 2]) -> Vec<TypedEdge> {
    values
        .into_iter()
        .flat_map(ObservationEvidence::edges)
        .collect()
}

macro_rules! impl_body_spec {
    ($payload:ty, $tag:ty, $edges:expr, $validate:expr) => {
        impl crate::body::sealed::BodySpec for $payload {}

        impl BodySpec for $payload {
            type Tag = $tag;

            fn edges(&self) -> Vec<TypedEdge> {
                ($edges)(self)
            }

            fn validate_local(&self) -> Result<(), BodyError> {
                ($validate)(self)
            }
        }
    };
}

impl_body_spec!(
    PublicationEvidence,
    PublicationEvidenceTag,
    |value: &PublicationEvidence| {
        let mut edges = vec![
            TypedEdge::new(&value.binding_digest),
            TypedEdge::new(&value.prepared_artifact_digest),
            TypedEdge::new(&value.source_before_observation_digest),
            TypedEdge::new(&value.target_before_observation_digest),
        ];
        edges.extend(evidence_edges([&value.source_after, &value.target_after]));
        edges
    },
    |value: &PublicationEvidence| validate_evidence_local(
        [&value.source_after, &value.target_after],
        value.assessed_at,
    )
);
impl_body_spec!(
    CausalityAssessment,
    CausalityAssessmentTag,
    |value: &CausalityAssessment| vec![TypedEdge::new(&value.evidence_digest)],
    |_value: &CausalityAssessment| Ok(())
);
impl_body_spec!(
    EffectReceipt,
    EffectReceiptTag,
    |value: &EffectReceipt| vec![
        TypedEdge::new(&value.binding_digest),
        TypedEdge::new(&value.evidence_digest),
        TypedEdge::new(&value.causality_digest),
    ],
    |value: &EffectReceipt| validate_publication_receipt_shape(value)
);
impl_body_spec!(
    ResourceDeed,
    ResourceDeedTag,
    |value: &ResourceDeed| vec![TypedEdge::new(&value.publication_receipt_digest)],
    |value: &ResourceDeed| {
        if derive_resource_key(&value.logical_address)? != value.resource_key {
            return Err(BodyError::Local(
                "deed resource key is not derived from its logical address".to_owned(),
            ));
        }
        Ok(())
    }
);
impl_body_spec!(
    SeparationEvidence,
    SeparationEvidenceTag,
    |value: &SeparationEvidence| {
        let mut edges = vec![
            TypedEdge::new(&value.binding_digest),
            TypedEdge::new(&value.deed_digest),
            TypedEdge::new(&value.active_before_observation_digest),
            TypedEdge::new(&value.quarantine_before_observation_digest),
        ];
        edges.extend(evidence_edges([
            &value.active_after,
            &value.quarantine_after,
        ]));
        edges
    },
    |value: &SeparationEvidence| validate_evidence_local(
        [&value.active_after, &value.quarantine_after],
        value.assessed_at,
    )
);
impl_body_spec!(
    SeparationReceipt,
    SeparationReceiptTag,
    |value: &SeparationReceipt| vec![
        TypedEdge::new(&value.binding_digest),
        TypedEdge::new(&value.evidence_digest),
        TypedEdge::new(&value.deed_digest),
    ],
    |value: &SeparationReceipt| validate_separation_receipt_shape(value)
);
impl_body_spec!(
    CustodyRecord,
    CustodyRecordTag,
    |value: &CustodyRecord| {
        let mut edges = Vec::new();
        if let OptionalValue::Present { value } = &value.deed_digest {
            edges.push(TypedEdge::new(value));
        }
        match &value.terminal_receipt {
            ProtocolRef::Publication { digest } => edges.push(TypedEdge::new(digest)),
            ProtocolRef::Separation { digest } => edges.push(TypedEdge::new(digest)),
        }
        edges
    },
    |value: &CustodyRecord| validate_custody_shape(value)
);
impl_body_spec!(
    RecoveryAssessment,
    RecoveryAssessmentTag,
    |value: &RecoveryAssessment| {
        let mut edges = Vec::with_capacity(3);
        match &value.binding_digest {
            ProtocolRef::Publication { digest } => edges.push(TypedEdge::new(digest)),
            ProtocolRef::Separation { digest } => edges.push(TypedEdge::new(digest)),
        }
        match &value.evidence_digest {
            ProtocolRef::Publication { digest } => edges.push(TypedEdge::new(digest)),
            ProtocolRef::Separation { digest } => edges.push(TypedEdge::new(digest)),
        }
        match &value.receipt_digest {
            ProtocolRef::Publication { digest } => edges.push(TypedEdge::new(digest)),
            ProtocolRef::Separation { digest } => edges.push(TypedEdge::new(digest)),
        }
        edges
    },
    |value: &RecoveryAssessment| validate_recovery_shape(value)
);

fn validate_evidence_local(
    values: [&ObservationEvidence; 2],
    assessed_at: UnixNanoseconds,
) -> Result<(), BodyError> {
    for value in values {
        value.validate_local()?;
        if value.attempted_at().is_some_and(|time| time > assessed_at) {
            return Err(BodyError::Local(
                "an observation attempt occurs after assessment".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_publication_receipt_shape(value: &EffectReceipt) -> Result<(), BodyError> {
    let valid_reason = match value.state {
        ReceiptState::Verified => value.reason == ReceiptReason::ArtifactVerified,
        ReceiptState::Failed => matches!(
            value.reason,
            ReceiptReason::SourceChanged
                | ReceiptReason::SourceInvalidAfterStart
                | ReceiptReason::DigestMismatchAfterStart
                | ReceiptReason::PublicationNoEffect
                | ReceiptReason::AuthoritativeAbsence
        ),
        ReceiptState::Indeterminate => matches!(
            value.reason,
            ReceiptReason::UnsupportedIdentity
                | ReceiptReason::WitnessUnavailable
                | ReceiptReason::PublicationAmbiguous
                | ReceiptReason::IncarnationAmbiguous
                | ReceiptReason::DuplicateIncarnation
        ),
    };
    let valid_result = matches!(
        value.result,
        OperationResult::PublishReportedSuccess
            | OperationResult::PublishReportedNoEffect
            | OperationResult::PublishReportedUncertain
            | OperationResult::PublishRecovered
    );
    if !valid_reason || !valid_result {
        return Err(BodyError::Local(
            "publication receipt state, result, and reason are inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_separation_receipt_shape(value: &SeparationReceipt) -> Result<(), BodyError> {
    let valid_reason = match value.state {
        ReceiptState::Verified => value.reason == ReceiptReason::SeparationVerified,
        ReceiptState::Failed => value.reason == ReceiptReason::SeparationNoMove,
        ReceiptState::Indeterminate => matches!(
            value.reason,
            ReceiptReason::UnsupportedIdentity
                | ReceiptReason::WitnessUnavailable
                | ReceiptReason::DuplicateIncarnation
                | ReceiptReason::SeparationAmbiguous
        ),
    };
    let valid_result = matches!(
        value.result,
        OperationResult::QuarantineReportedSuccess
            | OperationResult::QuarantineReportedNoEffect
            | OperationResult::QuarantineReportedUncertain
            | OperationResult::QuarantineRecovered
    );
    if !valid_reason || !valid_result || value.next_custody_generation.get() == 0 {
        return Err(BodyError::Local(
            "separation receipt state, result, reason, or generation is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_custody_shape(value: &CustodyRecord) -> Result<(), BodyError> {
    let valid = match &value.terminal_receipt {
        ProtocolRef::Publication { .. } => {
            matches!(value.quarantine_address, OptionalValue::Absent)
                && matches!(
                    (&value.state, &value.deed_digest),
                    (CustodyState::Owned, OptionalValue::Present { .. })
                        | (
                            CustodyState::Absent | CustodyState::Disputed,
                            OptionalValue::Absent
                        )
                )
        }
        ProtocolRef::Separation { .. } => {
            matches!(value.quarantine_address, OptionalValue::Present { .. })
                && matches!(value.deed_digest, OptionalValue::Present { .. })
                && matches!(
                    value.state,
                    CustodyState::Owned | CustodyState::Quarantined | CustodyState::Disputed
                )
        }
    };
    if !valid {
        return Err(BodyError::Local(
            "custody fields do not match a normative terminal row".to_owned(),
        ));
    }
    Ok(())
}

fn validate_recovery_shape(value: &RecoveryAssessment) -> Result<(), BodyError> {
    let publication = matches!(value.binding_digest, ProtocolRef::Publication { .. })
        && matches!(value.evidence_digest, ProtocolRef::Publication { .. })
        && matches!(value.receipt_digest, ProtocolRef::Publication { .. });
    let separation = matches!(value.binding_digest, ProtocolRef::Separation { .. })
        && matches!(value.evidence_digest, ProtocolRef::Separation { .. })
        && matches!(value.receipt_digest, ProtocolRef::Separation { .. });
    if !publication && !separation {
        return Err(BodyError::Local(
            "recovery references do not select one matching protocol".to_owned(),
        ));
    }
    Ok(())
}

macro_rules! wire_struct {
    ($name:ident { $($field:ident : $ty:ty),+ $(,)? }) => {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct $name {
            $($field: $ty),+
        }
    };
}

wire_struct!(PublicationEvidenceWire {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    prepared_artifact_digest: PreparedArtifactRef,
    command_report: CommandReport,
    source_before_observation_digest: LocalFileObservationRef,
    target_before_observation_digest: LocalFileObservationRef,
    source_after: ObservationEvidence,
    target_after: ObservationEvidence,
    postcondition: PublicationPostcondition,
    limitations: SortedUnique<EvidenceLimitation>,
    assessed_at: UnixNanoseconds,
});
wire_struct!(CausalityAssessmentWire {
    effect_id: EffectId,
    evidence_digest: PublicationEvidenceRef,
    outcome: CausalityOutcome,
});
wire_struct!(EffectReceiptWire {
    effect_id: EffectId,
    binding_digest: IdempotencyBindingRef,
    evidence_digest: PublicationEvidenceRef,
    causality_digest: CausalityAssessmentRef,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
    terminal_at: UnixNanoseconds,
});
wire_struct!(ResourceDeedWire {
    resource_key: ResourceKey,
    logical_address: LogicalAddress,
    artifact_name: ArtifactName,
    content_digest: RawDigest,
    byte_length: ByteLength,
    incarnation: IncarnationId,
    publication_receipt_digest: EffectReceiptRef,
    custody_generation: CustodyGeneration,
});
wire_struct!(SeparationEvidenceWire {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    deed_digest: ResourceDeedRef,
    active_before_observation_digest: LocalFileObservationRef,
    quarantine_before_observation_digest: LocalFileObservationRef,
    active_after: ObservationEvidence,
    quarantine_after: ObservationEvidence,
    command_report: CommandReport,
    postcondition: SeparationPostcondition,
    limitations: SortedUnique<EvidenceLimitation>,
    assessed_at: UnixNanoseconds,
});
wire_struct!(SeparationReceiptWire {
    effect_id: EffectId,
    binding_digest: SeparationBindingRef,
    evidence_digest: SeparationEvidenceRef,
    deed_digest: ResourceDeedRef,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
    terminal_at: UnixNanoseconds,
    next_custody_generation: CustodyGeneration,
});
wire_struct!(CustodyRecordWire {
    resource_key: ResourceKey,
    deed_digest: OptionalValue<ResourceDeedRef>,
    custody_generation: CustodyGeneration,
    state: CustodyState,
    terminal_receipt: TerminalReceiptRef,
    active_address: LogicalAddress,
    quarantine_address: OptionalValue<LogicalAddress>,
});
wire_struct!(RecoveryAssessmentWire {
    effect_id: EffectId,
    binding_digest: TerminalBindingRef,
    evidence_digest: TerminalEvidenceRef,
    receipt_digest: TerminalReceiptRef,
    recovered_at: UnixNanoseconds,
    state: ReceiptState,
    reason: ReceiptReason,
});

fn from_value<T: for<'de> Deserialize<'de>>(body: Value) -> Result<T, BodyError> {
    serde_json::from_value(body)
        .map_err(CanonicalError::from)
        .map_err(BodyError::from)
}

pub(crate) fn decode_publication_evidence(body: Value) -> Result<PublicationEvidence, BodyError> {
    let wire: PublicationEvidenceWire = from_value(body)?;
    Ok(PublicationEvidence {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        prepared_artifact_digest: wire.prepared_artifact_digest,
        command_report: wire.command_report,
        source_before_observation_digest: wire.source_before_observation_digest,
        target_before_observation_digest: wire.target_before_observation_digest,
        source_after: wire.source_after,
        target_after: wire.target_after,
        postcondition: wire.postcondition,
        limitations: wire.limitations,
        assessed_at: wire.assessed_at,
    })
}

pub(crate) fn decode_causality_assessment(body: Value) -> Result<CausalityAssessment, BodyError> {
    let wire: CausalityAssessmentWire = from_value(body)?;
    Ok(CausalityAssessment {
        effect_id: wire.effect_id,
        evidence_digest: wire.evidence_digest,
        outcome: wire.outcome,
    })
}

pub(crate) fn decode_effect_receipt(body: Value) -> Result<EffectReceipt, BodyError> {
    let wire: EffectReceiptWire = from_value(body)?;
    Ok(EffectReceipt {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        evidence_digest: wire.evidence_digest,
        causality_digest: wire.causality_digest,
        state: wire.state,
        result: wire.result,
        reason: wire.reason,
        terminal_at: wire.terminal_at,
    })
}

pub(crate) fn decode_resource_deed(body: Value) -> Result<ResourceDeed, BodyError> {
    let wire: ResourceDeedWire = from_value(body)?;
    Ok(ResourceDeed {
        resource_key: wire.resource_key,
        logical_address: wire.logical_address,
        artifact_name: wire.artifact_name,
        content_digest: wire.content_digest,
        byte_length: wire.byte_length,
        incarnation: wire.incarnation,
        publication_receipt_digest: wire.publication_receipt_digest,
        custody_generation: wire.custody_generation,
    })
}

pub(crate) fn decode_separation_evidence(body: Value) -> Result<SeparationEvidence, BodyError> {
    let wire: SeparationEvidenceWire = from_value(body)?;
    Ok(SeparationEvidence {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        deed_digest: wire.deed_digest,
        active_before_observation_digest: wire.active_before_observation_digest,
        quarantine_before_observation_digest: wire.quarantine_before_observation_digest,
        active_after: wire.active_after,
        quarantine_after: wire.quarantine_after,
        command_report: wire.command_report,
        postcondition: wire.postcondition,
        limitations: wire.limitations,
        assessed_at: wire.assessed_at,
    })
}

pub(crate) fn decode_separation_receipt(body: Value) -> Result<SeparationReceipt, BodyError> {
    let wire: SeparationReceiptWire = from_value(body)?;
    Ok(SeparationReceipt {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        evidence_digest: wire.evidence_digest,
        deed_digest: wire.deed_digest,
        state: wire.state,
        result: wire.result,
        reason: wire.reason,
        terminal_at: wire.terminal_at,
        next_custody_generation: wire.next_custody_generation,
    })
}

pub(crate) fn decode_custody_record(body: Value) -> Result<CustodyRecord, BodyError> {
    let wire: CustodyRecordWire = from_value(body)?;
    Ok(CustodyRecord {
        resource_key: wire.resource_key,
        deed_digest: wire.deed_digest,
        custody_generation: wire.custody_generation,
        state: wire.state,
        terminal_receipt: wire.terminal_receipt,
        active_address: wire.active_address,
        quarantine_address: wire.quarantine_address,
    })
}

pub(crate) fn decode_recovery_assessment(body: Value) -> Result<RecoveryAssessment, BodyError> {
    let wire: RecoveryAssessmentWire = from_value(body)?;
    Ok(RecoveryAssessment {
        effect_id: wire.effect_id,
        binding_digest: wire.binding_digest,
        evidence_digest: wire.evidence_digest,
        receipt_digest: wire.receipt_digest,
        recovered_at: wire.recovered_at,
        state: wire.state,
        reason: wire.reason,
    })
}

/// Closed failures while deriving evidence and proof-bearing terminal bodies.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvidenceError {
    #[error("effect is not in the required started state")]
    NotStarted,
    #[error("evidence references do not match the durable start")]
    StartReferenceMismatch,
    #[error("an observation occurs after assessment")]
    ObservationAfterAssessment,
    #[error("not_available is legal only during recovery")]
    RecoveryReportOnLivePath,
    #[error("conflicting observations do not name one address")]
    ConflictingAddress,
    #[error("custody generation is exhausted")]
    GenerationExhausted,
    #[error("body validation failed: {0}")]
    Body(#[from] BodyError),
}

/// Immutable preparation facts already committed by a publication start.
pub(crate) struct PreparedPublicationFacts<'a> {
    pub(crate) effect_id: &'a EffectId,
    pub(crate) binding_digest: &'a IdempotencyBindingRef,
    pub(crate) source_before_observation_digest: &'a LocalFileObservationRef,
    pub(crate) target_before_observation_digest: &'a LocalFileObservationRef,
    pub(crate) source_address: &'a LogicalAddress,
    pub(crate) target_address: &'a LogicalAddress,
    pub(crate) artifact_name: &'a ArtifactName,
    pub(crate) content_digest: &'a RawDigest,
    pub(crate) byte_length: ByteLength,
    pub(crate) incarnation: &'a IncarnationId,
}

/// Exact durable publication-start facts plus fresh probe attempts.
pub(crate) struct PublicationEvidenceInput<'a> {
    pub(crate) started: bool,
    pub(crate) effect_id: &'a EffectId,
    pub(crate) binding_digest: &'a IdempotencyBindingRef,
    pub(crate) prepared_artifact_digest: &'a PreparedArtifactRef,
    pub(crate) source_before: &'a ValidatedBody<LocalFileObservation>,
    pub(crate) target_before: &'a ValidatedBody<LocalFileObservation>,
    pub(crate) prepared: PreparedPublicationFacts<'a>,
    pub(crate) source_after: &'a ObservationAttempt,
    pub(crate) target_after: &'a ObservationAttempt,
    pub(crate) command_report: CommandReport,
    pub(crate) mutation_mode: MutationMode,
    pub(crate) started_at: UnixNanoseconds,
    pub(crate) assessed_at: UnixNanoseconds,
    pub(crate) recovery: bool,
    pub(crate) prior_custody: Option<&'a CustodyRecord>,
    pub(crate) existing_deeds: &'a [ResourceDeed],
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "independent probe limitations may coexist and feed separate normative derivations"
)]
#[derive(Debug, Clone)]
struct ResolvedAttempt {
    evidence: ObservationEvidence,
    observation: Option<LocalFileObservation>,
    unavailable: bool,
    unsupported: bool,
    conflicting: bool,
    stale: bool,
}

impl ResolvedAttempt {
    const fn authoritative_observation(&self) -> Option<&LocalFileObservation> {
        if self.unavailable || self.unsupported || self.conflicting || self.stale {
            None
        } else {
            self.observation.as_ref()
        }
    }

    const fn observation(&self) -> Option<&LocalFileObservation> {
        self.observation.as_ref()
    }
}

/// Derived publication evidence and independent causality, before terminal classification.
pub(crate) struct PublicationEvidenceMaterial {
    evidence: ValidatedBody<PublicationEvidence>,
    causality: ValidatedBody<CausalityAssessment>,
    source_before: LocalFileObservation,
    source_after: ResolvedAttempt,
    target_after: ResolvedAttempt,
    prepared_source_address: LogicalAddress,
    prepared_target_address: LogicalAddress,
    prepared_artifact_name: ArtifactName,
    prepared_content_digest: RawDigest,
    prepared_byte_length: ByteLength,
    prepared_incarnation: IncarnationId,
    custody_generation: CustodyGeneration,
    deed_conflict: bool,
}

impl PublicationEvidenceMaterial {
    pub(crate) const fn evidence(&self) -> &ValidatedBody<PublicationEvidence> {
        &self.evidence
    }

    pub(crate) const fn causality(&self) -> &ValidatedBody<CausalityAssessment> {
        &self.causality
    }
}

#[derive(Debug)]
struct DeedProof {
    resource_key: ResourceKey,
    logical_address: LogicalAddress,
    artifact_name: ArtifactName,
    content_digest: RawDigest,
    byte_length: ByteLength,
    incarnation: IncarnationId,
    custody_generation: CustodyGeneration,
}

/// Exhaustive publication classifier output. Its optional proof is private.
pub(crate) struct PublicationClassification {
    material: PublicationEvidenceMaterial,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
    deed_proof: Option<DeedProof>,
}

impl PublicationClassification {
    pub(crate) const fn state(&self) -> ReceiptState {
        self.state
    }

    pub(crate) const fn result(&self) -> OperationResult {
        self.result
    }

    pub(crate) const fn reason(&self) -> ReceiptReason {
        self.reason
    }

    pub(crate) const fn deed_expected(&self) -> bool {
        self.deed_proof.is_some()
    }
}

/// Derives immutable publication evidence and causality without caller-selected classifications.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the Task 7 transition interface deliberately consumes one complete input record"
)]
pub(crate) fn derive_publication_evidence(
    input: PublicationEvidenceInput<'_>,
) -> Result<PublicationEvidenceMaterial, EvidenceError> {
    if !input.started {
        return Err(EvidenceError::NotStarted);
    }
    if input.command_report == CommandReport::NotAvailable && !input.recovery {
        return Err(EvidenceError::RecoveryReportOnLivePath);
    }
    require_prepared_start_facts(&input)?;
    let custody_generation =
        next_publication_generation(input.prepared.target_address, input.prior_custody)?;
    let deed_conflict = input.existing_deeds.iter().any(|deed| {
        deed.logical_address() == input.prepared.target_address
            && deed.custody_generation() == custody_generation
    });

    let source_after = resolve_attempt(
        input.source_after,
        input.prepared.source_address,
        input.started_at,
        input.assessed_at,
    )?;
    let target_after = resolve_attempt(
        input.target_after,
        input.prepared.target_address,
        input.started_at,
        input.assessed_at,
    )?;
    let limitations = derive_limitations([&source_after, &target_after], input.mutation_mode)?;
    let postcondition = publication_postcondition(
        &target_after,
        input.target_before.payload(),
        &input.prepared,
    );
    let causality_outcome =
        publication_causality(&source_after, &target_after, &limitations, &input.prepared);
    let evidence = validated_body(PublicationEvidence {
        effect_id: input.effect_id.clone(),
        binding_digest: input.binding_digest.clone(),
        prepared_artifact_digest: input.prepared_artifact_digest.clone(),
        command_report: input.command_report,
        source_before_observation_digest: input.source_before.reference().clone(),
        target_before_observation_digest: input.target_before.reference().clone(),
        source_after: source_after.evidence.clone(),
        target_after: target_after.evidence.clone(),
        postcondition,
        limitations,
        assessed_at: input.assessed_at,
    })?;
    let causality = validated_body(CausalityAssessment {
        effect_id: input.effect_id.clone(),
        evidence_digest: evidence.reference().clone(),
        outcome: causality_outcome,
    })?;
    Ok(PublicationEvidenceMaterial {
        evidence,
        causality,
        source_before: input.source_before.payload().clone(),
        source_after,
        target_after,
        prepared_source_address: input.prepared.source_address.clone(),
        prepared_target_address: input.prepared.target_address.clone(),
        prepared_artifact_name: input.prepared.artifact_name.clone(),
        prepared_content_digest: input.prepared.content_digest.clone(),
        prepared_byte_length: input.prepared.byte_length,
        prepared_incarnation: input.prepared.incarnation.clone(),
        custody_generation,
        deed_conflict,
    })
}

fn next_publication_generation(
    target_address: &LogicalAddress,
    prior_custody: Option<&CustodyRecord>,
) -> Result<CustodyGeneration, EvidenceError> {
    let target_key = derive_resource_key(target_address).map_err(BodyError::from)?;
    match prior_custody {
        None => Ok(CustodyGeneration::from_u64(0)),
        Some(custody)
            if custody.state() == CustodyState::Absent
                && custody.resource_key() == &target_key
                && custody.active_address() == target_address =>
        {
            custody
                .custody_generation()
                .checked_add(1)
                .map_err(|_| EvidenceError::GenerationExhausted)
        }
        Some(_) => Err(EvidenceError::StartReferenceMismatch),
    }
}

fn require_prepared_start_facts(input: &PublicationEvidenceInput<'_>) -> Result<(), EvidenceError> {
    let source = input.source_before.payload();
    if input.effect_id != input.prepared.effect_id
        || input.binding_digest != input.prepared.binding_digest
        || input.source_before.reference() != input.prepared.source_before_observation_digest
        || input.target_before.reference() != input.prepared.target_before_observation_digest
        || source.logical_address() != input.prepared.source_address
        || input.target_before.payload().logical_address() != input.prepared.target_address
        || !matches_requested_content(source, &input.prepared)
        || source.incarnation() != Some(input.prepared.incarnation)
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    Ok(())
}

fn resolve_attempt(
    attempt: &ObservationAttempt,
    expected_address: &LogicalAddress,
    started_at: UnixNanoseconds,
    assessed_at: UnixNanoseconds,
) -> Result<ResolvedAttempt, EvidenceError> {
    match attempt {
        ObservationAttempt::Observed {
            observation,
            witness,
        } => {
            let payload = observation.payload();
            if payload.logical_address() != expected_address {
                return Err(EvidenceError::StartReferenceMismatch);
            }
            let observed_at = payload.observed_at();
            if observed_at > assessed_at {
                return Err(EvidenceError::ObservationAfterAssessment);
            }
            Ok(ResolvedAttempt {
                evidence: ObservationEvidence::Observed {
                    digest: observation.reference().clone(),
                },
                observation: Some(payload.clone()),
                unavailable: false,
                unsupported: *witness != WitnessStatus::AuthenticatedEnrolled,
                conflicting: false,
                stale: observed_at < started_at,
            })
        }
        ObservationAttempt::Unavailable {
            logical_address,
            witness_id,
            attempted_at,
        } => {
            require_nonobserved_attempt(
                logical_address,
                expected_address,
                *attempted_at,
                assessed_at,
            )?;
            Ok(ResolvedAttempt {
                evidence: ObservationEvidence::Unavailable {
                    logical_address: logical_address.clone(),
                    witness_id: witness_id.clone(),
                    attempted_at: *attempted_at,
                },
                observation: None,
                unavailable: true,
                unsupported: false,
                conflicting: false,
                stale: *attempted_at < started_at,
            })
        }
        ObservationAttempt::Unsupported {
            logical_address,
            witness_id,
            attempted_at,
        } => {
            require_nonobserved_attempt(
                logical_address,
                expected_address,
                *attempted_at,
                assessed_at,
            )?;
            Ok(ResolvedAttempt {
                evidence: ObservationEvidence::Unsupported {
                    logical_address: logical_address.clone(),
                    witness_id: witness_id.clone(),
                    attempted_at: *attempted_at,
                },
                observation: None,
                unavailable: false,
                unsupported: true,
                conflicting: false,
                stale: *attempted_at < started_at,
            })
        }
        ObservationAttempt::Conflicting {
            observations,
            witness,
            attempted_at,
        } => resolve_conflicting(
            observations,
            *witness,
            *attempted_at,
            expected_address,
            started_at,
            assessed_at,
        ),
    }
}

fn require_nonobserved_attempt(
    logical_address: &LogicalAddress,
    expected_address: &LogicalAddress,
    attempted_at: UnixNanoseconds,
    assessed_at: UnixNanoseconds,
) -> Result<(), EvidenceError> {
    if logical_address != expected_address {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    if attempted_at > assessed_at {
        return Err(EvidenceError::ObservationAfterAssessment);
    }
    Ok(())
}

fn resolve_conflicting(
    observations: &SortedUnique<ValidatedBody<LocalFileObservation>>,
    witness: WitnessStatus,
    attempted_at: UnixNanoseconds,
    expected_address: &LogicalAddress,
    started_at: UnixNanoseconds,
    assessed_at: UnixNanoseconds,
) -> Result<ResolvedAttempt, EvidenceError> {
    if !(2..=1_024).contains(&observations.len()) {
        return Err(EvidenceError::Body(BodyError::Local(
            "conflicting observation attempt requires 2..=1024 observations".to_owned(),
        )));
    }
    if attempted_at > assessed_at {
        return Err(EvidenceError::ObservationAfterAssessment);
    }
    let first = observations
        .as_slice()
        .first()
        .expect("length was checked")
        .payload();
    if first.logical_address() != expected_address
        || observations
            .as_slice()
            .iter()
            .any(|body| body.payload().logical_address() != first.logical_address())
    {
        return Err(EvidenceError::ConflictingAddress);
    }
    if observations
        .as_slice()
        .iter()
        .any(|body| body.payload().witness_id() != first.witness_id())
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    if observations
        .as_slice()
        .iter()
        .any(|body| body.payload().observed_at() > assessed_at)
    {
        return Err(EvidenceError::ObservationAfterAssessment);
    }
    let stale = attempted_at < started_at
        || observations
            .as_slice()
            .iter()
            .any(|body| body.payload().observed_at() < started_at);
    let refs = observations
        .as_slice()
        .iter()
        .map(|body| body.reference().clone())
        .collect();
    Ok(ResolvedAttempt {
        evidence: ObservationEvidence::Conflicting {
            logical_address: first.logical_address().clone(),
            witness_id: first.witness_id().clone(),
            attempted_at,
            observation_digests: SortedUnique::new_bounded(refs, 1_024)?,
        },
        observation: None,
        unavailable: false,
        unsupported: witness != WitnessStatus::AuthenticatedEnrolled,
        conflicting: true,
        stale,
    })
}

fn derive_limitations(
    attempts: [&ResolvedAttempt; 2],
    mutation_mode: MutationMode,
) -> Result<SortedUnique<EvidenceLimitation>, EvidenceError> {
    let mut limitations = Vec::new();
    if attempts.iter().any(|attempt| attempt.unavailable) {
        limitations.push(EvidenceLimitation::WitnessUnavailable);
    }
    if attempts.iter().any(|attempt| attempt.unsupported) {
        limitations.push(EvidenceLimitation::UnsupportedIdentity);
    }
    if attempts.iter().any(|attempt| attempt.conflicting) {
        limitations.push(EvidenceLimitation::ConflictingObservation);
    }
    if attempts.iter().any(|attempt| attempt.stale) {
        limitations.push(EvidenceLimitation::StaleObservation);
    }
    if mutation_mode == MutationMode::Unconditional {
        limitations.push(EvidenceLimitation::NonAtomicExternalOperation);
    }
    limitations.sort_by(|left, right| {
        canonical_bytes(left)
            .expect("closed evidence limitation serializes")
            .cmp(&canonical_bytes(right).expect("closed evidence limitation serializes"))
    });
    Ok(SortedUnique::new(limitations)?)
}

fn publication_postcondition(
    target_after: &ResolvedAttempt,
    target_before: &LocalFileObservation,
    prepared: &PreparedPublicationFacts<'_>,
) -> PublicationPostcondition {
    let Some(target) = target_after.authoritative_observation() else {
        return PublicationPostcondition::Ambiguous;
    };
    if matches_requested_content(target, prepared) {
        return PublicationPostcondition::ExactRequested;
    }
    if same_state(target, target_before) {
        return PublicationPostcondition::PriorStateUnchanged;
    }
    if matches!(target, LocalFileObservation::Absent { .. }) {
        return PublicationPostcondition::AuthoritativeAbsence;
    }
    PublicationPostcondition::ContentMismatch
}

fn publication_causality(
    source_after: &ResolvedAttempt,
    target_after: &ResolvedAttempt,
    limitations: &SortedUnique<EvidenceLimitation>,
    prepared: &PreparedPublicationFacts<'_>,
) -> CausalityOutcome {
    if contains_limitation(limitations, EvidenceLimitation::UnsupportedIdentity) {
        return CausalityOutcome::Unsupported;
    }
    if limitations.as_slice().iter().any(|limitation| {
        matches!(
            limitation,
            EvidenceLimitation::WitnessUnavailable
                | EvidenceLimitation::StaleObservation
                | EvidenceLimitation::ConflictingObservation
                | EvidenceLimitation::NonAtomicExternalOperation
        )
    }) {
        return CausalityOutcome::Ambiguous;
    }
    let source_has_prepared = source_after
        .observation()
        .is_some_and(|value| value.incarnation() == Some(prepared.incarnation));
    let target_has_prepared = target_after
        .observation()
        .is_some_and(|value| value.incarnation() == Some(prepared.incarnation));
    if source_has_prepared && target_has_prepared {
        return CausalityOutcome::DuplicateIncarnation;
    }
    if !source_has_prepared && target_has_prepared {
        return CausalityOutcome::ExactPreparedIncarnation;
    }
    if target_after.observation().is_some_and(|target| {
        matches_requested_content(target, prepared)
            && target
                .incarnation()
                .is_some_and(|value| value != prepared.incarnation)
    }) {
        return CausalityOutcome::DifferentIncarnation;
    }
    CausalityOutcome::Ambiguous
}

fn same_state(left: &LocalFileObservation, right: &LocalFileObservation) -> bool {
    match (left, right) {
        (LocalFileObservation::Absent { .. }, LocalFileObservation::Absent { .. }) => true,
        (LocalFileObservation::Present { .. }, LocalFileObservation::Present { .. }) => {
            left.artifact_name() == right.artifact_name()
                && left.content_digest() == right.content_digest()
                && left.byte_length() == right.byte_length()
                && left.incarnation() == right.incarnation()
                && left.quarantine_xattr_digest() == right.quarantine_xattr_digest()
        }
        _ => false,
    }
}

fn matches_requested_content(
    observation: &LocalFileObservation,
    prepared: &PreparedPublicationFacts<'_>,
) -> bool {
    observation.artifact_name() == Some(prepared.artifact_name)
        && observation.content_digest() == Some(prepared.content_digest)
        && observation.byte_length() == Some(prepared.byte_length)
}

fn is_exact_prepared(
    observation: &LocalFileObservation,
    source_address: &LogicalAddress,
    artifact_name: &ArtifactName,
    content_digest: &RawDigest,
    byte_length: ByteLength,
    incarnation: &IncarnationId,
) -> bool {
    observation.logical_address() == source_address
        && observation.artifact_name() == Some(artifact_name)
        && observation.content_digest() == Some(content_digest)
        && observation.byte_length() == Some(byte_length)
        && observation.incarnation() == Some(incarnation)
}

fn contains_limitation(
    limitations: &SortedUnique<EvidenceLimitation>,
    expected: EvidenceLimitation,
) -> bool {
    limitations.as_slice().contains(&expected)
}

/// Applies the twelve publication rows top to bottom.
#[allow(
    clippy::too_many_lines,
    reason = "the normative priority table remains linear and auditable row by row"
)]
pub(crate) fn classify_publication(
    material: PublicationEvidenceMaterial,
) -> Result<PublicationClassification, EvidenceError> {
    let limitations = material.evidence.payload().limitations();
    let postcondition = material.evidence.payload().postcondition();
    let causality = material.causality.payload().outcome();
    let source = material.source_after.observation();
    let target = material.target_after.authoritative_observation();

    let (state, reason) =
        if contains_limitation(limitations, EvidenceLimitation::UnsupportedIdentity) {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::UnsupportedIdentity,
            )
        } else if contains_limitation(limitations, EvidenceLimitation::WitnessUnavailable)
            || contains_limitation(limitations, EvidenceLimitation::StaleObservation)
        {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::WitnessUnavailable,
            )
        } else if contains_limitation(limitations, EvidenceLimitation::ConflictingObservation)
            || contains_limitation(limitations, EvidenceLimitation::NonAtomicExternalOperation)
        {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::PublicationAmbiguous,
            )
        } else if causality == CausalityOutcome::DuplicateIncarnation {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::DuplicateIncarnation,
            )
        } else if postcondition == PublicationPostcondition::ExactRequested
            && causality == CausalityOutcome::ExactPreparedIncarnation
            && limitations.is_empty()
        {
            (ReceiptState::Verified, ReceiptReason::ArtifactVerified)
        } else if postcondition == PublicationPostcondition::ExactRequested
            && causality == CausalityOutcome::DifferentIncarnation
        {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::IncarnationAmbiguous,
            )
        } else if source.is_some_and(|source| {
            source.is_present()
                && (source.incarnation() != Some(&material.prepared_incarnation)
                    || source.content_digest() != Some(&material.prepared_content_digest)
                    || source.byte_length() != Some(material.prepared_byte_length))
        }) {
            (ReceiptState::Failed, ReceiptReason::SourceChanged)
        } else if source.is_some_and(|source| matches!(source, LocalFileObservation::Absent { .. }))
            && target.is_some_and(|target| matches!(target, LocalFileObservation::Absent { .. }))
        {
            (ReceiptState::Failed, ReceiptReason::SourceInvalidAfterStart)
        } else if postcondition == PublicationPostcondition::ContentMismatch {
            (
                ReceiptState::Failed,
                ReceiptReason::DigestMismatchAfterStart,
            )
        } else if source.is_some_and(|source| {
            is_exact_prepared(
                source,
                &material.prepared_source_address,
                &material.prepared_artifact_name,
                &material.prepared_content_digest,
                material.prepared_byte_length,
                &material.prepared_incarnation,
            )
        }) && postcondition == PublicationPostcondition::PriorStateUnchanged
        {
            (ReceiptState::Failed, ReceiptReason::PublicationNoEffect)
        } else if postcondition == PublicationPostcondition::AuthoritativeAbsence
            && !source.is_some_and(|source| {
                is_exact_prepared(
                    source,
                    &material.prepared_source_address,
                    &material.prepared_artifact_name,
                    &material.prepared_content_digest,
                    material.prepared_byte_length,
                    &material.prepared_incarnation,
                )
            })
        {
            (ReceiptState::Failed, ReceiptReason::AuthoritativeAbsence)
        } else {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::PublicationAmbiguous,
            )
        };

    let result = publication_result(material.evidence.payload().command_report());
    let deed_proof = if state == ReceiptState::Verified {
        if material.deed_conflict {
            return Err(EvidenceError::StartReferenceMismatch);
        }
        Some(DeedProof {
            resource_key: derive_resource_key(&material.prepared_target_address)
                .map_err(BodyError::from)?,
            logical_address: material.prepared_target_address.clone(),
            artifact_name: material.prepared_artifact_name.clone(),
            content_digest: material.prepared_content_digest.clone(),
            byte_length: material.prepared_byte_length,
            incarnation: material.prepared_incarnation.clone(),
            custody_generation: material.custody_generation,
        })
    } else {
        None
    };
    Ok(PublicationClassification {
        material,
        state,
        result,
        reason,
        deed_proof,
    })
}

const fn publication_result(command_report: CommandReport) -> OperationResult {
    match command_report {
        CommandReport::ReportedSuccess => OperationResult::PublishReportedSuccess,
        CommandReport::ReportedNoEffect => OperationResult::PublishReportedNoEffect,
        CommandReport::ReportedUncertain => OperationResult::PublishReportedUncertain,
        CommandReport::NotAvailable => OperationResult::PublishRecovered,
    }
}

/// Exact durable separation-start facts plus fresh probe attempts.
pub(crate) struct SeparationEvidenceInput<'a> {
    pub(crate) started: bool,
    pub(crate) effect_id: &'a EffectId,
    pub(crate) binding_digest: &'a SeparationBindingRef,
    pub(crate) deed: &'a ValidatedBody<ResourceDeed>,
    pub(crate) active_before: &'a ValidatedBody<LocalFileObservation>,
    pub(crate) quarantine_before: &'a ValidatedBody<LocalFileObservation>,
    pub(crate) active_after: &'a ObservationAttempt,
    pub(crate) quarantine_after: &'a ObservationAttempt,
    pub(crate) quarantine_address: &'a LogicalAddress,
    pub(crate) quarantine_xattr_digest: &'a XattrValueRef,
    pub(crate) command_report: CommandReport,
    pub(crate) mutation_mode: MutationMode,
    pub(crate) started_at: UnixNanoseconds,
    pub(crate) assessed_at: UnixNanoseconds,
    pub(crate) recovery: bool,
    pub(crate) current_custody_generation: CustodyGeneration,
}

/// Derived separation evidence before terminal classification.
pub(crate) struct SeparationEvidenceMaterial {
    evidence: ValidatedBody<SeparationEvidence>,
    deed: ValidatedBody<ResourceDeed>,
    active_after: ResolvedAttempt,
    quarantine_after: ResolvedAttempt,
    quarantine_address: LogicalAddress,
    current_custody_generation: CustodyGeneration,
}

impl SeparationEvidenceMaterial {
    pub(crate) const fn evidence(&self) -> &ValidatedBody<SeparationEvidence> {
        &self.evidence
    }
}

/// Exhaustive separation classifier output.
pub(crate) struct SeparationClassification {
    material: SeparationEvidenceMaterial,
    state: ReceiptState,
    result: OperationResult,
    reason: ReceiptReason,
}

impl SeparationClassification {
    pub(crate) const fn state(&self) -> ReceiptState {
        self.state
    }

    pub(crate) const fn result(&self) -> OperationResult {
        self.result
    }

    pub(crate) const fn reason(&self) -> ReceiptReason {
        self.reason
    }
}

/// Derives immutable separation evidence without caller-selected classifications.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the Task 7 transition interface deliberately consumes one complete input record"
)]
pub(crate) fn derive_separation_evidence(
    input: SeparationEvidenceInput<'_>,
) -> Result<SeparationEvidenceMaterial, EvidenceError> {
    if !input.started {
        return Err(EvidenceError::NotStarted);
    }
    if input.command_report == CommandReport::NotAvailable && !input.recovery {
        return Err(EvidenceError::RecoveryReportOnLivePath);
    }
    if input.active_before.payload().logical_address() != input.deed.payload().logical_address()
        || input.quarantine_before.payload().logical_address() != input.quarantine_address
        || !is_exact_deed(input.active_before.payload(), input.deed.payload())
        || !matches!(
            input.quarantine_before.payload(),
            LocalFileObservation::Absent { .. }
        )
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    let active_after = resolve_attempt(
        input.active_after,
        input.deed.payload().logical_address(),
        input.started_at,
        input.assessed_at,
    )?;
    let quarantine_after = resolve_attempt(
        input.quarantine_after,
        input.quarantine_address,
        input.started_at,
        input.assessed_at,
    )?;
    let limitations = derive_limitations([&active_after, &quarantine_after], input.mutation_mode)?;
    let postcondition = separation_postcondition(
        &active_after,
        &quarantine_after,
        input.deed.payload(),
        input.quarantine_xattr_digest,
        &limitations,
    );
    let evidence = validated_body(SeparationEvidence {
        effect_id: input.effect_id.clone(),
        binding_digest: input.binding_digest.clone(),
        deed_digest: input.deed.reference().clone(),
        active_before_observation_digest: input.active_before.reference().clone(),
        quarantine_before_observation_digest: input.quarantine_before.reference().clone(),
        active_after: active_after.evidence.clone(),
        quarantine_after: quarantine_after.evidence.clone(),
        command_report: input.command_report,
        postcondition,
        limitations,
        assessed_at: input.assessed_at,
    })?;
    Ok(SeparationEvidenceMaterial {
        evidence,
        deed: input.deed.clone(),
        active_after,
        quarantine_after,
        quarantine_address: input.quarantine_address.clone(),
        current_custody_generation: input.current_custody_generation,
    })
}

fn separation_postcondition(
    active_after: &ResolvedAttempt,
    quarantine_after: &ResolvedAttempt,
    deed: &ResourceDeed,
    quarantine_xattr_digest: &XattrValueRef,
    limitations: &SortedUnique<EvidenceLimitation>,
) -> SeparationPostcondition {
    if !limitations.is_empty() {
        return SeparationPostcondition::Ambiguous;
    }
    let (Some(active), Some(quarantine)) = (
        active_after.authoritative_observation(),
        quarantine_after.authoritative_observation(),
    ) else {
        return SeparationPostcondition::Ambiguous;
    };
    if matches!(active, LocalFileObservation::Absent { .. })
        && matches_deed_content(quarantine, deed)
        && quarantine
            .quarantine_xattr_digest()
            .and_then(OptionalValue::value)
            == Some(quarantine_xattr_digest)
    {
        return SeparationPostcondition::ExactQuarantine;
    }
    if matches_deed_content(active, deed)
        && matches!(quarantine, LocalFileObservation::Absent { .. })
    {
        return SeparationPostcondition::NoMove;
    }
    SeparationPostcondition::Ambiguous
}

fn matches_deed_content(observation: &LocalFileObservation, deed: &ResourceDeed) -> bool {
    observation.artifact_name() == Some(deed.artifact_name())
        && observation.content_digest() == Some(deed.content_digest())
        && observation.byte_length() == Some(deed.byte_length())
}

fn is_exact_deed(observation: &LocalFileObservation, deed: &ResourceDeed) -> bool {
    observation.logical_address() == deed.logical_address()
        && matches_deed_content(observation, deed)
        && observation.incarnation() == Some(deed.incarnation())
}

/// Applies the six separation rows top to bottom.
#[allow(
    clippy::unnecessary_wraps,
    reason = "the frozen classifier seam shares the fail-closed Result contract with publication"
)]
pub(crate) fn classify_separation(
    material: SeparationEvidenceMaterial,
) -> Result<SeparationClassification, EvidenceError> {
    let limitations = material.evidence.payload().limitations();
    let active = material.active_after.observation();
    let quarantine = material.quarantine_after.observation();
    let deed = material.deed.payload();
    let postcondition = material.evidence.payload().postcondition();

    let (state, reason) =
        if contains_limitation(limitations, EvidenceLimitation::UnsupportedIdentity) {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::UnsupportedIdentity,
            )
        } else if contains_limitation(limitations, EvidenceLimitation::WitnessUnavailable)
            || contains_limitation(limitations, EvidenceLimitation::StaleObservation)
        {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::WitnessUnavailable,
            )
        } else if active.is_some_and(|value| is_exact_deed(value, deed))
            && quarantine.is_some_and(|value| {
                matches_deed_content(value, deed) && value.incarnation() == Some(deed.incarnation())
            })
        {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::DuplicateIncarnation,
            )
        } else if postcondition == SeparationPostcondition::ExactQuarantine
            && quarantine.is_some_and(|value| value.incarnation() == Some(deed.incarnation()))
            && limitations.is_empty()
        {
            (ReceiptState::Verified, ReceiptReason::SeparationVerified)
        } else if postcondition == SeparationPostcondition::NoMove
            && active.is_some_and(|value| value.incarnation() == Some(deed.incarnation()))
            && limitations.is_empty()
        {
            (ReceiptState::Failed, ReceiptReason::SeparationNoMove)
        } else {
            (
                ReceiptState::Indeterminate,
                ReceiptReason::SeparationAmbiguous,
            )
        };
    let result = separation_result(material.evidence.payload().command_report());
    Ok(SeparationClassification {
        material,
        state,
        result,
        reason,
    })
}

const fn separation_result(command_report: CommandReport) -> OperationResult {
    match command_report {
        CommandReport::ReportedSuccess => OperationResult::QuarantineReportedSuccess,
        CommandReport::ReportedNoEffect => OperationResult::QuarantineReportedNoEffect,
        CommandReport::ReportedUncertain => OperationResult::QuarantineReportedUncertain,
        CommandReport::NotAvailable => OperationResult::QuarantineRecovered,
    }
}

/// The complete set of derived publication terminal bodies.
pub(crate) struct PublicationTerminalBodies {
    pub(crate) receipt: ValidatedBody<EffectReceipt>,
    pub(crate) deed: Option<ValidatedBody<ResourceDeed>>,
    pub(crate) custody: ValidatedBody<CustodyRecord>,
}

/// Builds a receipt, optional deed, and custody record from a classifier output.
pub(crate) fn derive_publication_terminal(
    classification: PublicationClassification,
    terminal_at: UnixNanoseconds,
) -> Result<PublicationTerminalBodies, EvidenceError> {
    if terminal_at != classification.material.evidence.payload().assessed_at() {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    let receipt = validated_body(EffectReceipt {
        effect_id: classification
            .material
            .evidence
            .payload()
            .effect_id()
            .clone(),
        binding_digest: classification
            .material
            .evidence
            .payload()
            .binding_digest()
            .clone(),
        evidence_digest: classification.material.evidence.reference().clone(),
        causality_digest: classification.material.causality.reference().clone(),
        state: classification.state,
        result: classification.result,
        reason: classification.reason,
        terminal_at,
    })?;
    let deed = classification
        .deed_proof
        .map(|proof| mint_deed(proof, &receipt))
        .transpose()?;
    let state = match classification.state {
        ReceiptState::Verified => CustodyState::Owned,
        ReceiptState::Failed => CustodyState::Absent,
        ReceiptState::Indeterminate => CustodyState::Disputed,
    };
    let custody = validated_body(CustodyRecord {
        resource_key: derive_resource_key(&classification.material.prepared_target_address)
            .map_err(BodyError::from)?,
        deed_digest: deed.as_ref().map_or_else(OptionalValue::absent, |body| {
            OptionalValue::present(body.reference().clone())
        }),
        custody_generation: classification.material.custody_generation,
        state,
        terminal_receipt: ProtocolRef::publication(receipt.reference().clone()),
        active_address: classification.material.prepared_target_address,
        quarantine_address: OptionalValue::absent(),
    })?;
    Ok(PublicationTerminalBodies {
        receipt,
        deed,
        custody,
    })
}

fn mint_deed(
    proof: DeedProof,
    receipt: &ValidatedBody<EffectReceipt>,
) -> Result<ValidatedBody<ResourceDeed>, EvidenceError> {
    if receipt.payload().state() != ReceiptState::Verified
        || receipt.payload().reason() != ReceiptReason::ArtifactVerified
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    Ok(validated_body(ResourceDeed {
        resource_key: proof.resource_key,
        logical_address: proof.logical_address,
        artifact_name: proof.artifact_name,
        content_digest: proof.content_digest,
        byte_length: proof.byte_length,
        incarnation: proof.incarnation,
        publication_receipt_digest: receipt.reference().clone(),
        custody_generation: proof.custody_generation,
    })?)
}

/// The complete set of derived separation terminal bodies.
pub(crate) struct SeparationTerminalBodies {
    pub(crate) receipt: ValidatedBody<SeparationReceipt>,
    pub(crate) custody: ValidatedBody<CustodyRecord>,
}

/// Builds a separation receipt and retained-deed custody record.
pub(crate) fn derive_separation_terminal(
    classification: SeparationClassification,
    terminal_at: UnixNanoseconds,
) -> Result<SeparationTerminalBodies, EvidenceError> {
    if terminal_at != classification.material.evidence.payload().assessed_at() {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    let next_generation = classification
        .material
        .current_custody_generation
        .checked_add(1)
        .map_err(|_| EvidenceError::GenerationExhausted)?;
    let receipt = validated_body(SeparationReceipt {
        effect_id: classification
            .material
            .evidence
            .payload()
            .effect_id()
            .clone(),
        binding_digest: classification
            .material
            .evidence
            .payload()
            .binding_digest()
            .clone(),
        evidence_digest: classification.material.evidence.reference().clone(),
        deed_digest: classification.material.deed.reference().clone(),
        state: classification.state,
        result: classification.result,
        reason: classification.reason,
        terminal_at,
        next_custody_generation: next_generation,
    })?;
    let state = match classification.state {
        ReceiptState::Verified => CustodyState::Quarantined,
        ReceiptState::Failed => CustodyState::Owned,
        ReceiptState::Indeterminate => CustodyState::Disputed,
    };
    let deed = classification.material.deed;
    let custody = validated_body(CustodyRecord {
        resource_key: deed.payload().resource_key().clone(),
        deed_digest: OptionalValue::present(deed.reference().clone()),
        custody_generation: next_generation,
        state,
        terminal_receipt: ProtocolRef::separation(receipt.reference().clone()),
        active_address: deed.payload().logical_address().clone(),
        quarantine_address: OptionalValue::present(classification.material.quarantine_address),
    })?;
    Ok(SeparationTerminalBodies { receipt, custody })
}

/// Derives a publication recovery audit body from its exact receipt.
pub(crate) fn derive_publication_recovery_assessment(
    terminal: &PublicationTerminalBodies,
    recovered_at: UnixNanoseconds,
) -> Result<ValidatedBody<RecoveryAssessment>, EvidenceError> {
    let receipt = terminal.receipt.payload();
    if receipt.terminal_at() != recovered_at
        || receipt.result() != OperationResult::PublishRecovered
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    Ok(validated_body(RecoveryAssessment {
        effect_id: receipt.effect_id().clone(),
        binding_digest: ProtocolRef::publication(receipt.binding_digest().clone()),
        evidence_digest: ProtocolRef::publication(receipt.evidence_digest().clone()),
        receipt_digest: ProtocolRef::publication(terminal.receipt.reference().clone()),
        recovered_at,
        state: receipt.state(),
        reason: receipt.reason(),
    })?)
}

/// Derives a separation recovery audit body from its exact receipt.
pub(crate) fn derive_separation_recovery_assessment(
    terminal: &SeparationTerminalBodies,
    recovered_at: UnixNanoseconds,
) -> Result<ValidatedBody<RecoveryAssessment>, EvidenceError> {
    let receipt = terminal.receipt.payload();
    if receipt.terminal_at() != recovered_at
        || receipt.result() != OperationResult::QuarantineRecovered
    {
        return Err(EvidenceError::StartReferenceMismatch);
    }
    Ok(validated_body(RecoveryAssessment {
        effect_id: receipt.effect_id().clone(),
        binding_digest: ProtocolRef::separation(receipt.binding_digest().clone()),
        evidence_digest: ProtocolRef::separation(receipt.evidence_digest().clone()),
        receipt_digest: ProtocolRef::separation(terminal.receipt.reference().clone()),
        recovered_at,
        state: receipt.state(),
        reason: receipt.reason(),
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        body::{IdempotencyBindingRef, PreparedArtifactRef, SeparationBindingRef},
        scalar::Digest,
    };

    const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
    const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
    const FOUR: &str = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
    const FIVE: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";

    #[derive(Debug, Clone, Copy)]
    enum AttemptSpec {
        Exact,
        ExactWithXattr,
        DifferentIncarnation,
        Changed,
        NameOnlyChanged,
        Mismatch,
        Absent,
        Unavailable,
        Unsupported,
        UnauthenticatedExact,
        Conflicting,
        StaleExact,
    }

    fn digest(value: &str) -> Digest {
        Digest::parse(value).unwrap()
    }

    fn raw(value: &str) -> RawDigest {
        RawDigest::parse(value).unwrap()
    }

    fn incarnation(value: &str) -> IncarnationId {
        IncarnationId::parse(value).unwrap()
    }

    fn time(value: u64) -> UnixNanoseconds {
        UnixNanoseconds::parse(&value.to_string()).unwrap()
    }

    fn address(value: &str) -> LogicalAddress {
        LogicalAddress::parse(value).unwrap()
    }

    fn observed_present(
        logical_address: &LogicalAddress,
        observed_at: UnixNanoseconds,
        artifact_name: &str,
        content_digest: &str,
        byte_length: u64,
        incarnation_id: &str,
        xattr: OptionalValue<XattrValueRef>,
    ) -> ValidatedBody<LocalFileObservation> {
        validated_body(LocalFileObservation::present(
            logical_address.clone(),
            WitnessId::parse("host-probe").unwrap(),
            observed_at,
            ArtifactName::parse(artifact_name).unwrap(),
            raw(content_digest),
            ByteLength::from_u64(byte_length),
            incarnation(incarnation_id),
            xattr,
        ))
        .unwrap()
    }

    fn observed_absent(
        logical_address: &LogicalAddress,
        observed_at: UnixNanoseconds,
    ) -> ValidatedBody<LocalFileObservation> {
        validated_body(LocalFileObservation::absent(
            logical_address.clone(),
            WitnessId::parse("host-probe").unwrap(),
            observed_at,
        ))
        .unwrap()
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive fixture keeps every closed attempt shape visible to the matrices"
    )]
    fn attempt(
        spec: AttemptSpec,
        logical_address: &LogicalAddress,
        required_xattr: &XattrValueRef,
    ) -> ObservationAttempt {
        match spec {
            AttemptSpec::Exact | AttemptSpec::UnauthenticatedExact | AttemptSpec::StaleExact => {
                let observation = observed_present(
                    logical_address,
                    if matches!(spec, AttemptSpec::StaleExact) {
                        time(9)
                    } else {
                        time(20)
                    },
                    "app",
                    TWO,
                    42,
                    ONE,
                    OptionalValue::absent(),
                );
                ObservationAttempt::Observed {
                    observation,
                    witness: if matches!(spec, AttemptSpec::UnauthenticatedExact) {
                        WitnessStatus::Unauthenticated
                    } else {
                        WitnessStatus::AuthenticatedEnrolled
                    },
                }
            }
            AttemptSpec::ExactWithXattr => ObservationAttempt::Observed {
                observation: observed_present(
                    logical_address,
                    time(20),
                    "app",
                    TWO,
                    42,
                    ONE,
                    OptionalValue::present(required_xattr.clone()),
                ),
                witness: WitnessStatus::AuthenticatedEnrolled,
            },
            AttemptSpec::DifferentIncarnation => ObservationAttempt::Observed {
                observation: observed_present(
                    logical_address,
                    time(20),
                    "app",
                    TWO,
                    42,
                    THREE,
                    OptionalValue::absent(),
                ),
                witness: WitnessStatus::AuthenticatedEnrolled,
            },
            AttemptSpec::Changed | AttemptSpec::Mismatch => ObservationAttempt::Observed {
                observation: observed_present(
                    logical_address,
                    time(20),
                    "app",
                    THREE,
                    41,
                    FOUR,
                    OptionalValue::absent(),
                ),
                witness: WitnessStatus::AuthenticatedEnrolled,
            },
            AttemptSpec::NameOnlyChanged => ObservationAttempt::Observed {
                observation: observed_present(
                    logical_address,
                    time(20),
                    "other",
                    TWO,
                    42,
                    ONE,
                    OptionalValue::absent(),
                ),
                witness: WitnessStatus::AuthenticatedEnrolled,
            },
            AttemptSpec::Absent => ObservationAttempt::Observed {
                observation: observed_absent(logical_address, time(20)),
                witness: WitnessStatus::AuthenticatedEnrolled,
            },
            AttemptSpec::Unavailable => ObservationAttempt::Unavailable {
                logical_address: logical_address.clone(),
                witness_id: WitnessId::parse("host-probe").unwrap(),
                attempted_at: time(20),
            },
            AttemptSpec::Unsupported => ObservationAttempt::Unsupported {
                logical_address: logical_address.clone(),
                witness_id: WitnessId::parse("host-probe").unwrap(),
                attempted_at: time(20),
            },
            AttemptSpec::Conflicting => {
                let left = observed_absent(logical_address, time(20));
                let right = observed_present(
                    logical_address,
                    time(20),
                    "app",
                    TWO,
                    42,
                    ONE,
                    OptionalValue::absent(),
                );
                let mut observations = vec![left, right];
                observations.sort_by(|a, b| a.reference().cmp(b.reference()));
                ObservationAttempt::Conflicting {
                    observations: SortedUnique::new(observations).unwrap(),
                    witness: WitnessStatus::AuthenticatedEnrolled,
                    attempted_at: time(20),
                }
            }
        }
    }

    struct PublicationCase {
        name: &'static str,
        source_after: AttemptSpec,
        target_after: AttemptSpec,
        target_before_present: bool,
        mutation_mode: MutationMode,
        command_report: CommandReport,
        expected_postcondition: PublicationPostcondition,
        expected_causality: CausalityOutcome,
        expected_state: ReceiptState,
        expected_reason: ReceiptReason,
        deed_expected: bool,
    }

    fn publication_material(
        case: &PublicationCase,
        recovery: bool,
    ) -> Result<PublicationEvidenceMaterial, EvidenceError> {
        let source_address = address("local-file:///staging/app");
        let target_address = address("local-file:///active/app");
        let source_before = observed_present(
            &source_address,
            time(10),
            "app",
            TWO,
            42,
            ONE,
            OptionalValue::absent(),
        );
        let target_before = if case.target_before_present {
            observed_present(
                &target_address,
                time(10),
                "old",
                THREE,
                7,
                FOUR,
                OptionalValue::absent(),
            )
        } else {
            observed_absent(&target_address, time(10))
        };
        let xattr = XattrValueRef::from_digest(digest(FIVE));
        let source_after = attempt(case.source_after, &source_address, &xattr);
        let target_after = attempt(case.target_after, &target_address, &xattr);
        let effect_id = EffectId::parse(ONE).unwrap();
        let binding = IdempotencyBindingRef::from_digest(digest(FOUR));
        let prepared_ref = PreparedArtifactRef::from_digest(digest(FIVE));
        let artifact_name = ArtifactName::parse("app").unwrap();
        let content_digest = raw(TWO);
        let prepared_incarnation = incarnation(ONE);
        derive_publication_evidence(PublicationEvidenceInput {
            started: true,
            effect_id: &effect_id,
            binding_digest: &binding,
            prepared_artifact_digest: &prepared_ref,
            source_before: &source_before,
            target_before: &target_before,
            prepared: PreparedPublicationFacts {
                effect_id: &effect_id,
                binding_digest: &binding,
                source_before_observation_digest: source_before.reference(),
                target_before_observation_digest: target_before.reference(),
                source_address: &source_address,
                target_address: &target_address,
                artifact_name: &artifact_name,
                content_digest: &content_digest,
                byte_length: ByteLength::from_u64(42),
                incarnation: &prepared_incarnation,
            },
            source_after: &source_after,
            target_after: &target_after,
            command_report: case.command_report,
            mutation_mode: case.mutation_mode,
            started_at: time(10),
            assessed_at: time(20),
            recovery,
            prior_custody: None,
            existing_deeds: &[],
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the twelve literal normative rows form one auditable priority table"
    )]
    #[test]
    fn publication_classifier_covers_all_twelve_priority_rows() {
        let cases = [
            PublicationCase {
                name: "1 unsupported identity",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::UnauthenticatedExact,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::Ambiguous,
                expected_causality: CausalityOutcome::Unsupported,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::UnsupportedIdentity,
                deed_expected: false,
            },
            PublicationCase {
                name: "2 unavailable",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::Unavailable,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedUncertain,
                expected_postcondition: PublicationPostcondition::Ambiguous,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::WitnessUnavailable,
                deed_expected: false,
            },
            PublicationCase {
                name: "3 non atomic",
                source_after: AttemptSpec::Absent,
                target_after: AttemptSpec::Exact,
                target_before_present: false,
                mutation_mode: MutationMode::Unconditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::ExactRequested,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::PublicationAmbiguous,
                deed_expected: false,
            },
            PublicationCase {
                name: "4 duplicate",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::Exact,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::ExactRequested,
                expected_causality: CausalityOutcome::DuplicateIncarnation,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::DuplicateIncarnation,
                deed_expected: false,
            },
            PublicationCase {
                name: "5 verified",
                source_after: AttemptSpec::Absent,
                target_after: AttemptSpec::Exact,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::ExactRequested,
                expected_causality: CausalityOutcome::ExactPreparedIncarnation,
                expected_state: ReceiptState::Verified,
                expected_reason: ReceiptReason::ArtifactVerified,
                deed_expected: true,
            },
            PublicationCase {
                name: "6 same bytes different incarnation",
                source_after: AttemptSpec::Absent,
                target_after: AttemptSpec::DifferentIncarnation,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::ExactRequested,
                expected_causality: CausalityOutcome::DifferentIncarnation,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::IncarnationAmbiguous,
                deed_expected: false,
            },
            PublicationCase {
                name: "7 source changed despite success report",
                source_after: AttemptSpec::Changed,
                target_after: AttemptSpec::Absent,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::PriorStateUnchanged,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::SourceChanged,
                deed_expected: false,
            },
            PublicationCase {
                name: "8 source invalid despite success report",
                source_after: AttemptSpec::Absent,
                target_after: AttemptSpec::Absent,
                target_before_present: true,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::AuthoritativeAbsence,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::SourceInvalidAfterStart,
                deed_expected: false,
            },
            PublicationCase {
                name: "9 target content mismatch",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::Mismatch,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedSuccess,
                expected_postcondition: PublicationPostcondition::ContentMismatch,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::DigestMismatchAfterStart,
                deed_expected: false,
            },
            PublicationCase {
                name: "10 unchanged",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::Absent,
                target_before_present: false,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedNoEffect,
                expected_postcondition: PublicationPostcondition::PriorStateUnchanged,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::PublicationNoEffect,
                deed_expected: false,
            },
            PublicationCase {
                name: "11 authoritative absence with non-unchanged source",
                source_after: AttemptSpec::NameOnlyChanged,
                target_after: AttemptSpec::Absent,
                target_before_present: true,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedNoEffect,
                expected_postcondition: PublicationPostcondition::AuthoritativeAbsence,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::AuthoritativeAbsence,
                deed_expected: false,
            },
            PublicationCase {
                name: "12 authoritative absence with unchanged source falls through",
                source_after: AttemptSpec::Exact,
                target_after: AttemptSpec::Absent,
                target_before_present: true,
                mutation_mode: MutationMode::Conditional,
                command_report: CommandReport::ReportedNoEffect,
                expected_postcondition: PublicationPostcondition::AuthoritativeAbsence,
                expected_causality: CausalityOutcome::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::PublicationAmbiguous,
                deed_expected: false,
            },
        ];

        for case in cases {
            let material = publication_material(&case, false).unwrap_or_else(|error| {
                panic!("{} failed evidence derivation: {error}", case.name)
            });
            assert_eq!(
                material.evidence().payload().postcondition(),
                case.expected_postcondition,
                "{} postcondition",
                case.name
            );
            assert_eq!(
                material.causality().payload().outcome(),
                case.expected_causality,
                "{} causality",
                case.name
            );
            let classified = classify_publication(material).unwrap();
            assert_eq!(
                classified.state(),
                case.expected_state,
                "{} state",
                case.name
            );
            assert_eq!(
                classified.reason(),
                case.expected_reason,
                "{} reason",
                case.name
            );
            assert_eq!(
                classified.deed_expected(),
                case.deed_expected,
                "{} deed",
                case.name
            );
        }
    }

    #[test]
    fn limitation_derivation_is_exhaustive_and_not_available_is_recovery_only() {
        let mut case = PublicationCase {
            name: "limitations",
            source_after: AttemptSpec::Conflicting,
            target_after: AttemptSpec::StaleExact,
            target_before_present: false,
            mutation_mode: MutationMode::Unconditional,
            command_report: CommandReport::ReportedUncertain,
            expected_postcondition: PublicationPostcondition::Ambiguous,
            expected_causality: CausalityOutcome::Ambiguous,
            expected_state: ReceiptState::Indeterminate,
            expected_reason: ReceiptReason::WitnessUnavailable,
            deed_expected: false,
        };
        let material = publication_material(&case, false).unwrap();
        let limitations = material.evidence().payload().limitations().as_slice();
        assert!(limitations.contains(&EvidenceLimitation::ConflictingObservation));
        assert!(limitations.contains(&EvidenceLimitation::StaleObservation));
        assert!(limitations.contains(&EvidenceLimitation::NonAtomicExternalOperation));

        case.command_report = CommandReport::NotAvailable;
        assert!(matches!(
            publication_material(&case, false),
            Err(EvidenceError::RecoveryReportOnLivePath)
        ));
        let recovered = publication_material(&case, true).unwrap();
        assert_eq!(
            classify_publication(recovered).unwrap().result(),
            OperationResult::PublishRecovered
        );
    }

    fn unit_deed(generation: u64) -> ValidatedBody<ResourceDeed> {
        let active = address("local-file:///active/app");
        validated_body(ResourceDeed {
            resource_key: derive_resource_key(&active).unwrap(),
            logical_address: active,
            artifact_name: ArtifactName::parse("app").unwrap(),
            content_digest: raw(TWO),
            byte_length: ByteLength::from_u64(42),
            incarnation: incarnation(ONE),
            publication_receipt_digest: EffectReceiptRef::from_digest(digest(FOUR)),
            custody_generation: CustodyGeneration::from_u64(generation),
        })
        .unwrap()
    }

    struct SeparationCase {
        name: &'static str,
        active_after: AttemptSpec,
        quarantine_after: AttemptSpec,
        mutation_mode: MutationMode,
        expected_postcondition: SeparationPostcondition,
        expected_state: ReceiptState,
        expected_reason: ReceiptReason,
    }

    fn separation_material(
        case: &SeparationCase,
        current_generation: CustodyGeneration,
        command_report: CommandReport,
        recovery: bool,
    ) -> Result<SeparationEvidenceMaterial, EvidenceError> {
        let deed = unit_deed(0);
        let active_address = deed.payload().logical_address().clone();
        let quarantine_address = address("local-file:///quarantine/app");
        let active_before = observed_present(
            &active_address,
            time(10),
            "app",
            TWO,
            42,
            ONE,
            OptionalValue::absent(),
        );
        let quarantine_before = observed_absent(&quarantine_address, time(10));
        let xattr = XattrValueRef::from_digest(digest(FIVE));
        let active_after = attempt(case.active_after, &active_address, &xattr);
        let quarantine_after = attempt(case.quarantine_after, &quarantine_address, &xattr);
        let effect_id = EffectId::parse(THREE).unwrap();
        let binding = SeparationBindingRef::from_digest(digest(THREE));
        derive_separation_evidence(SeparationEvidenceInput {
            started: true,
            effect_id: &effect_id,
            binding_digest: &binding,
            deed: &deed,
            active_before: &active_before,
            quarantine_before: &quarantine_before,
            active_after: &active_after,
            quarantine_after: &quarantine_after,
            quarantine_address: &quarantine_address,
            quarantine_xattr_digest: &xattr,
            command_report,
            mutation_mode: case.mutation_mode,
            started_at: time(10),
            assessed_at: time(20),
            recovery,
            current_custody_generation: current_generation,
        })
    }

    #[test]
    fn separation_classifier_covers_all_six_priority_rows() {
        let cases = [
            SeparationCase {
                name: "1 unsupported",
                active_after: AttemptSpec::Absent,
                quarantine_after: AttemptSpec::UnauthenticatedExact,
                mutation_mode: MutationMode::Conditional,
                expected_postcondition: SeparationPostcondition::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::UnsupportedIdentity,
            },
            SeparationCase {
                name: "2 unavailable",
                active_after: AttemptSpec::Absent,
                quarantine_after: AttemptSpec::Unavailable,
                mutation_mode: MutationMode::Conditional,
                expected_postcondition: SeparationPostcondition::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::WitnessUnavailable,
            },
            SeparationCase {
                name: "3 duplicate",
                active_after: AttemptSpec::Exact,
                quarantine_after: AttemptSpec::ExactWithXattr,
                mutation_mode: MutationMode::Conditional,
                expected_postcondition: SeparationPostcondition::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::DuplicateIncarnation,
            },
            SeparationCase {
                name: "4 verified",
                active_after: AttemptSpec::Absent,
                quarantine_after: AttemptSpec::ExactWithXattr,
                mutation_mode: MutationMode::Conditional,
                expected_postcondition: SeparationPostcondition::ExactQuarantine,
                expected_state: ReceiptState::Verified,
                expected_reason: ReceiptReason::SeparationVerified,
            },
            SeparationCase {
                name: "5 safe no move",
                active_after: AttemptSpec::Exact,
                quarantine_after: AttemptSpec::Absent,
                mutation_mode: MutationMode::Conditional,
                expected_postcondition: SeparationPostcondition::NoMove,
                expected_state: ReceiptState::Failed,
                expected_reason: ReceiptReason::SeparationNoMove,
            },
            SeparationCase {
                name: "6 non atomic",
                active_after: AttemptSpec::Absent,
                quarantine_after: AttemptSpec::ExactWithXattr,
                mutation_mode: MutationMode::Unconditional,
                expected_postcondition: SeparationPostcondition::Ambiguous,
                expected_state: ReceiptState::Indeterminate,
                expected_reason: ReceiptReason::SeparationAmbiguous,
            },
        ];
        for case in cases {
            let material = separation_material(
                &case,
                CustodyGeneration::from_u64(3),
                CommandReport::ReportedSuccess,
                false,
            )
            .unwrap_or_else(|error| panic!("{} derivation: {error}", case.name));
            assert_eq!(
                material.evidence().payload().postcondition(),
                case.expected_postcondition,
                "{} postcondition",
                case.name
            );
            let classified = classify_separation(material).unwrap();
            assert_eq!(
                classified.state(),
                case.expected_state,
                "{} state",
                case.name
            );
            assert_eq!(
                classified.reason(),
                case.expected_reason,
                "{} reason",
                case.name
            );
        }
    }

    #[test]
    fn terminal_bodies_derive_deed_custody_generation_and_recovery() {
        let verified = PublicationCase {
            name: "verified",
            source_after: AttemptSpec::Absent,
            target_after: AttemptSpec::Exact,
            target_before_present: false,
            mutation_mode: MutationMode::Conditional,
            command_report: CommandReport::NotAvailable,
            expected_postcondition: PublicationPostcondition::ExactRequested,
            expected_causality: CausalityOutcome::ExactPreparedIncarnation,
            expected_state: ReceiptState::Verified,
            expected_reason: ReceiptReason::ArtifactVerified,
            deed_expected: true,
        };
        let classification =
            classify_publication(publication_material(&verified, true).unwrap()).unwrap();
        let terminal = derive_publication_terminal(classification, time(20)).unwrap();
        assert_eq!(terminal.receipt.payload().state(), ReceiptState::Verified);
        let deed = terminal.deed.as_ref().unwrap();
        assert_eq!(deed.payload().custody_generation().get(), 0);
        assert_eq!(terminal.custody.payload().state(), CustodyState::Owned);
        assert_eq!(
            terminal.custody.payload().deed_digest().value(),
            Some(deed.reference())
        );
        let recovery = derive_publication_recovery_assessment(&terminal, time(20)).unwrap();
        assert_eq!(recovery.payload().state(), ReceiptState::Verified);
        assert!(matches!(
            recovery.payload().receipt_digest(),
            ProtocolRef::Publication { .. }
        ));

        let separated = SeparationCase {
            name: "verified separation",
            active_after: AttemptSpec::Absent,
            quarantine_after: AttemptSpec::ExactWithXattr,
            mutation_mode: MutationMode::Conditional,
            expected_postcondition: SeparationPostcondition::ExactQuarantine,
            expected_state: ReceiptState::Verified,
            expected_reason: ReceiptReason::SeparationVerified,
        };
        let material = separation_material(
            &separated,
            CustodyGeneration::from_u64(7),
            CommandReport::NotAvailable,
            true,
        )
        .unwrap();
        let terminal =
            derive_separation_terminal(classify_separation(material).unwrap(), time(20)).unwrap();
        assert_eq!(
            terminal.receipt.payload().next_custody_generation().get(),
            8
        );
        assert_eq!(
            terminal.custody.payload().state(),
            CustodyState::Quarantined
        );
        assert!(terminal.custody.payload().deed_digest().value().is_some());
        let recovery = derive_separation_recovery_assessment(&terminal, time(20)).unwrap();
        assert!(matches!(
            recovery.payload().receipt_digest(),
            ProtocolRef::Separation { .. }
        ));

        let exhausted = separation_material(
            &separated,
            CustodyGeneration::from_u64(u64::MAX),
            CommandReport::ReportedSuccess,
            false,
        )
        .unwrap();
        assert!(matches!(
            derive_separation_terminal(classify_separation(exhausted).unwrap(), time(20)),
            Err(EvidenceError::GenerationExhausted)
        ));
    }

    fn prior_publication_custody(
        state: CustodyState,
        generation: u64,
    ) -> ValidatedBody<CustodyRecord> {
        let active = address("local-file:///active/app");
        let deed = unit_deed(generation);
        validated_body(CustodyRecord {
            resource_key: derive_resource_key(&active).unwrap(),
            deed_digest: if state == CustodyState::Owned {
                OptionalValue::present(deed.reference().clone())
            } else {
                OptionalValue::absent()
            },
            custody_generation: CustodyGeneration::from_u64(generation),
            state,
            terminal_receipt: ProtocolRef::publication(EffectReceiptRef::from_digest(digest(FOUR))),
            active_address: active,
            quarantine_address: OptionalValue::absent(),
        })
        .unwrap()
    }

    #[test]
    fn publication_generation_and_deed_slot_are_derived_not_selected() {
        let target = address("local-file:///active/app");
        assert_eq!(next_publication_generation(&target, None).unwrap().get(), 0);
        let absent = prior_publication_custody(CustodyState::Absent, 7);
        assert_eq!(
            next_publication_generation(&target, Some(absent.payload()))
                .unwrap()
                .get(),
            8
        );
        let owned = prior_publication_custody(CustodyState::Owned, 7);
        assert!(matches!(
            next_publication_generation(&target, Some(owned.payload())),
            Err(EvidenceError::StartReferenceMismatch)
        ));
        let exhausted = prior_publication_custody(CustodyState::Absent, u64::MAX);
        assert!(matches!(
            next_publication_generation(&target, Some(exhausted.payload())),
            Err(EvidenceError::GenerationExhausted)
        ));

        let verified = PublicationCase {
            name: "deed conflict",
            source_after: AttemptSpec::Absent,
            target_after: AttemptSpec::Exact,
            target_before_present: false,
            mutation_mode: MutationMode::Conditional,
            command_report: CommandReport::ReportedSuccess,
            expected_postcondition: PublicationPostcondition::ExactRequested,
            expected_causality: CausalityOutcome::ExactPreparedIncarnation,
            expected_state: ReceiptState::Verified,
            expected_reason: ReceiptReason::ArtifactVerified,
            deed_expected: true,
        };
        let mut material = publication_material(&verified, false).unwrap();
        material.deed_conflict = true;
        assert!(matches!(
            classify_publication(material),
            Err(EvidenceError::StartReferenceMismatch)
        ));
    }

    #[test]
    fn conflicting_attempts_must_name_exactly_one_address() {
        let first = observed_absent(&address("local-file:///active/app"), time(20));
        let second = observed_absent(&address("local-file:///other/app"), time(20));
        let mut observations = vec![first, second];
        observations.sort_by(|a, b| a.reference().cmp(b.reference()));
        let attempt = ObservationAttempt::Conflicting {
            observations: SortedUnique::new(observations).unwrap(),
            witness: WitnessStatus::AuthenticatedEnrolled,
            attempted_at: time(20),
        };
        assert!(matches!(
            resolve_attempt(
                &attempt,
                &address("local-file:///active/app"),
                time(10),
                time(20)
            ),
            Err(EvidenceError::ConflictingAddress)
        ));
    }
}
