//! Typed immutable bodies and fail-closed content-addressed graph validation.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    marker::PhantomData,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest as ShaDigest, Sha256};

use crate::{
    canonical::{CanonicalError, canonical_bytes, strict_from_slice},
    scalar::{
        ArtifactName, ByteLength, Digest, Identifier, IncarnationId, LogicalAddress, RawDigest,
        U64Decimal, UnixNanoseconds, XattrName,
    },
    schema::{FieldType, SchemaDescriptor, SchemaId, descriptor},
};

/// The exact closed set of body kinds in effect protocol v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BodyKind {
    #[serde(rename = "installation-enrollment/v1")]
    InstallationEnrollment,
    #[serde(rename = "authority-policy/v1")]
    AuthorityPolicy,
    #[serde(rename = "schema-descriptor/v1")]
    SchemaDescriptor,
    #[serde(rename = "local-file-observation/v1")]
    LocalFileObservation,
    #[serde(rename = "xattr-value/v1")]
    XattrValue,
    #[serde(rename = "static-artifact-publish-input/v1")]
    StaticArtifactPublishInput,
    #[serde(rename = "static-artifact-publish-precondition/v1")]
    StaticArtifactPublishPrecondition,
    #[serde(rename = "static-artifact-separation-input/v1")]
    StaticArtifactSeparationInput,
    #[serde(rename = "static-artifact-separation-precondition/v1")]
    StaticArtifactSeparationPrecondition,
    #[serde(rename = "publication-warrant/v1")]
    PublicationWarrant,
    #[serde(rename = "publication-approval/v1")]
    PublicationApproval,
    #[serde(rename = "publication-revocation/v1")]
    PublicationRevocation,
    #[serde(rename = "effect-lease/v1")]
    EffectLease,
    #[serde(rename = "idempotency-binding/v1")]
    IdempotencyBinding,
    #[serde(rename = "prepared-artifact/v1")]
    PreparedArtifact,
    #[serde(rename = "publication-evidence/v1")]
    PublicationEvidence,
    #[serde(rename = "causality-assessment/v1")]
    CausalityAssessment,
    #[serde(rename = "effect-receipt/v1")]
    EffectReceipt,
    #[serde(rename = "resource-deed/v1")]
    ResourceDeed,
    #[serde(rename = "separation-warrant/v1")]
    SeparationWarrant,
    #[serde(rename = "separation-approval/v1")]
    SeparationApproval,
    #[serde(rename = "separation-revocation/v1")]
    SeparationRevocation,
    #[serde(rename = "separation-lease/v1")]
    SeparationLease,
    #[serde(rename = "separation-binding/v1")]
    SeparationBinding,
    #[serde(rename = "separation-evidence/v1")]
    SeparationEvidence,
    #[serde(rename = "separation-receipt/v1")]
    SeparationReceipt,
    #[serde(rename = "custody-record/v1")]
    CustodyRecord,
    #[serde(rename = "recovery-assessment/v1")]
    RecoveryAssessment,
    #[serde(rename = "dossier-summary/v1")]
    DossierSummary,
}

impl BodyKind {
    pub const ALL: [Self; 29] = [
        Self::InstallationEnrollment,
        Self::AuthorityPolicy,
        Self::SchemaDescriptor,
        Self::LocalFileObservation,
        Self::XattrValue,
        Self::StaticArtifactPublishInput,
        Self::StaticArtifactPublishPrecondition,
        Self::StaticArtifactSeparationInput,
        Self::StaticArtifactSeparationPrecondition,
        Self::PublicationWarrant,
        Self::PublicationApproval,
        Self::PublicationRevocation,
        Self::EffectLease,
        Self::IdempotencyBinding,
        Self::PreparedArtifact,
        Self::PublicationEvidence,
        Self::CausalityAssessment,
        Self::EffectReceipt,
        Self::ResourceDeed,
        Self::SeparationWarrant,
        Self::SeparationApproval,
        Self::SeparationRevocation,
        Self::SeparationLease,
        Self::SeparationBinding,
        Self::SeparationEvidence,
        Self::SeparationReceipt,
        Self::CustodyRecord,
        Self::RecoveryAssessment,
        Self::DossierSummary,
    ];

    /// Returns the frozen protocol identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstallationEnrollment => "installation-enrollment/v1",
            Self::AuthorityPolicy => "authority-policy/v1",
            Self::SchemaDescriptor => "schema-descriptor/v1",
            Self::LocalFileObservation => "local-file-observation/v1",
            Self::XattrValue => "xattr-value/v1",
            Self::StaticArtifactPublishInput => "static-artifact-publish-input/v1",
            Self::StaticArtifactPublishPrecondition => "static-artifact-publish-precondition/v1",
            Self::StaticArtifactSeparationInput => "static-artifact-separation-input/v1",
            Self::StaticArtifactSeparationPrecondition => {
                "static-artifact-separation-precondition/v1"
            }
            Self::PublicationWarrant => "publication-warrant/v1",
            Self::PublicationApproval => "publication-approval/v1",
            Self::PublicationRevocation => "publication-revocation/v1",
            Self::EffectLease => "effect-lease/v1",
            Self::IdempotencyBinding => "idempotency-binding/v1",
            Self::PreparedArtifact => "prepared-artifact/v1",
            Self::PublicationEvidence => "publication-evidence/v1",
            Self::CausalityAssessment => "causality-assessment/v1",
            Self::EffectReceipt => "effect-receipt/v1",
            Self::ResourceDeed => "resource-deed/v1",
            Self::SeparationWarrant => "separation-warrant/v1",
            Self::SeparationApproval => "separation-approval/v1",
            Self::SeparationRevocation => "separation-revocation/v1",
            Self::SeparationLease => "separation-lease/v1",
            Self::SeparationBinding => "separation-binding/v1",
            Self::SeparationEvidence => "separation-evidence/v1",
            Self::SeparationReceipt => "separation-receipt/v1",
            Self::CustodyRecord => "custody-record/v1",
            Self::RecoveryAssessment => "recovery-assessment/v1",
            Self::DossierSummary => "dossier-summary/v1",
        }
    }

    fn parse(input: &str) -> Result<Self, BodyError> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == input)
            .ok_or_else(|| BodyError::UnknownKind {
                kind: input.to_owned(),
            })
    }

    /// Returns every target kind permitted by protocol §7.2 for this source kind.
    #[must_use]
    pub const fn permitted_target_kinds(self) -> &'static [Self] {
        permitted_target_kinds(self)
    }
}

impl fmt::Display for BodyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub(crate) mod sealed {
    pub trait BodyTag {}
    pub trait BodySpec {}
}

/// A compile-time body-kind marker from the closed v1 registry.
pub trait BodyTag: sealed::BodyTag {
    const KIND: BodyKind;
}

macro_rules! body_tags {
    ($(($tag:ident, $reference:ident, $kind:ident)),+ $(,)?) => {
        $(
            /// Uninhabited marker for one frozen body kind.
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub enum $tag {}
            impl sealed::BodyTag for $tag {}
            impl BodyTag for $tag {
                const KIND: BodyKind = BodyKind::$kind;
            }
            pub type $reference = BodyRef<$tag>;
        )+
    };
}

body_tags!(
    (
        InstallationEnrollmentTag,
        InstallationEnrollmentRef,
        InstallationEnrollment
    ),
    (AuthorityPolicyTag, AuthorityPolicyRef, AuthorityPolicy),
    (SchemaDescriptorTag, SchemaDescriptorRef, SchemaDescriptor),
    (
        LocalFileObservationTag,
        LocalFileObservationRef,
        LocalFileObservation
    ),
    (XattrValueTag, XattrValueRef, XattrValue),
    (
        StaticArtifactPublishInputTag,
        StaticArtifactPublishInputRef,
        StaticArtifactPublishInput
    ),
    (
        StaticArtifactPublishPreconditionTag,
        StaticArtifactPublishPreconditionRef,
        StaticArtifactPublishPrecondition
    ),
    (
        StaticArtifactSeparationInputTag,
        StaticArtifactSeparationInputRef,
        StaticArtifactSeparationInput
    ),
    (
        StaticArtifactSeparationPreconditionTag,
        StaticArtifactSeparationPreconditionRef,
        StaticArtifactSeparationPrecondition
    ),
    (
        PublicationWarrantTag,
        PublicationWarrantRef,
        PublicationWarrant
    ),
    (
        PublicationApprovalTag,
        PublicationApprovalRef,
        PublicationApproval
    ),
    (
        PublicationRevocationTag,
        PublicationRevocationRef,
        PublicationRevocation
    ),
    (EffectLeaseTag, EffectLeaseRef, EffectLease),
    (
        IdempotencyBindingTag,
        IdempotencyBindingRef,
        IdempotencyBinding
    ),
    (PreparedArtifactTag, PreparedArtifactRef, PreparedArtifact),
    (
        PublicationEvidenceTag,
        PublicationEvidenceRef,
        PublicationEvidence
    ),
    (
        CausalityAssessmentTag,
        CausalityAssessmentRef,
        CausalityAssessment
    ),
    (EffectReceiptTag, EffectReceiptRef, EffectReceipt),
    (ResourceDeedTag, ResourceDeedRef, ResourceDeed),
    (
        SeparationWarrantTag,
        SeparationWarrantRef,
        SeparationWarrant
    ),
    (
        SeparationApprovalTag,
        SeparationApprovalRef,
        SeparationApproval
    ),
    (
        SeparationRevocationTag,
        SeparationRevocationRef,
        SeparationRevocation
    ),
    (SeparationLeaseTag, SeparationLeaseRef, SeparationLease),
    (
        SeparationBindingTag,
        SeparationBindingRef,
        SeparationBinding
    ),
    (
        SeparationEvidenceTag,
        SeparationEvidenceRef,
        SeparationEvidence
    ),
    (
        SeparationReceiptTag,
        SeparationReceiptRef,
        SeparationReceipt
    ),
    (CustodyRecordTag, CustodyRecordRef, CustodyRecord),
    (
        RecoveryAssessmentTag,
        RecoveryAssessmentRef,
        RecoveryAssessment
    ),
    (DossierSummaryTag, DossierSummaryRef, DossierSummary),
);

/// A typed claim about a body digest. Its kind is proved only by graph resolution.
pub struct BodyRef<K: BodyTag> {
    digest: Digest,
    marker: PhantomData<fn() -> K>,
}

impl<K: BodyTag> BodyRef<K> {
    #[must_use]
    pub fn from_digest(digest: Digest) -> Self {
        Self {
            digest,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }
}

impl<K: BodyTag> Clone for BodyRef<K> {
    fn clone(&self) -> Self {
        Self::from_digest(self.digest.clone())
    }
}

impl<K: BodyTag> fmt::Debug for BodyRef<K> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BodyRef")
            .field(&self.digest)
            .finish()
    }
}

impl<K: BodyTag> PartialEq for BodyRef<K> {
    fn eq(&self, other: &Self) -> bool {
        self.digest == other.digest
    }
}

impl<K: BodyTag> Eq for BodyRef<K> {}

impl<K: BodyTag> std::hash::Hash for BodyRef<K> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.digest, state);
    }
}

impl<K: BodyTag> PartialOrd for BodyRef<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: BodyTag> Ord for BodyRef<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.digest.cmp(&other.digest)
    }
}

impl<K: BodyTag> Serialize for BodyRef<K> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.digest.serialize(serializer)
    }
}

impl<'de, K: BodyTag> Deserialize<'de> for BodyRef<K> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Digest::deserialize(deserializer).map(Self::from_digest)
    }
}

/// A reference whose target family is selected by an explicit wire tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case", deny_unknown_fields)]
#[serde(bound(serialize = "", deserialize = ""))]
pub enum ProtocolRef<P: BodyTag, S: BodyTag> {
    Publication { digest: BodyRef<P> },
    Separation { digest: BodyRef<S> },
}

impl<P: BodyTag, S: BodyTag> ProtocolRef<P, S> {
    #[must_use]
    pub const fn publication(digest: BodyRef<P>) -> Self {
        Self::Publication { digest }
    }

    #[must_use]
    pub const fn separation(digest: BodyRef<S>) -> Self {
        Self::Separation { digest }
    }

    #[must_use]
    pub const fn publication_digest(&self) -> Option<&BodyRef<P>> {
        match self {
            Self::Publication { digest } => Some(digest),
            Self::Separation { .. } => None,
        }
    }

    #[must_use]
    pub const fn separation_digest(&self) -> Option<&BodyRef<S>> {
        match self {
            Self::Publication { .. } => None,
            Self::Separation { digest } => Some(digest),
        }
    }
}

/// A closed explicit optional value; protocol values never omit fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum OptionalValue<T> {
    Absent,
    Present { value: T },
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum OptionalAbsentState {
    Absent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionalAbsentWire {
    state: OptionalAbsentState,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum OptionalPresentState {
    Present,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionalPresentWire<T> {
    state: OptionalPresentState,
    value: T,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionalWire<T> {
    Absent(OptionalAbsentWire),
    Present(OptionalPresentWire<T>),
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for OptionalValue<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match OptionalWire::<T>::deserialize(deserializer)? {
            OptionalWire::Absent(OptionalAbsentWire {
                state: OptionalAbsentState::Absent,
            }) => Ok(Self::Absent),
            OptionalWire::Present(OptionalPresentWire {
                state: OptionalPresentState::Present,
                value,
            }) => Ok(Self::Present { value }),
        }
    }
}

impl<T> OptionalValue<T> {
    #[must_use]
    pub const fn absent() -> Self {
        Self::Absent
    }

    #[must_use]
    pub const fn present(value: T) -> Self {
        Self::Present { value }
    }

    #[must_use]
    pub const fn as_ref(&self) -> OptionalValue<&T> {
        match self {
            Self::Absent => OptionalValue::Absent,
            Self::Present { value } => OptionalValue::Present { value },
        }
    }

    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Absent => None,
            Self::Present { value } => Some(value),
        }
    }
}

/// One typed outbound graph edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedEdge {
    target: Digest,
    expected: BodyKind,
}

impl TypedEdge {
    pub(crate) fn new<K: BodyTag>(target: &BodyRef<K>) -> Self {
        Self {
            target: target.digest().clone(),
            expected: K::KIND,
        }
    }

    #[must_use]
    pub const fn target(&self) -> &Digest {
        &self.target
    }

    #[must_use]
    pub const fn expected_kind(&self) -> BodyKind {
        self.expected
    }
}

/// The behavior every concrete identity-bearing payload supplies.
pub trait BodySpec: sealed::BodySpec + Clone + Serialize {
    type Tag: BodyTag;

    fn edges(&self) -> Vec<TypedEdge>;

    /// Validates constraints that do not require graph resolution.
    ///
    /// # Errors
    ///
    /// Returns a closed body-local or canonical validation failure.
    fn validate_local(&self) -> Result<(), BodyError>;
}

/// A sequence whose caller-provided order is already canonical and duplicate-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SortedUnique<T> {
    values: Vec<T>,
}

impl<T: Serialize> SortedUnique<T> {
    /// Validates order by canonical bytes without sorting or normalizing.
    ///
    /// # Errors
    ///
    /// Returns [`BodyError::NonCanonicalSet`] for an out-of-order or duplicate pair.
    pub fn new(values: Vec<T>) -> Result<Self, BodyError> {
        let mut previous: Option<Vec<u8>> = None;
        for value in &values {
            let bytes = canonical_bytes(value)?;
            if previous.as_ref().is_some_and(|prior| prior >= &bytes) {
                return Err(BodyError::NonCanonicalSet);
            }
            previous = Some(bytes);
        }
        Ok(Self { values })
    }

    /// Validates a protocol set with an explicit maximum length.
    ///
    /// # Errors
    ///
    /// Returns a local validation error when the cap is exceeded, or the ordering error from
    /// [`Self::new`].
    pub fn new_bounded(values: Vec<T>, maximum: usize) -> Result<Self, BodyError> {
        if values.len() > maximum {
            return Err(BodyError::Local(format!(
                "set length exceeds protocol maximum {maximum}"
            )));
        }
        Self::new(values)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.values
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<'de, T> Deserialize<'de> for SortedUnique<T>
where
    T: Deserialize<'de> + Serialize,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<T>::deserialize(deserializer)?;
        Self::new(values).map_err(serde::de::Error::custom)
    }
}

pub type NonEmptySortedSet<T> = SortedUnique<T>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XattrEntry {
    name: XattrName,
    value_digest: RawDigest,
    byte_length: ByteLength,
}

impl XattrEntry {
    #[must_use]
    pub const fn new(name: XattrName, value_digest: RawDigest, byte_length: ByteLength) -> Self {
        Self {
            name,
            value_digest,
            byte_length,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &XattrName {
        &self.name
    }

    #[must_use]
    pub const fn value_digest(&self) -> &RawDigest {
        &self.value_digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> ByteLength {
        self.byte_length
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct XattrValue {
    entries: NonEmptySortedSet<XattrEntry>,
}

impl XattrValue {
    /// Builds a nonempty canonical xattr metadata set.
    ///
    /// # Errors
    ///
    /// Returns a local validation error for an empty set or a canonical ordering error.
    pub fn new(entries: Vec<XattrEntry>) -> Result<Self, BodyError> {
        let entries = SortedUnique::new(entries)?;
        if entries.is_empty() {
            return Err(BodyError::Local(
                "xattr entries must be nonempty".to_owned(),
            ));
        }
        Ok(Self { entries })
    }

    #[must_use]
    pub const fn entries(&self) -> &NonEmptySortedSet<XattrEntry> {
        &self.entries
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum LocalFileObservation {
    Absent {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: Identifier,
        #[serde(rename = "observedAt")]
        observed_at: UnixNanoseconds,
    },
    Present {
        #[serde(rename = "logicalAddress")]
        logical_address: LogicalAddress,
        #[serde(rename = "witnessId")]
        witness_id: Identifier,
        #[serde(rename = "observedAt")]
        observed_at: UnixNanoseconds,
        #[serde(rename = "artifactName")]
        artifact_name: ArtifactName,
        #[serde(rename = "contentDigest")]
        content_digest: RawDigest,
        #[serde(rename = "byteLength")]
        byte_length: ByteLength,
        incarnation: IncarnationId,
        #[serde(rename = "quarantineXattrDigest")]
        quarantine_xattr_digest: OptionalValue<XattrValueRef>,
    },
}

impl LocalFileObservation {
    #[must_use]
    pub const fn absent(
        logical_address: LogicalAddress,
        witness_id: Identifier,
        observed_at: UnixNanoseconds,
    ) -> Self {
        Self::Absent {
            logical_address,
            witness_id,
            observed_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn present(
        logical_address: LogicalAddress,
        witness_id: Identifier,
        observed_at: UnixNanoseconds,
        artifact_name: ArtifactName,
        content_digest: RawDigest,
        byte_length: ByteLength,
        incarnation: IncarnationId,
        quarantine_xattr_digest: OptionalValue<XattrValueRef>,
    ) -> Self {
        Self::Present {
            logical_address,
            witness_id,
            observed_at,
            artifact_name,
            content_digest,
            byte_length,
            incarnation,
            quarantine_xattr_digest,
        }
    }

    #[must_use]
    pub const fn logical_address(&self) -> &LogicalAddress {
        match self {
            Self::Absent {
                logical_address, ..
            }
            | Self::Present {
                logical_address, ..
            } => logical_address,
        }
    }

    #[must_use]
    pub const fn witness_id(&self) -> &Identifier {
        match self {
            Self::Absent { witness_id, .. } | Self::Present { witness_id, .. } => witness_id,
        }
    }

    #[must_use]
    pub const fn observed_at(&self) -> UnixNanoseconds {
        match self {
            Self::Absent { observed_at, .. } | Self::Present { observed_at, .. } => *observed_at,
        }
    }

    #[must_use]
    pub const fn artifact_name(&self) -> Option<&ArtifactName> {
        match self {
            Self::Absent { .. } => None,
            Self::Present { artifact_name, .. } => Some(artifact_name),
        }
    }

    #[must_use]
    pub const fn content_digest(&self) -> Option<&RawDigest> {
        match self {
            Self::Absent { .. } => None,
            Self::Present { content_digest, .. } => Some(content_digest),
        }
    }

    #[must_use]
    pub const fn byte_length(&self) -> Option<ByteLength> {
        match self {
            Self::Absent { .. } => None,
            Self::Present { byte_length, .. } => Some(*byte_length),
        }
    }

    #[must_use]
    pub const fn incarnation(&self) -> Option<&IncarnationId> {
        match self {
            Self::Absent { .. } => None,
            Self::Present { incarnation, .. } => Some(incarnation),
        }
    }

    #[must_use]
    pub const fn quarantine_xattr_digest(&self) -> Option<&OptionalValue<XattrValueRef>> {
        match self {
            Self::Absent { .. } => None,
            Self::Present {
                quarantine_xattr_digest,
                ..
            } => Some(quarantine_xattr_digest),
        }
    }

    #[must_use]
    pub const fn is_present(&self) -> bool {
        matches!(self, Self::Present { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AbsentState {
    Absent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AbsentExpectedState {
    state: AbsentState,
}

impl AbsentExpectedState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: AbsentState::Absent,
        }
    }
}

impl Default for AbsentExpectedState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PresentState {
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentExpectedState {
    state: PresentState,
    artifact_name: ArtifactName,
    content_digest: RawDigest,
    byte_length: ByteLength,
    incarnation: IncarnationId,
}

impl PresentExpectedState {
    #[must_use]
    pub const fn new(
        artifact_name: ArtifactName,
        content_digest: RawDigest,
        byte_length: ByteLength,
        incarnation: IncarnationId,
    ) -> Self {
        Self {
            state: PresentState::Present,
            artifact_name,
            content_digest,
            byte_length,
            incarnation,
        }
    }

    #[must_use]
    pub const fn artifact_name(&self) -> &ArtifactName {
        &self.artifact_name
    }

    #[must_use]
    pub const fn content_digest(&self) -> &RawDigest {
        &self.content_digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> ByteLength {
        self.byte_length
    }

    #[must_use]
    pub const fn incarnation(&self) -> &IncarnationId {
        &self.incarnation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExpectedState {
    Absent(AbsentExpectedState),
    Present(PresentExpectedState),
}

impl ExpectedState {
    #[must_use]
    pub const fn absent() -> Self {
        Self::Absent(AbsentExpectedState::new())
    }

    #[must_use]
    pub const fn present(state: PresentExpectedState) -> Self {
        Self::Present(state)
    }

    #[must_use]
    pub const fn present_value(&self) -> Option<&PresentExpectedState> {
        match self {
            Self::Absent(_) => None,
            Self::Present(value) => Some(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticArtifactPublishInput {
    artifact_name: ArtifactName,
    source_observation_digest: LocalFileObservationRef,
    target_logical_address: LogicalAddress,
}

impl StaticArtifactPublishInput {
    /// Constructs the locally valid portion of a publication input.
    ///
    /// Cross-body source state, name, and address rules are checked during graph validation.
    ///
    /// # Errors
    ///
    /// This constructor is fallible to preserve the closed payload-construction API as schemas
    /// gain local constraints.
    pub const fn new(
        artifact_name: ArtifactName,
        source_observation_digest: LocalFileObservationRef,
        target_logical_address: LogicalAddress,
    ) -> Result<Self, BodyError> {
        Ok(Self {
            artifact_name,
            source_observation_digest,
            target_logical_address,
        })
    }

    #[must_use]
    pub const fn artifact_name(&self) -> &ArtifactName {
        &self.artifact_name
    }

    #[must_use]
    pub const fn source_observation_digest(&self) -> &LocalFileObservationRef {
        &self.source_observation_digest
    }

    #[must_use]
    pub const fn target_logical_address(&self) -> &LogicalAddress {
        &self.target_logical_address
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticArtifactPublishPrecondition {
    target_logical_address: LogicalAddress,
    expected_target: ExpectedState,
    expected_custody_generation: OptionalValue<U64Decimal>,
}

impl StaticArtifactPublishPrecondition {
    #[must_use]
    pub const fn new(
        target_logical_address: LogicalAddress,
        expected_target: ExpectedState,
        expected_custody_generation: OptionalValue<U64Decimal>,
    ) -> Self {
        Self {
            target_logical_address,
            expected_target,
            expected_custody_generation,
        }
    }

    #[must_use]
    pub const fn target_logical_address(&self) -> &LogicalAddress {
        &self.target_logical_address
    }

    #[must_use]
    pub const fn expected_target(&self) -> &ExpectedState {
        &self.expected_target
    }

    #[must_use]
    pub const fn expected_custody_generation(&self) -> &OptionalValue<U64Decimal> {
        &self.expected_custody_generation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaticArtifactSeparationInput {
    deed_digest: ResourceDeedRef,
    quarantine_address: LogicalAddress,
    quarantine_xattr_digest: XattrValueRef,
}

impl StaticArtifactSeparationInput {
    /// Constructs a separation input; deed-derived cross-body rules are checked on resolution.
    ///
    /// # Errors
    ///
    /// This constructor is fallible to retain the closed schema construction boundary.
    pub const fn new(
        deed_digest: ResourceDeedRef,
        quarantine_address: LogicalAddress,
        quarantine_xattr_digest: XattrValueRef,
    ) -> Result<Self, BodyError> {
        Ok(Self {
            deed_digest,
            quarantine_address,
            quarantine_xattr_digest,
        })
    }

    #[must_use]
    pub const fn deed_digest(&self) -> &ResourceDeedRef {
        &self.deed_digest
    }

    #[must_use]
    pub const fn quarantine_address(&self) -> &LogicalAddress {
        &self.quarantine_address
    }

    #[must_use]
    pub const fn quarantine_xattr_digest(&self) -> &XattrValueRef {
        &self.quarantine_xattr_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_field_names)]
pub struct StaticArtifactSeparationPrecondition {
    expected_active: PresentExpectedState,
    expected_quarantine: AbsentExpectedState,
    expected_custody_generation: U64Decimal,
}

impl StaticArtifactSeparationPrecondition {
    #[must_use]
    pub const fn new(
        expected_active: PresentExpectedState,
        expected_quarantine: AbsentExpectedState,
        expected_custody_generation: U64Decimal,
    ) -> Self {
        Self {
            expected_active,
            expected_quarantine,
            expected_custody_generation,
        }
    }

    #[must_use]
    pub const fn expected_active(&self) -> &PresentExpectedState {
        &self.expected_active
    }

    #[must_use]
    pub const fn expected_quarantine(&self) -> &AbsentExpectedState {
        &self.expected_quarantine
    }

    #[must_use]
    pub const fn expected_custody_generation(&self) -> U64Decimal {
        self.expected_custody_generation
    }
}

macro_rules! body_spec {
    ($payload:ty, $tag:ty, $edges:expr) => {
        impl sealed::BodySpec for $payload {}

        impl BodySpec for $payload {
            type Tag = $tag;

            fn edges(&self) -> Vec<TypedEdge> {
                ($edges)(self)
            }

            fn validate_local(&self) -> Result<(), BodyError> {
                Ok(())
            }
        }
    };
}

impl sealed::BodySpec for XattrValue {}

impl BodySpec for XattrValue {
    type Tag = XattrValueTag;

    fn edges(&self) -> Vec<TypedEdge> {
        Vec::new()
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        if self.entries.is_empty() {
            return Err(BodyError::Local(
                "xattr entries must be nonempty".to_owned(),
            ));
        }
        Ok(())
    }
}

impl sealed::BodySpec for LocalFileObservation {}

impl BodySpec for LocalFileObservation {
    type Tag = LocalFileObservationTag;

    fn edges(&self) -> Vec<TypedEdge> {
        match self {
            Self::Absent { .. }
            | Self::Present {
                quarantine_xattr_digest: OptionalValue::Absent,
                ..
            } => Vec::new(),
            Self::Present {
                quarantine_xattr_digest: OptionalValue::Present { value },
                ..
            } => vec![TypedEdge::new(value)],
        }
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Ok(())
    }
}

body_spec!(
    StaticArtifactPublishInput,
    StaticArtifactPublishInputTag,
    |value: &StaticArtifactPublishInput| { vec![TypedEdge::new(&value.source_observation_digest)] }
);
body_spec!(
    StaticArtifactPublishPrecondition,
    StaticArtifactPublishPreconditionTag,
    |_value: &StaticArtifactPublishPrecondition| Vec::new()
);
body_spec!(
    StaticArtifactSeparationInput,
    StaticArtifactSeparationInputTag,
    |value: &StaticArtifactSeparationInput| {
        vec![
            TypedEdge::new(&value.deed_digest),
            TypedEdge::new(&value.quarantine_xattr_digest),
        ]
    }
);
body_spec!(
    StaticArtifactSeparationPrecondition,
    StaticArtifactSeparationPreconditionTag,
    |_value: &StaticArtifactSeparationPrecondition| Vec::new()
);

impl sealed::BodySpec for &'static SchemaDescriptor {}

impl BodySpec for &'static SchemaDescriptor {
    type Tag = SchemaDescriptorTag;

    fn edges(&self) -> Vec<TypedEdge> {
        Vec::new()
    }

    fn validate_local(&self) -> Result<(), BodyError> {
        Ok(())
    }
}

/// A typed payload after local validation and canonical identity computation.
#[derive(Debug)]
pub struct ValidatedBody<P: BodySpec> {
    reference: BodyRef<P::Tag>,
    payload: P,
    stored: StoredBody,
}

impl<P: BodySpec> Clone for ValidatedBody<P> {
    fn clone(&self) -> Self {
        Self {
            reference: self.reference.clone(),
            payload: self.payload.clone(),
            stored: self.stored.clone(),
        }
    }
}

impl<P: BodySpec> ValidatedBody<P> {
    #[must_use]
    pub const fn reference(&self) -> &BodyRef<P::Tag> {
        &self.reference
    }

    #[must_use]
    pub const fn payload(&self) -> &P {
        &self.payload
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.stored.canonical_bytes
    }

    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        P::Tag::KIND
    }

    #[must_use]
    pub fn into_stored(self) -> StoredBody {
        self.stored
    }
}

/// One immutable canonical body and its exact extracted edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBody {
    digest: Digest,
    kind: BodyKind,
    canonical_bytes: Vec<u8>,
    edges: Vec<TypedEdge>,
}

impl StoredBody {
    #[must_use]
    pub const fn digest(&self) -> &Digest {
        &self.digest
    }

    #[must_use]
    pub const fn kind(&self) -> BodyKind {
        self.kind
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    #[must_use]
    pub fn edges(&self) -> &[TypedEdge] {
        &self.edges
    }
}

/// An immutable validated body graph.
#[derive(Debug, Clone, Default)]
pub struct BodyGraph {
    bodies: BTreeMap<Digest, StoredBody>,
}

impl BodyGraph {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bodies: BTreeMap::new(),
        }
    }

    /// Strictly reconstructs and validates canonical stored entries.
    ///
    /// # Errors
    ///
    /// Rejects malformed/corrupt canonical bytes, key mismatches, unavailable staged payloads,
    /// invalid edges, missing references, type confusion, and cycles.
    pub fn from_canonical_entries(entries: BTreeMap<Digest, Vec<u8>>) -> Result<Self, BodyError> {
        let decoded = entries
            .into_iter()
            .map(|(key, bytes)| decode_entry(&key, &bytes))
            .collect::<Result<Vec<_>, _>>()?;
        build_validated_graph(BTreeMap::new(), decoded)
    }

    #[must_use]
    pub fn get(&self, digest: &Digest) -> Option<&StoredBody> {
        self.bodies.get(digest)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    pub(crate) fn require_validated_body<P: BodySpec>(
        &self,
        body: &ValidatedBody<P>,
    ) -> Result<(), BodyError> {
        let Some(stored) = self.bodies.get(body.reference().digest()) else {
            return Err(BodyError::Local(
                "validated body is not a member of the resolved body graph".to_owned(),
            ));
        };
        if stored.kind != body.kind() || stored.canonical_bytes != body.canonical_bytes() {
            return Err(BodyError::Local(
                "validated body does not equal its resolved graph member".to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn require_kind(
        &self,
        digest: &Digest,
        expected: BodyKind,
    ) -> Result<(), BodyError> {
        match self.bodies.get(digest) {
            Some(body) if body.kind == expected => Ok(()),
            _ => Err(BodyError::Local(
                "required authority body is absent from the resolved body graph".to_owned(),
            )),
        }
    }

    pub(crate) fn publication_revoked_at(
        &self,
        warrant_digest: &Digest,
    ) -> Result<Option<UnixNanoseconds>, BodyError> {
        self.revoked_at(warrant_digest, true)
    }

    pub(crate) fn separation_revoked_at(
        &self,
        warrant_digest: &Digest,
    ) -> Result<Option<UnixNanoseconds>, BodyError> {
        self.revoked_at(warrant_digest, false)
    }

    fn revoked_at(
        &self,
        warrant_digest: &Digest,
        publication: bool,
    ) -> Result<Option<UnixNanoseconds>, BodyError> {
        let mut earliest: Option<UnixNanoseconds> = None;
        for body in self.bodies.values() {
            let decoded = decode_entry(&body.digest, &body.canonical_bytes)?;
            let candidate = match decoded.facts {
                BodyFacts::PublicationRevocation(revocation)
                    if publication && revocation.warrant_digest().digest() == warrant_digest =>
                {
                    Some(revocation.revoked_at())
                }
                BodyFacts::SeparationRevocation(revocation)
                    if !publication && revocation.warrant_digest().digest() == warrant_digest =>
                {
                    Some(revocation.revoked_at())
                }
                _ => None,
            };
            if let Some(candidate) = candidate {
                earliest = Some(earliest.map_or(candidate, |current| current.min(candidate)));
            }
        }
        Ok(earliest)
    }
}

/// A proposed set of immutable bodies to insert atomically.
#[derive(Debug)]
pub struct BodyBatch {
    bodies: Vec<StoredBody>,
}

impl BodyBatch {
    /// Creates a batch. Every identity is recomputed by [`validate_batch`].
    ///
    /// # Errors
    ///
    /// Returns a digest collision if the same claimed digest names differing canonical bytes.
    pub fn new(bodies: Vec<StoredBody>) -> Result<Self, BodyError> {
        let mut seen = BTreeMap::<&Digest, &[u8]>::new();
        for body in &bodies {
            if let Some(previous) = seen.insert(&body.digest, &body.canonical_bytes)
                && previous != body.canonical_bytes
            {
                return Err(BodyError::DigestCollision {
                    digest: body.digest.clone(),
                });
            }
        }
        Ok(Self { bodies })
    }
}

/// Closed failures for body construction, replay, and graph resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BodyError {
    Local(String),
    Canonical(CanonicalError),
    KeyMismatch {
        key: Digest,
        computed: Digest,
    },
    DigestCollision {
        digest: Digest,
    },
    UnknownKind {
        kind: String,
    },
    PayloadModuleUnavailable {
        kind: BodyKind,
    },
    MissingReference {
        source: Digest,
        target: Digest,
    },
    WrongTargetKind {
        source: BodyKind,
        expected: BodyKind,
        actual: BodyKind,
    },
    Cycle {
        digest: Digest,
    },
    NonCanonicalSet,
}

impl fmt::Display for BodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local(message) => write!(formatter, "body-local validation failed: {message}"),
            Self::Canonical(error) => write!(formatter, "canonical body encoding failed: {error}"),
            Self::KeyMismatch { .. } => {
                formatter.write_str("body key does not equal its computed digest")
            }
            Self::DigestCollision { .. } => {
                formatter.write_str("one digest names different canonical bytes")
            }
            Self::UnknownKind { kind } => write!(formatter, "unknown body kind `{kind}`"),
            Self::PayloadModuleUnavailable { kind } => write!(
                formatter,
                "the frozen body kind `{kind}` has no payload decoder in this reviewed increment"
            ),
            Self::MissingReference { .. } => formatter.write_str("referenced body is missing"),
            Self::WrongTargetKind { .. } => {
                formatter.write_str("typed edge resolves to the wrong body kind")
            }
            Self::Cycle { .. } => formatter.write_str("body graph contains a cycle"),
            Self::NonCanonicalSet => {
                formatter.write_str("set-like body field is not strictly sorted and unique")
            }
        }
    }
}

impl std::error::Error for BodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Canonical(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CanonicalError> for BodyError {
    fn from(error: CanonicalError) -> Self {
        Self::Canonical(error)
    }
}

#[derive(Serialize)]
struct BodyEnvelope<'a, T: ?Sized> {
    body: &'a T,
    kind: BodyKind,
}

/// Validates a typed payload and computes its sole canonical graph identity.
///
/// # Errors
///
/// Returns the payload's local validation or canonical encoding failure.
pub fn validated_body<P: BodySpec>(payload: P) -> Result<ValidatedBody<P>, BodyError> {
    payload.validate_local()?;
    let kind = P::Tag::KIND;
    let canonical_bytes = canonical_bytes(&BodyEnvelope {
        body: &payload,
        kind,
    })?;
    let digest = digest_bytes(&canonical_bytes)?;
    let edges = payload.edges();
    let stored = StoredBody {
        digest: digest.clone(),
        kind,
        canonical_bytes,
        edges,
    };
    Ok(ValidatedBody {
        reference: BodyRef::from_digest(digest),
        payload,
        stored,
    })
}

/// Recomputes all identities, inserts idempotently, and validates the combined graph.
///
/// # Errors
///
/// Rejects corruption, collision, missing/type-confused/forbidden edges, cross-body violations,
/// and cycles.
pub fn validate_batch(base: &BodyGraph, batch: BodyBatch) -> Result<BodyGraph, BodyError> {
    let mut base_bodies = BTreeMap::new();
    for body in base.bodies.values() {
        let decoded = decode_entry(&body.digest, &body.canonical_bytes)?;
        base_bodies.insert(decoded.stored.digest.clone(), decoded.stored);
    }

    let decoded = batch
        .bodies
        .into_iter()
        .map(|body| decode_entry(&body.digest, &body.canonical_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    build_validated_graph(base_bodies, decoded)
}

#[derive(Debug, Clone)]
enum BodyFacts {
    None,
    Observation(LocalFileObservation),
    PublishInput(StaticArtifactPublishInput),
    PublishPrecondition(StaticArtifactPublishPrecondition),
    Policy(crate::authority::AuthorityPolicy),
    Enrollment(crate::authority::InstallationEnrollment),
    PublicationWarrant(crate::authority::PublicationWarrant),
    PublicationApproval(crate::authority::PublicationApproval),
    PublicationRevocation(crate::authority::PublicationRevocation),
    IdempotencyBinding(crate::lease::IdempotencyBinding),
    EffectLease(crate::lease::EffectLease),
    SeparationWarrant(crate::authority::SeparationWarrant),
    SeparationApproval(crate::authority::SeparationApproval),
    SeparationRevocation(crate::authority::SeparationRevocation),
    SeparationBinding(crate::lease::SeparationBinding),
    SeparationLease(crate::lease::SeparationLease),
}

struct DecodedBody {
    stored: StoredBody,
    facts: BodyFacts,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope {
    body: Value,
    kind: String,
}

fn decode_entry(key: &Digest, bytes: &[u8]) -> Result<DecodedBody, BodyError> {
    let raw: RawEnvelope = strict_from_slice(bytes)?;
    let kind = BodyKind::parse(&raw.kind)?;
    let (body, edges, facts) = decode_payload(kind, raw.body)?;
    let canonical = canonical_bytes(&BodyEnvelope { body: &body, kind })?;
    let computed = digest_bytes(&canonical)?;
    if key != &computed {
        return Err(BodyError::KeyMismatch {
            key: key.clone(),
            computed,
        });
    }
    if bytes != canonical {
        return Err(BodyError::Local(
            "stored body bytes are not the exact canonical body encoding".to_owned(),
        ));
    }
    Ok(DecodedBody {
        stored: StoredBody {
            digest: key.clone(),
            kind,
            canonical_bytes: canonical,
            edges,
        },
        facts,
    })
}

fn decode_payload(
    kind: BodyKind,
    body: Value,
) -> Result<(Value, Vec<TypedEdge>, BodyFacts), BodyError> {
    match kind {
        BodyKind::InstallationEnrollment => {
            decode_typed::<crate::authority::InstallationEnrollment>(body, BodyFacts::Enrollment)
        }
        BodyKind::AuthorityPolicy => {
            decode_typed::<crate::authority::AuthorityPolicy>(body, BodyFacts::Policy)
        }
        BodyKind::SchemaDescriptor => decode_schema_descriptor(body),
        BodyKind::LocalFileObservation => {
            decode_typed::<LocalFileObservation>(body, BodyFacts::Observation)
        }
        BodyKind::XattrValue => decode_typed::<XattrValue>(body, |_| BodyFacts::None),
        BodyKind::StaticArtifactPublishInput => {
            decode_typed::<StaticArtifactPublishInput>(body, BodyFacts::PublishInput)
        }
        BodyKind::StaticArtifactPublishPrecondition => {
            decode_typed::<StaticArtifactPublishPrecondition>(body, BodyFacts::PublishPrecondition)
        }
        BodyKind::StaticArtifactSeparationInput => {
            decode_typed::<StaticArtifactSeparationInput>(body, |_| BodyFacts::None)
        }
        BodyKind::StaticArtifactSeparationPrecondition => {
            decode_typed::<StaticArtifactSeparationPrecondition>(body, |_| BodyFacts::None)
        }
        BodyKind::PublicationWarrant => decode_typed::<crate::authority::PublicationWarrant>(
            body,
            BodyFacts::PublicationWarrant,
        ),
        BodyKind::PublicationApproval => decode_typed::<crate::authority::PublicationApproval>(
            body,
            BodyFacts::PublicationApproval,
        ),
        BodyKind::PublicationRevocation => decode_typed::<crate::authority::PublicationRevocation>(
            body,
            BodyFacts::PublicationRevocation,
        ),
        BodyKind::EffectLease => decode_private_typed(
            crate::lease::decode_effect_lease(body)?,
            BodyFacts::EffectLease,
        ),
        BodyKind::IdempotencyBinding => decode_private_typed(
            crate::lease::decode_idempotency_binding(body)?,
            BodyFacts::IdempotencyBinding,
        ),
        BodyKind::SeparationWarrant => {
            decode_typed::<crate::authority::SeparationWarrant>(body, BodyFacts::SeparationWarrant)
        }
        BodyKind::SeparationApproval => decode_typed::<crate::authority::SeparationApproval>(
            body,
            BodyFacts::SeparationApproval,
        ),
        BodyKind::SeparationRevocation => decode_typed::<crate::authority::SeparationRevocation>(
            body,
            BodyFacts::SeparationRevocation,
        ),
        BodyKind::SeparationLease => decode_private_typed(
            crate::lease::decode_separation_lease(body)?,
            BodyFacts::SeparationLease,
        ),
        BodyKind::SeparationBinding => decode_private_typed(
            crate::lease::decode_separation_binding(body)?,
            BodyFacts::SeparationBinding,
        ),
        BodyKind::PreparedArtifact
        | BodyKind::PublicationEvidence
        | BodyKind::CausalityAssessment
        | BodyKind::EffectReceipt
        | BodyKind::ResourceDeed
        | BodyKind::SeparationEvidence
        | BodyKind::SeparationReceipt
        | BodyKind::CustodyRecord
        | BodyKind::RecoveryAssessment
        | BodyKind::DossierSummary => Err(BodyError::PayloadModuleUnavailable { kind }),
    }
}

fn decode_private_typed<T: BodySpec>(
    payload: T,
    facts: impl FnOnce(T) -> BodyFacts,
) -> Result<(Value, Vec<TypedEdge>, BodyFacts), BodyError> {
    payload.validate_local()?;
    let edges = payload.edges();
    let encoded = serde_json::to_value(&payload).map_err(CanonicalError::from)?;
    Ok((encoded, edges, facts(payload)))
}

fn decode_typed<T>(
    body: Value,
    facts: impl FnOnce(T) -> BodyFacts,
) -> Result<(Value, Vec<TypedEdge>, BodyFacts), BodyError>
where
    T: BodySpec + DeserializeOwned,
{
    let payload: T = serde_json::from_value(body).map_err(CanonicalError::from)?;
    payload.validate_local()?;
    let edges = payload.edges();
    let encoded = serde_json::to_value(&payload).map_err(CanonicalError::from)?;
    Ok((encoded, edges, facts(payload)))
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SchemaDescriptorWire {
    schema_id: SchemaId,
    fields: Vec<FieldDescriptorWire>,
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FieldDescriptorWire {
    name: String,
    field_type: FieldType,
    required: bool,
}

fn decode_schema_descriptor(body: Value) -> Result<(Value, Vec<TypedEdge>, BodyFacts), BodyError> {
    let wire: SchemaDescriptorWire = serde_json::from_value(body).map_err(CanonicalError::from)?;
    let compiled = descriptor(wire.schema_id);
    let expected: Vec<_> = compiled
        .fields()
        .iter()
        .map(|field| FieldDescriptorWire {
            name: field.name().to_owned(),
            field_type: field.field_type(),
            required: field.required(),
        })
        .collect();
    if wire.fields != expected {
        return Err(BodyError::Local(
            "schema descriptor does not equal the compiled descriptor".to_owned(),
        ));
    }
    let encoded = serde_json::to_value(&wire).map_err(CanonicalError::from)?;
    Ok((encoded, Vec::new(), BodyFacts::None))
}

fn digest_bytes(bytes: &[u8]) -> Result<Digest, BodyError> {
    let hash = Sha256::digest(bytes);
    Digest::parse(&format!("sha256:{}", hex::encode(hash)))
        .map_err(CanonicalError::Digest)
        .map_err(BodyError::Canonical)
}

fn build_validated_graph(
    mut bodies: BTreeMap<Digest, StoredBody>,
    decoded: Vec<DecodedBody>,
) -> Result<BodyGraph, BodyError> {
    for decoded_body in &decoded {
        let body = &decoded_body.stored;
        if let Some(existing) = bodies.get(&body.digest)
            && existing.canonical_bytes != body.canonical_bytes
        {
            return Err(BodyError::DigestCollision {
                digest: body.digest.clone(),
            });
        }
        bodies
            .entry(body.digest.clone())
            .or_insert_with(|| body.clone());
    }

    let mut facts = BTreeMap::new();
    for body in bodies.values() {
        let decoded_body = decode_entry(&body.digest, &body.canonical_bytes)?;
        facts.insert(body.digest.clone(), decoded_body.facts);
    }
    for decoded_body in decoded {
        facts.insert(decoded_body.stored.digest, decoded_body.facts);
    }

    validate_edges(&bodies)?;
    validate_cross_body(&bodies, &facts)?;
    validate_cycles(&bodies)?;
    Ok(BodyGraph { bodies })
}

fn validate_edges(bodies: &BTreeMap<Digest, StoredBody>) -> Result<(), BodyError> {
    for (source_digest, source) in bodies {
        let allowed = permitted_target_kinds(source.kind);
        for edge in &source.edges {
            if !allowed.contains(&edge.expected) {
                return Err(BodyError::Local(format!(
                    "{} contains a reference to forbidden kind {}",
                    source.kind, edge.expected
                )));
            }
            let target = bodies
                .get(&edge.target)
                .ok_or_else(|| BodyError::MissingReference {
                    source: source_digest.clone(),
                    target: edge.target.clone(),
                })?;
            if target.kind != edge.expected {
                return Err(BodyError::WrongTargetKind {
                    source: source.kind,
                    expected: edge.expected,
                    actual: target.kind,
                });
            }
        }
    }
    Ok(())
}

fn validate_cross_body(
    bodies: &BTreeMap<Digest, StoredBody>,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    for (digest, fact) in facts {
        match fact {
            BodyFacts::PublishInput(input) => {
                validate_publish_input(digest, input, bodies, facts)?;
            }
            BodyFacts::PublicationWarrant(warrant) => {
                validate_publication_warrant(warrant, facts)?;
            }
            BodyFacts::PublicationApproval(approval) => {
                validate_publication_approval(approval, facts)?;
            }
            BodyFacts::PublicationRevocation(revocation) => {
                validate_publication_revocation(revocation, facts)?;
            }
            BodyFacts::IdempotencyBinding(binding) => {
                validate_publication_binding(binding, facts)?;
            }
            BodyFacts::EffectLease(lease) => validate_publication_lease(lease, facts)?,
            BodyFacts::SeparationWarrant(warrant) => {
                validate_separation_warrant(warrant, facts)?;
            }
            BodyFacts::SeparationApproval(approval) => {
                validate_separation_approval(approval, facts)?;
            }
            BodyFacts::SeparationRevocation(revocation) => {
                validate_separation_revocation(revocation, facts)?;
            }
            BodyFacts::SeparationBinding(binding) => {
                validate_separation_binding(binding, facts)?;
            }
            BodyFacts::SeparationLease(lease) => validate_separation_lease(lease, facts)?,
            BodyFacts::None
            | BodyFacts::Observation(_)
            | BodyFacts::PublishPrecondition(_)
            | BodyFacts::Policy(_)
            | BodyFacts::Enrollment(_) => {}
        }
    }
    Ok(())
}

fn validate_publish_input(
    digest: &Digest,
    input: &StaticArtifactPublishInput,
    bodies: &BTreeMap<Digest, StoredBody>,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let target_digest = input.source_observation_digest.digest();
    let Some(BodyFacts::Observation(observation)) = facts.get(target_digest) else {
        return Err(BodyError::WrongTargetKind {
            source: BodyKind::StaticArtifactPublishInput,
            expected: BodyKind::LocalFileObservation,
            actual: bodies
                .get(target_digest)
                .map_or(BodyKind::StaticArtifactPublishInput, StoredBody::kind),
        });
    };
    let Some(source_artifact) = observation.artifact_name() else {
        return Err(BodyError::Local(format!(
            "publish input {} source observation must be present",
            digest.as_str()
        )));
    };
    if source_artifact != &input.artifact_name {
        return Err(BodyError::Local(format!(
            "publish input {} artifact name differs from source observation",
            digest.as_str()
        )));
    }
    if observation.logical_address() == &input.target_logical_address {
        return Err(BodyError::Local(format!(
            "publish input {} source and target addresses must differ",
            digest.as_str()
        )));
    }
    Ok(())
}

fn fact_policy<'a>(
    facts: &'a BTreeMap<Digest, BodyFacts>,
    digest: &Digest,
) -> Result<&'a crate::authority::AuthorityPolicy, BodyError> {
    match facts.get(digest) {
        Some(BodyFacts::Policy(policy)) => Ok(policy),
        _ => Err(BodyError::Local(
            "authority reference did not resolve to decoded policy facts".to_owned(),
        )),
    }
}

fn validate_claim(
    policy: &crate::authority::AuthorityPolicy,
    class: crate::lease::BudgetClass,
    claim: &crate::authority::BudgetClaim,
) -> Result<(), BodyError> {
    if policy
        .budget_capacity(class, claim.key())
        .is_none_or(|capacity| capacity.get() < claim.amount().get())
    {
        return Err(BodyError::Local(
            "warrant budget claim is not admitted by its immutable policy".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publication_warrant(
    warrant: &crate::authority::PublicationWarrant,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::Enrollment(enrollment)) = facts.get(warrant.installation_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication warrant enrollment facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    if enrollment.policy_digest() != warrant.policy_digest()
        || warrant.policy_generation() != policy.generation()
        || !policy.contains_proposer(warrant.proposer_id())
    {
        return Err(BodyError::Local(
            "publication warrant does not match enrollment and immutable policy".to_owned(),
        ));
    }
    validate_claim(
        policy,
        crate::lease::BudgetClass::Reservation,
        warrant.reservation_budget(),
    )?;
    validate_claim(
        policy,
        crate::lease::BudgetClass::Start,
        warrant.start_budget(),
    )?;
    let Some(BodyFacts::PublishInput(input)) = facts.get(warrant.input_digest().digest()) else {
        return Err(BodyError::Local(
            "publication warrant input facts are unavailable".to_owned(),
        ));
    };
    let Some(BodyFacts::PublishPrecondition(precondition)) =
        facts.get(warrant.precondition_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication warrant precondition facts are unavailable".to_owned(),
        ));
    };
    if input.target_logical_address() != precondition.target_logical_address() {
        return Err(BodyError::Local(
            "publication input and precondition target addresses differ".to_owned(),
        ));
    }
    let Some(BodyFacts::Observation(source)) =
        facts.get(input.source_observation_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication source observation facts are unavailable".to_owned(),
        ));
    };
    let mut exact_keys = [
        crate::lease::derive_resource_key(source.logical_address())?,
        crate::lease::derive_resource_key(input.target_logical_address())?,
    ];
    exact_keys.sort();
    if warrant.resource_keys() != &exact_keys {
        return Err(BodyError::Local(
            "publication warrant resource keys are not its exact source and target keys".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publication_approval(
    approval: &crate::authority::PublicationApproval,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::PublicationWarrant(warrant)) =
        facts.get(approval.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication approval warrant facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    if !policy.contains_approver(approval.approver_id())
        || policy.require_distinct_approval_principal()
            && approval.approver_id() == warrant.proposer_id()
        || approval.approved_at() < warrant.issued_at()
        || approval.approved_at() >= warrant.expires_at()
    {
        return Err(BodyError::Local(
            "publication approval is not admitted by warrant and policy".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publication_revocation(
    revocation: &crate::authority::PublicationRevocation,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::PublicationWarrant(warrant)) =
        facts.get(revocation.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication revocation warrant facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    let approved_before_revocation = facts.values().any(|fact| {
        matches!(
            fact,
            BodyFacts::PublicationApproval(approval)
                if approval.warrant_digest() == revocation.warrant_digest()
                    && approval.approved_at() <= revocation.revoked_at()
        )
    });
    if !policy.contains_revoker(revocation.revoker_id()) || !approved_before_revocation {
        return Err(BodyError::Local(
            "publication revocation lacks enrolled authority or prior approval".to_owned(),
        ));
    }
    Ok(())
}

fn publication_effect_id(
    warrant_digest: &Digest,
    warrant: &crate::authority::PublicationWarrant,
) -> Result<crate::scalar::EffectId, BodyError> {
    let resources = SortedUnique::new(warrant.resource_keys().to_vec())?;
    crate::lease::derive_effect_id(
        warrant.installation_digest().digest(),
        warrant_digest,
        crate::authority::EffectKind::StaticArtifactPublish,
        &resources,
        warrant.input_digest().digest(),
        warrant.precondition_digest().digest(),
    )
    .map_err(BodyError::from)
}

fn validate_publication_binding(
    binding: &crate::lease::IdempotencyBinding,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::PublicationWarrant(warrant)) = facts.get(binding.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication binding warrant facts are unavailable".to_owned(),
        ));
    };
    if binding.idempotency_key() != warrant.idempotency_key()
        || binding.effect_id()
            != &publication_effect_id(binding.warrant_digest().digest(), warrant)?
    {
        return Err(BodyError::Local(
            "publication binding does not equal its warrant-derived identity".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publication_lease(
    lease: &crate::lease::EffectLease,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::IdempotencyBinding(binding)) = facts.get(lease.binding_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication lease binding facts are unavailable".to_owned(),
        ));
    };
    let Some(BodyFacts::PublicationWarrant(warrant)) = facts.get(binding.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "publication lease warrant facts are unavailable".to_owned(),
        ));
    };
    validate_lease_relationships(
        lease.effect_id(),
        binding.effect_id(),
        lease.resource_fences(),
        warrant.resource_keys(),
        lease.reservation_budget_hold(),
        warrant.reservation_budget(),
        lease.start_budget_hold(),
        warrant.start_budget(),
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_lease_relationships(
    lease_effect: &crate::scalar::EffectId,
    binding_effect: &crate::scalar::EffectId,
    resource_fences: &[crate::lease::ResourceFence; 2],
    resource_keys: &[crate::scalar::ResourceKey; 2],
    reservation_hold: &crate::lease::BudgetHold,
    reservation_claim: &crate::authority::BudgetClaim,
    start_hold: &crate::lease::BudgetHold,
    start_claim: &crate::authority::BudgetClaim,
) -> Result<(), BodyError> {
    let fence_keys = [
        resource_fences[0].resource_key().clone(),
        resource_fences[1].resource_key().clone(),
    ];
    if lease_effect != binding_effect
        || &fence_keys != resource_keys
        || reservation_hold.key() != reservation_claim.key()
        || reservation_hold.amount() != reservation_claim.amount()
        || start_hold.key() != start_claim.key()
        || start_hold.amount() != start_claim.amount()
    {
        return Err(BodyError::Local(
            "lease does not equal its binding, resource, and budget facts".to_owned(),
        ));
    }
    Ok(())
}

fn validate_separation_warrant(
    warrant: &crate::authority::SeparationWarrant,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::Enrollment(enrollment)) = facts.get(warrant.installation_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation warrant enrollment facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    if enrollment.policy_digest() != warrant.policy_digest()
        || warrant.policy_generation() != policy.generation()
        || !policy.contains_proposer(warrant.proposer_id())
    {
        return Err(BodyError::Local(
            "separation warrant does not match enrollment and immutable policy".to_owned(),
        ));
    }
    validate_claim(
        policy,
        crate::lease::BudgetClass::Reservation,
        warrant.reservation_budget(),
    )?;
    validate_claim(
        policy,
        crate::lease::BudgetClass::Start,
        warrant.start_budget(),
    )
}

fn validate_separation_approval(
    approval: &crate::authority::SeparationApproval,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::SeparationWarrant(warrant)) = facts.get(approval.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation approval warrant facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    if !policy.contains_approver(approval.approver_id())
        || approval.approver_id() == warrant.proposer_id()
        || approval.approved_at() < warrant.issued_at()
        || approval.approved_at() >= warrant.expires_at()
    {
        return Err(BodyError::Local(
            "separation approval is not admitted by warrant and policy".to_owned(),
        ));
    }
    Ok(())
}

fn validate_separation_revocation(
    revocation: &crate::authority::SeparationRevocation,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::SeparationWarrant(warrant)) =
        facts.get(revocation.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation revocation warrant facts are unavailable".to_owned(),
        ));
    };
    let policy = fact_policy(facts, warrant.policy_digest().digest())?;
    let approved_before_revocation = facts.values().any(|fact| {
        matches!(
            fact,
            BodyFacts::SeparationApproval(approval)
                if approval.warrant_digest() == revocation.warrant_digest()
                    && approval.approved_at() <= revocation.revoked_at()
        )
    });
    if !policy.contains_revoker(revocation.revoker_id()) || !approved_before_revocation {
        return Err(BodyError::Local(
            "separation revocation lacks enrolled authority or prior approval".to_owned(),
        ));
    }
    Ok(())
}

fn separation_effect_id(
    warrant_digest: &Digest,
    warrant: &crate::authority::SeparationWarrant,
) -> Result<crate::scalar::EffectId, BodyError> {
    let resources = SortedUnique::new(warrant.resource_keys().to_vec())?;
    crate::lease::derive_effect_id(
        warrant.installation_digest().digest(),
        warrant_digest,
        crate::authority::EffectKind::StaticArtifactSeparation,
        &resources,
        warrant.input_digest().digest(),
        warrant.precondition_digest().digest(),
    )
    .map_err(BodyError::from)
}

fn validate_separation_binding(
    binding: &crate::lease::SeparationBinding,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::SeparationWarrant(warrant)) = facts.get(binding.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation binding warrant facts are unavailable".to_owned(),
        ));
    };
    if binding.idempotency_key() != warrant.idempotency_key()
        || binding.effect_id() != &separation_effect_id(binding.warrant_digest().digest(), warrant)?
    {
        return Err(BodyError::Local(
            "separation binding does not equal its warrant-derived identity".to_owned(),
        ));
    }
    Ok(())
}

fn validate_separation_lease(
    lease: &crate::lease::SeparationLease,
    facts: &BTreeMap<Digest, BodyFacts>,
) -> Result<(), BodyError> {
    let Some(BodyFacts::SeparationBinding(binding)) = facts.get(lease.binding_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation lease binding facts are unavailable".to_owned(),
        ));
    };
    let Some(BodyFacts::SeparationWarrant(warrant)) = facts.get(binding.warrant_digest().digest())
    else {
        return Err(BodyError::Local(
            "separation lease warrant facts are unavailable".to_owned(),
        ));
    };
    validate_lease_relationships(
        lease.effect_id(),
        binding.effect_id(),
        lease.resource_fences(),
        warrant.resource_keys(),
        lease.reservation_budget_hold(),
        warrant.reservation_budget(),
        lease.start_budget_hold(),
        warrant.start_budget(),
    )
}

fn validate_cycles(bodies: &BTreeMap<Digest, StoredBody>) -> Result<(), BodyError> {
    fn visit(
        digest: &Digest,
        bodies: &BTreeMap<Digest, StoredBody>,
        active: &mut BTreeSet<Digest>,
        complete: &mut BTreeSet<Digest>,
    ) -> Result<(), BodyError> {
        if complete.contains(digest) {
            return Ok(());
        }
        if !active.insert(digest.clone()) {
            return Err(BodyError::Cycle {
                digest: digest.clone(),
            });
        }
        let body = bodies
            .get(digest)
            .ok_or_else(|| BodyError::MissingReference {
                source: digest.clone(),
                target: digest.clone(),
            })?;
        for edge in &body.edges {
            visit(&edge.target, bodies, active, complete)?;
        }
        active.remove(digest);
        complete.insert(digest.clone());
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for digest in bodies.keys() {
        visit(digest, bodies, &mut active, &mut complete)?;
    }
    Ok(())
}

/// Proves that the complete frozen kind-level edge relation is acyclic.
///
/// # Errors
///
/// Returns a cycle error if the static protocol manifest contains a cycle.
pub fn validate_kind_edge_manifest() -> Result<(), BodyError> {
    fn visit(
        kind: BodyKind,
        active: &mut BTreeSet<BodyKind>,
        complete: &mut BTreeSet<BodyKind>,
    ) -> Result<(), BodyError> {
        if complete.contains(&kind) {
            return Ok(());
        }
        if !active.insert(kind) {
            let digest = Digest::parse(
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            )
            .expect("static nonzero digest is valid");
            return Err(BodyError::Cycle { digest });
        }
        for target in permitted_target_kinds(kind) {
            visit(*target, active, complete)?;
        }
        active.remove(&kind);
        complete.insert(kind);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for kind in BodyKind::ALL {
        visit(kind, &mut active, &mut complete)?;
    }
    Ok(())
}

const fn permitted_target_kinds(kind: BodyKind) -> &'static [BodyKind] {
    use BodyKind as K;
    match kind {
        K::InstallationEnrollment => &[K::AuthorityPolicy],
        K::AuthorityPolicy
        | K::SchemaDescriptor
        | K::XattrValue
        | K::StaticArtifactPublishPrecondition
        | K::StaticArtifactSeparationPrecondition => &[],
        K::LocalFileObservation => &[K::XattrValue],
        K::StaticArtifactPublishInput => &[K::LocalFileObservation],
        K::StaticArtifactSeparationInput => &[K::ResourceDeed, K::XattrValue],
        K::PublicationWarrant => &[
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::StaticArtifactPublishInput,
            K::StaticArtifactPublishPrecondition,
        ],
        K::PublicationApproval | K::PublicationRevocation | K::IdempotencyBinding => {
            &[K::PublicationWarrant]
        }
        K::EffectLease => &[K::IdempotencyBinding],
        K::PreparedArtifact => &[
            K::IdempotencyBinding,
            K::StaticArtifactPublishInput,
            K::LocalFileObservation,
        ],
        K::PublicationEvidence => &[
            K::IdempotencyBinding,
            K::PreparedArtifact,
            K::LocalFileObservation,
        ],
        K::CausalityAssessment => &[K::PublicationEvidence],
        K::EffectReceipt => &[
            K::IdempotencyBinding,
            K::PublicationEvidence,
            K::CausalityAssessment,
        ],
        K::ResourceDeed => &[K::EffectReceipt],
        K::SeparationWarrant => &[
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::StaticArtifactSeparationInput,
            K::StaticArtifactSeparationPrecondition,
        ],
        K::SeparationApproval | K::SeparationRevocation | K::SeparationBinding => {
            &[K::SeparationWarrant]
        }
        K::SeparationLease => &[K::SeparationBinding],
        K::SeparationEvidence => &[
            K::SeparationBinding,
            K::ResourceDeed,
            K::LocalFileObservation,
        ],
        K::SeparationReceipt => &[K::SeparationBinding, K::SeparationEvidence, K::ResourceDeed],
        K::CustodyRecord => &[K::ResourceDeed, K::EffectReceipt, K::SeparationReceipt],
        K::RecoveryAssessment => &[
            K::IdempotencyBinding,
            K::SeparationBinding,
            K::PublicationEvidence,
            K::SeparationEvidence,
            K::EffectReceipt,
            K::SeparationReceipt,
        ],
        K::DossierSummary => &[
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::CustodyRecord,
            K::EffectReceipt,
            K::SeparationReceipt,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_digest(byte: char) -> Digest {
        Digest::parse(&format!("sha256:{}", byte.to_string().repeat(64))).unwrap()
    }

    #[test]
    fn cycle_walk_rejects_a_fabricated_back_edge() {
        let left = test_digest('1');
        let right = test_digest('2');
        let bodies = BTreeMap::from([
            (
                left.clone(),
                StoredBody {
                    digest: left.clone(),
                    kind: BodyKind::LocalFileObservation,
                    canonical_bytes: Vec::new(),
                    edges: vec![TypedEdge {
                        target: right.clone(),
                        expected: BodyKind::XattrValue,
                    }],
                },
            ),
            (
                right.clone(),
                StoredBody {
                    digest: right,
                    kind: BodyKind::XattrValue,
                    canonical_bytes: Vec::new(),
                    edges: vec![TypedEdge {
                        target: left,
                        expected: BodyKind::LocalFileObservation,
                    }],
                },
            ),
        ]);
        assert!(matches!(
            validate_cycles(&bodies),
            Err(BodyError::Cycle { .. })
        ));
    }

    #[test]
    fn batch_constructor_rejects_same_digest_with_different_bytes() {
        let digest = test_digest('3');
        let first = StoredBody {
            digest: digest.clone(),
            kind: BodyKind::XattrValue,
            canonical_bytes: b"first".to_vec(),
            edges: Vec::new(),
        };
        let second = StoredBody {
            digest: digest.clone(),
            kind: BodyKind::XattrValue,
            canonical_bytes: b"second".to_vec(),
            edges: Vec::new(),
        };
        assert!(matches!(
            BodyBatch::new(vec![first, second]),
            Err(BodyError::DigestCollision { digest: actual }) if actual == digest
        ));
    }
}
