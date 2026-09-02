//! Closed event vocabulary, canonical envelopes, and anchored chain validation.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{
    body::{
        BodyError, BodyGraph, BodyKind, CustodyRecordRef, EffectLeaseRef, EffectReceiptRef,
        IdempotencyBindingRef, InstallationEnrollmentRef, LocalFileObservationRef,
        PreparedArtifactRef, PublicationApprovalRef, PublicationEvidenceRef,
        PublicationRevocationRef, PublicationWarrantRef, RecoveryAssessmentRef, ResourceDeedRef,
        SeparationApprovalRef, SeparationBindingRef, SeparationLeaseRef, SeparationReceiptRef,
        SeparationRevocationRef, SeparationWarrantRef,
    },
    canonical::{CanonicalError, canonical_bytes, canonical_digest, strict_from_slice},
    evidence::{MutationMode, TerminalReceiptRef},
    protocol::EVENT_SCHEMA_VERSION,
    scalar::{Digest, Identifier, U64Decimal, UnixNanoseconds},
};

const UNKNOWN_EVENT_TYPE_MARKER: &str = "unknown effect-kernel event type";
const TYPE_CONFUSED_PAYLOAD_MARKER: &str = "event payload is not valid for its event type";

/// The exact closed event vocabulary in effect protocol v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum EventType {
    #[serde(rename = "installation_enrolled")]
    InstallationEnrolled,
    #[serde(rename = "warrant_proposed")]
    WarrantProposed,
    #[serde(rename = "warrant_approved")]
    WarrantApproved,
    #[serde(rename = "warrant_revoked")]
    WarrantRevoked,
    #[serde(rename = "warrant_expired")]
    WarrantExpired,
    #[serde(rename = "effect_reserved")]
    EffectReserved,
    #[serde(rename = "effect_cancelled_before_start")]
    EffectCancelledBeforeStart,
    #[serde(rename = "effect_started")]
    EffectStarted,
    #[serde(rename = "artifact_prepared")]
    ArtifactPrepared,
    #[serde(rename = "artifact_published")]
    ArtifactPublished,
    #[serde(rename = "artifact_published_recovered")]
    ArtifactPublishedRecovered,
    #[serde(rename = "effect_verified")]
    EffectVerified,
    #[serde(rename = "effect_failed")]
    EffectFailed,
    #[serde(rename = "effect_indeterminate")]
    EffectIndeterminate,
    #[serde(rename = "separation_warrant_proposed")]
    SeparationWarrantProposed,
    #[serde(rename = "separation_warrant_approved")]
    SeparationWarrantApproved,
    #[serde(rename = "separation_warrant_revoked")]
    SeparationWarrantRevoked,
    #[serde(rename = "separation_warrant_expired")]
    SeparationWarrantExpired,
    #[serde(rename = "separation_reserved")]
    SeparationReserved,
    #[serde(rename = "separation_cancelled_before_start")]
    SeparationCancelledBeforeStart,
    #[serde(rename = "separation_started")]
    SeparationStarted,
    #[serde(rename = "separation_verified")]
    SeparationVerified,
    #[serde(rename = "separation_failed")]
    SeparationFailed,
    #[serde(rename = "separation_indeterminate")]
    SeparationIndeterminate,
    #[serde(rename = "custody_absent")]
    CustodyAbsent,
    #[serde(rename = "custody_disputed")]
    CustodyDisputed,
}

impl EventType {
    pub const ALL: [Self; 26] = [
        Self::InstallationEnrolled,
        Self::WarrantProposed,
        Self::WarrantApproved,
        Self::WarrantRevoked,
        Self::WarrantExpired,
        Self::EffectReserved,
        Self::EffectCancelledBeforeStart,
        Self::EffectStarted,
        Self::ArtifactPrepared,
        Self::ArtifactPublished,
        Self::ArtifactPublishedRecovered,
        Self::EffectVerified,
        Self::EffectFailed,
        Self::EffectIndeterminate,
        Self::SeparationWarrantProposed,
        Self::SeparationWarrantApproved,
        Self::SeparationWarrantRevoked,
        Self::SeparationWarrantExpired,
        Self::SeparationReserved,
        Self::SeparationCancelledBeforeStart,
        Self::SeparationStarted,
        Self::SeparationVerified,
        Self::SeparationFailed,
        Self::SeparationIndeterminate,
        Self::CustodyAbsent,
        Self::CustodyDisputed,
    ];

    /// Returns the frozen v1 wire identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstallationEnrolled => "installation_enrolled",
            Self::WarrantProposed => "warrant_proposed",
            Self::WarrantApproved => "warrant_approved",
            Self::WarrantRevoked => "warrant_revoked",
            Self::WarrantExpired => "warrant_expired",
            Self::EffectReserved => "effect_reserved",
            Self::EffectCancelledBeforeStart => "effect_cancelled_before_start",
            Self::EffectStarted => "effect_started",
            Self::ArtifactPrepared => "artifact_prepared",
            Self::ArtifactPublished => "artifact_published",
            Self::ArtifactPublishedRecovered => "artifact_published_recovered",
            Self::EffectVerified => "effect_verified",
            Self::EffectFailed => "effect_failed",
            Self::EffectIndeterminate => "effect_indeterminate",
            Self::SeparationWarrantProposed => "separation_warrant_proposed",
            Self::SeparationWarrantApproved => "separation_warrant_approved",
            Self::SeparationWarrantRevoked => "separation_warrant_revoked",
            Self::SeparationWarrantExpired => "separation_warrant_expired",
            Self::SeparationReserved => "separation_reserved",
            Self::SeparationCancelledBeforeStart => "separation_cancelled_before_start",
            Self::SeparationStarted => "separation_started",
            Self::SeparationVerified => "separation_verified",
            Self::SeparationFailed => "separation_failed",
            Self::SeparationIndeterminate => "separation_indeterminate",
            Self::CustodyAbsent => "custody_absent",
            Self::CustodyDisputed => "custody_disputed",
        }
    }

    fn parse(input: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|event_type| event_type.as_str() == input)
    }
}

impl<'de> Deserialize<'de> for EventType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).ok_or_else(|| serde::de::Error::custom(UNKNOWN_EVENT_TYPE_MARKER))
    }
}

/// Explicit event-chain origin or non-genesis predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreviousEvent {
    Genesis,
    Previous { digest: Digest },
}

/// The six values permitted on a committed pre-start cancellation event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationReason {
    RequestDisconnected,
    ReservationDeadline,
    AuthorizationIneligible,
    PeerIdentityChanged,
    PreconditionChanged,
    RecoveryOrphaned,
}

/// Whether a separation terminal was derived from a live report or recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalMode {
    Live,
    Recovered,
}

macro_rules! payload_struct {
    ($name:ident { $($field:ident : $type:ty),+ $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        #[allow(
            clippy::struct_field_names,
            reason = "field names are frozen by the wire protocol"
        )]
        pub struct $name {
            $($field: $type),+
        }

        impl $name {
            #[allow(
                dead_code,
                reason = "used by reviewed transition modules added after Task 8"
            )]
            pub(crate) const fn new($($field: $type),+) -> Self {
                Self { $($field),+ }
            }

            $(
                #[must_use]
                pub const fn $field(&self) -> &$type {
                    &self.$field
                }
            )+
        }
    };
}

payload_struct!(InstallationEnrolledPayload {
    enrollment_digest: InstallationEnrollmentRef,
});
payload_struct!(WarrantProposedPayload {
    warrant_digest: PublicationWarrantRef,
});
payload_struct!(WarrantApprovedPayload {
    approval_digest: PublicationApprovalRef,
});
payload_struct!(WarrantRevokedPayload {
    revocation_digest: PublicationRevocationRef,
});
payload_struct!(WarrantExpiredPayload {
    warrant_digest: PublicationWarrantRef,
});
payload_struct!(EffectReservedPayload {
    binding_digest: IdempotencyBindingRef,
    lease_digest: EffectLeaseRef,
});
payload_struct!(EffectCancelledBeforeStartPayload {
    binding_digest: IdempotencyBindingRef,
    lease_digest: EffectLeaseRef,
    reason: CancellationReason,
});
payload_struct!(EffectStartedPayload {
    binding_digest: IdempotencyBindingRef,
    lease_digest: EffectLeaseRef,
    prepared_artifact_digest: PreparedArtifactRef,
    source_before_observation_digest: LocalFileObservationRef,
    target_before_observation_digest: LocalFileObservationRef,
    mutation_mode: MutationMode,
});
payload_struct!(ArtifactPreparedPayload {
    prepared_artifact_digest: PreparedArtifactRef,
});
payload_struct!(ArtifactPublishedPayload {
    evidence_digest: PublicationEvidenceRef,
});
payload_struct!(ArtifactPublishedRecoveredPayload {
    recovery_assessment_digest: RecoveryAssessmentRef,
});
payload_struct!(EffectVerifiedPayload {
    receipt_digest: EffectReceiptRef,
    deed_digest: ResourceDeedRef,
    custody_record_digest: CustodyRecordRef,
});
payload_struct!(EffectFailedPayload {
    receipt_digest: EffectReceiptRef,
});
payload_struct!(EffectIndeterminatePayload {
    receipt_digest: EffectReceiptRef,
});
payload_struct!(SeparationWarrantProposedPayload {
    warrant_digest: SeparationWarrantRef,
});
payload_struct!(SeparationWarrantApprovedPayload {
    approval_digest: SeparationApprovalRef,
});
payload_struct!(SeparationWarrantRevokedPayload {
    revocation_digest: SeparationRevocationRef,
});
payload_struct!(SeparationWarrantExpiredPayload {
    warrant_digest: SeparationWarrantRef,
});
payload_struct!(SeparationReservedPayload {
    binding_digest: SeparationBindingRef,
    lease_digest: SeparationLeaseRef,
});
payload_struct!(SeparationCancelledBeforeStartPayload {
    binding_digest: SeparationBindingRef,
    lease_digest: SeparationLeaseRef,
    reason: CancellationReason,
});
payload_struct!(SeparationStartedPayload {
    binding_digest: SeparationBindingRef,
    lease_digest: SeparationLeaseRef,
    deed_digest: ResourceDeedRef,
    active_before_observation_digest: LocalFileObservationRef,
    quarantine_before_observation_digest: LocalFileObservationRef,
    mutation_mode: MutationMode,
});

macro_rules! separation_terminal_payload {
    ($name:ident, $wire:ident, custody) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            mode: TerminalMode,
            recovery_assessment_digest: Option<RecoveryAssessmentRef>,
            receipt_digest: SeparationReceiptRef,
            custody_record_digest: CustodyRecordRef,
        }

        impl $name {
            #[allow(
                dead_code,
                reason = "used by reviewed transition modules added after Task 8"
            )]
            pub(crate) const fn live(
                receipt_digest: SeparationReceiptRef,
                custody_record_digest: CustodyRecordRef,
            ) -> Self {
                Self {
                    mode: TerminalMode::Live,
                    recovery_assessment_digest: None,
                    receipt_digest,
                    custody_record_digest,
                }
            }

            #[allow(
                dead_code,
                reason = "used by reviewed transition modules added after Task 8"
            )]
            pub(crate) const fn recovered(
                recovery_assessment_digest: RecoveryAssessmentRef,
                receipt_digest: SeparationReceiptRef,
                custody_record_digest: CustodyRecordRef,
            ) -> Self {
                Self {
                    mode: TerminalMode::Recovered,
                    recovery_assessment_digest: Some(recovery_assessment_digest),
                    receipt_digest,
                    custody_record_digest,
                }
            }

            #[must_use]
            pub const fn mode(&self) -> TerminalMode {
                self.mode
            }

            #[must_use]
            pub const fn recovery_assessment_digest(&self) -> Option<&RecoveryAssessmentRef> {
                self.recovery_assessment_digest.as_ref()
            }

            #[must_use]
            pub const fn receipt_digest(&self) -> &SeparationReceiptRef {
                &self.receipt_digest
            }

            #[must_use]
            pub const fn custody_record_digest(&self) -> &CustodyRecordRef {
                &self.custody_record_digest
            }
        }

        #[derive(Serialize, Deserialize)]
        #[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
        enum $wire {
            Live {
                #[serde(rename = "receiptDigest")]
                receipt_digest: SeparationReceiptRef,
                #[serde(rename = "custodyRecordDigest")]
                custody_record_digest: CustodyRecordRef,
            },
            Recovered {
                #[serde(rename = "recoveryAssessmentDigest")]
                recovery_assessment_digest: RecoveryAssessmentRef,
                #[serde(rename = "receiptDigest")]
                receipt_digest: SeparationReceiptRef,
                #[serde(rename = "custodyRecordDigest")]
                custody_record_digest: CustodyRecordRef,
            },
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                match (&self.mode, &self.recovery_assessment_digest) {
                    (TerminalMode::Live, None) => $wire::Live {
                        receipt_digest: self.receipt_digest.clone(),
                        custody_record_digest: self.custody_record_digest.clone(),
                    }
                    .serialize(serializer),
                    (TerminalMode::Recovered, Some(recovery_assessment_digest)) => {
                        $wire::Recovered {
                            recovery_assessment_digest: recovery_assessment_digest.clone(),
                            receipt_digest: self.receipt_digest.clone(),
                            custody_record_digest: self.custody_record_digest.clone(),
                        }
                        .serialize(serializer)
                    }
                    _ => Err(serde::ser::Error::custom(
                        "invalid separation terminal mode",
                    )),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(match $wire::deserialize(deserializer)? {
                    $wire::Live {
                        receipt_digest,
                        custody_record_digest,
                    } => Self {
                        mode: TerminalMode::Live,
                        recovery_assessment_digest: None,
                        receipt_digest,
                        custody_record_digest,
                    },
                    $wire::Recovered {
                        recovery_assessment_digest,
                        receipt_digest,
                        custody_record_digest,
                    } => Self {
                        mode: TerminalMode::Recovered,
                        recovery_assessment_digest: Some(recovery_assessment_digest),
                        receipt_digest,
                        custody_record_digest,
                    },
                })
            }
        }
    };
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            mode: TerminalMode,
            recovery_assessment_digest: Option<RecoveryAssessmentRef>,
            receipt_digest: SeparationReceiptRef,
        }

        impl $name {
            #[allow(
                dead_code,
                reason = "used by reviewed transition modules added after Task 8"
            )]
            pub(crate) const fn live(receipt_digest: SeparationReceiptRef) -> Self {
                Self {
                    mode: TerminalMode::Live,
                    recovery_assessment_digest: None,
                    receipt_digest,
                }
            }

            #[allow(
                dead_code,
                reason = "used by reviewed transition modules added after Task 8"
            )]
            pub(crate) const fn recovered(
                recovery_assessment_digest: RecoveryAssessmentRef,
                receipt_digest: SeparationReceiptRef,
            ) -> Self {
                Self {
                    mode: TerminalMode::Recovered,
                    recovery_assessment_digest: Some(recovery_assessment_digest),
                    receipt_digest,
                }
            }

            #[must_use]
            pub const fn mode(&self) -> TerminalMode {
                self.mode
            }

            #[must_use]
            pub const fn recovery_assessment_digest(&self) -> Option<&RecoveryAssessmentRef> {
                self.recovery_assessment_digest.as_ref()
            }

            #[must_use]
            pub const fn receipt_digest(&self) -> &SeparationReceiptRef {
                &self.receipt_digest
            }
        }

        #[derive(Serialize, Deserialize)]
        #[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
        enum IndeterminateTerminalWire {
            Live {
                #[serde(rename = "receiptDigest")]
                receipt_digest: SeparationReceiptRef,
            },
            Recovered {
                #[serde(rename = "recoveryAssessmentDigest")]
                recovery_assessment_digest: RecoveryAssessmentRef,
                #[serde(rename = "receiptDigest")]
                receipt_digest: SeparationReceiptRef,
            },
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                match (&self.mode, &self.recovery_assessment_digest) {
                    (TerminalMode::Live, None) => IndeterminateTerminalWire::Live {
                        receipt_digest: self.receipt_digest.clone(),
                    }
                    .serialize(serializer),
                    (TerminalMode::Recovered, Some(recovery_assessment_digest)) => {
                        IndeterminateTerminalWire::Recovered {
                            recovery_assessment_digest: recovery_assessment_digest.clone(),
                            receipt_digest: self.receipt_digest.clone(),
                        }
                        .serialize(serializer)
                    }
                    _ => Err(serde::ser::Error::custom(
                        "invalid separation terminal mode",
                    )),
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                Ok(
                    match IndeterminateTerminalWire::deserialize(deserializer)? {
                        IndeterminateTerminalWire::Live { receipt_digest } => Self {
                            mode: TerminalMode::Live,
                            recovery_assessment_digest: None,
                            receipt_digest,
                        },
                        IndeterminateTerminalWire::Recovered {
                            recovery_assessment_digest,
                            receipt_digest,
                        } => Self {
                            mode: TerminalMode::Recovered,
                            recovery_assessment_digest: Some(recovery_assessment_digest),
                            receipt_digest,
                        },
                    },
                )
            }
        }
    };
}

separation_terminal_payload!(SeparationVerifiedPayload, SeparationVerifiedWire, custody);
separation_terminal_payload!(SeparationFailedPayload, SeparationFailedWire, custody);
separation_terminal_payload!(SeparationIndeterminatePayload);

payload_struct!(CustodyAbsentPayload {
    receipt_digest: EffectReceiptRef,
    custody_record_digest: CustodyRecordRef,
});
payload_struct!(CustodyDisputedPayload {
    terminal_receipt: TerminalReceiptRef,
    custody_record_digest: CustodyRecordRef,
});

/// A payload that has already been decoded according to its event type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum EventPayload {
    InstallationEnrolled(InstallationEnrolledPayload),
    WarrantProposed(WarrantProposedPayload),
    WarrantApproved(WarrantApprovedPayload),
    WarrantRevoked(WarrantRevokedPayload),
    WarrantExpired(WarrantExpiredPayload),
    EffectReserved(EffectReservedPayload),
    EffectCancelledBeforeStart(EffectCancelledBeforeStartPayload),
    EffectStarted(EffectStartedPayload),
    ArtifactPrepared(ArtifactPreparedPayload),
    ArtifactPublished(ArtifactPublishedPayload),
    ArtifactPublishedRecovered(ArtifactPublishedRecoveredPayload),
    EffectVerified(EffectVerifiedPayload),
    EffectFailed(EffectFailedPayload),
    EffectIndeterminate(EffectIndeterminatePayload),
    SeparationWarrantProposed(SeparationWarrantProposedPayload),
    SeparationWarrantApproved(SeparationWarrantApprovedPayload),
    SeparationWarrantRevoked(SeparationWarrantRevokedPayload),
    SeparationWarrantExpired(SeparationWarrantExpiredPayload),
    SeparationReserved(SeparationReservedPayload),
    SeparationCancelledBeforeStart(SeparationCancelledBeforeStartPayload),
    SeparationStarted(SeparationStartedPayload),
    SeparationVerified(SeparationVerifiedPayload),
    SeparationFailed(SeparationFailedPayload),
    SeparationIndeterminate(SeparationIndeterminatePayload),
    CustodyAbsent(CustodyAbsentPayload),
    CustodyDisputed(CustodyDisputedPayload),
}

impl EventPayload {
    #[must_use]
    pub const fn event_type(&self) -> EventType {
        match self {
            Self::InstallationEnrolled(_) => EventType::InstallationEnrolled,
            Self::WarrantProposed(_) => EventType::WarrantProposed,
            Self::WarrantApproved(_) => EventType::WarrantApproved,
            Self::WarrantRevoked(_) => EventType::WarrantRevoked,
            Self::WarrantExpired(_) => EventType::WarrantExpired,
            Self::EffectReserved(_) => EventType::EffectReserved,
            Self::EffectCancelledBeforeStart(_) => EventType::EffectCancelledBeforeStart,
            Self::EffectStarted(_) => EventType::EffectStarted,
            Self::ArtifactPrepared(_) => EventType::ArtifactPrepared,
            Self::ArtifactPublished(_) => EventType::ArtifactPublished,
            Self::ArtifactPublishedRecovered(_) => EventType::ArtifactPublishedRecovered,
            Self::EffectVerified(_) => EventType::EffectVerified,
            Self::EffectFailed(_) => EventType::EffectFailed,
            Self::EffectIndeterminate(_) => EventType::EffectIndeterminate,
            Self::SeparationWarrantProposed(_) => EventType::SeparationWarrantProposed,
            Self::SeparationWarrantApproved(_) => EventType::SeparationWarrantApproved,
            Self::SeparationWarrantRevoked(_) => EventType::SeparationWarrantRevoked,
            Self::SeparationWarrantExpired(_) => EventType::SeparationWarrantExpired,
            Self::SeparationReserved(_) => EventType::SeparationReserved,
            Self::SeparationCancelledBeforeStart(_) => EventType::SeparationCancelledBeforeStart,
            Self::SeparationStarted(_) => EventType::SeparationStarted,
            Self::SeparationVerified(_) => EventType::SeparationVerified,
            Self::SeparationFailed(_) => EventType::SeparationFailed,
            Self::SeparationIndeterminate(_) => EventType::SeparationIndeterminate,
            Self::CustodyAbsent(_) => EventType::CustodyAbsent,
            Self::CustodyDisputed(_) => EventType::CustodyDisputed,
        }
    }

    fn decode(event_type: EventType, payload: Value) -> Result<Self, serde_json::Error> {
        fn typed<T: DeserializeOwned>(payload: Value) -> Result<T, serde_json::Error> {
            serde_json::from_value(payload)
        }

        Ok(match event_type {
            EventType::InstallationEnrolled => Self::InstallationEnrolled(typed(payload)?),
            EventType::WarrantProposed => Self::WarrantProposed(typed(payload)?),
            EventType::WarrantApproved => Self::WarrantApproved(typed(payload)?),
            EventType::WarrantRevoked => Self::WarrantRevoked(typed(payload)?),
            EventType::WarrantExpired => Self::WarrantExpired(typed(payload)?),
            EventType::EffectReserved => Self::EffectReserved(typed(payload)?),
            EventType::EffectCancelledBeforeStart => {
                Self::EffectCancelledBeforeStart(typed(payload)?)
            }
            EventType::EffectStarted => Self::EffectStarted(typed(payload)?),
            EventType::ArtifactPrepared => Self::ArtifactPrepared(typed(payload)?),
            EventType::ArtifactPublished => Self::ArtifactPublished(typed(payload)?),
            EventType::ArtifactPublishedRecovered => {
                Self::ArtifactPublishedRecovered(typed(payload)?)
            }
            EventType::EffectVerified => Self::EffectVerified(typed(payload)?),
            EventType::EffectFailed => Self::EffectFailed(typed(payload)?),
            EventType::EffectIndeterminate => Self::EffectIndeterminate(typed(payload)?),
            EventType::SeparationWarrantProposed => {
                Self::SeparationWarrantProposed(typed(payload)?)
            }
            EventType::SeparationWarrantApproved => {
                Self::SeparationWarrantApproved(typed(payload)?)
            }
            EventType::SeparationWarrantRevoked => Self::SeparationWarrantRevoked(typed(payload)?),
            EventType::SeparationWarrantExpired => Self::SeparationWarrantExpired(typed(payload)?),
            EventType::SeparationReserved => Self::SeparationReserved(typed(payload)?),
            EventType::SeparationCancelledBeforeStart => {
                Self::SeparationCancelledBeforeStart(typed(payload)?)
            }
            EventType::SeparationStarted => Self::SeparationStarted(typed(payload)?),
            EventType::SeparationVerified => Self::SeparationVerified(typed(payload)?),
            EventType::SeparationFailed => Self::SeparationFailed(typed(payload)?),
            EventType::SeparationIndeterminate => Self::SeparationIndeterminate(typed(payload)?),
            EventType::CustodyAbsent => Self::CustodyAbsent(typed(payload)?),
            EventType::CustodyDisputed => Self::CustodyDisputed(typed(payload)?),
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the frozen event/body edge table stays visibly exhaustive"
    )]
    fn references(&self) -> Vec<(&Digest, BodyKind)> {
        fn one<K: crate::body::BodyTag>(
            reference: &crate::body::BodyRef<K>,
        ) -> (&Digest, BodyKind) {
            (reference.digest(), K::KIND)
        }

        match self {
            Self::InstallationEnrolled(value) => vec![one(value.enrollment_digest())],
            Self::WarrantProposed(value) => vec![one(value.warrant_digest())],
            Self::WarrantExpired(value) => vec![one(value.warrant_digest())],
            Self::WarrantApproved(value) => vec![one(value.approval_digest())],
            Self::WarrantRevoked(value) => vec![one(value.revocation_digest())],
            Self::EffectReserved(value) => {
                vec![one(value.binding_digest()), one(value.lease_digest())]
            }
            Self::EffectCancelledBeforeStart(value) => {
                vec![one(value.binding_digest()), one(value.lease_digest())]
            }
            Self::EffectStarted(value) => vec![
                one(value.binding_digest()),
                one(value.lease_digest()),
                one(value.prepared_artifact_digest()),
                one(value.source_before_observation_digest()),
                one(value.target_before_observation_digest()),
            ],
            Self::ArtifactPrepared(value) => vec![one(value.prepared_artifact_digest())],
            Self::ArtifactPublished(value) => vec![one(value.evidence_digest())],
            Self::ArtifactPublishedRecovered(value) => {
                vec![one(value.recovery_assessment_digest())]
            }
            Self::EffectVerified(value) => vec![
                one(value.receipt_digest()),
                one(value.deed_digest()),
                one(value.custody_record_digest()),
            ],
            Self::EffectFailed(value) => vec![one(value.receipt_digest())],
            Self::EffectIndeterminate(value) => vec![one(value.receipt_digest())],
            Self::SeparationWarrantProposed(value) => vec![one(value.warrant_digest())],
            Self::SeparationWarrantExpired(value) => vec![one(value.warrant_digest())],
            Self::SeparationWarrantApproved(value) => vec![one(value.approval_digest())],
            Self::SeparationWarrantRevoked(value) => vec![one(value.revocation_digest())],
            Self::SeparationReserved(value) => {
                vec![one(value.binding_digest()), one(value.lease_digest())]
            }
            Self::SeparationCancelledBeforeStart(value) => {
                vec![one(value.binding_digest()), one(value.lease_digest())]
            }
            Self::SeparationStarted(value) => vec![
                one(value.binding_digest()),
                one(value.lease_digest()),
                one(value.deed_digest()),
                one(value.active_before_observation_digest()),
                one(value.quarantine_before_observation_digest()),
            ],
            Self::SeparationVerified(value) => {
                let mut references = vec![
                    one(value.receipt_digest()),
                    one(value.custody_record_digest()),
                ];
                if let Some(recovery) = value.recovery_assessment_digest() {
                    references.push(one(recovery));
                }
                references
            }
            Self::SeparationFailed(value) => {
                let mut references = vec![
                    one(value.receipt_digest()),
                    one(value.custody_record_digest()),
                ];
                if let Some(recovery) = value.recovery_assessment_digest() {
                    references.push(one(recovery));
                }
                references
            }
            Self::SeparationIndeterminate(value) => {
                let mut references = vec![one(value.receipt_digest())];
                if let Some(recovery) = value.recovery_assessment_digest() {
                    references.push(one(recovery));
                }
                references
            }
            Self::CustodyAbsent(value) => {
                vec![
                    one(value.receipt_digest()),
                    one(value.custody_record_digest()),
                ]
            }
            Self::CustodyDisputed(value) => {
                let receipt = match value.terminal_receipt() {
                    TerminalReceiptRef::Publication { digest } => one(digest),
                    TerminalReceiptRef::Separation { digest } => one(digest),
                };
                vec![receipt, one(value.custody_record_digest())]
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventPreimageWire {
    schema_version: String,
    sequence: U64Decimal,
    previous_event: PreviousEvent,
    installation_digest: InstallationEnrollmentRef,
    occurred_at: UnixNanoseconds,
    event_type: String,
    payload: Value,
}

/// The sole identity-bearing preimage for an event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventPreimage {
    schema_version: String,
    sequence: U64Decimal,
    previous_event: PreviousEvent,
    installation_digest: InstallationEnrollmentRef,
    occurred_at: UnixNanoseconds,
    event_type: EventType,
    payload: EventPayload,
}

impl EventPreimage {
    #[allow(
        dead_code,
        reason = "used by the reviewed transition modules added after Task 8"
    )]
    pub(crate) fn new(
        sequence: U64Decimal,
        previous_event: PreviousEvent,
        installation_digest: InstallationEnrollmentRef,
        occurred_at: UnixNanoseconds,
        payload: EventPayload,
    ) -> Self {
        let event_type = payload.event_type();
        Self {
            schema_version: EVENT_SCHEMA_VERSION.to_owned(),
            sequence,
            previous_event,
            installation_digest,
            occurred_at,
            event_type,
            payload,
        }
    }

    #[must_use]
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    #[must_use]
    pub const fn sequence(&self) -> U64Decimal {
        self.sequence
    }

    #[must_use]
    pub const fn previous_event(&self) -> &PreviousEvent {
        &self.previous_event
    }

    #[must_use]
    pub const fn installation_digest(&self) -> &InstallationEnrollmentRef {
        &self.installation_digest
    }

    #[must_use]
    pub const fn occurred_at(&self) -> UnixNanoseconds {
        self.occurred_at
    }

    #[must_use]
    pub const fn event_type(&self) -> EventType {
        self.event_type
    }

    #[must_use]
    pub const fn payload(&self) -> &EventPayload {
        &self.payload
    }
}

impl<'de> Deserialize<'de> for EventPreimage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = EventPreimageWire::deserialize(deserializer)?;
        let event_type = EventType::parse(&raw.event_type)
            .ok_or_else(|| serde::de::Error::custom(UNKNOWN_EVENT_TYPE_MARKER))?;
        let payload = EventPayload::decode(event_type, raw.payload)
            .map_err(|_| serde::de::Error::custom(TYPE_CONFUSED_PAYLOAD_MARKER))?;
        Ok(Self {
            schema_version: raw.schema_version,
            sequence: raw.sequence,
            previous_event: raw.previous_event,
            installation_digest: raw.installation_digest,
            occurred_at: raw.occurred_at,
            event_type,
            payload,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventEnvelopeWire {
    digest: String,
    preimage: EventPreimage,
}

/// Stored event envelope. Its digest covers only the canonical preimage.
///
/// Only reviewed transition code inside this crate can seal a new event. Public callers may
/// reconstruct untrusted stored envelopes, then validate them against an independent head.
///
/// ```compile_fail
/// use guild_effect_kernel::event::seal_event;
/// let _sealer = seal_event;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventEnvelope {
    digest: Digest,
    preimage: EventPreimage,
}

impl EventEnvelope {
    /// Strictly decodes and recomputes a stored envelope identity.
    ///
    /// # Errors
    ///
    /// Rejects duplicate members, unknown event types, type-confused payloads, malformed values,
    /// and any claimed digest that differs from the canonical preimage digest.
    pub fn from_json(input: &[u8]) -> Result<Self, ChainError> {
        let wire: EventEnvelopeWire = strict_from_slice(input).map_err(map_decode_error)?;
        let claimed_digest = Digest::parse(&wire.digest).map_err(|_| ChainError::DigestMismatch)?;
        let sealed = seal_event(wire.preimage)?;
        if sealed.digest != claimed_digest {
            return Err(ChainError::DigestMismatch);
        }
        Ok(sealed)
    }

    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    #[must_use]
    pub const fn preimage(&self) -> &EventPreimage {
        &self.preimage
    }

    /// Returns the exact canonical bytes whose hash is this event's identity.
    ///
    /// # Errors
    ///
    /// Returns a canonicalization failure if the sealed typed value cannot be encoded.
    pub fn canonical_preimage_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        canonical_bytes(&self.preimage)
    }
}

/// An independently authenticated durable-store anchor supplied to chain validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedHead {
    installation_digest: InstallationEnrollmentRef,
    head_digest: Digest,
    anchored_at: UnixNanoseconds,
    trusted_store_id: Identifier,
}

impl TrustedHead {
    /// Packages values already authenticated by the outer trust boundary.
    #[must_use]
    pub const fn new(
        installation_digest: InstallationEnrollmentRef,
        head_digest: Digest,
        anchored_at: UnixNanoseconds,
        trusted_store_id: Identifier,
    ) -> Self {
        Self {
            installation_digest,
            head_digest,
            anchored_at,
            trusted_store_id,
        }
    }

    #[must_use]
    pub const fn installation_digest(&self) -> &InstallationEnrollmentRef {
        &self.installation_digest
    }

    #[must_use]
    pub const fn head_digest(&self) -> &Digest {
        &self.head_digest
    }

    #[must_use]
    pub const fn anchored_at(&self) -> UnixNanoseconds {
        self.anchored_at
    }

    #[must_use]
    pub const fn trusted_store_id(&self) -> &Identifier {
        &self.trusted_store_id
    }
}

/// Closed event decoding, chain corruption, and proposal validation failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    #[error("event chain is empty")]
    Empty,
    #[error("trusted head does not match the chain")]
    HeadMismatch,
    #[error("event sequence is discontinuous")]
    SequenceDiscontinuity,
    #[error("previous-event link is invalid")]
    PreviousLinkMismatch,
    #[error("event time decreased")]
    TimeRegression,
    #[error("event map key or envelope digest is invalid")]
    DigestMismatch,
    #[error("event history forks")]
    Fork,
    #[error("event history has a gap")]
    Gap,
    #[error("event payload is not valid for its event type")]
    TypeConfusedPayload,
    #[error("event JSON contains a duplicate member")]
    DuplicateMember,
    #[error("unknown event type")]
    UnknownEventType,
    #[error("event tail is truncated relative to the trusted head")]
    TruncatedTail,
    #[error("event chain is not rooted in canonical enrollment genesis")]
    InvalidGenesis,
    #[error("transition bundle is not atomic or internally linked")]
    InvalidBundle,
    #[error("body graph validation failed: {0}")]
    Body(#[from] BodyError),
}

fn map_decode_error(error: CanonicalError) -> ChainError {
    match error {
        CanonicalError::DuplicateMember { .. } => ChainError::DuplicateMember,
        CanonicalError::Decode(error) if error.to_string().contains(UNKNOWN_EVENT_TYPE_MARKER) => {
            ChainError::UnknownEventType
        }
        CanonicalError::Decode(error)
            if error.to_string().contains(TYPE_CONFUSED_PAYLOAD_MARKER) =>
        {
            ChainError::TypeConfusedPayload
        }
        CanonicalError::Decode(_) | CanonicalError::Number | CanonicalError::Digest(_) => {
            ChainError::TypeConfusedPayload
        }
        CanonicalError::Encode(_) => ChainError::InvalidBundle,
    }
}

/// Seals a transition-created preimage by hashing its canonical bytes.
pub(crate) fn seal_event(preimage: EventPreimage) -> Result<EventEnvelope, ChainError> {
    if preimage.schema_version != EVENT_SCHEMA_VERSION
        || preimage.event_type != preimage.payload.event_type()
    {
        return Err(ChainError::TypeConfusedPayload);
    }
    let digest = canonical_digest(&preimage).map_err(|_| ChainError::DigestMismatch)?;
    Ok(EventEnvelope { digest, preimage })
}

pub(crate) fn validate_envelope_identity(
    key: &Digest,
    event: &EventEnvelope,
) -> Result<(), ChainError> {
    let computed = canonical_digest(event.preimage()).map_err(|_| ChainError::DigestMismatch)?;
    if key != event.digest() || &computed != event.digest() {
        return Err(ChainError::DigestMismatch);
    }
    if event.preimage.schema_version != EVENT_SCHEMA_VERSION
        || event.preimage.event_type != event.preimage.payload.event_type()
    {
        return Err(ChainError::TypeConfusedPayload);
    }
    Ok(())
}

pub(crate) fn validate_event_body_refs(
    bodies: &BodyGraph,
    event: &EventEnvelope,
) -> Result<(), ChainError> {
    for (digest, expected) in event.preimage.payload.references() {
        match bodies.get(digest) {
            Some(body) if body.kind() == expected => {}
            Some(body) => {
                return Err(BodyError::WrongTargetKind {
                    source: body.kind(),
                    expected,
                    actual: body.kind(),
                }
                .into());
            }
            None => {
                return Err(BodyError::MissingReference {
                    source: event.digest.clone(),
                    target: digest.clone(),
                }
                .into());
            }
        }
    }
    Ok(())
}

/// Validates exactly the history ending at an independently supplied durable-store head.
///
/// Events from other installations are identity-checked but do not create a fork in this
/// installation. Every same-installation event must belong to the anchored ancestry.
///
/// # Errors
///
/// Returns a closed corruption category for malformed identity, missing or forked ancestry,
/// invalid genesis, discontinuous sequence/time, or unresolved typed payload references.
#[allow(
    clippy::too_many_lines,
    reason = "the closed chain proof keeps its ordered corruption taxonomy visibly auditable"
)]
pub fn validate_chain(
    bodies: &BodyGraph,
    events: &BTreeMap<Digest, EventEnvelope>,
    expected_head: &TrustedHead,
) -> Result<Vec<EventEnvelope>, ChainError> {
    if events.is_empty() {
        return Err(ChainError::Empty);
    }
    for (key, event) in events {
        validate_envelope_identity(key, event)?;
    }

    let Some(head) = events.get(expected_head.head_digest()) else {
        return Err(ChainError::TruncatedTail);
    };
    if head.preimage.installation_digest != expected_head.installation_digest {
        return Err(ChainError::HeadMismatch);
    }

    let scoped: Vec<_> = events
        .values()
        .filter(|event| event.preimage.installation_digest == expected_head.installation_digest)
        .collect();
    let mut roots = 0_usize;
    let mut child_counts = BTreeMap::<Digest, usize>::new();
    let mut referenced_predecessors = BTreeSet::new();
    for event in &scoped {
        match event.preimage.previous_event() {
            PreviousEvent::Genesis => roots += 1,
            PreviousEvent::Previous { digest } => {
                let Some(predecessor) = events.get(digest) else {
                    return Err(ChainError::Gap);
                };
                if predecessor.preimage.installation_digest != expected_head.installation_digest {
                    return Err(ChainError::PreviousLinkMismatch);
                }
                let child_count = child_counts.entry(digest.clone()).or_default();
                *child_count += 1;
                if *child_count > 1 {
                    return Err(ChainError::Fork);
                }
                referenced_predecessors.insert(digest.clone());
            }
        }
    }
    if roots > 1 {
        return Err(ChainError::Fork);
    }
    if roots == 0 {
        return Err(ChainError::PreviousLinkMismatch);
    }
    if referenced_predecessors.contains(expected_head.head_digest()) {
        return Err(ChainError::HeadMismatch);
    }

    let mut reverse = Vec::new();
    let mut visited = BTreeSet::new();
    let mut cursor = expected_head.head_digest();
    loop {
        if !visited.insert(cursor.clone()) {
            return Err(ChainError::PreviousLinkMismatch);
        }
        let event = events.get(cursor).ok_or(ChainError::Gap)?;
        if event.preimage.installation_digest != expected_head.installation_digest {
            return Err(ChainError::PreviousLinkMismatch);
        }
        reverse.push(event.clone());
        match event.preimage.previous_event() {
            PreviousEvent::Genesis => break,
            PreviousEvent::Previous { digest } => cursor = digest,
        }
    }
    reverse.reverse();

    let Some(first) = reverse.first() else {
        return Err(ChainError::Empty);
    };
    if first.preimage.sequence().get() != 0 {
        return Err(ChainError::PreviousLinkMismatch);
    }
    let EventPayload::InstallationEnrolled(genesis_payload) = first.preimage.payload() else {
        return Err(ChainError::InvalidGenesis);
    };
    if !matches!(first.preimage.previous_event(), PreviousEvent::Genesis)
        || genesis_payload.enrollment_digest() != first.preimage.installation_digest()
        || first.preimage.installation_digest() != expected_head.installation_digest()
    {
        return Err(ChainError::InvalidGenesis);
    }

    for pair in reverse.windows(2) {
        let [previous, current] = pair else {
            unreachable!("windows(2) always contains two events")
        };
        let expected_sequence = previous
            .preimage
            .sequence()
            .checked_add(1)
            .map_err(|_| ChainError::SequenceDiscontinuity)?;
        if current.preimage.sequence() != expected_sequence {
            return Err(ChainError::SequenceDiscontinuity);
        }
        if !matches!(
            current.preimage.previous_event(),
            PreviousEvent::Previous { digest } if digest == previous.digest()
        ) {
            return Err(ChainError::PreviousLinkMismatch);
        }
        if current.preimage.occurred_at() < previous.preimage.occurred_at() {
            return Err(ChainError::TimeRegression);
        }
    }

    if visited.len() != scoped.len() {
        return Err(ChainError::PreviousLinkMismatch);
    }
    for event in &reverse {
        validate_event_body_refs(bodies, event)?;
    }
    Ok(reverse)
}

#[cfg(test)]
mod tests {
    use super::{
        EventPayload, InstallationEnrolledPayload, PreviousEvent, SeparationIndeterminatePayload,
        SeparationVerifiedPayload, TerminalMode, TrustedHead, seal_event, validate_chain,
    };
    use crate::{
        body::{
            BodyGraph, CustodyRecordRef, InstallationEnrollmentRef, RecoveryAssessmentRef,
            SeparationReceiptRef,
        },
        scalar::{Digest, Identifier, U64Decimal, UnixNanoseconds},
    };

    fn digest(nibble: char) -> Digest {
        Digest::parse(&format!("sha256:{}", nibble.to_string().repeat(64))).unwrap()
    }

    #[test]
    fn crate_transitions_can_build_typed_payloads_without_open_json() {
        let enrollment = InstallationEnrollmentRef::from_digest(digest('1'));
        let payload = InstallationEnrolledPayload::new(enrollment.clone());
        assert_eq!(payload.enrollment_digest(), &enrollment);
        assert_eq!(
            EventPayload::InstallationEnrolled(payload).event_type(),
            super::EventType::InstallationEnrolled
        );

        let receipt = SeparationReceiptRef::from_digest(digest('2'));
        let custody = CustodyRecordRef::from_digest(digest('3'));
        let recovery = RecoveryAssessmentRef::from_digest(digest('4'));
        let live = SeparationVerifiedPayload::live(receipt.clone(), custody);
        assert_eq!(live.mode(), TerminalMode::Live);
        assert!(live.recovery_assessment_digest().is_none());
        let recovered = SeparationIndeterminatePayload::recovered(recovery.clone(), receipt);
        assert_eq!(recovered.mode(), TerminalMode::Recovered);
        assert_eq!(recovered.recovery_assessment_digest(), Some(&recovery));
    }

    #[test]
    fn a_fabricated_event_cycle_fails_identity_before_topology() {
        let installation = InstallationEnrollmentRef::from_digest(digest('1'));
        let payload = EventPayload::InstallationEnrolled(InstallationEnrolledPayload::new(
            installation.clone(),
        ));
        let mut event = seal_event(super::EventPreimage::new(
            U64Decimal::from_u64(0),
            PreviousEvent::Genesis,
            installation.clone(),
            UnixNanoseconds::parse("0").unwrap(),
            payload,
        ))
        .unwrap();
        let original_digest = event.digest.clone();
        event.preimage.previous_event = PreviousEvent::Previous {
            digest: original_digest.clone(),
        };
        let events = [(original_digest.clone(), event)].into_iter().collect();
        let trusted = TrustedHead::new(
            installation,
            original_digest,
            UnixNanoseconds::parse("0").unwrap(),
            Identifier::parse("trusted-store").unwrap(),
        );
        assert_eq!(
            validate_chain(&BodyGraph::empty(), &events, &trusted).unwrap_err(),
            super::ChainError::DigestMismatch
        );
    }
}
