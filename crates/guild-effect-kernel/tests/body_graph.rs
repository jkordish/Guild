mod support;

use std::collections::BTreeMap;

use guild_effect_kernel::{
    body::{
        AbsentExpectedState, BodyBatch, BodyError, BodyGraph, BodyKind, ExpectedState,
        LocalFileObservation, LocalFileObservationRef, OptionalValue, PresentExpectedState,
        ProtocolRef, ResourceDeedRef, StaticArtifactPublishInput,
        StaticArtifactPublishPrecondition, StaticArtifactSeparationInput,
        StaticArtifactSeparationPrecondition, XattrEntry, XattrValue, XattrValueRef,
        validate_batch, validate_kind_edge_manifest, validated_body,
    },
    canonical::{CanonicalError, canonical_bytes, canonical_digest},
    protocol::BODY_KIND_IDS,
    scalar::{
        ArtifactName, ByteLength, Digest, Identifier, IncarnationId, LogicalAddress, RawDigest,
        U64Decimal, UnixNanoseconds, XattrName,
    },
    schema::{SchemaId, descriptor},
};

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

fn digest(value: &str) -> Digest {
    Digest::parse(value).unwrap()
}

fn raw(value: &str) -> RawDigest {
    RawDigest::parse(value).unwrap()
}

fn incarnation(value: &str) -> IncarnationId {
    IncarnationId::parse(value).unwrap()
}

fn present_observation(
    address: &str,
    artifact: &str,
    xattr: OptionalValue<XattrValueRef>,
) -> guild_effect_kernel::body::ValidatedBody<LocalFileObservation> {
    validated_body(LocalFileObservation::present(
        LogicalAddress::parse(address).unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("1788210000000000000").unwrap(),
        ArtifactName::parse(artifact).unwrap(),
        raw(ONE),
        ByteLength::from_u64(42),
        incarnation(TWO),
        xattr,
    ))
    .unwrap()
}

fn xattrs() -> guild_effect_kernel::body::ValidatedBody<XattrValue> {
    validated_body(
        XattrValue::new(vec![XattrEntry::new(
            XattrName::parse("com.apple.quarantine").unwrap(),
            raw(THREE),
            ByteLength::from_u64(1),
        )])
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn body_map_key_must_equal_canonical_identity() {
    let body = support::absent_observation("local-file:///staging/app");
    let wrong = Digest::parse(&format!("sha256:{}1", "0".repeat(63))).unwrap();
    let entries = BTreeMap::from([(wrong, body.canonical_bytes().to_vec())]);
    assert!(matches!(
        BodyGraph::from_canonical_entries(entries),
        Err(BodyError::KeyMismatch { .. })
    ));
}

#[test]
fn typed_reference_rejects_a_body_of_the_wrong_kind() {
    let xattrs = validated_body(
        XattrValue::new(vec![XattrEntry::new(
            XattrName::parse("com.apple.quarantine").unwrap(),
            RawDigest::parse(&format!("sha256:{}", "1".repeat(64))).unwrap(),
            ByteLength::from_u64(1),
        )])
        .unwrap(),
    )
    .unwrap();
    let lied_about_kind = LocalFileObservationRef::from_digest(xattrs.reference().digest().clone());
    let input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            lied_about_kind,
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let batch = BodyBatch::new(vec![xattrs.into_stored(), input.into_stored()]).unwrap();
    assert!(matches!(
        validate_batch(&BodyGraph::empty(), batch),
        Err(BodyError::WrongTargetKind { .. })
    ));
}

#[test]
fn graph_rejects_missing_edges_and_the_kind_manifest_is_acyclic() {
    let missing = LocalFileObservationRef::from_digest(
        Digest::parse(&format!("sha256:{}", "2".repeat(64))).unwrap(),
    );
    let input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            missing,
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let batch = BodyBatch::new(vec![input.into_stored()]).unwrap();
    assert!(matches!(
        validate_batch(&BodyGraph::empty(), batch),
        Err(BodyError::MissingReference { .. })
    ));
    assert_eq!(validate_kind_edge_manifest(), Ok(()));
}

#[test]
fn every_protocol_kind_has_one_manifest_entry() {
    assert_eq!(BodyKind::ALL.len(), 29);
    assert_eq!(BodyKind::ALL.map(BodyKind::as_str), BODY_KIND_IDS);
}

#[test]
fn absent_observation_preserves_the_protocol_golden_identity() {
    let body = support::absent_observation("local-file:///canonical/path");
    assert_eq!(
        body.canonical_bytes(),
        br#"{"body":{"logicalAddress":"local-file:///canonical/path","observedAt":"1788210000000000000","state":"absent","witnessId":"host-probe"},"kind":"local-file-observation/v1"}"#
    );
    assert_eq!(
        body.reference().digest().as_str(),
        "sha256:37acdc8236b6c57c87a7d68b0ed51cf02d9a97ba78edd6d13a3b3f754000cf81"
    );
}

#[test]
fn body_kind_and_typed_reference_wire_values_are_closed() {
    for (kind, wire) in BodyKind::ALL.into_iter().zip(BODY_KIND_IDS) {
        assert_eq!(kind.as_str(), wire);
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!(r#""{wire}""#)
        );
        assert_eq!(
            serde_json::from_str::<BodyKind>(&format!(r#""{wire}""#)).unwrap(),
            kind
        );
    }
    assert!(serde_json::from_str::<BodyKind>(r#""future-kind/v1""#).is_err());

    let claimed = LocalFileObservationRef::from_digest(digest(ONE));
    assert_eq!(
        serde_json::to_string(&claimed).unwrap(),
        format!(r#""{ONE}""#)
    );
    let decoded: LocalFileObservationRef = serde_json::from_str(&format!(r#""{ONE}""#)).unwrap();
    assert_eq!(decoded.digest().as_str(), ONE);
}

#[test]
fn optional_and_protocol_refs_have_exact_closed_shapes() {
    let absent: OptionalValue<U64Decimal> = OptionalValue::absent();
    let present = OptionalValue::present(U64Decimal::from_u64(7));
    assert_eq!(canonical_bytes(&absent).unwrap(), br#"{"state":"absent"}"#);
    assert_eq!(
        canonical_bytes(&present).unwrap(),
        br#"{"state":"present","value":"7"}"#
    );

    let publication: ProtocolRef<
        guild_effect_kernel::body::EffectReceiptTag,
        guild_effect_kernel::body::SeparationReceiptTag,
    > = ProtocolRef::publication(guild_effect_kernel::body::EffectReceiptRef::from_digest(
        digest(ONE),
    ));
    let separation: ProtocolRef<
        guild_effect_kernel::body::EffectReceiptTag,
        guild_effect_kernel::body::SeparationReceiptTag,
    > = ProtocolRef::separation(
        guild_effect_kernel::body::SeparationReceiptRef::from_digest(digest(ONE)),
    );
    assert_eq!(
        canonical_bytes(&publication).unwrap(),
        format!(r#"{{"digest":"{ONE}","protocol":"publication"}}"#).as_bytes()
    );
    assert_eq!(
        canonical_bytes(&separation).unwrap(),
        format!(r#"{{"digest":"{ONE}","protocol":"separation"}}"#).as_bytes()
    );
    for hostile in [
        r#"{"state":"missing"}"#,
        r#"{"state":"absent","value":"7"}"#,
        r#"{"state":"present"}"#,
        r#"{"protocol":"other","digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111"}"#,
        r#"{"protocol":"publication","digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","extra":true}"#,
    ] {
        assert!(
            serde_json::from_str::<OptionalValue<U64Decimal>>(hostile).is_err()
                || serde_json::from_str::<
                    ProtocolRef<
                        guild_effect_kernel::body::EffectReceiptTag,
                        guild_effect_kernel::body::SeparationReceiptTag,
                    >,
                >(hostile)
                .is_err()
        );
    }
}

#[test]
fn xattr_entries_must_be_nonempty_strictly_sorted_and_unique_by_canonical_bytes() {
    let first = XattrEntry::new(
        XattrName::parse("a").unwrap(),
        raw(ONE),
        ByteLength::from_u64(1),
    );
    let second = XattrEntry::new(
        XattrName::parse("b").unwrap(),
        raw(TWO),
        ByteLength::from_u64(2),
    );
    assert!(XattrValue::new(vec![]).is_err());
    assert!(XattrValue::new(vec![second.clone(), first.clone()]).is_err());
    assert!(XattrValue::new(vec![first.clone(), first]).is_err());
    assert!(XattrValue::new(vec![second]).is_ok());
}

#[test]
fn each_task_five_payload_has_the_exact_wire_shape() {
    let xattrs = xattrs();
    let present = present_observation(
        "local-file:///staging/app",
        "app",
        OptionalValue::present(XattrValueRef::from_digest(
            xattrs.reference().digest().clone(),
        )),
    );
    assert_eq!(present.kind(), BodyKind::LocalFileObservation);
    assert_eq!(xattrs.kind(), BodyKind::XattrValue);

    let publish_input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            LocalFileObservationRef::from_digest(present.reference().digest().clone()),
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let publish_precondition = validated_body(StaticArtifactPublishPrecondition::new(
        LogicalAddress::parse("local-file:///active/app").unwrap(),
        ExpectedState::absent(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let separation_input = validated_body(
        StaticArtifactSeparationInput::new(
            ResourceDeedRef::from_digest(digest(TWO)),
            LogicalAddress::parse("local-file:///quarantine/app").unwrap(),
            XattrValueRef::from_digest(xattrs.reference().digest().clone()),
        )
        .unwrap(),
    )
    .unwrap();
    let separation_precondition = validated_body(StaticArtifactSeparationPrecondition::new(
        PresentExpectedState::new(
            ArtifactName::parse("app").unwrap(),
            raw(ONE),
            ByteLength::from_u64(42),
            incarnation(TWO),
        ),
        AbsentExpectedState::new(),
        U64Decimal::from_u64(3),
    ))
    .unwrap();

    assert_eq!(publish_input.kind(), BodyKind::StaticArtifactPublishInput);
    assert_eq!(
        publish_precondition.kind(),
        BodyKind::StaticArtifactPublishPrecondition
    );
    assert_eq!(
        separation_input.kind(),
        BodyKind::StaticArtifactSeparationInput
    );
    assert_eq!(
        separation_precondition.kind(),
        BodyKind::StaticArtifactSeparationPrecondition
    );

    assert!(
        String::from_utf8(publish_input.canonical_bytes().to_vec())
            .unwrap()
            .contains(r#""sourceObservationDigest":"sha256:"#)
    );
    assert!(
        String::from_utf8(separation_input.canonical_bytes().to_vec())
            .unwrap()
            .contains(r#""deedDigest":"sha256:"#)
    );
}

#[test]
fn graph_extracts_every_task_five_edge_and_validates_cross_body_publish_rules() {
    let xattrs = xattrs();
    let present = present_observation(
        "local-file:///staging/app",
        "app",
        OptionalValue::present(XattrValueRef::from_digest(
            xattrs.reference().digest().clone(),
        )),
    );
    let input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            LocalFileObservationRef::from_digest(present.reference().digest().clone()),
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let graph = validate_batch(
        &BodyGraph::empty(),
        BodyBatch::new(vec![
            xattrs.into_stored(),
            present.into_stored(),
            input.into_stored(),
        ])
        .unwrap(),
    )
    .unwrap();
    assert_eq!(graph.len(), 3);

    let absent = support::absent_observation("local-file:///staging/app");
    let absent_input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            LocalFileObservationRef::from_digest(absent.reference().digest().clone()),
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let batch = BodyBatch::new(vec![absent.into_stored(), absent_input.into_stored()]).unwrap();
    assert!(matches!(
        validate_batch(&BodyGraph::empty(), batch),
        Err(BodyError::Local(_))
    ));

    let present = present_observation(
        "local-file:///staging/app",
        "different",
        OptionalValue::absent(),
    );
    let mismatch = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            LocalFileObservationRef::from_digest(present.reference().digest().clone()),
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![present.into_stored(), mismatch.into_stored()]).unwrap()
        ),
        Err(BodyError::Local(_))
    ));

    let present = present_observation("local-file:///same/app", "app", OptionalValue::absent());
    let same_address = StaticArtifactPublishInput::new(
        ArtifactName::parse("app").unwrap(),
        LocalFileObservationRef::from_digest(present.reference().digest().clone()),
        LogicalAddress::parse("local-file:///same/app").unwrap(),
    )
    .unwrap();
    let same_address = validated_body(same_address).unwrap();
    assert!(matches!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![present.into_stored(), same_address.into_stored()]).unwrap()
        ),
        Err(BodyError::Local(_))
    ));
}

#[test]
fn present_observation_optional_xattr_reference_is_typed_and_required_when_present() {
    let absent = present_observation("local-file:///staging/app", "app", OptionalValue::absent());
    assert!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![absent.into_stored()]).unwrap()
        )
        .is_ok()
    );

    let missing = present_observation(
        "local-file:///staging/app",
        "app",
        OptionalValue::present(XattrValueRef::from_digest(digest(THREE))),
    );
    assert!(matches!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![missing.into_stored()]).unwrap()
        ),
        Err(BodyError::MissingReference { .. })
    ));
}

#[test]
fn replay_strictly_rejects_hostile_envelopes_and_payloads() {
    let hostile = [
        br#"{"kind":"local-file-observation/v1","kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":"1"}}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","logicalAddress":"local-file:///b","witnessId":"host-probe","observedAt":"1"}}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":"1","extra":true}}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":"1"},"extra":true}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":1}}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"absent","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":"1","artifactName":"illegal"}}"#.as_slice(),
        br#"{"kind":"local-file-observation/v1","body":{"state":"present","logicalAddress":"local-file:///a","witnessId":"host-probe","observedAt":"1"}}"#.as_slice(),
    ];
    for bytes in hostile {
        let key = canonical_digest(&serde_json::json!({"fixture": String::from_utf8_lossy(bytes)}))
            .unwrap();
        assert!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes.to_vec())])).is_err()
        );
    }

    let unknown = br#"{"body":{},"kind":"future-body/v1"}"#.to_vec();
    let key = canonical_digest(&serde_json::json!({"body": {}, "kind":"future-body/v1"})).unwrap();
    assert!(matches!(
        BodyGraph::from_canonical_entries(BTreeMap::from([(key, unknown)])),
        Err(BodyError::UnknownKind { .. })
    ));
}

#[test]
fn replay_decodes_owned_base_kinds_and_stages_later_payload_modules() {
    let base = support::absent_observation("local-file:///canonical/path");
    let entries = BTreeMap::from([(
        base.reference().digest().clone(),
        base.canonical_bytes().to_vec(),
    )]);
    let graph = BodyGraph::from_canonical_entries(entries).unwrap();
    assert_eq!(graph.len(), 1);

    let unavailable: Vec<_> = BodyKind::ALL
        .into_iter()
        .filter(|kind| {
            !matches!(
                kind,
                BodyKind::SchemaDescriptor
                    | BodyKind::LocalFileObservation
                    | BodyKind::XattrValue
                    | BodyKind::StaticArtifactPublishInput
                    | BodyKind::StaticArtifactPublishPrecondition
                    | BodyKind::StaticArtifactSeparationInput
                    | BodyKind::StaticArtifactSeparationPrecondition
            )
        })
        .collect();
    assert_eq!(unavailable.len(), 22);
    for kind in unavailable {
        let value = serde_json::json!({"body": {}, "kind": kind});
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(matches!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])),
            Err(BodyError::PayloadModuleUnavailable { kind: actual }) if actual == kind
        ));
    }
}

#[test]
fn kind_edge_manifest_matches_every_protocol_row_exactly() {
    use BodyKind as K;

    let mut expected =
        BTreeMap::<K, Vec<K>>::from_iter(K::ALL.into_iter().map(|kind| (kind, Vec::new())));
    expected.insert(K::InstallationEnrollment, vec![K::AuthorityPolicy]);
    expected.insert(K::LocalFileObservation, vec![K::XattrValue]);
    expected.insert(K::StaticArtifactPublishInput, vec![K::LocalFileObservation]);
    expected.insert(
        K::StaticArtifactSeparationInput,
        vec![K::ResourceDeed, K::XattrValue],
    );
    expected.insert(
        K::PublicationWarrant,
        vec![
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::StaticArtifactPublishInput,
            K::StaticArtifactPublishPrecondition,
        ],
    );
    for kind in [K::PublicationApproval, K::PublicationRevocation] {
        expected.insert(kind, vec![K::PublicationWarrant]);
    }
    expected.insert(K::IdempotencyBinding, vec![K::PublicationWarrant]);
    expected.insert(K::EffectLease, vec![K::IdempotencyBinding]);
    expected.insert(
        K::PreparedArtifact,
        vec![
            K::IdempotencyBinding,
            K::StaticArtifactPublishInput,
            K::LocalFileObservation,
        ],
    );
    expected.insert(
        K::PublicationEvidence,
        vec![
            K::IdempotencyBinding,
            K::PreparedArtifact,
            K::LocalFileObservation,
        ],
    );
    expected.insert(K::CausalityAssessment, vec![K::PublicationEvidence]);
    expected.insert(
        K::EffectReceipt,
        vec![
            K::IdempotencyBinding,
            K::PublicationEvidence,
            K::CausalityAssessment,
        ],
    );
    expected.insert(K::ResourceDeed, vec![K::EffectReceipt]);
    expected.insert(
        K::SeparationWarrant,
        vec![
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::StaticArtifactSeparationInput,
            K::StaticArtifactSeparationPrecondition,
        ],
    );
    for kind in [K::SeparationApproval, K::SeparationRevocation] {
        expected.insert(kind, vec![K::SeparationWarrant]);
    }
    expected.insert(K::SeparationBinding, vec![K::SeparationWarrant]);
    expected.insert(K::SeparationLease, vec![K::SeparationBinding]);
    expected.insert(
        K::SeparationEvidence,
        vec![
            K::SeparationBinding,
            K::ResourceDeed,
            K::LocalFileObservation,
        ],
    );
    expected.insert(
        K::SeparationReceipt,
        vec![K::SeparationBinding, K::SeparationEvidence, K::ResourceDeed],
    );
    expected.insert(
        K::CustodyRecord,
        vec![K::ResourceDeed, K::EffectReceipt, K::SeparationReceipt],
    );
    expected.insert(
        K::RecoveryAssessment,
        vec![
            K::IdempotencyBinding,
            K::SeparationBinding,
            K::PublicationEvidence,
            K::SeparationEvidence,
            K::EffectReceipt,
            K::SeparationReceipt,
        ],
    );
    expected.insert(
        K::DossierSummary,
        vec![
            K::InstallationEnrollment,
            K::AuthorityPolicy,
            K::CustodyRecord,
            K::EffectReceipt,
            K::SeparationReceipt,
        ],
    );

    assert_eq!(expected.len(), 29);
    for kind in K::ALL {
        assert_eq!(kind.permitted_target_kinds(), expected[&kind]);
    }
}

#[test]
fn replay_rejects_noncanonical_bytes_even_when_the_key_names_canonical_content() {
    let body = support::absent_observation("local-file:///canonical/path");
    let padded = [b" \n".as_slice(), body.canonical_bytes(), b"\n".as_slice()].concat();
    assert!(
        BodyGraph::from_canonical_entries(BTreeMap::from([(
            body.reference().digest().clone(),
            padded,
        )]))
        .is_err()
    );
}

#[test]
fn identical_insertions_are_idempotent() {
    let body = support::absent_observation("local-file:///canonical/path");
    let stored = body.clone().into_stored();
    let graph = validate_batch(
        &BodyGraph::empty(),
        BodyBatch::new(vec![stored.clone()]).unwrap(),
    )
    .unwrap();
    let graph = validate_batch(&graph, BodyBatch::new(vec![stored]).unwrap()).unwrap();
    assert_eq!(graph.len(), 1);
}

#[test]
fn replay_covers_schema_and_every_task_five_owned_payload_decoder() {
    let schemas = [
        SchemaId::LocalFileObservationV1,
        SchemaId::StaticArtifactPublishInputV1,
        SchemaId::StaticArtifactPublishPreconditionV1,
        SchemaId::StaticArtifactSeparationInputV1,
        SchemaId::StaticArtifactSeparationPreconditionV1,
    ]
    .map(|schema_id| validated_body(descriptor(schema_id)).unwrap());
    let xattrs = xattrs();
    let present = present_observation(
        "local-file:///staging/app",
        "app",
        OptionalValue::present(XattrValueRef::from_digest(
            xattrs.reference().digest().clone(),
        )),
    );
    let publish_input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            LocalFileObservationRef::from_digest(present.reference().digest().clone()),
            LogicalAddress::parse("local-file:///active/app").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let publish_precondition = validated_body(StaticArtifactPublishPrecondition::new(
        LogicalAddress::parse("local-file:///active/app").unwrap(),
        ExpectedState::present(PresentExpectedState::new(
            ArtifactName::parse("old-app").unwrap(),
            raw(TWO),
            ByteLength::from_u64(9),
            incarnation(THREE),
        )),
        OptionalValue::present(U64Decimal::from_u64(2)),
    ))
    .unwrap();
    let separation_precondition = validated_body(StaticArtifactSeparationPrecondition::new(
        PresentExpectedState::new(
            ArtifactName::parse("app").unwrap(),
            raw(ONE),
            ByteLength::from_u64(42),
            incarnation(TWO),
        ),
        AbsentExpectedState::new(),
        U64Decimal::from_u64(3),
    ))
    .unwrap();

    let mut entries: BTreeMap<_, _> = schemas
        .iter()
        .map(|schema| {
            (
                schema.reference().digest().clone(),
                schema.canonical_bytes().to_vec(),
            )
        })
        .collect();
    entries.extend([
        (
            xattrs.reference().digest().clone(),
            xattrs.canonical_bytes().to_vec(),
        ),
        (
            present.reference().digest().clone(),
            present.canonical_bytes().to_vec(),
        ),
        (
            publish_input.reference().digest().clone(),
            publish_input.canonical_bytes().to_vec(),
        ),
        (
            publish_precondition.reference().digest().clone(),
            publish_precondition.canonical_bytes().to_vec(),
        ),
        (
            separation_precondition.reference().digest().clone(),
            separation_precondition.canonical_bytes().to_vec(),
        ),
    ]);
    let replayed = BodyGraph::from_canonical_entries(entries).unwrap();
    assert_eq!(replayed.len(), 10);
}

#[test]
fn every_owned_payload_strictly_rejects_an_unknown_field() {
    let xattrs = xattrs();
    let present = present_observation("local-file:///staging/app", "app", OptionalValue::absent());
    let bodies = vec![
        support::absent_observation("local-file:///absent"),
        present.clone(),
    ];
    for body in bodies {
        let mut envelope: serde_json::Value =
            serde_json::from_slice(body.canonical_bytes()).unwrap();
        envelope["body"]["unexpected"] = serde_json::Value::Bool(true);
        let bytes = canonical_bytes(&envelope).unwrap();
        let key = canonical_digest(&envelope).unwrap();
        assert!(matches!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])),
            Err(BodyError::Canonical(_))
        ));
    }

    let owned_payloads = vec![
        serde_json::json!({
            "kind": "schema-descriptor/v1",
            "body": {
                "schemaId":"local-file-observation/v1",
                "fields":[],
                "unexpected":true
            }
        }),
        serde_json::json!({
            "kind": "xattr-value/v1",
            "body": {
                "entries": [{"name":"a","valueDigest":ONE,"byteLength":"1"}],
                "unexpected": true
            }
        }),
        serde_json::json!({
            "kind": "static-artifact-publish-input/v1",
            "body": {
                "artifactName":"app",
                "sourceObservationDigest": present.reference().digest(),
                "targetLogicalAddress":"local-file:///active/app",
                "unexpected":true
            }
        }),
        serde_json::json!({
            "kind": "static-artifact-publish-precondition/v1",
            "body": {
                "targetLogicalAddress":"local-file:///active/app",
                "expectedTarget":{"state":"absent"},
                "expectedCustodyGeneration":{"state":"absent"},
                "unexpected":true
            }
        }),
        serde_json::json!({
            "kind": "static-artifact-separation-input/v1",
            "body": {
                "deedDigest":TWO,
                "quarantineAddress":"local-file:///quarantine/app",
                "quarantineXattrDigest":xattrs.reference().digest(),
                "unexpected":true
            }
        }),
        serde_json::json!({
            "kind": "static-artifact-separation-precondition/v1",
            "body": {
                "expectedActive":{
                    "state":"present","artifactName":"app","contentDigest":ONE,
                    "byteLength":"42","incarnation":TWO
                },
                "expectedQuarantine":{"state":"absent"},
                "expectedCustodyGeneration":"3",
                "unexpected":true
            }
        }),
    ];
    for value in owned_payloads {
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(matches!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])),
            Err(BodyError::Canonical(_))
        ));
    }
}

#[test]
fn replay_rejects_the_full_forbidden_number_model_before_payload_decoding() {
    for body in [
        br#"{"kind":"xattr-value/v1","body":{"entries":[{"name":"a","valueDigest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","byteLength":-1}]}}"#.as_slice(),
        br#"{"kind":"xattr-value/v1","body":{"entries":[{"name":"a","valueDigest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","byteLength":1.0}]}}"#.as_slice(),
        br#"{"kind":"xattr-value/v1","body":{"entries":[{"name":"a","valueDigest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","byteLength":9007199254740992}]}}"#.as_slice(),
        br#"{"kind":"xattr-value/v1","body":{"entries":[{"name":"a","valueDigest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","byteLength":1e0}]}}"#.as_slice(),
    ] {
        assert!(matches!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(digest(TWO), body.to_vec())])),
            Err(BodyError::Canonical(CanonicalError::Number))
        ));
    }
}

#[test]
fn hostile_xattr_sets_and_nested_state_unions_fail_closed_on_replay() {
    let hostile = [
        serde_json::json!({
            "kind":"xattr-value/v1",
            "body":{"entries":[]}
        }),
        serde_json::json!({
            "kind":"xattr-value/v1",
            "body":{"entries":[
                {"name":"b","valueDigest":TWO,"byteLength":"2"},
                {"name":"a","valueDigest":ONE,"byteLength":"1"}
            ]}
        }),
        serde_json::json!({
            "kind":"static-artifact-publish-precondition/v1",
            "body":{
                "targetLogicalAddress":"local-file:///active/app",
                "expectedTarget":{"state":"absent","artifactName":"illegal"},
                "expectedCustodyGeneration":{"state":"absent"}
            }
        }),
        serde_json::json!({
            "kind":"static-artifact-separation-precondition/v1",
            "body":{
                "expectedActive":{"state":"present","artifactName":"app","contentDigest":ONE,"byteLength":"42","incarnation":TWO,"extra":true},
                "expectedQuarantine":{"state":"absent"},
                "expectedCustodyGeneration":"3"
            }
        }),
    ];
    for value in hostile {
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])).is_err());
    }
}

#[test]
fn separation_input_extracts_both_exact_typed_edges() {
    let value = validated_body(
        StaticArtifactSeparationInput::new(
            ResourceDeedRef::from_digest(digest(TWO)),
            LogicalAddress::parse("local-file:///quarantine/app").unwrap(),
            XattrValueRef::from_digest(digest(THREE)),
        )
        .unwrap(),
    )
    .unwrap();
    let edges = value.clone().into_stored().edges().to_vec();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].target().as_str(), TWO);
    assert_eq!(edges[0].expected_kind(), BodyKind::ResourceDeed);
    assert_eq!(edges[1].target().as_str(), THREE);
    assert_eq!(edges[1].expected_kind(), BodyKind::XattrValue);

    assert!(matches!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![value.into_stored()]).unwrap()
        ),
        Err(BodyError::MissingReference { .. })
    ));
}
