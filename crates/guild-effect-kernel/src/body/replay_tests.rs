use super::*;

const CONTENT: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const INCARNATION: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ENROLLMENT_INCARNATION: &str =
    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[derive(Clone, Copy)]
enum PublicationOutcome {
    Verified,
    Failed,
    Indeterminate,
}

struct PublicationReplayFixture {
    entries: BTreeMap<Digest, Vec<u8>>,
    prepared: StoredBody,
    policy: ValidatedBody<crate::authority::AuthorityPolicy>,
    enrollment: ValidatedBody<crate::authority::InstallationEnrollment>,
    deed: Option<ValidatedBody<crate::evidence::ResourceDeed>>,
    custody: ValidatedBody<crate::evidence::CustodyRecord>,
}

fn insert_body<P: BodySpec>(entries: &mut BTreeMap<Digest, Vec<u8>>, body: &ValidatedBody<P>) {
    entries.insert(
        body.reference().digest().clone(),
        body.canonical_bytes().to_vec(),
    );
}

fn replay_with_staged_prepared(fixture: &PublicationReplayFixture) -> Result<BodyGraph, BodyError> {
    let decoded = fixture
        .entries
        .iter()
        .map(|(key, bytes)| decode_entry(key, bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let prepared_digest = digest_bytes(&fixture.prepared.canonical_bytes)?;
    if prepared_digest != fixture.prepared.digest {
        return Err(BodyError::KeyMismatch {
            key: fixture.prepared.digest.clone(),
            computed: prepared_digest,
        });
    }

    let mut bodies = BTreeMap::from([(fixture.prepared.digest.clone(), fixture.prepared.clone())]);
    let mut facts = BTreeMap::from([(fixture.prepared.digest.clone(), BodyFacts::None)]);
    for decoded_body in decoded {
        let digest = decoded_body.stored.digest.clone();
        bodies.insert(digest.clone(), decoded_body.stored);
        facts.insert(digest, decoded_body.facts);
    }
    validate_edges(&bodies)?;
    validate_cross_body(&bodies, &facts)?;
    validate_cycles(&bodies)?;
    Ok(BodyGraph { bodies })
}

fn complete_entries(fixture: &PublicationReplayFixture) -> BTreeMap<Digest, Vec<u8>> {
    let mut entries = fixture.entries.clone();
    entries.insert(
        fixture.prepared.digest.clone(),
        fixture.prepared.canonical_bytes.clone(),
    );
    entries
}

fn authority_roots(
    entries: &mut BTreeMap<Digest, Vec<u8>>,
) -> (
    ValidatedBody<crate::authority::AuthorityPolicy>,
    ValidatedBody<crate::authority::InstallationEnrollment>,
) {
    let policy: crate::authority::AuthorityPolicy = serde_json::from_value(serde_json::json!({
        "policyId":"workstation-policy",
        "generation":"0",
        "proposerIds":["proposer"],
        "approverIds":["approver"],
        "revokerIds":["revoker"],
        "witnessIds":["host-probe"],
        "requireDistinctApprovalPrincipal":true,
        "reservationBudgets":[{"key":"reservation", "capacity":100}],
        "startBudgets":[{"key":"start", "capacity":100}],
        "trustedClockId":"trusted-clock",
        "trustedStoreId":"trusted-store"
    }))
    .unwrap();
    let policy = validated_body(policy).unwrap();
    insert_body(entries, &policy);
    let enrollment = validated_body(
        crate::authority::InstallationEnrollment::new(
            Identifier::parse("macbook-pro").unwrap(),
            IncarnationId::parse(ENROLLMENT_INCARNATION).unwrap(),
            policy.reference().clone(),
            UnixNanoseconds::parse("0").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    insert_body(entries, &enrollment);
    (policy, enrollment)
}

fn staged_prepared_artifact(
    effect_id: &crate::scalar::EffectId,
    binding: &ValidatedBody<crate::lease::IdempotencyBinding>,
    input: &ValidatedBody<StaticArtifactPublishInput>,
    source_before: &ValidatedBody<LocalFileObservation>,
    target_before: &ValidatedBody<LocalFileObservation>,
) -> StoredBody {
    let envelope = serde_json::json!({
        "body": {
            "effectId": effect_id,
            "bindingDigest": binding.reference(),
            "inputDigest": input.reference(),
            "sourceBeforeObservationDigest": source_before.reference(),
            "targetBeforeObservationDigest": target_before.reference(),
            "contentDigest": CONTENT,
            "byteLength": "42",
            "preparedIncarnation": INCARNATION,
            "preparedAt": "10"
        },
        "kind": "prepared-artifact/v1"
    });
    let canonical_bytes = canonical_bytes(&envelope).unwrap();
    let digest = digest_bytes(&canonical_bytes).unwrap();
    StoredBody {
        digest,
        kind: BodyKind::PreparedArtifact,
        canonical_bytes,
        edges: vec![
            TypedEdge::new(binding.reference()),
            TypedEdge::new(input.reference()),
            TypedEdge::new(source_before.reference()),
            TypedEdge::new(target_before.reference()),
        ],
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the fixture spells out the full canonical publication replay graph"
)]
fn publication_fixture(
    outcome: PublicationOutcome,
    expected_generation: Option<u64>,
    deed_generation: u64,
    custody_generation: u64,
    custody_address: &str,
) -> PublicationReplayFixture {
    let mut entries = BTreeMap::new();
    let (policy, enrollment) = authority_roots(&mut entries);
    let source_address = LogicalAddress::parse("local-file:///staging/app").unwrap();
    let target_address = LogicalAddress::parse("local-file:///active/app").unwrap();
    let witness = Identifier::parse("host-probe").unwrap();
    let source_before = validated_body(LocalFileObservation::present(
        source_address.clone(),
        witness.clone(),
        UnixNanoseconds::parse("10").unwrap(),
        ArtifactName::parse("app").unwrap(),
        RawDigest::parse(CONTENT).unwrap(),
        ByteLength::from_u64(42),
        IncarnationId::parse(INCARNATION).unwrap(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let target_before = validated_body(LocalFileObservation::absent(
        target_address.clone(),
        witness.clone(),
        UnixNanoseconds::parse("10").unwrap(),
    ))
    .unwrap();
    insert_body(&mut entries, &source_before);
    insert_body(&mut entries, &target_before);
    let input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            source_before.reference().clone(),
            target_address.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &input);
    let expected_generation = expected_generation
        .map_or_else(OptionalValue::absent, |generation| {
            OptionalValue::present(U64Decimal::from_u64(generation))
        });
    let precondition = validated_body(StaticArtifactPublishPrecondition::new(
        target_address.clone(),
        ExpectedState::absent(),
        expected_generation,
    ))
    .unwrap();
    insert_body(&mut entries, &precondition);
    let mut resource_keys = [
        crate::lease::derive_resource_key(&source_address).unwrap(),
        crate::lease::derive_resource_key(&target_address).unwrap(),
    ];
    resource_keys.sort();
    let warrant: crate::authority::PublicationWarrant = serde_json::from_value(serde_json::json!({
        "installationDigest": enrollment.reference(),
        "policyDigest": policy.reference(),
        "policyGeneration":"0",
        "effectKind":"static_artifact_publish",
        "proposerId":"proposer",
        "inputDigest":input.reference(),
        "preconditionDigest":precondition.reference(),
        "idempotencyKey":"aggregate-publish-0001",
        "resourceKeys":resource_keys,
        "reservationBudget":{"key":"reservation", "amount":1},
        "startBudget":{"key":"start", "amount":1},
        "issuedAt":"1",
        "expiresAt":"30",
        "nonce":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
    }))
    .unwrap();
    let warrant = validated_body(warrant).unwrap();
    insert_body(&mut entries, &warrant);
    let effect_id = publication_effect_id(warrant.reference().digest(), warrant.payload()).unwrap();
    let binding = validated_body(
        crate::lease::decode_idempotency_binding(serde_json::json!({
            "idempotencyKey":"aggregate-publish-0001",
            "effectId":effect_id,
            "warrantDigest":warrant.reference()
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &binding);
    let prepared =
        staged_prepared_artifact(&effect_id, &binding, &input, &source_before, &target_before);

    let (source_after, target_after, source_evidence, target_evidence) = match outcome {
        PublicationOutcome::Verified => {
            let source_after = validated_body(LocalFileObservation::absent(
                source_address,
                witness.clone(),
                UnixNanoseconds::parse("20").unwrap(),
            ))
            .unwrap();
            let target_after = validated_body(LocalFileObservation::present(
                target_address.clone(),
                witness,
                UnixNanoseconds::parse("20").unwrap(),
                ArtifactName::parse("app").unwrap(),
                RawDigest::parse(CONTENT).unwrap(),
                ByteLength::from_u64(42),
                IncarnationId::parse(INCARNATION).unwrap(),
                OptionalValue::absent(),
            ))
            .unwrap();
            let source_ref = source_after.reference().clone();
            let target_ref = target_after.reference().clone();
            (
                Some(source_after),
                Some(target_after),
                serde_json::json!({"state":"observed", "digest":source_ref}),
                serde_json::json!({"state":"observed", "digest":target_ref}),
            )
        }
        PublicationOutcome::Failed => {
            let source_after = validated_body(LocalFileObservation::present(
                source_address,
                witness.clone(),
                UnixNanoseconds::parse("20").unwrap(),
                ArtifactName::parse("app").unwrap(),
                RawDigest::parse(CONTENT).unwrap(),
                ByteLength::from_u64(42),
                IncarnationId::parse(INCARNATION).unwrap(),
                OptionalValue::absent(),
            ))
            .unwrap();
            let target_after = validated_body(LocalFileObservation::absent(
                target_address.clone(),
                witness,
                UnixNanoseconds::parse("20").unwrap(),
            ))
            .unwrap();
            let source_ref = source_after.reference().clone();
            let target_ref = target_after.reference().clone();
            (
                Some(source_after),
                Some(target_after),
                serde_json::json!({"state":"observed", "digest":source_ref}),
                serde_json::json!({"state":"observed", "digest":target_ref}),
            )
        }
        PublicationOutcome::Indeterminate => (
            None,
            None,
            serde_json::json!({
                "state":"unavailable",
                "logicalAddress":"local-file:///staging/app",
                "witnessId":"host-probe",
                "attemptedAt":"20"
            }),
            serde_json::json!({
                "state":"unavailable",
                "logicalAddress":"local-file:///active/app",
                "witnessId":"host-probe",
                "attemptedAt":"20"
            }),
        ),
    };
    if let Some(body) = &source_after {
        insert_body(&mut entries, body);
    }
    if let Some(body) = &target_after {
        insert_body(&mut entries, body);
    }
    let (
        command_report,
        postcondition,
        limitations,
        causality,
        state,
        result,
        reason,
        custody_state,
    ) = match outcome {
        PublicationOutcome::Verified => (
            "reported_success",
            "exact_requested",
            serde_json::json!([]),
            "exact_prepared_incarnation",
            "verified",
            "publish_reported_success",
            "artifact_verified",
            "owned",
        ),
        PublicationOutcome::Failed => (
            "reported_no_effect",
            "prior_state_unchanged",
            serde_json::json!([]),
            "ambiguous",
            "failed",
            "publish_reported_no_effect",
            "publication_no_effect",
            "absent",
        ),
        PublicationOutcome::Indeterminate => (
            "reported_uncertain",
            "ambiguous",
            serde_json::json!(["witness_unavailable"]),
            "ambiguous",
            "indeterminate",
            "publish_reported_uncertain",
            "witness_unavailable",
            "disputed",
        ),
    };
    let evidence = validated_body(
        crate::evidence::decode_publication_evidence(serde_json::json!({
            "effectId":effect_id,
            "bindingDigest":binding.reference(),
            "preparedArtifactDigest":prepared.digest,
            "commandReport":command_report,
            "sourceBeforeObservationDigest":source_before.reference(),
            "targetBeforeObservationDigest":target_before.reference(),
            "sourceAfter":source_evidence,
            "targetAfter":target_evidence,
            "postcondition":postcondition,
            "limitations":limitations,
            "assessedAt":"20"
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &evidence);
    let causality = validated_body(
        crate::evidence::decode_causality_assessment(serde_json::json!({
            "effectId":effect_id,
            "evidenceDigest":evidence.reference(),
            "outcome":causality
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &causality);
    let receipt = validated_body(
        crate::evidence::decode_effect_receipt(serde_json::json!({
            "effectId":effect_id,
            "bindingDigest":binding.reference(),
            "evidenceDigest":evidence.reference(),
            "causalityDigest":causality.reference(),
            "state":state,
            "result":result,
            "reason":reason,
            "terminalAt":"20"
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &receipt);
    let deed = if matches!(outcome, PublicationOutcome::Verified) {
        let deed = validated_body(
            crate::evidence::decode_resource_deed(serde_json::json!({
                "resourceKey":crate::lease::derive_resource_key(&target_address).unwrap(),
                "logicalAddress":target_address,
                "artifactName":"app",
                "contentDigest":CONTENT,
                "byteLength":"42",
                "incarnation":INCARNATION,
                "publicationReceiptDigest":receipt.reference(),
                "custodyGeneration":deed_generation.to_string()
            }))
            .unwrap(),
        )
        .unwrap();
        insert_body(&mut entries, &deed);
        Some(deed)
    } else {
        None
    };
    let deed_digest = deed.as_ref().map_or_else(
        || serde_json::json!({"state":"absent"}),
        |deed| serde_json::json!({"state":"present", "value":deed.reference()}),
    );
    let custody_address = LogicalAddress::parse(custody_address).unwrap();
    let custody = validated_body(
        crate::evidence::decode_custody_record(serde_json::json!({
            "resourceKey":crate::lease::derive_resource_key(&custody_address).unwrap(),
            "deedDigest":deed_digest,
            "custodyGeneration":custody_generation.to_string(),
            "state":custody_state,
            "terminalReceipt":{"protocol":"publication", "digest":receipt.reference()},
            "activeAddress":custody_address,
            "quarantineAddress":{"state":"absent"}
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut entries, &custody);
    PublicationReplayFixture {
        entries,
        prepared,
        policy,
        enrollment,
        deed,
        custody,
    }
}

#[test]
fn complete_publication_replay_is_blocked_exactly_at_the_staged_prepared_decoder() {
    let fixture = publication_fixture(
        PublicationOutcome::Verified,
        None,
        0,
        0,
        "local-file:///active/app",
    );

    assert!(matches!(
        BodyGraph::from_canonical_entries(complete_entries(&fixture)),
        Err(BodyError::PayloadModuleUnavailable {
            kind: BodyKind::PreparedArtifact
        })
    ));
}

#[test]
fn aggregate_replay_rejects_failed_and_indeterminate_publication_custody_off_target() {
    for outcome in [
        PublicationOutcome::Failed,
        PublicationOutcome::Indeterminate,
    ] {
        let valid = publication_fixture(outcome, None, 0, 0, "local-file:///active/app");
        assert!(replay_with_staged_prepared(&valid).is_ok());
        let hostile =
            publication_fixture(outcome, None, 0, 0, "local-file:///attacker-selected/app");
        assert!(matches!(
            replay_with_staged_prepared(&hostile),
            Err(BodyError::Local(message))
                if message.contains("address, resource key, and generation")
        ));
    }
}

#[test]
fn aggregate_replay_rejects_publication_deed_and_custody_generation_selection() {
    let mut forged_deed = publication_fixture(
        PublicationOutcome::Verified,
        None,
        7,
        0,
        "local-file:///active/app",
    );
    forged_deed
        .entries
        .remove(forged_deed.custody.reference().digest());
    assert!(matches!(
        replay_with_staged_prepared(&forged_deed),
        Err(BodyError::Local(message))
            if message.contains("deed fields are not derived from the verified publication")
    ));

    for outcome in [
        PublicationOutcome::Verified,
        PublicationOutcome::Failed,
        PublicationOutcome::Indeterminate,
    ] {
        let forged_custody = publication_fixture(outcome, None, 0, 7, "local-file:///active/app");
        assert!(matches!(
            replay_with_staged_prepared(&forged_custody),
            Err(BodyError::Local(message))
                if message.contains("publication custody address, resource key, and generation")
        ));
    }
}

#[test]
fn aggregate_replay_rejects_exhausted_publication_generation_for_every_outcome() {
    for outcome in [
        PublicationOutcome::Verified,
        PublicationOutcome::Failed,
        PublicationOutcome::Indeterminate,
    ] {
        let exhausted =
            publication_fixture(outcome, Some(u64::MAX), 0, 0, "local-file:///active/app");
        assert!(matches!(
            replay_with_staged_prepared(&exhausted),
            Err(BodyError::Local(message)) if message.contains("generation is exhausted")
        ));
    }
}

#[derive(Clone, Copy)]
enum SeparationOutcome {
    Verified,
    Failed,
    Indeterminate,
}

struct SeparationReplayBodies {
    precondition: ValidatedBody<StaticArtifactSeparationPrecondition>,
    receipt: ValidatedBody<crate::evidence::SeparationReceipt>,
    custody: ValidatedBody<crate::evidence::CustodyRecord>,
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the fixture spells out one complete canonical separation terminal subgraph"
)]
fn add_separation(
    fixture: &mut PublicationReplayFixture,
    outcome: SeparationOutcome,
    admitted_generation: u64,
    receipt_generation: u64,
    custody_generation: u64,
    sequence: u8,
) -> SeparationReplayBodies {
    let deed = fixture
        .deed
        .as_ref()
        .expect("separation fixture requires a verified publication deed");
    let active_address = LogicalAddress::parse("local-file:///active/app").unwrap();
    let quarantine_address = LogicalAddress::parse("local-file:///quarantine/app").unwrap();
    let witness = Identifier::parse("host-probe").unwrap();
    let before_at = (30_u64 + u64::from(sequence) * 20).to_string();
    let assessed_at = (40_u64 + u64::from(sequence) * 20).to_string();
    let xattr = validated_body(
        XattrValue::new(vec![XattrEntry::new(
            crate::scalar::XattrName::parse("com.apple.quarantine").unwrap(),
            RawDigest::parse(
                "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            )
            .unwrap(),
            ByteLength::from_u64(1),
        )])
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &xattr);
    let active_before = validated_body(LocalFileObservation::present(
        active_address.clone(),
        witness.clone(),
        UnixNanoseconds::parse(&before_at).unwrap(),
        ArtifactName::parse("app").unwrap(),
        RawDigest::parse(CONTENT).unwrap(),
        ByteLength::from_u64(42),
        IncarnationId::parse(INCARNATION).unwrap(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let quarantine_before = validated_body(LocalFileObservation::absent(
        quarantine_address.clone(),
        witness.clone(),
        UnixNanoseconds::parse(&before_at).unwrap(),
    ))
    .unwrap();
    insert_body(&mut fixture.entries, &active_before);
    insert_body(&mut fixture.entries, &quarantine_before);
    let input = validated_body(
        StaticArtifactSeparationInput::new(
            deed.reference().clone(),
            quarantine_address.clone(),
            xattr.reference().clone(),
        )
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &input);
    let precondition = validated_body(StaticArtifactSeparationPrecondition::new(
        PresentExpectedState::new(
            ArtifactName::parse("app").unwrap(),
            RawDigest::parse(CONTENT).unwrap(),
            ByteLength::from_u64(42),
            IncarnationId::parse(INCARNATION).unwrap(),
        ),
        AbsentExpectedState::new(),
        U64Decimal::from_u64(admitted_generation),
    ))
    .unwrap();
    insert_body(&mut fixture.entries, &precondition);
    let mut resource_keys = [
        deed.payload().resource_key().clone(),
        crate::lease::derive_resource_key(&quarantine_address).unwrap(),
    ];
    resource_keys.sort();
    let idempotency_key = format!("aggregate-separate-{sequence:04}");
    let nonce_digit = char::from_digit(u32::from(sequence), 16).unwrap();
    let warrant: crate::authority::SeparationWarrant = serde_json::from_value(serde_json::json!({
        "installationDigest":fixture.enrollment.reference(),
        "policyDigest":fixture.policy.reference(),
        "policyGeneration":"0",
        "effectKind":"static_artifact_separation",
        "proposerId":"proposer",
        "inputDigest":input.reference(),
        "preconditionDigest":precondition.reference(),
        "idempotencyKey":idempotency_key,
        "resourceKeys":resource_keys,
        "reservationBudget":{"key":"reservation", "amount":1},
        "startBudget":{"key":"start", "amount":1},
        "issuedAt":before_at,
        "expiresAt":"200",
        "nonce":nonce_digit.to_string().repeat(64)
    }))
    .unwrap();
    let warrant = validated_body(warrant).unwrap();
    insert_body(&mut fixture.entries, &warrant);
    let effect_id = separation_effect_id(warrant.reference().digest(), warrant.payload()).unwrap();
    let binding = validated_body(
        crate::lease::decode_separation_binding(serde_json::json!({
            "idempotencyKey":idempotency_key,
            "effectId":effect_id,
            "warrantDigest":warrant.reference()
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &binding);

    let (active_after, quarantine_after, active_evidence, quarantine_evidence) = match outcome {
        SeparationOutcome::Verified => {
            let active_after = validated_body(LocalFileObservation::absent(
                active_address.clone(),
                witness.clone(),
                UnixNanoseconds::parse(&assessed_at).unwrap(),
            ))
            .unwrap();
            let quarantine_after = validated_body(LocalFileObservation::present(
                quarantine_address.clone(),
                witness,
                UnixNanoseconds::parse(&assessed_at).unwrap(),
                ArtifactName::parse("app").unwrap(),
                RawDigest::parse(CONTENT).unwrap(),
                ByteLength::from_u64(42),
                IncarnationId::parse(INCARNATION).unwrap(),
                OptionalValue::present(xattr.reference().clone()),
            ))
            .unwrap();
            let active_ref = active_after.reference().clone();
            let quarantine_ref = quarantine_after.reference().clone();
            (
                Some(active_after),
                Some(quarantine_after),
                serde_json::json!({"state":"observed", "digest":active_ref}),
                serde_json::json!({"state":"observed", "digest":quarantine_ref}),
            )
        }
        SeparationOutcome::Failed => {
            let active_after = validated_body(LocalFileObservation::present(
                active_address.clone(),
                witness.clone(),
                UnixNanoseconds::parse(&assessed_at).unwrap(),
                ArtifactName::parse("app").unwrap(),
                RawDigest::parse(CONTENT).unwrap(),
                ByteLength::from_u64(42),
                IncarnationId::parse(INCARNATION).unwrap(),
                OptionalValue::absent(),
            ))
            .unwrap();
            let quarantine_after = validated_body(LocalFileObservation::absent(
                quarantine_address.clone(),
                witness,
                UnixNanoseconds::parse(&assessed_at).unwrap(),
            ))
            .unwrap();
            let active_ref = active_after.reference().clone();
            let quarantine_ref = quarantine_after.reference().clone();
            (
                Some(active_after),
                Some(quarantine_after),
                serde_json::json!({"state":"observed", "digest":active_ref}),
                serde_json::json!({"state":"observed", "digest":quarantine_ref}),
            )
        }
        SeparationOutcome::Indeterminate => (
            None,
            None,
            serde_json::json!({
                "state":"unavailable",
                "logicalAddress":active_address,
                "witnessId":"host-probe",
                "attemptedAt":assessed_at
            }),
            serde_json::json!({
                "state":"unavailable",
                "logicalAddress":quarantine_address,
                "witnessId":"host-probe",
                "attemptedAt":assessed_at
            }),
        ),
    };
    if let Some(body) = &active_after {
        insert_body(&mut fixture.entries, body);
    }
    if let Some(body) = &quarantine_after {
        insert_body(&mut fixture.entries, body);
    }
    let (command_report, postcondition, limitations, state, result, reason, custody_state) =
        match outcome {
            SeparationOutcome::Verified => (
                "reported_success",
                "exact_quarantine",
                serde_json::json!([]),
                "verified",
                "quarantine_reported_success",
                "separation_verified",
                "quarantined",
            ),
            SeparationOutcome::Failed => (
                "reported_no_effect",
                "no_move",
                serde_json::json!([]),
                "failed",
                "quarantine_reported_no_effect",
                "separation_no_move",
                "owned",
            ),
            SeparationOutcome::Indeterminate => (
                "reported_uncertain",
                "ambiguous",
                serde_json::json!(["witness_unavailable"]),
                "indeterminate",
                "quarantine_reported_uncertain",
                "witness_unavailable",
                "disputed",
            ),
        };
    let evidence = validated_body(
        crate::evidence::decode_separation_evidence(serde_json::json!({
            "effectId":effect_id,
            "bindingDigest":binding.reference(),
            "deedDigest":deed.reference(),
            "activeBeforeObservationDigest":active_before.reference(),
            "quarantineBeforeObservationDigest":quarantine_before.reference(),
            "activeAfter":active_evidence,
            "quarantineAfter":quarantine_evidence,
            "commandReport":command_report,
            "postcondition":postcondition,
            "limitations":limitations,
            "assessedAt":assessed_at
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &evidence);
    let receipt = validated_body(
        crate::evidence::decode_separation_receipt(serde_json::json!({
            "effectId":effect_id,
            "bindingDigest":binding.reference(),
            "evidenceDigest":evidence.reference(),
            "deedDigest":deed.reference(),
            "state":state,
            "result":result,
            "reason":reason,
            "terminalAt":assessed_at,
            "nextCustodyGeneration":receipt_generation.to_string()
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &receipt);
    let custody = validated_body(
        crate::evidence::decode_custody_record(serde_json::json!({
            "resourceKey":deed.payload().resource_key(),
            "deedDigest":{"state":"present", "value":deed.reference()},
            "custodyGeneration":custody_generation.to_string(),
            "state":custody_state,
            "terminalReceipt":{"protocol":"separation", "digest":receipt.reference()},
            "activeAddress":active_address,
            "quarantineAddress":{"state":"present", "value":quarantine_address}
        }))
        .unwrap(),
    )
    .unwrap();
    insert_body(&mut fixture.entries, &custody);
    SeparationReplayBodies {
        precondition,
        receipt,
        custody,
    }
}

fn verified_publication_fixture() -> PublicationReplayFixture {
    publication_fixture(
        PublicationOutcome::Verified,
        None,
        0,
        0,
        "local-file:///active/app",
    )
}

#[test]
fn aggregate_replay_accepts_repeated_safe_no_move_with_predecessor_custody_history() {
    let mut fixture = verified_publication_fixture();
    let publication_custody_digest = fixture.custody.reference().digest().clone();
    let first = add_separation(&mut fixture, SeparationOutcome::Failed, 0, 1, 1, 1);
    let first_custody_digest = first.custody.reference().digest().clone();
    let second = add_separation(&mut fixture, SeparationOutcome::Failed, 1, 2, 2, 2);
    let second_custody_digest = second.custody.reference().digest().clone();

    let graph = replay_with_staged_prepared(&fixture).unwrap();
    assert!(graph.get(&publication_custody_digest).is_some());
    assert!(graph.get(&first_custody_digest).is_some());
    assert!(graph.get(&second_custody_digest).is_some());
    assert_eq!(
        fixture
            .deed
            .as_ref()
            .unwrap()
            .payload()
            .custody_generation()
            .get(),
        0
    );
    assert_eq!(
        first
            .precondition
            .payload()
            .expected_custody_generation()
            .get(),
        0
    );
    assert_eq!(first.receipt.payload().next_custody_generation().get(), 1);
    assert_eq!(first.custody.payload().custody_generation().get(), 1);
    assert_eq!(
        second
            .precondition
            .payload()
            .expected_custody_generation()
            .get(),
        1
    );
    assert_eq!(second.receipt.payload().next_custody_generation().get(), 2);
    assert_eq!(second.custody.payload().custody_generation().get(), 2);
}

#[test]
fn aggregate_replay_rejects_caller_selected_separation_receipt_successor_for_every_outcome() {
    for (index, outcome) in [
        SeparationOutcome::Verified,
        SeparationOutcome::Failed,
        SeparationOutcome::Indeterminate,
    ]
    .into_iter()
    .enumerate()
    {
        let sequence = u8::try_from(index + 1).unwrap();
        let mut valid = verified_publication_fixture();
        add_separation(&mut valid, outcome, 7, 8, 8, sequence);
        assert!(replay_with_staged_prepared(&valid).is_ok());

        let mut hostile = verified_publication_fixture();
        let hostile_bodies = add_separation(&mut hostile, outcome, 7, 42, 42, sequence);
        hostile
            .entries
            .remove(hostile_bodies.custody.reference().digest());
        assert!(matches!(
            replay_with_staged_prepared(&hostile),
            Err(BodyError::Local(message))
                if message.contains("separation receipt is not uniquely derived")
        ));
    }
}

#[test]
fn aggregate_replay_rejects_caller_selected_separation_custody_successor_for_every_outcome() {
    for (index, outcome) in [
        SeparationOutcome::Verified,
        SeparationOutcome::Failed,
        SeparationOutcome::Indeterminate,
    ]
    .into_iter()
    .enumerate()
    {
        let sequence = u8::try_from(index + 1).unwrap();
        let mut hostile = verified_publication_fixture();
        add_separation(&mut hostile, outcome, 7, 8, 42, sequence);
        assert!(matches!(
            replay_with_staged_prepared(&hostile),
            Err(BodyError::Local(message))
                if message.contains("separation custody is not uniquely receipt/deed/input-derived")
        ));
    }
}

#[test]
fn aggregate_replay_rejects_exhausted_separation_generation_for_every_outcome() {
    for (index, outcome) in [
        SeparationOutcome::Verified,
        SeparationOutcome::Failed,
        SeparationOutcome::Indeterminate,
    ]
    .into_iter()
    .enumerate()
    {
        let mut fixture = verified_publication_fixture();
        add_separation(
            &mut fixture,
            outcome,
            u64::MAX,
            1,
            1,
            u8::try_from(index + 1).unwrap(),
        );
        assert!(matches!(
            replay_with_staged_prepared(&fixture),
            Err(BodyError::Local(message)) if message.contains("generation is exhausted")
        ));
    }
}
