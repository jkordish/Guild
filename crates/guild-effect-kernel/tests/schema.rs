use guild_effect_kernel::{
    canonical::strict_from_slice,
    schema::{FieldType, SchemaId, descriptor},
};

type ExpectedField = (&'static str, FieldType, bool);
type SchemaCase = (SchemaId, &'static str, &'static [ExpectedField]);

#[test]
fn schema_and_field_type_wire_values_are_closed() {
    assert!(strict_from_slice::<SchemaId>(br#""unknown-schema/v1""#).is_err());
    assert!(strict_from_slice::<FieldType>(br#""unknown_field_type""#).is_err());

    let schema: SchemaId = strict_from_slice(br#""local-file-observation/v1""#).unwrap();
    assert_eq!(schema, SchemaId::LocalFileObservationV1);
    let field_type: FieldType = strict_from_slice(br#""raw_digest""#).unwrap();
    assert_eq!(field_type, FieldType::RawDigest);
}

#[test]
fn field_type_has_exactly_the_seventeen_protocol_wire_values() {
    let cases = [
        (FieldType::ObservationState, "observation_state"),
        (FieldType::LogicalAddress, "logical_address"),
        (FieldType::WitnessId, "witness_id"),
        (FieldType::UnixNanoseconds, "unix_nanoseconds"),
        (FieldType::ArtifactName, "artifact_name"),
        (FieldType::RawDigest, "raw_digest"),
        (FieldType::ByteLength, "byte_length"),
        (FieldType::IncarnationId, "incarnation_id"),
        (
            FieldType::OptionalBodyRefXattrValue,
            "optional_body_ref_xattr_value",
        ),
        (
            FieldType::BodyRefLocalFileObservation,
            "body_ref_local_file_observation",
        ),
        (FieldType::BodyRefResourceDeed, "body_ref_resource_deed"),
        (FieldType::BodyRefXattrValue, "body_ref_xattr_value"),
        (FieldType::ExpectedState, "expected_state"),
        (FieldType::PresentExpectedState, "present_expected_state"),
        (FieldType::AbsentExpectedState, "absent_expected_state"),
        (
            FieldType::OptionalCustodyGeneration,
            "optional_custody_generation",
        ),
        (FieldType::CustodyGeneration, "custody_generation"),
    ];

    assert_eq!(cases.len(), 17);
    for (field_type, wire) in cases {
        assert_eq!(
            serde_json::to_string(&field_type).unwrap(),
            format!(r#""{wire}""#)
        );
        assert_eq!(
            strict_from_slice::<FieldType>(format!(r#""{wire}""#).as_bytes()).unwrap(),
            field_type
        );
    }
}

#[test]
fn all_five_descriptors_match_the_exact_sorted_protocol_rows() {
    let cases: &[SchemaCase] = &[
        (
            SchemaId::LocalFileObservationV1,
            "local-file-observation/v1",
            &[
                ("artifactName", FieldType::ArtifactName, false),
                ("byteLength", FieldType::ByteLength, false),
                ("contentDigest", FieldType::RawDigest, false),
                ("incarnation", FieldType::IncarnationId, false),
                ("logicalAddress", FieldType::LogicalAddress, true),
                ("observedAt", FieldType::UnixNanoseconds, true),
                (
                    "quarantineXattrDigest",
                    FieldType::OptionalBodyRefXattrValue,
                    false,
                ),
                ("state", FieldType::ObservationState, true),
                ("witnessId", FieldType::WitnessId, true),
            ],
        ),
        (
            SchemaId::StaticArtifactPublishInputV1,
            "static-artifact-publish-input/v1",
            &[
                ("artifactName", FieldType::ArtifactName, true),
                (
                    "sourceObservationDigest",
                    FieldType::BodyRefLocalFileObservation,
                    true,
                ),
                ("targetLogicalAddress", FieldType::LogicalAddress, true),
            ],
        ),
        (
            SchemaId::StaticArtifactPublishPreconditionV1,
            "static-artifact-publish-precondition/v1",
            &[
                (
                    "expectedCustodyGeneration",
                    FieldType::OptionalCustodyGeneration,
                    true,
                ),
                ("expectedTarget", FieldType::ExpectedState, true),
                ("targetLogicalAddress", FieldType::LogicalAddress, true),
            ],
        ),
        (
            SchemaId::StaticArtifactSeparationInputV1,
            "static-artifact-separation-input/v1",
            &[
                ("deedDigest", FieldType::BodyRefResourceDeed, true),
                ("quarantineAddress", FieldType::LogicalAddress, true),
                ("quarantineXattrDigest", FieldType::BodyRefXattrValue, true),
            ],
        ),
        (
            SchemaId::StaticArtifactSeparationPreconditionV1,
            "static-artifact-separation-precondition/v1",
            &[
                ("expectedActive", FieldType::PresentExpectedState, true),
                (
                    "expectedCustodyGeneration",
                    FieldType::CustodyGeneration,
                    true,
                ),
                ("expectedQuarantine", FieldType::AbsentExpectedState, true),
            ],
        ),
    ];

    assert_eq!(cases.len(), 5);
    for (schema_id, wire, expected_fields) in cases {
        assert_eq!(
            serde_json::to_string(schema_id).unwrap(),
            format!(r#""{wire}""#)
        );
        let actual = descriptor(*schema_id);
        assert_eq!(actual.schema_id(), *schema_id);
        assert_eq!(actual.fields().len(), expected_fields.len());
        for (field, (name, field_type, required)) in
            actual.fields().iter().zip(expected_fields.iter())
        {
            assert_eq!(field.name(), *name);
            assert_eq!(field.field_type(), *field_type);
            assert_eq!(field.required(), *required);
        }
        assert!(
            actual
                .fields()
                .windows(2)
                .all(|fields| fields[0].name() < fields[1].name())
        );
    }
}
