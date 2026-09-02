mod support;

use std::collections::BTreeMap;

use guild_effect_kernel::{
    body::{BodyError, InstallationEnrollmentRef},
    canonical::canonical_digest,
    event::{ChainError, EventEnvelope, EventType, PreviousEvent, TrustedHead, validate_chain},
    protocol::{EVENT_SCHEMA_VERSION, EVENT_TYPE_IDS},
    scalar::{Digest, Identifier, UnixNanoseconds},
    store::{ExpectedHead, ImmutableStore, TrustedCommitOutcome},
};
use serde_json::{Map, Value, json};

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

fn previous(digest: &Digest) -> Value {
    json!({ "state": "previous", "digest": digest })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "owned JSON values keep each hostile integration fixture self-contained"
)]
fn preimage(
    installation: &InstallationEnrollmentRef,
    sequence: &str,
    previous_event: Value,
    occurred_at: &str,
    event_type: &str,
    payload: Value,
) -> Value {
    json!({
        "schemaVersion": EVENT_SCHEMA_VERSION,
        "sequence": sequence,
        "previousEvent": previous_event,
        "installationDigest": installation,
        "occurredAt": occurred_at,
        "eventType": event_type,
        "payload": payload,
    })
}

fn decoded(preimage: Value) -> EventEnvelope {
    let digest = canonical_digest(&preimage).unwrap();
    let bytes = serde_json::to_vec(&json!({ "digest": digest, "preimage": preimage })).unwrap();
    EventEnvelope::from_json(&bytes).unwrap()
}

fn claimed(preimage: Value, digest: &str) -> Result<EventEnvelope, ChainError> {
    let bytes = serde_json::to_vec(&json!({ "digest": digest, "preimage": preimage })).unwrap();
    EventEnvelope::from_json(&bytes)
}

fn anchor(installation: InstallationEnrollmentRef, head: Digest, anchored_at: &str) -> TrustedHead {
    TrustedHead::new(
        installation,
        head,
        UnixNanoseconds::parse(anchored_at).unwrap(),
        Identifier::parse("trusted-store").unwrap(),
    )
}

fn genesis(installation: &InstallationEnrollmentRef, occurred_at: &str) -> EventEnvelope {
    decoded(preimage(
        installation,
        "0",
        json!({ "state": "genesis" }),
        occurred_at,
        "installation_enrolled",
        json!({ "enrollmentDigest": installation }),
    ))
}

fn proposed(
    installation: &InstallationEnrollmentRef,
    genesis: &EventEnvelope,
    sequence: &str,
    occurred_at: &str,
    warrant_digest: &Digest,
) -> EventEnvelope {
    decoded(preimage(
        installation,
        sequence,
        previous(genesis.digest()),
        occurred_at,
        "warrant_proposed",
        json!({ "warrantDigest": warrant_digest }),
    ))
}

fn map(events: impl IntoIterator<Item = EventEnvelope>) -> BTreeMap<Digest, EventEnvelope> {
    events
        .into_iter()
        .map(|event: EventEnvelope| (event.digest().clone(), event))
        .collect()
}

#[test]
fn event_registry_and_preimage_identity_are_frozen() {
    assert_eq!(EventType::ALL.len(), 26);
    assert_eq!(EventType::ALL.map(EventType::as_str), EVENT_TYPE_IDS);

    let installation = InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap());
    let event = genesis(&installation, "0");
    assert_eq!(event.preimage().schema_version(), EVENT_SCHEMA_VERSION);
    assert_eq!(event.preimage().sequence().get(), 0);
    assert!(matches!(
        event.preimage().previous_event(),
        PreviousEvent::Genesis
    ));
    assert_eq!(event.preimage().installation_digest(), &installation);
    assert_eq!(
        event.preimage().event_type(),
        EventType::InstallationEnrolled
    );
    assert_eq!(
        event.digest().as_str(),
        "sha256:72525322df625c89911ce4034836ad87f35acd413383ccce9c34e45c0803f460"
    );

    let canonical = String::from_utf8(event.canonical_preimage_bytes().unwrap()).unwrap();
    assert_eq!(
        canonical,
        concat!(
            "{\"eventType\":\"installation_enrolled\",",
            "\"installationDigest\":\"sha256:1111111111111111111111111111111111111111111111111111111111111111\",",
            "\"occurredAt\":\"0\",\"payload\":{\"enrollmentDigest\":",
            "\"sha256:1111111111111111111111111111111111111111111111111111111111111111\"},",
            "\"previousEvent\":{\"state\":\"genesis\"},",
            "\"schemaVersion\":\"jidoka.dev/events/v1\",\"sequence\":\"0\"}"
        )
    );
}

#[test]
fn valid_chain_is_walked_from_the_independent_anchor() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let second = proposed(
        &installation,
        &first,
        "1",
        "11",
        fixture.warrant().reference().digest(),
    );
    let expected = anchor(installation, second.digest().clone(), "99");

    let ordered = validate_chain(fixture.graph(), &map([second, first]), &expected).unwrap();
    assert_eq!(ordered.len(), 2);
    assert_eq!(ordered[0].preimage().sequence().get(), 0);
    assert_eq!(ordered[1].preimage().sequence().get(), 1);
}

#[test]
fn empty_and_non_enrollment_roots_fail_with_their_exact_categories() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let empty = BTreeMap::new();
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &empty,
            &anchor(installation.clone(), Digest::parse(THREE).unwrap(), "0"),
        )
        .unwrap_err(),
        ChainError::Empty
    );

    let invalid_root = decoded(preimage(
        &installation,
        "0",
        json!({ "state": "genesis" }),
        "0",
        "warrant_proposed",
        json!({ "warrantDigest": fixture.warrant().reference().digest() }),
    ));
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([invalid_root.clone()]),
            &anchor(installation, invalid_root.digest().clone(), "0"),
        )
        .unwrap_err(),
        ChainError::InvalidGenesis
    );
}

#[test]
fn anchor_time_is_metadata_and_cannot_replace_the_independent_head() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let events = map([first.clone()]);

    for anchored_at in ["0", "18446744073709551615"] {
        let trusted = anchor(installation.clone(), first.digest().clone(), anchored_at);
        assert_eq!(
            validate_chain(fixture.graph(), &events, &trusted)
                .unwrap()
                .len(),
            1
        );
    }

    let missing = anchor(
        installation,
        Digest::parse(THREE).unwrap(),
        "18446744073709551615",
    );
    assert_eq!(
        validate_chain(fixture.graph(), &events, &missing).unwrap_err(),
        ChainError::TruncatedTail
    );
}

#[test]
fn chain_corruption_categories_are_distinct() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let valid_second = proposed(
        &installation,
        &first,
        "1",
        "11",
        fixture.warrant().reference().digest(),
    );

    let wrong_installation = anchor(
        InstallationEnrollmentRef::from_digest(Digest::parse(THREE).unwrap()),
        valid_second.digest().clone(),
        "12",
    );
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), valid_second.clone()]),
            &wrong_installation,
        )
        .unwrap_err(),
        ChainError::HeadMismatch
    );

    let discontinuous = proposed(
        &installation,
        &first,
        "2",
        "11",
        fixture.warrant().reference().digest(),
    );
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), discontinuous.clone()]),
            &anchor(installation.clone(), discontinuous.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::SequenceDiscontinuity
    );

    let regressed = proposed(
        &installation,
        &first,
        "1",
        "9",
        fixture.warrant().reference().digest(),
    );
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), regressed.clone()]),
            &anchor(installation.clone(), regressed.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::TimeRegression
    );

    let invalid_previous = decoded(preimage(
        &installation,
        "1",
        json!({ "state": "genesis" }),
        "11",
        "warrant_proposed",
        json!({ "warrantDigest": fixture.warrant().reference().digest() }),
    ));
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([invalid_previous.clone()]),
            &anchor(
                installation.clone(),
                invalid_previous.digest().clone(),
                "12"
            ),
        )
        .unwrap_err(),
        ChainError::PreviousLinkMismatch
    );

    let gap = decoded(preimage(
        &installation,
        "1",
        json!({ "state": "previous", "digest": THREE }),
        "11",
        "warrant_proposed",
        json!({ "warrantDigest": fixture.warrant().reference().digest() }),
    ));
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), gap.clone()]),
            &anchor(installation.clone(), gap.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::Gap
    );

    let mut wrong_key = map([first.clone()]);
    let misplaced = wrong_key.remove(first.digest()).unwrap();
    wrong_key.insert(Digest::parse(THREE).unwrap(), misplaced);
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &wrong_key,
            &anchor(installation.clone(), first.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::DigestMismatch
    );
}

#[test]
fn same_installation_extraneous_history_is_a_fork() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let selected = proposed(
        &installation,
        &first,
        "1",
        "11",
        fixture.warrant().reference().digest(),
    );
    let sibling = decoded(preimage(
        &installation,
        "1",
        previous(first.digest()),
        "12",
        "warrant_expired",
        json!({ "warrantDigest": fixture.warrant().reference().digest() }),
    ));
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first, selected.clone(), sibling]),
            &anchor(installation, selected.digest().clone(), "13"),
        )
        .unwrap_err(),
        ChainError::Fork
    );
}

#[test]
fn extra_linear_tail_is_a_head_mismatch_not_a_fork() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let descendant = proposed(
        &installation,
        &first,
        "1",
        "11",
        fixture.warrant().reference().digest(),
    );
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), descendant]),
            &anchor(installation, first.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::HeadMismatch
    );
}

#[test]
fn unreachable_event_with_a_missing_predecessor_is_a_gap_not_a_fork() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let orphan = decoded(preimage(
        &installation,
        "1",
        json!({ "state": "previous", "digest": THREE }),
        "11",
        "warrant_proposed",
        json!({ "warrantDigest": fixture.warrant().reference().digest() }),
    ));
    assert_eq!(
        validate_chain(
            fixture.graph(),
            &map([first.clone(), orphan]),
            &anchor(installation, first.digest().clone(), "12"),
        )
        .unwrap_err(),
        ChainError::Gap
    );
}

#[test]
fn foreign_installation_history_does_not_create_a_local_fork() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "10");
    let foreign_installation =
        InstallationEnrollmentRef::from_digest(Digest::parse(THREE).unwrap());
    let foreign = genesis(&foreign_installation, "10");
    let ordered = validate_chain(
        fixture.graph(),
        &map([first.clone(), foreign]),
        &anchor(installation, first.digest().clone(), "11"),
    )
    .unwrap();
    assert_eq!(ordered.len(), 1);
}

#[test]
fn envelope_and_wire_errors_fail_closed() {
    let installation = InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap());
    let valid = preimage(
        &installation,
        "0",
        json!({ "state": "genesis" }),
        "0",
        "installation_enrolled",
        json!({ "enrollmentDigest": installation }),
    );
    assert_eq!(
        claimed(valid.clone(), TWO).unwrap_err(),
        ChainError::DigestMismatch
    );
    let invalid_envelope_digest = serde_json::to_vec(&json!({
        "digest": "sha256:NOT-A-DIGEST",
        "preimage": valid,
    }))
    .unwrap();
    assert_eq!(
        EventEnvelope::from_json(&invalid_envelope_digest).unwrap_err(),
        ChainError::DigestMismatch
    );

    let unknown = preimage(
        &InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap()),
        "0",
        json!({ "state": "genesis" }),
        "0",
        "installation_reimagined",
        json!({ "enrollmentDigest": ONE }),
    );
    assert_eq!(
        claimed(unknown, TWO).unwrap_err(),
        ChainError::UnknownEventType
    );

    let confused = preimage(
        &InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap()),
        "0",
        json!({ "state": "genesis" }),
        "0",
        "installation_enrolled",
        json!({ "warrantDigest": TWO }),
    );
    assert_eq!(
        claimed(confused, TWO).unwrap_err(),
        ChainError::TypeConfusedPayload
    );

    let duplicate = concat!(
        "{\"digest\":\"sha256:2222222222222222222222222222222222222222222222222222222222222222\",",
        "\"digest\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",",
        "\"preimage\":{}}"
    );
    assert!(matches!(
        EventEnvelope::from_json(duplicate.as_bytes()),
        Err(ChainError::DuplicateMember)
    ));

    let zero_previous = concat!(
        "{\"state\":\"previous\",\"digest\":",
        "\"sha256:0000000000000000000000000000000000000000000000000000000000000000\"}"
    );
    assert!(serde_json::from_str::<PreviousEvent>(zero_previous).is_err());
}

fn payloads() -> Vec<(&'static str, Value)> {
    vec![
        ("installation_enrolled", json!({ "enrollmentDigest": ONE })),
        ("warrant_proposed", json!({ "warrantDigest": ONE })),
        ("warrant_approved", json!({ "approvalDigest": ONE })),
        ("warrant_revoked", json!({ "revocationDigest": ONE })),
        ("warrant_expired", json!({ "warrantDigest": ONE })),
        (
            "effect_reserved",
            json!({ "bindingDigest": ONE, "leaseDigest": TWO }),
        ),
        (
            "effect_cancelled_before_start",
            json!({
                "bindingDigest": ONE,
                "leaseDigest": TWO,
                "reason": "request_disconnected"
            }),
        ),
        (
            "effect_started",
            json!({
                "bindingDigest": ONE,
                "leaseDigest": TWO,
                "preparedArtifactDigest": THREE,
                "sourceBeforeObservationDigest": ONE,
                "targetBeforeObservationDigest": TWO,
                "mutationMode": "conditional"
            }),
        ),
        (
            "artifact_prepared",
            json!({ "preparedArtifactDigest": ONE }),
        ),
        ("artifact_published", json!({ "evidenceDigest": ONE })),
        (
            "artifact_published_recovered",
            json!({ "recoveryAssessmentDigest": ONE }),
        ),
        (
            "effect_verified",
            json!({
                "receiptDigest": ONE,
                "deedDigest": TWO,
                "custodyRecordDigest": THREE
            }),
        ),
        ("effect_failed", json!({ "receiptDigest": ONE })),
        ("effect_indeterminate", json!({ "receiptDigest": ONE })),
        (
            "separation_warrant_proposed",
            json!({ "warrantDigest": ONE }),
        ),
        (
            "separation_warrant_approved",
            json!({ "approvalDigest": ONE }),
        ),
        (
            "separation_warrant_revoked",
            json!({ "revocationDigest": ONE }),
        ),
        (
            "separation_warrant_expired",
            json!({ "warrantDigest": ONE }),
        ),
        (
            "separation_reserved",
            json!({ "bindingDigest": ONE, "leaseDigest": TWO }),
        ),
        (
            "separation_cancelled_before_start",
            json!({
                "bindingDigest": ONE,
                "leaseDigest": TWO,
                "reason": "recovery_orphaned"
            }),
        ),
        (
            "separation_started",
            json!({
                "bindingDigest": ONE,
                "leaseDigest": TWO,
                "deedDigest": THREE,
                "activeBeforeObservationDigest": ONE,
                "quarantineBeforeObservationDigest": TWO,
                "mutationMode": "unconditional"
            }),
        ),
        (
            "separation_verified",
            json!({
                "mode": "live",
                "receiptDigest": ONE,
                "custodyRecordDigest": TWO
            }),
        ),
        (
            "separation_failed",
            json!({
                "mode": "live",
                "receiptDigest": ONE,
                "custodyRecordDigest": TWO
            }),
        ),
        (
            "separation_indeterminate",
            json!({ "mode": "live", "receiptDigest": ONE }),
        ),
        (
            "custody_absent",
            json!({ "receiptDigest": ONE, "custodyRecordDigest": TWO }),
        ),
        (
            "custody_disputed",
            json!({
                "terminalReceipt": { "protocol": "publication", "digest": ONE },
                "custodyRecordDigest": TWO
            }),
        ),
    ]
}

#[test]
fn all_twenty_six_payloads_have_exact_closed_field_sets() {
    let installation = InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap());
    let payloads = payloads();
    assert_eq!(payloads.len(), 26);

    for (index, (event_type, payload)) in payloads.into_iter().enumerate() {
        let event = decoded(preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            event_type,
            payload.clone(),
        ));
        assert_eq!(event.preimage().event_type().as_str(), event_type);
        let encoded = serde_json::to_value(&event).unwrap();
        assert_eq!(encoded["preimage"]["payload"], payload);

        let mut extra = payload.clone();
        extra
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), Value::Bool(true));
        let invalid = preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            event_type,
            extra,
        );
        assert_eq!(
            claimed(invalid, TWO).unwrap_err(),
            ChainError::TypeConfusedPayload,
            "payload row {index} accepted an unknown field"
        );

        let mut missing = payload;
        let first_key = missing.as_object().unwrap().keys().next().unwrap().clone();
        missing.as_object_mut().unwrap().remove(&first_key);
        let invalid = preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            event_type,
            missing,
        );
        assert_eq!(
            claimed(invalid, TWO).unwrap_err(),
            ChainError::TypeConfusedPayload,
            "payload row {index} accepted a missing `{first_key}`"
        );
    }
}

#[test]
fn recovered_separation_payload_variants_are_exact_and_closed() {
    let installation = InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap());
    for (event_type, custody) in [
        ("separation_verified", true),
        ("separation_failed", true),
        ("separation_indeterminate", false),
    ] {
        let mut payload = Map::from_iter([
            ("mode".to_owned(), json!("recovered")),
            ("recoveryAssessmentDigest".to_owned(), json!(THREE)),
            ("receiptDigest".to_owned(), json!(ONE)),
        ]);
        if custody {
            payload.insert("custodyRecordDigest".to_owned(), json!(TWO));
        }
        let payload = Value::Object(payload);
        let event = decoded(preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            event_type,
            payload.clone(),
        ));
        assert_eq!(
            serde_json::to_value(event).unwrap()["preimage"]["payload"],
            payload
        );
    }

    let illegal_reason = preimage(
        &installation,
        "0",
        json!({ "state": "genesis" }),
        "0",
        "effect_cancelled_before_start",
        json!({
            "bindingDigest": ONE,
            "leaseDigest": TWO,
            "reason": "budget_unavailable"
        }),
    );
    assert_eq!(
        claimed(illegal_reason, TWO).unwrap_err(),
        ChainError::TypeConfusedPayload
    );

    let live_with_recovery = preimage(
        &installation,
        "0",
        json!({ "state": "genesis" }),
        "0",
        "separation_verified",
        json!({
            "mode": "live",
            "recoveryAssessmentDigest": THREE,
            "receiptDigest": ONE,
            "custodyRecordDigest": TWO
        }),
    );
    assert_eq!(
        claimed(live_with_recovery, TWO).unwrap_err(),
        ChainError::TypeConfusedPayload
    );

    let recovered_without_assessment = preimage(
        &installation,
        "0",
        json!({ "state": "genesis" }),
        "0",
        "separation_indeterminate",
        json!({ "mode": "recovered", "receiptDigest": ONE }),
    );
    assert_eq!(
        claimed(recovered_without_assessment, TWO).unwrap_err(),
        ChainError::TypeConfusedPayload
    );
}

#[test]
fn cancellation_event_reason_is_exactly_the_six_value_subset() {
    let installation = InstallationEnrollmentRef::from_digest(Digest::parse(ONE).unwrap());
    for reason in [
        "request_disconnected",
        "reservation_deadline",
        "authorization_ineligible",
        "peer_identity_changed",
        "precondition_changed",
        "recovery_orphaned",
    ] {
        let event = decoded(preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            "effect_cancelled_before_start",
            json!({ "bindingDigest": ONE, "leaseDigest": TWO, "reason": reason }),
        ));
        assert_eq!(
            event.preimage().event_type(),
            EventType::EffectCancelledBeforeStart
        );
    }

    for reason in [
        "budget_unavailable",
        "separation_precondition_refused",
        "other",
    ] {
        let invalid = preimage(
            &installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            "separation_cancelled_before_start",
            json!({ "bindingDigest": ONE, "leaseDigest": TWO, "reason": reason }),
        );
        assert_eq!(
            claimed(invalid, THREE).unwrap_err(),
            ChainError::TypeConfusedPayload
        );
    }
}

#[test]
fn event_payload_references_resolve_to_the_exact_body_kind() {
    let fixture = support::authority();
    let installation = fixture.enrollment().reference().clone();
    let first = genesis(&installation, "0");
    let confused = proposed(
        &installation,
        &first,
        "1",
        "1",
        fixture.enrollment().reference().digest(),
    );
    assert!(matches!(
        validate_chain(
            fixture.graph(),
            &map([first, confused.clone()]),
            &anchor(installation, confused.digest().clone(), "2"),
        ),
        Err(ChainError::Body(BodyError::WrongTargetKind { .. }))
    ));
}

#[test]
fn immutable_store_starts_empty_and_commit_outcome_is_one_shot_data() {
    let store = ImmutableStore::empty();
    assert!(store.bodies().is_empty());
    assert!(store.events().is_empty());
    assert_eq!(store.head(), None);

    assert_eq!(
        serde_json::to_value(ExpectedHead::Empty).unwrap(),
        json!({ "state": "empty" })
    );
    assert_eq!(
        serde_json::to_value(ExpectedHead::Present(Digest::parse(ONE).unwrap())).unwrap(),
        json!({ "state": "present", "digest": ONE })
    );
    assert!(matches!(
        TrustedCommitOutcome::Unknown,
        TrustedCommitOutcome::Unknown
    ));
}
