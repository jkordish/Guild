mod support;

use std::collections::BTreeMap;

use guild_effect_kernel::lease::derive_resource_key;
use guild_effect_kernel::{
    body::{
        BodyError, BodyGraph, BodyKind, LocalFileObservation, OptionalValue, SortedUnique,
        validated_body,
    },
    canonical::{canonical_bytes, canonical_digest},
    evidence::{
        CausalityOutcome, CommandReport, CustodyState, EvidenceLimitation, MutationMode,
        ObservationAttempt, ObservationEvidence, OperationResult, PublicationPostcondition,
        ReceiptReason, ReceiptState, SeparationPostcondition, WitnessStatus,
    },
    scalar::{
        ArtifactName, ByteLength, Digest, Identifier, IncarnationId, LogicalAddress, RawDigest,
        UnixNanoseconds,
    },
};

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

fn exact_wire<T: serde::Serialize>(values: &[(T, &str)]) {
    for (value, expected) in values {
        assert_eq!(
            serde_json::to_string(value).unwrap(),
            format!(r#""{expected}""#)
        );
    }
}

#[test]
fn evidence_vocabulary_uses_exact_closed_snake_case_wire_values() {
    exact_wire(&[
        (
            EvidenceLimitation::WitnessUnavailable,
            "witness_unavailable",
        ),
        (
            EvidenceLimitation::UnsupportedIdentity,
            "unsupported_identity",
        ),
        (
            EvidenceLimitation::NonAtomicExternalOperation,
            "non_atomic_external_operation",
        ),
        (EvidenceLimitation::StaleObservation, "stale_observation"),
        (
            EvidenceLimitation::ConflictingObservation,
            "conflicting_observation",
        ),
    ]);
    exact_wire(&[
        (CommandReport::ReportedSuccess, "reported_success"),
        (CommandReport::ReportedNoEffect, "reported_no_effect"),
        (CommandReport::ReportedUncertain, "reported_uncertain"),
        (CommandReport::NotAvailable, "not_available"),
    ]);
    exact_wire(&[
        (PublicationPostcondition::ExactRequested, "exact_requested"),
        (
            PublicationPostcondition::AuthoritativeAbsence,
            "authoritative_absence",
        ),
        (
            PublicationPostcondition::PriorStateUnchanged,
            "prior_state_unchanged",
        ),
        (
            PublicationPostcondition::ContentMismatch,
            "content_mismatch",
        ),
        (PublicationPostcondition::Ambiguous, "ambiguous"),
    ]);
    exact_wire(&[
        (SeparationPostcondition::ExactQuarantine, "exact_quarantine"),
        (SeparationPostcondition::NoMove, "no_move"),
        (SeparationPostcondition::Ambiguous, "ambiguous"),
    ]);
    exact_wire(&[
        (
            CausalityOutcome::ExactPreparedIncarnation,
            "exact_prepared_incarnation",
        ),
        (
            CausalityOutcome::DifferentIncarnation,
            "different_incarnation",
        ),
        (
            CausalityOutcome::DuplicateIncarnation,
            "duplicate_incarnation",
        ),
        (CausalityOutcome::Ambiguous, "ambiguous"),
        (CausalityOutcome::Unsupported, "unsupported"),
    ]);
    exact_wire(&[
        (ReceiptState::Verified, "verified"),
        (ReceiptState::Failed, "failed"),
        (ReceiptState::Indeterminate, "indeterminate"),
    ]);
    exact_wire(&[
        (MutationMode::Conditional, "conditional"),
        (MutationMode::Unconditional, "unconditional"),
    ]);
    exact_wire(&[
        (
            WitnessStatus::AuthenticatedEnrolled,
            "authenticated_enrolled",
        ),
        (WitnessStatus::Unauthenticated, "unauthenticated"),
        (WitnessStatus::Unenrolled, "unenrolled"),
    ]);
    exact_wire(&[
        (CustodyState::Owned, "owned"),
        (CustodyState::Quarantined, "quarantined"),
        (CustodyState::Absent, "absent"),
        (CustodyState::Disputed, "disputed"),
    ]);

    exact_wire(&[
        (OperationResult::NotAttempted, "not_attempted"),
        (OperationResult::PreparedOnly, "prepared_only"),
        (
            OperationResult::PublishReportedSuccess,
            "publish_reported_success",
        ),
        (
            OperationResult::PublishReportedNoEffect,
            "publish_reported_no_effect",
        ),
        (
            OperationResult::PublishReportedUncertain,
            "publish_reported_uncertain",
        ),
        (OperationResult::PublishRecovered, "publish_recovered"),
        (
            OperationResult::QuarantineReportedSuccess,
            "quarantine_reported_success",
        ),
        (
            OperationResult::QuarantineReportedNoEffect,
            "quarantine_reported_no_effect",
        ),
        (
            OperationResult::QuarantineReportedUncertain,
            "quarantine_reported_uncertain",
        ),
        (OperationResult::QuarantineRecovered, "quarantine_recovered"),
    ]);

    exact_wire(&[
        (ReceiptReason::ArtifactVerified, "artifact_verified"),
        (ReceiptReason::SeparationVerified, "separation_verified"),
        (ReceiptReason::SourceChanged, "source_changed"),
        (
            ReceiptReason::SourceInvalidAfterStart,
            "source_invalid_after_start",
        ),
        (
            ReceiptReason::DigestMismatchAfterStart,
            "digest_mismatch_after_start",
        ),
        (ReceiptReason::PublicationNoEffect, "publication_no_effect"),
        (ReceiptReason::AuthoritativeAbsence, "authoritative_absence"),
        (
            ReceiptReason::SeparationPreconditionRefused,
            "separation_precondition_refused",
        ),
        (ReceiptReason::SeparationNoMove, "separation_no_move"),
        (ReceiptReason::WitnessUnavailable, "witness_unavailable"),
        (ReceiptReason::PublicationAmbiguous, "publication_ambiguous"),
        (ReceiptReason::IncarnationAmbiguous, "incarnation_ambiguous"),
        (ReceiptReason::DuplicateIncarnation, "duplicate_incarnation"),
        (ReceiptReason::SeparationAmbiguous, "separation_ambiguous"),
        (ReceiptReason::UnsupportedIdentity, "unsupported_identity"),
    ]);

    for hostile in [
        r#""ReportedSuccess""#,
        r#""future""#,
        r#""artifact-verified""#,
    ] {
        assert!(serde_json::from_str::<CommandReport>(hostile).is_err());
        assert!(serde_json::from_str::<ReceiptReason>(hostile).is_err());
    }
}

#[test]
fn observation_evidence_is_the_exact_closed_union() {
    let observed: ObservationEvidence =
        serde_json::from_str(&format!(r#"{{"state":"observed","digest":"{ONE}"}}"#)).unwrap();
    assert_eq!(
        canonical_bytes(&observed).unwrap(),
        format!(r#"{{"digest":"{ONE}","state":"observed"}}"#).as_bytes()
    );

    for variant in ["unavailable", "unsupported"] {
        let wire = format!(
            r#"{{"state":"{variant}","logicalAddress":"local-file:///active/app","witnessId":"host-probe","attemptedAt":"10"}}"#
        );
        let value: ObservationEvidence = serde_json::from_str(&wire).unwrap();
        let encoded = String::from_utf8(canonical_bytes(&value).unwrap()).unwrap();
        assert!(encoded.contains(&format!(r#""state":"{variant}""#)));
    }

    let conflicting: ObservationEvidence = serde_json::from_str(&format!(
        r#"{{"state":"conflicting","logicalAddress":"local-file:///active/app","witnessId":"host-probe","attemptedAt":"10","observationDigests":["{ONE}","{TWO}"]}}"#
    ))
    .unwrap();
    assert!(
        String::from_utf8(canonical_bytes(&conflicting).unwrap())
            .unwrap()
            .contains("observationDigests")
    );

    for hostile in [
        format!(r#"{{"state":"observed","digest":"{ONE}","witnessId":"x"}}"#),
        format!(
            r#"{{"state":"conflicting","logicalAddress":"local-file:///active/app","witnessId":"host-probe","attemptedAt":"10","observationDigests":["{ONE}"]}}"#
        ),
        format!(
            r#"{{"state":"conflicting","logicalAddress":"local-file:///active/app","witnessId":"host-probe","attemptedAt":"10","observationDigests":["{TWO}","{ONE}"]}}"#
        ),
        r#"{"state":"future"}"#.to_owned(),
    ] {
        assert!(
            serde_json::from_str::<ObservationEvidence>(&hostile).is_err(),
            "accepted hostile observation evidence: {hostile}"
        );
    }
}

#[test]
fn observation_attempts_expose_probe_facts_only() {
    let observed = validated_body(LocalFileObservation::present(
        LogicalAddress::parse("local-file:///active/app").unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("10").unwrap(),
        ArtifactName::parse("app").unwrap(),
        RawDigest::parse(TWO).unwrap(),
        ByteLength::from_u64(42),
        IncarnationId::parse(THREE).unwrap(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let attempt = support::authenticated_attempt(observed);
    let debug = format!("{attempt:?}");
    for forbidden in [
        "postcondition",
        "causality",
        "reason",
        "result",
        "generation",
        "custody",
        "deed",
    ] {
        assert!(!debug.to_ascii_lowercase().contains(forbidden));
    }

    let unsupported = ObservationAttempt::Unsupported {
        logical_address: LogicalAddress::parse("local-file:///active/app").unwrap(),
        witness_id: Identifier::parse("host-probe").unwrap(),
        attempted_at: UnixNanoseconds::parse("10").unwrap(),
    };
    assert!(matches!(
        unsupported,
        ObservationAttempt::Unsupported { .. }
    ));

    let first = validated_body(LocalFileObservation::absent(
        LogicalAddress::parse("local-file:///active/app").unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("10").unwrap(),
    ))
    .unwrap();
    let second = validated_body(LocalFileObservation::present(
        LogicalAddress::parse("local-file:///active/app").unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("10").unwrap(),
        ArtifactName::parse("app").unwrap(),
        RawDigest::parse(TWO).unwrap(),
        ByteLength::from_u64(42),
        IncarnationId::parse(THREE).unwrap(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let mut observations = vec![first, second];
    observations.sort_by(|a, b| a.reference().cmp(b.reference()));
    let attempt = ObservationAttempt::Conflicting {
        observations: SortedUnique::new(observations).unwrap(),
        witness: WitnessStatus::AuthenticatedEnrolled,
        attempted_at: UnixNanoseconds::parse("10").unwrap(),
    };
    assert!(matches!(attempt, ObservationAttempt::Conflicting { .. }));
}

fn envelope(kind: BodyKind, body: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"body": body, "kind": kind.as_str()})
}

#[test]
fn all_eight_task_seven_replay_decoders_run_before_reference_resolution() {
    let cases = [
        envelope(
            BodyKind::PublicationEvidence,
            serde_json::json!({
                "effectId": ONE,
                "bindingDigest": ONE,
                "preparedArtifactDigest": TWO,
                "commandReport": "reported_success",
                "sourceBeforeObservationDigest": ONE,
                "targetBeforeObservationDigest": TWO,
                "sourceAfter": {"state":"observed", "digest":ONE},
                "targetAfter": {"state":"unavailable", "logicalAddress":"local-file:///active/app", "witnessId":"host-probe", "attemptedAt":"10"},
                "postcondition": "ambiguous",
                "limitations": ["witness_unavailable"],
                "assessedAt": "10"
            }),
        ),
        envelope(
            BodyKind::CausalityAssessment,
            serde_json::json!({"effectId":ONE, "evidenceDigest":ONE, "outcome":"ambiguous"}),
        ),
        envelope(
            BodyKind::EffectReceipt,
            serde_json::json!({
                "effectId":ONE, "bindingDigest":ONE, "evidenceDigest":TWO,
                "causalityDigest":THREE, "state":"indeterminate",
                "result":"publish_reported_uncertain", "reason":"publication_ambiguous",
                "terminalAt":"10"
            }),
        ),
        envelope(
            BodyKind::ResourceDeed,
            serde_json::json!({
                "resourceKey":derive_resource_key(&LogicalAddress::parse("local-file:///active/app").unwrap()).unwrap(), "logicalAddress":"local-file:///active/app",
                "artifactName":"app", "contentDigest":TWO, "byteLength":"42",
                "incarnation":THREE, "publicationReceiptDigest":ONE,
                "custodyGeneration":"0"
            }),
        ),
        envelope(
            BodyKind::SeparationEvidence,
            serde_json::json!({
                "effectId":ONE, "bindingDigest":ONE, "deedDigest":TWO,
                "activeBeforeObservationDigest":ONE,
                "quarantineBeforeObservationDigest":TWO,
                "activeAfter":{"state":"observed","digest":ONE},
                "quarantineAfter":{"state":"unsupported","logicalAddress":"local-file:///quarantine/app","witnessId":"host-probe","attemptedAt":"10"},
                "commandReport":"reported_uncertain", "postcondition":"ambiguous",
                "limitations":["unsupported_identity"], "assessedAt":"10"
            }),
        ),
        envelope(
            BodyKind::SeparationReceipt,
            serde_json::json!({
                "effectId":ONE, "bindingDigest":ONE, "evidenceDigest":TWO,
                "deedDigest":THREE, "state":"indeterminate",
                "result":"quarantine_reported_uncertain", "reason":"separation_ambiguous",
                "terminalAt":"10", "nextCustodyGeneration":"1"
            }),
        ),
        envelope(
            BodyKind::CustodyRecord,
            serde_json::json!({
                "resourceKey":ONE, "deedDigest":{"state":"absent"},
                "custodyGeneration":"0", "state":"absent",
                "terminalReceipt":{"protocol":"publication","digest":ONE},
                "activeAddress":"local-file:///active/app",
                "quarantineAddress":{"state":"absent"}
            }),
        ),
        envelope(
            BodyKind::RecoveryAssessment,
            serde_json::json!({
                "effectId":ONE,
                "bindingDigest":{"protocol":"publication","digest":ONE},
                "evidenceDigest":{"protocol":"publication","digest":TWO},
                "receiptDigest":{"protocol":"publication","digest":THREE},
                "recoveredAt":"10", "state":"indeterminate",
                "reason":"publication_ambiguous"
            }),
        ),
    ];

    for value in cases {
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(
            matches!(
                BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])),
                Err(BodyError::MissingReference { .. })
            ),
            "decoder did not reach reference resolution for {}",
            value["kind"]
        );
    }
}

#[test]
fn hostile_non_derived_receipt_deed_and_custody_shapes_fail_closed() {
    let hostile = [
        envelope(
            BodyKind::EffectReceipt,
            serde_json::json!({
                "effectId":ONE, "bindingDigest":ONE, "evidenceDigest":TWO,
                "causalityDigest":THREE, "state":"verified",
                "result":"publish_reported_success", "reason":"artifact_verified",
                "terminalAt":"10", "callerSelected":true
            }),
        ),
        envelope(
            BodyKind::ResourceDeed,
            serde_json::json!({
                "resourceKey":ONE, "logicalAddress":"local-file:///active/app",
                "artifactName":"app", "contentDigest":TWO, "byteLength":"42",
                "incarnation":THREE, "publicationReceiptDigest":ONE,
                "custodyGeneration":"0", "proof":"claimed"
            }),
        ),
        envelope(
            BodyKind::CustodyRecord,
            serde_json::json!({
                "resourceKey":ONE, "deedDigest":{"state":"absent"},
                "custodyGeneration":"0", "state":"owned",
                "terminalReceipt":{"protocol":"publication","digest":ONE},
                "activeAddress":"local-file:///active/app",
                "quarantineAddress":{"state":"absent"}, "generationWasChosen":true
            }),
        ),
    ];
    for value in hostile {
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])).is_err());
    }
}

#[test]
fn malformed_cross_field_shapes_fail_before_graph_resolution() {
    let malformed = [
        envelope(
            BodyKind::CustodyRecord,
            serde_json::json!({
                "resourceKey":ONE, "deedDigest":{"state":"absent"},
                "custodyGeneration":"0", "state":"owned",
                "terminalReceipt":{"protocol":"publication","digest":ONE},
                "activeAddress":"local-file:///active/app",
                "quarantineAddress":{"state":"present","value":"local-file:///q/app"}
            }),
        ),
        envelope(
            BodyKind::RecoveryAssessment,
            serde_json::json!({
                "effectId":ONE,
                "bindingDigest":{"protocol":"publication","digest":ONE},
                "evidenceDigest":{"protocol":"separation","digest":TWO},
                "receiptDigest":{"protocol":"publication","digest":THREE},
                "recoveredAt":"10", "state":"indeterminate",
                "reason":"publication_ambiguous"
            }),
        ),
    ];
    for value in malformed {
        let bytes = canonical_bytes(&value).unwrap();
        let key = canonical_digest(&value).unwrap();
        assert!(matches!(
            BodyGraph::from_canonical_entries(BTreeMap::from([(key, bytes)])),
            Err(BodyError::Local(_))
        ));
    }
}

#[test]
fn digest_parser_used_by_wire_fixtures_remains_nonzero() {
    assert_eq!(Digest::parse(ONE).unwrap().as_str(), ONE);
}
