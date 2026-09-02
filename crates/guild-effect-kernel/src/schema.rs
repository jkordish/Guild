use serde::{Deserialize, Serialize};

/// The closed effect-input and precondition schema registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SchemaId {
    #[serde(rename = "local-file-observation/v1")]
    LocalFileObservationV1,
    #[serde(rename = "static-artifact-publish-input/v1")]
    StaticArtifactPublishInputV1,
    #[serde(rename = "static-artifact-publish-precondition/v1")]
    StaticArtifactPublishPreconditionV1,
    #[serde(rename = "static-artifact-separation-input/v1")]
    StaticArtifactSeparationInputV1,
    #[serde(rename = "static-artifact-separation-precondition/v1")]
    StaticArtifactSeparationPreconditionV1,
}

/// The closed set of field validation types used by the five schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    ObservationState,
    LogicalAddress,
    WitnessId,
    UnixNanoseconds,
    ArtifactName,
    RawDigest,
    ByteLength,
    IncarnationId,
    OptionalBodyRefXattrValue,
    BodyRefLocalFileObservation,
    BodyRefResourceDeed,
    BodyRefXattrValue,
    ExpectedState,
    PresentExpectedState,
    AbsentExpectedState,
    OptionalCustodyGeneration,
    CustodyGeneration,
}

/// One sealed field row in a compiled schema descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDescriptor {
    name: &'static str,
    field_type: FieldType,
    required: bool,
}

impl FieldDescriptor {
    const fn new(name: &'static str, field_type: FieldType, required: bool) -> Self {
        Self {
            name,
            field_type,
            required,
        }
    }

    /// Returns the statically validated lower-camel-case field name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the closed validation type for this field.
    #[must_use]
    pub const fn field_type(&self) -> FieldType {
        self.field_type
    }

    /// Reports whether the field is unconditionally required.
    #[must_use]
    pub const fn required(&self) -> bool {
        self.required
    }
}

/// One sealed schema identifier and its exact sorted field rows.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDescriptor {
    schema_id: SchemaId,
    fields: &'static [FieldDescriptor],
}

impl SchemaDescriptor {
    const fn new(schema_id: SchemaId, fields: &'static [FieldDescriptor]) -> Self {
        Self { schema_id, fields }
    }

    /// Returns the closed schema identifier.
    #[must_use]
    pub const fn schema_id(&self) -> SchemaId {
        self.schema_id
    }

    /// Returns the exact, statically validated field rows in field-name order.
    #[must_use]
    pub const fn fields(&self) -> &'static [FieldDescriptor] {
        self.fields
    }
}

const LOCAL_FILE_OBSERVATION_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("artifactName", FieldType::ArtifactName, false),
    FieldDescriptor::new("byteLength", FieldType::ByteLength, false),
    FieldDescriptor::new("contentDigest", FieldType::RawDigest, false),
    FieldDescriptor::new("incarnation", FieldType::IncarnationId, false),
    FieldDescriptor::new("logicalAddress", FieldType::LogicalAddress, true),
    FieldDescriptor::new("observedAt", FieldType::UnixNanoseconds, true),
    FieldDescriptor::new(
        "quarantineXattrDigest",
        FieldType::OptionalBodyRefXattrValue,
        false,
    ),
    FieldDescriptor::new("state", FieldType::ObservationState, true),
    FieldDescriptor::new("witnessId", FieldType::WitnessId, true),
];

const STATIC_ARTIFACT_PUBLISH_INPUT_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("artifactName", FieldType::ArtifactName, true),
    FieldDescriptor::new(
        "sourceObservationDigest",
        FieldType::BodyRefLocalFileObservation,
        true,
    ),
    FieldDescriptor::new("targetLogicalAddress", FieldType::LogicalAddress, true),
];

const STATIC_ARTIFACT_PUBLISH_PRECONDITION_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new(
        "expectedCustodyGeneration",
        FieldType::OptionalCustodyGeneration,
        true,
    ),
    FieldDescriptor::new("expectedTarget", FieldType::ExpectedState, true),
    FieldDescriptor::new("targetLogicalAddress", FieldType::LogicalAddress, true),
];

const STATIC_ARTIFACT_SEPARATION_INPUT_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("deedDigest", FieldType::BodyRefResourceDeed, true),
    FieldDescriptor::new("quarantineAddress", FieldType::LogicalAddress, true),
    FieldDescriptor::new("quarantineXattrDigest", FieldType::BodyRefXattrValue, true),
];

const STATIC_ARTIFACT_SEPARATION_PRECONDITION_FIELDS: &[FieldDescriptor] = &[
    FieldDescriptor::new("expectedActive", FieldType::PresentExpectedState, true),
    FieldDescriptor::new(
        "expectedCustodyGeneration",
        FieldType::CustodyGeneration,
        true,
    ),
    FieldDescriptor::new("expectedQuarantine", FieldType::AbsentExpectedState, true),
];

static LOCAL_FILE_OBSERVATION: SchemaDescriptor = SchemaDescriptor::new(
    SchemaId::LocalFileObservationV1,
    LOCAL_FILE_OBSERVATION_FIELDS,
);
static STATIC_ARTIFACT_PUBLISH_INPUT: SchemaDescriptor = SchemaDescriptor::new(
    SchemaId::StaticArtifactPublishInputV1,
    STATIC_ARTIFACT_PUBLISH_INPUT_FIELDS,
);
static STATIC_ARTIFACT_PUBLISH_PRECONDITION: SchemaDescriptor = SchemaDescriptor::new(
    SchemaId::StaticArtifactPublishPreconditionV1,
    STATIC_ARTIFACT_PUBLISH_PRECONDITION_FIELDS,
);
static STATIC_ARTIFACT_SEPARATION_INPUT: SchemaDescriptor = SchemaDescriptor::new(
    SchemaId::StaticArtifactSeparationInputV1,
    STATIC_ARTIFACT_SEPARATION_INPUT_FIELDS,
);
static STATIC_ARTIFACT_SEPARATION_PRECONDITION: SchemaDescriptor = SchemaDescriptor::new(
    SchemaId::StaticArtifactSeparationPreconditionV1,
    STATIC_ARTIFACT_SEPARATION_PRECONDITION_FIELDS,
);

/// Returns the one statically compiled descriptor for `schema_id`.
#[must_use]
pub const fn descriptor(schema_id: SchemaId) -> &'static SchemaDescriptor {
    match schema_id {
        SchemaId::LocalFileObservationV1 => &LOCAL_FILE_OBSERVATION,
        SchemaId::StaticArtifactPublishInputV1 => &STATIC_ARTIFACT_PUBLISH_INPUT,
        SchemaId::StaticArtifactPublishPreconditionV1 => &STATIC_ARTIFACT_PUBLISH_PRECONDITION,
        SchemaId::StaticArtifactSeparationInputV1 => &STATIC_ARTIFACT_SEPARATION_INPUT,
        SchemaId::StaticArtifactSeparationPreconditionV1 => {
            &STATIC_ARTIFACT_SEPARATION_PRECONDITION
        }
    }
}

const fn valid_field_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 63 || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphanumeric() {
            return false;
        }
        index += 1;
    }
    true
}

const fn name_less_than(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let mut index = 0;
    while index < left.len() && index < right.len() {
        if left[index] < right[index] {
            return true;
        }
        if left[index] > right[index] {
            return false;
        }
        index += 1;
    }
    left.len() < right.len()
}

const fn fields_are_valid_and_sorted(fields: &[FieldDescriptor]) -> bool {
    let mut index = 0;
    while index < fields.len() {
        if !valid_field_name(fields[index].name) {
            return false;
        }
        if index > 0 && !name_less_than(fields[index - 1].name, fields[index].name) {
            return false;
        }
        index += 1;
    }
    true
}

const _: () = assert!(fields_are_valid_and_sorted(LOCAL_FILE_OBSERVATION_FIELDS));
const _: () = assert!(fields_are_valid_and_sorted(
    STATIC_ARTIFACT_PUBLISH_INPUT_FIELDS
));
const _: () = assert!(fields_are_valid_and_sorted(
    STATIC_ARTIFACT_PUBLISH_PRECONDITION_FIELDS
));
const _: () = assert!(fields_are_valid_and_sorted(
    STATIC_ARTIFACT_SEPARATION_INPUT_FIELDS
));
const _: () = assert!(fields_are_valid_and_sorted(
    STATIC_ARTIFACT_SEPARATION_PRECONDITION_FIELDS
));
