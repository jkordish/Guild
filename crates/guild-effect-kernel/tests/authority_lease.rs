mod support;

use guild_effect_kernel::{
    authority::{
        AuthorityPolicy, BudgetAmount, BudgetCapacity, BudgetClaim, EffectKind,
        InstallationEnrollment, PublicationApproval, PublicationRevocation, PublicationWarrant,
        SeparationApproval, SeparationRevocation, SeparationWarrant,
    },
    body::{
        BodyBatch, BodyError, BodyGraph, ExpectedState, LocalFileObservation, OptionalValue,
        PresentExpectedState, ResourceDeedRef, SortedUnique, StaticArtifactPublishInput,
        StaticArtifactPublishInputRef, StaticArtifactPublishPrecondition,
        StaticArtifactSeparationInput, StaticArtifactSeparationPrecondition, XattrValueRef,
        validate_batch, validated_body,
    },
    canonical::{CanonicalError, canonical_bytes, canonical_digest},
    lease::{
        AdmissionError, BudgetClass, LeaseProjection, PreStartReason, PreStartResult,
        checked_next_generation, derive_effect_id, derive_resource_key,
    },
    scalar::{
        ArtifactName, ByteLength, Digest, Hex256, IdempotencyKey, Identifier, IncarnationId,
        LogicalAddress, RawDigest, ResourceKey, SafeUInt, U64Decimal, UnixNanoseconds,
    },
};
use proptest::prelude::*;

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

struct PublicationScenario {
    graph: BodyGraph,
    policy: guild_effect_kernel::body::ValidatedBody<AuthorityPolicy>,
    enrollment: guild_effect_kernel::body::ValidatedBody<InstallationEnrollment>,
    source: guild_effect_kernel::body::ValidatedBody<LocalFileObservation>,
    input: guild_effect_kernel::body::ValidatedBody<StaticArtifactPublishInput>,
    precondition: guild_effect_kernel::body::ValidatedBody<StaticArtifactPublishPrecondition>,
    warrant: guild_effect_kernel::body::ValidatedBody<PublicationWarrant>,
    approval: guild_effect_kernel::body::ValidatedBody<PublicationApproval>,
    resource_keys: [guild_effect_kernel::scalar::ResourceKey; 2],
    budget_key: Identifier,
}

fn publication_scenario(idempotency_key: &str) -> PublicationScenario {
    let budget_key = Identifier::parse("shared-budget").unwrap();
    let policy = validated_body(
        AuthorityPolicy::new(
            Identifier::parse("workstation-policy").unwrap(),
            U64Decimal::from_u64(0),
            SortedUnique::new(vec![Identifier::parse("proposer").unwrap()]).unwrap(),
            SortedUnique::new(vec![Identifier::parse("approver").unwrap()]).unwrap(),
            SortedUnique::new(vec![Identifier::parse("revoker").unwrap()]).unwrap(),
            SortedUnique::new(vec![Identifier::parse("host-probe").unwrap()]).unwrap(),
            true,
            SortedUnique::new(vec![BudgetCapacity::new(
                budget_key.clone(),
                SafeUInt::new(8).unwrap(),
            )])
            .unwrap(),
            SortedUnique::new(vec![BudgetCapacity::new(
                budget_key.clone(),
                SafeUInt::new(8).unwrap(),
            )])
            .unwrap(),
            Identifier::parse("trusted-clock").unwrap(),
            Identifier::parse("trusted-store").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let enrollment = validated_body(
        InstallationEnrollment::new(
            Identifier::parse("macbook-pro").unwrap(),
            IncarnationId::parse(ONE).unwrap(),
            policy.reference().clone(),
            UnixNanoseconds::parse("0").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let source_address = LogicalAddress::parse("local-file:///staging/app").unwrap();
    let target_address = LogicalAddress::parse("local-file:///active/app").unwrap();
    let source = validated_body(LocalFileObservation::present(
        source_address.clone(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("0").unwrap(),
        ArtifactName::parse("app").unwrap(),
        RawDigest::parse(TWO).unwrap(),
        ByteLength::from_u64(42),
        IncarnationId::parse(ONE).unwrap(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            source.reference().clone(),
            target_address.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let precondition = validated_body(StaticArtifactPublishPrecondition::new(
        target_address.clone(),
        ExpectedState::absent(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let mut resource_keys = [
        derive_resource_key(&source_address).unwrap(),
        derive_resource_key(&target_address).unwrap(),
    ];
    resource_keys.sort();
    let warrant = publication_warrant(
        &policy,
        &enrollment,
        input.reference().clone(),
        precondition.reference().clone(),
        IdempotencyKey::parse(idempotency_key).unwrap(),
        resource_keys.clone(),
        &budget_key,
        'a',
    );
    let approval = validated_body(
        PublicationApproval::new(
            &warrant,
            &policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let graph = validate_batch(
        &BodyGraph::empty(),
        BodyBatch::new(vec![
            policy.clone().into_stored(),
            enrollment.clone().into_stored(),
            source.clone().into_stored(),
            input.clone().into_stored(),
            precondition.clone().into_stored(),
            warrant.clone().into_stored(),
            approval.clone().into_stored(),
        ])
        .unwrap(),
    )
    .unwrap();
    PublicationScenario {
        graph,
        policy,
        enrollment,
        source,
        input,
        precondition,
        warrant,
        approval,
        resource_keys,
        budget_key,
    }
}

fn graph_for_publication(
    scenario: &PublicationScenario,
    enrollment: &guild_effect_kernel::body::ValidatedBody<InstallationEnrollment>,
    warrant: &guild_effect_kernel::body::ValidatedBody<PublicationWarrant>,
    approval: &guild_effect_kernel::body::ValidatedBody<PublicationApproval>,
    revocation: Option<&guild_effect_kernel::body::ValidatedBody<PublicationRevocation>>,
) -> BodyGraph {
    let mut bodies = vec![
        scenario.policy.clone().into_stored(),
        enrollment.clone().into_stored(),
        scenario.source.clone().into_stored(),
        scenario.input.clone().into_stored(),
        scenario.precondition.clone().into_stored(),
        warrant.clone().into_stored(),
        approval.clone().into_stored(),
    ];
    if let Some(revocation) = revocation {
        bodies.push(revocation.clone().into_stored());
    }
    validate_batch(&BodyGraph::empty(), BodyBatch::new(bodies).unwrap()).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn publication_warrant(
    policy: &guild_effect_kernel::body::ValidatedBody<AuthorityPolicy>,
    enrollment: &guild_effect_kernel::body::ValidatedBody<InstallationEnrollment>,
    input: StaticArtifactPublishInputRef,
    precondition: guild_effect_kernel::body::StaticArtifactPublishPreconditionRef,
    idempotency_key: IdempotencyKey,
    resource_keys: [guild_effect_kernel::scalar::ResourceKey; 2],
    budget_key: &Identifier,
    nonce: char,
) -> guild_effect_kernel::body::ValidatedBody<PublicationWarrant> {
    validated_body(
        PublicationWarrant::new(
            enrollment,
            policy,
            Identifier::parse("proposer").unwrap(),
            input,
            precondition,
            idempotency_key,
            resource_keys,
            BudgetClaim::new(
                budget_key.clone(),
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            BudgetClaim::new(
                budget_key.clone(),
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            UnixNanoseconds::parse("0").unwrap(),
            UnixNanoseconds::parse("20000000000").unwrap(),
            Hex256::parse(&nonce.to_string().repeat(64)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn projection_values(
    projection: &LeaseProjection,
    scenario: &PublicationScenario,
) -> (
    U64Decimal,
    U64Decimal,
    u64,
    u64,
    Option<U64Decimal>,
    Option<U64Decimal>,
    bool,
    bool,
) {
    let reservation = projection
        .budget_balance(BudgetClass::Reservation, &scenario.budget_key)
        .unwrap();
    let start = projection
        .budget_balance(BudgetClass::Start, &scenario.budget_key)
        .unwrap();
    (
        projection.head_sequence(),
        projection.terminal_sequence_reserve(),
        reservation.available(),
        start.available(),
        projection.resource_fence(&scenario.resource_keys[0]),
        projection.resource_fence(&scenario.resource_keys[1]),
        projection
            .resource_lock(&scenario.resource_keys[0])
            .is_some(),
        projection
            .resource_lock(&scenario.resource_keys[1])
            .is_some(),
    )
}

fn assert_owned_decoder_extracts_edge(
    kind: &str,
    body: serde_json::Value,
    expected_target: &Digest,
) {
    let envelope = serde_json::json!({ "body": body, "kind": kind });
    let bytes = canonical_bytes(&envelope).unwrap();
    let digest = canonical_digest(&envelope).unwrap();
    assert_eq!(
        BodyGraph::from_canonical_entries(std::collections::BTreeMap::from([(digest, bytes)]))
            .unwrap_err(),
        BodyError::MissingReference {
            source: canonical_digest(&envelope).unwrap(),
            target: expected_target.clone(),
        },
        "{kind} must be strictly decoded and expose its typed edge",
    );
}

#[test]
fn first_policy_requires_distinct_enrolled_authority() {
    let fixtures = support::authority();
    assert!(fixtures.approve_as(fixtures.proposer_id()).is_err());
    assert!(fixtures.approve_as(fixtures.approver_id()).is_ok());
}

#[test]
fn lease_is_live_strictly_before_but_not_at_expiry() {
    let lease = support::publication_lease_at("1000000000");
    assert!(!lease.is_live_at(UnixNanoseconds::parse("999999999").unwrap()));
    assert!(lease.is_live_at(UnixNanoseconds::parse("1000000000").unwrap()));
    assert!(lease.is_live_at(UnixNanoseconds::parse("5999999999").unwrap()));
    assert!(!lease.is_live_at(UnixNanoseconds::parse("6000000000").unwrap()));
}

#[test]
fn policy_budget_and_effect_wire_values_are_closed() {
    assert!(BudgetAmount::new(SafeUInt::new(0).unwrap()).is_err());
    assert_eq!(
        canonical_bytes(&EffectKind::StaticArtifactPublish).unwrap(),
        br#""static_artifact_publish""#
    );
    assert_eq!(
        canonical_bytes(&EffectKind::StaticArtifactSeparation).unwrap(),
        br#""static_artifact_separation""#
    );
    assert!(serde_json::from_str::<EffectKind>(r#""future_effect""#).is_err());

    let scenario = publication_scenario("wire-shape-test-0001");
    let mut value = serde_json::to_value(scenario.policy.payload()).unwrap();
    value["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AuthorityPolicy>(value).is_err());
}

#[test]
fn policy_is_generation_zero_distinct_and_role_sets_are_nonempty() {
    let scenario = publication_scenario("policy-boundary-0001");
    let base = scenario.policy.payload();
    let empty = SortedUnique::new(vec![]).unwrap();
    assert!(
        AuthorityPolicy::new(
            base.policy_id().clone(),
            U64Decimal::from_u64(1),
            base.proposer_ids().clone(),
            base.approver_ids().clone(),
            base.revoker_ids().clone(),
            base.witness_ids().clone(),
            true,
            base.reservation_budgets().clone(),
            base.start_budgets().clone(),
            base.trusted_clock_id().clone(),
            base.trusted_store_id().clone(),
        )
        .is_err()
    );
    assert!(
        AuthorityPolicy::new(
            base.policy_id().clone(),
            U64Decimal::from_u64(0),
            empty,
            base.approver_ids().clone(),
            base.revoker_ids().clone(),
            base.witness_ids().clone(),
            true,
            base.reservation_budgets().clone(),
            base.start_budgets().clone(),
            base.trusted_clock_id().clone(),
            base.trusted_store_id().clone(),
        )
        .is_err()
    );
    assert!(
        AuthorityPolicy::new(
            base.policy_id().clone(),
            U64Decimal::from_u64(0),
            base.proposer_ids().clone(),
            base.approver_ids().clone(),
            base.revoker_ids().clone(),
            base.witness_ids().clone(),
            false,
            base.reservation_budgets().clone(),
            base.start_budgets().clone(),
            base.trusted_clock_id().clone(),
            base.trusted_store_id().clone(),
        )
        .is_err()
    );
}

#[test]
fn policy_rejects_duplicate_budget_keys_even_when_capacities_differ() {
    let scenario = publication_scenario("duplicate-budget-key-0001");
    let base = scenario.policy.payload();
    let duplicate_keys = SortedUnique::new(vec![
        BudgetCapacity::new(
            Identifier::parse("duplicate").unwrap(),
            SafeUInt::new(1).unwrap(),
        ),
        BudgetCapacity::new(
            Identifier::parse("duplicate").unwrap(),
            SafeUInt::new(2).unwrap(),
        ),
    ])
    .unwrap();
    assert!(
        AuthorityPolicy::new(
            base.policy_id().clone(),
            U64Decimal::from_u64(0),
            base.proposer_ids().clone(),
            base.approver_ids().clone(),
            base.revoker_ids().clone(),
            base.witness_ids().clone(),
            true,
            duplicate_keys,
            base.start_budgets().clone(),
            base.trusted_clock_id().clone(),
            base.trusted_store_id().clone(),
        )
        .is_err()
    );
}

#[test]
fn resource_and_effect_ids_match_exact_jcs_preimages() {
    assert_eq!(
        derive_resource_key(&LogicalAddress::parse("local-file:///active/app").unwrap())
            .unwrap()
            .as_str(),
        "sha256:103db6f1e3a404d66a2de716e22f65a3e4ecc8d0072036413432f61ae96fc21e"
    );
    let resources = SortedUnique::new(vec![
        ResourceKey::parse(THREE).unwrap(),
        ResourceKey::parse(
            "sha256:4444444444444444444444444444444444444444444444444444444444444444",
        )
        .unwrap(),
    ])
    .unwrap();
    assert_eq!(
        derive_effect_id(
            &Digest::parse(ONE).unwrap(),
            &Digest::parse(TWO).unwrap(),
            EffectKind::StaticArtifactPublish,
            &resources,
            &Digest::parse(
                "sha256:5555555555555555555555555555555555555555555555555555555555555555",
            )
            .unwrap(),
            &Digest::parse(
                "sha256:6666666666666666666666666666666666666666666666666666666666666666",
            )
            .unwrap(),
        )
        .unwrap()
        .as_str(),
        "sha256:3e8ab969dd55480a35df8e086e17261e41d958e52fc8e815acdaab8991a2b19f"
    );
}

#[test]
fn authority_and_lease_bodies_strictly_replay_with_exact_edges() {
    let scenario = publication_scenario("graph-replay-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let reservation = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    let revocation = validated_body(
        PublicationRevocation::new(
            &scenario.warrant,
            &scenario.approval,
            &scenario.policy,
            Identifier::parse("revoker").unwrap(),
            UnixNanoseconds::parse("2000000000").unwrap(),
            Identifier::parse("operator-stop").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let bodies = vec![
        scenario.policy.clone().into_stored(),
        scenario.enrollment.clone().into_stored(),
        scenario.source.clone().into_stored(),
        scenario.input.clone().into_stored(),
        scenario.precondition.clone().into_stored(),
        scenario.warrant.clone().into_stored(),
        scenario.approval.clone().into_stored(),
        revocation.into_stored(),
        reservation.binding().clone().into_stored(),
        reservation.lease().clone().into_stored(),
    ];
    let graph = validate_batch(&BodyGraph::empty(), BodyBatch::new(bodies).unwrap()).unwrap();
    assert_eq!(graph.len(), 10);
}

#[test]
fn replay_rejects_hostile_binding_members_and_binding_identity_lies() {
    let scenario = publication_scenario("binding-replay-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let reservation = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();

    let mut hostile_body = serde_json::to_value(reservation.binding().payload()).unwrap();
    hostile_body["extra"] = serde_json::json!(true);
    let hostile = serde_json::json!({
        "body": hostile_body,
        "kind": "idempotency-binding/v1"
    });
    let hostile_bytes = canonical_bytes(&hostile).unwrap();
    let hostile_key = guild_effect_kernel::canonical::canonical_digest(&hostile).unwrap();
    assert!(matches!(
        BodyGraph::from_canonical_entries(std::collections::BTreeMap::from([(
            hostile_key,
            hostile_bytes
        )])),
        Err(BodyError::Canonical(_))
    ));

    let mut lied_body = serde_json::to_value(reservation.binding().payload()).unwrap();
    lied_body["effectId"] = serde_json::json!(THREE);
    let lied = serde_json::json!({
        "body": lied_body,
        "kind": "idempotency-binding/v1"
    });
    let lied_bytes = canonical_bytes(&lied).unwrap();
    let lied_key = guild_effect_kernel::canonical::canonical_digest(&lied).unwrap();
    let mut entries = std::collections::BTreeMap::new();
    for body in [
        scenario.policy.clone().into_stored(),
        scenario.enrollment.clone().into_stored(),
        scenario.source.clone().into_stored(),
        scenario.input.clone().into_stored(),
        scenario.precondition.clone().into_stored(),
        scenario.warrant.clone().into_stored(),
        scenario.approval.clone().into_stored(),
    ] {
        entries.insert(body.digest().clone(), body.canonical_bytes().to_vec());
    }
    entries.insert(lied_key, lied_bytes);
    assert!(matches!(
        BodyGraph::from_canonical_entries(entries),
        Err(BodyError::Local(_))
    ));
}

#[test]
fn graph_rejects_warrant_policy_identity_and_resource_key_lies() {
    let scenario = publication_scenario("graph-cross-body-0001");

    let other_policy = validated_body(
        AuthorityPolicy::new(
            Identifier::parse("other-policy").unwrap(),
            U64Decimal::from_u64(0),
            scenario.policy.payload().proposer_ids().clone(),
            scenario.policy.payload().approver_ids().clone(),
            scenario.policy.payload().revoker_ids().clone(),
            scenario.policy.payload().witness_ids().clone(),
            true,
            scenario.policy.payload().reservation_budgets().clone(),
            scenario.policy.payload().start_budgets().clone(),
            Identifier::parse("trusted-clock").unwrap(),
            Identifier::parse("trusted-store").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut policy_lie = serde_json::to_value(scenario.warrant.payload()).unwrap();
    policy_lie["policyDigest"] = serde_json::json!(other_policy.reference().digest().as_str());
    let policy_lie: PublicationWarrant = serde_json::from_value(policy_lie).unwrap();
    let policy_lie = validated_body(policy_lie).unwrap();
    let bodies = vec![
        scenario.policy.clone().into_stored(),
        other_policy.into_stored(),
        scenario.enrollment.clone().into_stored(),
        scenario.source.clone().into_stored(),
        scenario.input.clone().into_stored(),
        scenario.precondition.clone().into_stored(),
        policy_lie.into_stored(),
    ];
    assert!(matches!(
        validate_batch(&BodyGraph::empty(), BodyBatch::new(bodies).unwrap()),
        Err(BodyError::Local(_))
    ));

    let mut key_lie = serde_json::to_value(scenario.warrant.payload()).unwrap();
    let false_key =
        derive_resource_key(&LogicalAddress::parse("local-file:///unrelated/address").unwrap())
            .unwrap();
    let mut keys = [scenario.resource_keys[0].clone(), false_key];
    keys.sort();
    key_lie["resourceKeys"] = serde_json::to_value(keys).unwrap();
    let key_lie =
        validated_body(serde_json::from_value::<PublicationWarrant>(key_lie).unwrap()).unwrap();
    let bodies = vec![
        scenario.policy.clone().into_stored(),
        scenario.enrollment.clone().into_stored(),
        scenario.source.clone().into_stored(),
        scenario.input.clone().into_stored(),
        scenario.precondition.clone().into_stored(),
        key_lie.into_stored(),
    ];
    assert!(matches!(
        validate_batch(&BodyGraph::empty(), BodyBatch::new(bodies).unwrap()),
        Err(BodyError::Local(_))
    ));
}

#[test]
fn publication_warrant_replay_rejects_the_separation_effect_literal() {
    let scenario = publication_scenario("effect-literal-test-0001");
    let mut lie = serde_json::to_value(scenario.warrant.payload()).unwrap();
    lie["effectKind"] = serde_json::json!("static_artifact_separation");
    let lie = serde_json::from_value::<PublicationWarrant>(lie).unwrap();
    assert!(matches!(validated_body(lie), Err(BodyError::Local(_))));
}

#[test]
fn approval_interval_is_closed_at_issue_and_open_at_expiry() {
    let scenario = publication_scenario("approval-time-test-0001");
    assert!(
        PublicationApproval::new(
            &scenario.warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            scenario.warrant.payload().issued_at(),
        )
        .is_ok()
    );
    assert!(
        PublicationApproval::new(
            &scenario.warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            scenario.warrant.payload().expires_at(),
        )
        .is_err()
    );
}

#[test]
fn graph_rejects_unenrolled_or_same_principal_approval() {
    let scenario = publication_scenario("approval-replay-test-0001");
    for principal in ["proposer", "outsider"] {
        let mut lie = serde_json::to_value(scenario.approval.payload()).unwrap();
        lie["approverId"] = serde_json::json!(principal);
        let lie =
            validated_body(serde_json::from_value::<PublicationApproval>(lie).unwrap()).unwrap();
        let bodies = vec![
            scenario.policy.clone().into_stored(),
            scenario.enrollment.clone().into_stored(),
            scenario.source.clone().into_stored(),
            scenario.input.clone().into_stored(),
            scenario.precondition.clone().into_stored(),
            scenario.warrant.clone().into_stored(),
            lie.into_stored(),
        ];
        assert!(matches!(
            validate_batch(&BodyGraph::empty(), BodyBatch::new(bodies).unwrap()),
            Err(BodyError::Local(_))
        ));
    }
}

#[test]
fn revocation_is_effective_at_its_timestamp_and_requires_approval_order() {
    let scenario = publication_scenario("revocation-boundary-0001");
    let revocation = validated_body(
        PublicationRevocation::new(
            &scenario.warrant,
            &scenario.approval,
            &scenario.policy,
            Identifier::parse("revoker").unwrap(),
            UnixNanoseconds::parse("2000000000").unwrap(),
            Identifier::parse("operator-stop").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let revoked_graph = graph_for_publication(
        &scenario,
        &scenario.enrollment,
        &scenario.warrant,
        &scenario.approval,
        Some(&revocation),
    );
    let mut before =
        LeaseProjection::new(&revoked_graph, &scenario.enrollment, &scenario.policy).unwrap();
    assert!(
        before
            .reserve_publication(
                &revoked_graph,
                &scenario.policy,
                &scenario.warrant,
                &scenario.approval,
                UnixNanoseconds::parse("1999999999").unwrap(),
            )
            .is_ok()
    );
    let mut at =
        LeaseProjection::new(&revoked_graph, &scenario.enrollment, &scenario.policy).unwrap();
    assert_eq!(
        at.reserve_publication(
            &revoked_graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("2000000000").unwrap(),
        )
        .unwrap_err(),
        AdmissionError::WarrantRevoked
    );

    assert!(
        PublicationRevocation::new(
            &scenario.warrant,
            &scenario.approval,
            &scenario.policy,
            Identifier::parse("revoker").unwrap(),
            UnixNanoseconds::parse("499999999").unwrap(),
            Identifier::parse("too-early").unwrap(),
        )
        .is_err()
    );
}

#[test]
fn publication_start_refuses_an_effective_post_reservation_revocation() {
    let scenario = publication_scenario("start-revocation-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    projection
        .mark_prepared(
            material.effect_id(),
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();

    let revocation = validated_body(
        PublicationRevocation::new(
            &scenario.warrant,
            &scenario.approval,
            &scenario.policy,
            Identifier::parse("revoker").unwrap(),
            UnixNanoseconds::parse("2000000000").unwrap(),
            Identifier::parse("operator-stop").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let revoked_graph = graph_for_publication(
        &scenario,
        &scenario.enrollment,
        &scenario.warrant,
        &scenario.approval,
        Some(&revocation),
    );
    let before = projection_values(&projection, &scenario);

    assert_eq!(
        projection
            .start(
                &revoked_graph,
                material.effect_id(),
                UnixNanoseconds::parse("2000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::WarrantRevoked,
    );
    assert_eq!(projection_values(&projection, &scenario), before);
}

#[test]
fn expiry_equality_refuses_before_any_reservation_state() {
    let scenario = publication_scenario("expiry-boundary-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let before = projection_values(&projection, &scenario);
    assert_eq!(
        projection
            .reserve_publication(
                &scenario.graph,
                &scenario.policy,
                &scenario.warrant,
                &scenario.approval,
                UnixNanoseconds::parse("20000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::WarrantExpired
    );
    assert_eq!(projection_values(&projection, &scenario), before);
}

#[test]
fn reservation_rejects_warrant_keys_unrelated_to_the_publish_input() {
    let scenario = publication_scenario("forged-resource-keys-0001");
    let mut unrelated = [
        derive_resource_key(&LogicalAddress::parse("local-file:///unrelated/x").unwrap()).unwrap(),
        derive_resource_key(&LogicalAddress::parse("local-file:///unrelated/y").unwrap()).unwrap(),
    ];
    unrelated.sort();
    let warrant = publication_warrant(
        &scenario.policy,
        &scenario.enrollment,
        scenario.input.reference().clone(),
        scenario.precondition.reference().clone(),
        IdempotencyKey::parse("forged-resource-keys-0002").unwrap(),
        unrelated,
        &scenario.budget_key,
        'e',
    );
    let approval = validated_body(
        PublicationApproval::new(
            &warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![
                scenario.policy.clone().into_stored(),
                scenario.enrollment.clone().into_stored(),
                scenario.source.clone().into_stored(),
                scenario.input.clone().into_stored(),
                scenario.precondition.clone().into_stored(),
                warrant.clone().into_stored(),
                approval.clone().into_stored(),
            ])
            .unwrap(),
        )
        .unwrap_err(),
        BodyError::Local(
            "publication warrant resource keys are not its exact source and target keys".to_owned(),
        ),
    );
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    assert_eq!(
        projection
            .reserve_publication(
                &scenario.graph,
                &scenario.policy,
                &warrant,
                &approval,
                UnixNanoseconds::parse("1000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::AuthorityRefused,
    );
}

#[test]
fn reservation_rejects_deserialized_unenrolled_proposer() {
    let scenario = publication_scenario("forged-proposer-test-0001");
    let mut body = serde_json::to_value(scenario.warrant.payload()).unwrap();
    body["proposerId"] = serde_json::json!("outsider");
    let warrant =
        validated_body(serde_json::from_value::<PublicationWarrant>(body).unwrap()).unwrap();
    let approval = validated_body(
        PublicationApproval::new(
            &warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        validate_batch(
            &BodyGraph::empty(),
            BodyBatch::new(vec![
                scenario.policy.clone().into_stored(),
                scenario.enrollment.clone().into_stored(),
                scenario.source.clone().into_stored(),
                scenario.input.clone().into_stored(),
                scenario.precondition.clone().into_stored(),
                warrant.clone().into_stored(),
                approval.clone().into_stored(),
            ])
            .unwrap(),
        )
        .unwrap_err(),
        BodyError::Local(
            "publication warrant does not match enrollment and immutable policy".to_owned(),
        ),
    );
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    assert_eq!(
        projection
            .reserve_publication(
                &scenario.graph,
                &scenario.policy,
                &warrant,
                &approval,
                UnixNanoseconds::parse("1000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::AuthorityRefused,
    );
}

#[test]
fn reservation_rejects_a_different_enrollment_under_the_same_policy() {
    let scenario = publication_scenario("wrong-enrollment-test-0001");
    let other_enrollment = validated_body(
        InstallationEnrollment::new(
            Identifier::parse("other-installation").unwrap(),
            IncarnationId::parse(THREE).unwrap(),
            scenario.policy.reference().clone(),
            UnixNanoseconds::parse("0").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let warrant = publication_warrant(
        &scenario.policy,
        &other_enrollment,
        scenario.input.reference().clone(),
        scenario.precondition.reference().clone(),
        IdempotencyKey::parse("wrong-enrollment-test-0002").unwrap(),
        scenario.resource_keys.clone(),
        &scenario.budget_key,
        'f',
    );
    let approval = validated_body(
        PublicationApproval::new(
            &warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let other_graph =
        graph_for_publication(&scenario, &other_enrollment, &warrant, &approval, None);
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    assert_eq!(
        projection
            .reserve_publication(
                &other_graph,
                &scenario.policy,
                &warrant,
                &approval,
                UnixNanoseconds::parse("1000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::AuthorityRefused,
    );
}

#[test]
fn publication_start_requires_preparation_and_live_warrant() {
    let scenario = publication_scenario("start-boundary-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    assert_eq!(
        projection
            .start(
                &scenario.graph,
                material.effect_id(),
                UnixNanoseconds::parse("2000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::PreconditionRefused,
    );
    assert_eq!(
        projection
            .mark_prepared(
                material.effect_id(),
                UnixNanoseconds::parse("6000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::WarrantExpired,
    );
}

#[test]
fn publication_start_refuses_after_warrant_expiry_even_while_lease_is_live() {
    let scenario = publication_scenario("short-warrant-test-0001");
    let mut body = serde_json::to_value(scenario.warrant.payload()).unwrap();
    body["expiresAt"] = serde_json::json!("3000000000");
    body["nonce"] = serde_json::json!("9".repeat(64));
    let warrant =
        validated_body(serde_json::from_value::<PublicationWarrant>(body).unwrap()).unwrap();
    let approval = validated_body(
        PublicationApproval::new(
            &warrant,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let short_graph =
        graph_for_publication(&scenario, &scenario.enrollment, &warrant, &approval, None);
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &short_graph,
            &scenario.policy,
            &warrant,
            &approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    projection
        .mark_prepared(
            material.effect_id(),
            UnixNanoseconds::parse("2000000000").unwrap(),
        )
        .unwrap();
    assert_eq!(
        projection
            .start(
                &short_graph,
                material.effect_id(),
                UnixNanoseconds::parse("4000000000").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::WarrantExpired,
    );
}

proptest! {
    #[test]
    fn duplicate_reservation_is_a_map_noop_and_changed_identity_conflicts(
        suffix in "[A-Za-z0-9]{16,40}"
    ) {
        let scenario = publication_scenario(&suffix);
        let mut projection = LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
        let first = projection.reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        ).unwrap();
        let after_first = projection_values(&projection, &scenario);
        let second = projection.reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000001").unwrap(),
        ).unwrap();
        prop_assert!(second.delta().is_empty());
        prop_assert_eq!(first.binding().reference(), second.binding().reference());
        prop_assert_eq!(first.lease().reference(), second.lease().reference());
        prop_assert_eq!(projection_values(&projection, &scenario), after_first);

        let changed = publication_warrant(
            &scenario.policy,
            &scenario.enrollment,
            scenario.input.reference().clone(),
            scenario.precondition.reference().clone(),
            scenario.warrant.payload().idempotency_key().clone(),
            scenario.resource_keys.clone(),
            &scenario.budget_key,
            'b',
        );
        let changed_approval = validated_body(PublicationApproval::new(
            &changed,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        ).unwrap()).unwrap();
        let changed_graph = graph_for_publication(
            &scenario,
            &scenario.enrollment,
            &changed,
            &changed_approval,
            None,
        );
        prop_assert_eq!(
            projection.reserve_publication(
                &changed_graph,
                &scenario.policy,
                &changed,
                &changed_approval,
                UnixNanoseconds::parse("1000000001").unwrap(),
            ).unwrap_err(),
            AdmissionError::IdempotencyConflict,
        );
        prop_assert_eq!(projection_values(&projection, &scenario), after_first);
    }
}

#[test]
fn namespaced_budgets_hold_consume_and_never_replenish_on_terminal() {
    let scenario = publication_scenario("budget-state-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    for class in [BudgetClass::Reservation, BudgetClass::Start] {
        let held = projection
            .budget_balance(class, &scenario.budget_key)
            .unwrap();
        assert_eq!((held.available(), held.held(), held.consumed()), (7, 1, 0));
    }
    projection
        .mark_prepared(
            material.effect_id(),
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();
    projection
        .start(
            &scenario.graph,
            material.effect_id(),
            UnixNanoseconds::parse("2000000000").unwrap(),
        )
        .unwrap();
    for class in [BudgetClass::Reservation, BudgetClass::Start] {
        let consumed = projection
            .budget_balance(class, &scenario.budget_key)
            .unwrap();
        assert_eq!(
            (consumed.available(), consumed.held(), consumed.consumed()),
            (7, 0, 1)
        );
    }
    let before_duplicate = projection_values(&projection, &scenario);
    assert!(
        projection
            .start(
                &scenario.graph,
                material.effect_id(),
                UnixNanoseconds::parse("2000000001").unwrap(),
            )
            .is_err()
    );
    assert_eq!(projection_values(&projection, &scenario), before_duplicate);
    projection
        .terminalize(
            material.effect_id(),
            2,
            UnixNanoseconds::parse("3000000000").unwrap(),
        )
        .unwrap();
    for class in [BudgetClass::Reservation, BudgetClass::Start] {
        let terminal = projection
            .budget_balance(class, &scenario.budget_key)
            .unwrap();
        assert_eq!(
            (terminal.available(), terminal.held(), terminal.consumed()),
            (7, 0, 1)
        );
    }
}

#[test]
fn two_key_lock_conflict_is_atomic_and_locks_survive_start() {
    let scenario = publication_scenario("resource-lock-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let first = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    projection
        .mark_prepared(
            first.effect_id(),
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();
    projection
        .start(
            &scenario.graph,
            first.effect_id(),
            UnixNanoseconds::parse("2000000000").unwrap(),
        )
        .unwrap();
    for key in &scenario.resource_keys {
        assert_eq!(
            projection.resource_lock(key).unwrap().effect_id(),
            first.effect_id()
        );
    }

    let competing_target = LogicalAddress::parse("local-file:///active/other").unwrap();
    let competing_input = validated_body(
        StaticArtifactPublishInput::new(
            ArtifactName::parse("app").unwrap(),
            scenario.source.reference().clone(),
            competing_target.clone(),
        )
        .unwrap(),
    )
    .unwrap();
    let competing_precondition = validated_body(StaticArtifactPublishPrecondition::new(
        competing_target.clone(),
        ExpectedState::absent(),
        OptionalValue::absent(),
    ))
    .unwrap();
    let third = derive_resource_key(&competing_target).unwrap();
    let mut overlapping = [
        derive_resource_key(scenario.source.payload().logical_address()).unwrap(),
        third.clone(),
    ];
    overlapping.sort();
    let competing = publication_warrant(
        &scenario.policy,
        &scenario.enrollment,
        competing_input.reference().clone(),
        competing_precondition.reference().clone(),
        IdempotencyKey::parse("resource-lock-test-0002").unwrap(),
        overlapping,
        &scenario.budget_key,
        'c',
    );
    let competing_approval = validated_body(
        PublicationApproval::new(
            &competing,
            &scenario.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let competing_graph = validate_batch(
        &BodyGraph::empty(),
        BodyBatch::new(vec![
            scenario.policy.clone().into_stored(),
            scenario.enrollment.clone().into_stored(),
            scenario.source.clone().into_stored(),
            competing_input.into_stored(),
            competing_precondition.into_stored(),
            competing.clone().into_stored(),
            competing_approval.clone().into_stored(),
        ])
        .unwrap(),
    )
    .unwrap();
    let before = projection_values(&projection, &scenario);
    assert_eq!(
        projection
            .reserve_publication(
                &competing_graph,
                &scenario.policy,
                &competing,
                &competing_approval,
                UnixNanoseconds::parse("2000000001").unwrap(),
            )
            .unwrap_err(),
        AdmissionError::ResourceConflict
    );
    assert_eq!(projection_values(&projection, &scenario), before);
    assert!(projection.resource_fence(&third).is_none());
}

#[test]
fn cancellation_releases_holds_and_locks_but_retains_binding_spend_and_fences() {
    let scenario = publication_scenario("cancellation-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    let outcome = projection
        .cancel(
            material.effect_id(),
            PreStartReason::RequestDisconnected,
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();
    assert_eq!(outcome.result(), PreStartResult::NotAttempted);
    assert_eq!(outcome.reason(), PreStartReason::RequestDisconnected);
    assert!(outcome.binding_digest().value().is_some());
    assert_eq!(
        projection.binding_effect(scenario.warrant.payload().idempotency_key()),
        Some(material.effect_id())
    );
    assert!(projection.is_warrant_spent(scenario.warrant.reference().digest()));
    for key in &scenario.resource_keys {
        assert!(projection.resource_lock(key).is_none());
        assert_eq!(projection.resource_fence(key).unwrap().get(), 1);
    }
    for class in [BudgetClass::Reservation, BudgetClass::Start] {
        let balance = projection
            .budget_balance(class, &scenario.budget_key)
            .unwrap();
        assert_eq!(
            (balance.available(), balance.held(), balance.consumed()),
            (8, 0, 0)
        );
    }
    assert!(
        projection
            .cancel(
                material.effect_id(),
                PreStartReason::BudgetUnavailable,
                UnixNanoseconds::parse("1500000001").unwrap(),
            )
            .is_err()
    );

    let before_repeat = projection_values(&projection, &scenario);
    let existing = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1500000002").unwrap(),
        )
        .unwrap();
    assert!(existing.delta().is_empty());
    assert_eq!(projection_values(&projection, &scenario), before_repeat);
}

#[test]
fn cancellation_deadline_is_not_legal_before_lease_expiry() {
    let scenario = publication_scenario("deadline-cancel-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    assert!(
        projection
            .cancel(
                material.effect_id(),
                PreStartReason::ReservationDeadline,
                UnixNanoseconds::parse("5999999999").unwrap(),
            )
            .is_err()
    );
    assert!(
        projection
            .cancel(
                material.effect_id(),
                PreStartReason::ReservationDeadline,
                UnixNanoseconds::parse("6000000000").unwrap(),
            )
            .is_ok()
    );
}

#[test]
fn prepared_cancellation_reports_prepared_only_and_releases_at_commit() {
    let scenario = publication_scenario("prepared-cancel-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    projection
        .mark_prepared(
            material.effect_id(),
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();
    assert!(
        scenario
            .resource_keys
            .iter()
            .all(|key| projection.resource_lock(key).is_some())
    );
    let outcome = projection
        .cancel(
            material.effect_id(),
            PreStartReason::PreconditionChanged,
            UnixNanoseconds::parse("1600000000").unwrap(),
        )
        .unwrap();
    assert_eq!(outcome.result(), PreStartResult::PreparedOnly);
    assert!(
        scenario
            .resource_keys
            .iter()
            .all(|key| projection.resource_lock(key).is_none())
    );
}

#[test]
fn generation_and_event_sequence_exhaustion_are_closed() {
    assert_eq!(checked_next_generation(None).unwrap().get(), 0);
    assert_eq!(
        checked_next_generation(Some(U64Decimal::from_u64(u64::MAX))),
        Err(AdmissionError::CounterExhausted)
    );

    let scenario = publication_scenario("sequence-reserve-test-0001");
    let mut projection = LeaseProjection::with_head_sequence(
        &scenario.graph,
        &scenario.enrollment,
        &scenario.policy,
        U64Decimal::from_u64(u64::MAX - 6),
    )
    .unwrap();
    let material = projection
        .reserve_publication(
            &scenario.graph,
            &scenario.policy,
            &scenario.warrant,
            &scenario.approval,
            UnixNanoseconds::parse("1000000000").unwrap(),
        )
        .unwrap();
    projection
        .mark_prepared(
            material.effect_id(),
            UnixNanoseconds::parse("1500000000").unwrap(),
        )
        .unwrap();
    projection
        .start(
            &scenario.graph,
            material.effect_id(),
            UnixNanoseconds::parse("2000000000").unwrap(),
        )
        .unwrap();
    assert_eq!(projection.terminal_sequence_reserve().get(), 3);
    assert_eq!(projection.head_sequence().get(), u64::MAX - 3);
    assert_eq!(
        projection
            .admit_ordinary_events(1, UnixNanoseconds::parse("2000000001").unwrap())
            .unwrap_err(),
        AdmissionError::SequenceExhausted
    );
    projection
        .terminalize(
            material.effect_id(),
            2,
            UnixNanoseconds::parse("3000000000").unwrap(),
        )
        .unwrap();
    assert_eq!(projection.terminal_sequence_reserve().get(), 0);
    assert_eq!(projection.head_sequence().get(), u64::MAX - 1);
    projection
        .admit_ordinary_events(1, UnixNanoseconds::parse("3000000001").unwrap())
        .unwrap();
    assert_eq!(projection.head_sequence().get(), u64::MAX);
    assert_eq!(
        projection
            .admit_ordinary_events(1, UnixNanoseconds::parse("3000000002").unwrap())
            .unwrap_err(),
        AdmissionError::SequenceExhausted
    );
}

#[test]
fn transition_time_never_moves_backwards() {
    let scenario = publication_scenario("time-regression-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();
    projection
        .admit_ordinary_events(1, UnixNanoseconds::parse("100").unwrap())
        .unwrap();
    assert_eq!(
        projection
            .admit_ordinary_events(1, UnixNanoseconds::parse("99").unwrap())
            .unwrap_err(),
        AdmissionError::TimeRegression
    );
}

#[test]
fn zero_ordinary_event_bundle_is_rejected_without_advancing_time() {
    let scenario = publication_scenario("zero-event-test-0001");
    let mut projection =
        LeaseProjection::new(&scenario.graph, &scenario.enrollment, &scenario.policy).unwrap();

    assert_eq!(
        projection
            .admit_ordinary_events(0, UnixNanoseconds::parse("100").unwrap())
            .unwrap_err(),
        AdmissionError::AuthorityRefused,
    );
    projection
        .admit_ordinary_events(1, UnixNanoseconds::parse("99").unwrap())
        .unwrap();
    assert_eq!(projection.head_sequence().get(), 1);
}

#[test]
fn admission_error_clone_preserves_the_exact_body_error_variant() {
    let error = AdmissionError::Body(BodyError::Canonical(CanonicalError::Decode(
        "invalid canonical member".to_owned(),
    )));
    let cloned = error.clone();

    assert_eq!(cloned, error);
    assert!(matches!(
        cloned,
        AdmissionError::Body(BodyError::Canonical(CanonicalError::Decode(message)))
            if message == "invalid canonical member"
    ));
}

#[test]
fn pre_start_reason_vocabulary_is_exact_and_cancellation_is_a_strict_subset() {
    let cases = [
        (
            PreStartReason::RequestDisconnected,
            "request_disconnected",
            true,
        ),
        (
            PreStartReason::ReservationDeadline,
            "reservation_deadline",
            true,
        ),
        (
            PreStartReason::AuthorizationIneligible,
            "authorization_ineligible",
            true,
        ),
        (
            PreStartReason::PeerIdentityChanged,
            "peer_identity_changed",
            true,
        ),
        (
            PreStartReason::PreconditionChanged,
            "precondition_changed",
            true,
        ),
        (PreStartReason::RecoveryOrphaned, "recovery_orphaned", true),
        (
            PreStartReason::BudgetUnavailable,
            "budget_unavailable",
            false,
        ),
        (
            PreStartReason::SeparationPreconditionRefused,
            "separation_precondition_refused",
            false,
        ),
    ];
    for (reason, wire, cancellation) in cases {
        assert_eq!(
            serde_json::to_string(&reason).unwrap(),
            format!(r#""{wire}""#)
        );
        assert_eq!(reason.is_cancellation_reason(), cancellation);
    }
    assert!(serde_json::from_str::<PreStartReason>(r#""other""#).is_err());
}

#[test]
fn separation_generation_boundary_and_owned_decoders_are_closed() {
    let publication = publication_scenario("separation-foundation-0001");
    let active = LogicalAddress::parse("local-file:///active/app").unwrap();
    let quarantine = LogicalAddress::parse("local-file:///quarantine/app").unwrap();
    let input = validated_body(
        StaticArtifactSeparationInput::new(
            ResourceDeedRef::from_digest(Digest::parse(ONE).unwrap()),
            quarantine.clone(),
            XattrValueRef::from_digest(Digest::parse(TWO).unwrap()),
        )
        .unwrap(),
    )
    .unwrap();
    let precondition = validated_body(StaticArtifactSeparationPrecondition::new(
        PresentExpectedState::new(
            ArtifactName::parse("app").unwrap(),
            RawDigest::parse(TWO).unwrap(),
            ByteLength::from_u64(42),
            IncarnationId::parse(ONE).unwrap(),
        ),
        guild_effect_kernel::body::AbsentExpectedState::new(),
        U64Decimal::from_u64(u64::MAX - 1),
    ))
    .unwrap();
    let mut resource_keys = [
        derive_resource_key(&active).unwrap(),
        derive_resource_key(&quarantine).unwrap(),
    ];
    resource_keys.sort();
    let warrant = validated_body(
        SeparationWarrant::new(
            &publication.enrollment,
            &publication.policy,
            Identifier::parse("proposer").unwrap(),
            input.reference().clone(),
            precondition.reference().clone(),
            IdempotencyKey::parse("separation-generation-0001").unwrap(),
            resource_keys.clone(),
            BudgetClaim::new(
                publication.budget_key.clone(),
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            BudgetClaim::new(
                publication.budget_key.clone(),
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            UnixNanoseconds::parse("0").unwrap(),
            UnixNanoseconds::parse("20000000000").unwrap(),
            Hex256::parse(&"d".repeat(64)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let approval = validated_body(
        SeparationApproval::new(
            &warrant,
            &publication.policy,
            Identifier::parse("approver").unwrap(),
            UnixNanoseconds::parse("500000000").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(
        checked_next_generation(Some(U64Decimal::from_u64(u64::MAX))),
        Err(AdmissionError::CounterExhausted)
    );
    let revocation = SeparationRevocation::new(
        &warrant,
        &approval,
        &publication.policy,
        Identifier::parse("revoker").unwrap(),
        UnixNanoseconds::parse("2500000000").unwrap(),
        Identifier::parse("operator-stop").unwrap(),
    )
    .unwrap();
    assert_owned_decoder_extracts_edge(
        "separation-warrant/v1",
        serde_json::to_value(warrant.payload()).unwrap(),
        warrant.payload().installation_digest().digest(),
    );
    assert_owned_decoder_extracts_edge(
        "separation-approval/v1",
        serde_json::to_value(approval.payload()).unwrap(),
        warrant.reference().digest(),
    );
    assert_owned_decoder_extracts_edge(
        "separation-revocation/v1",
        serde_json::to_value(&revocation).unwrap(),
        warrant.reference().digest(),
    );
    let resources = SortedUnique::new(resource_keys.clone().to_vec()).unwrap();
    let effect_id = derive_effect_id(
        warrant.payload().installation_digest().digest(),
        warrant.reference().digest(),
        EffectKind::StaticArtifactSeparation,
        &resources,
        warrant.payload().input_digest().digest(),
        warrant.payload().precondition_digest().digest(),
    )
    .unwrap();
    let binding_body = serde_json::json!({
        "idempotencyKey": warrant.payload().idempotency_key(),
        "effectId": effect_id,
        "warrantDigest": warrant.reference().digest(),
    });
    let binding_envelope = serde_json::json!({
        "body": binding_body.clone(),
        "kind": "separation-binding/v1",
    });
    let binding_digest = canonical_digest(&binding_envelope).unwrap();
    assert_owned_decoder_extracts_edge(
        "separation-binding/v1",
        binding_body,
        warrant.reference().digest(),
    );
    let lease_body = serde_json::json!({
        "effectId": effect_id,
        "bindingDigest": binding_digest,
        "resourceFences": [
            { "resourceKey": resource_keys[0], "fence": "1" },
            { "resourceKey": resource_keys[1], "fence": "1" },
        ],
        "reservationBudgetHold": { "key": publication.budget_key, "amount": 1 },
        "startBudgetHold": { "key": publication.budget_key, "amount": 1 },
        "reservedAt": "1000000000",
        "expiresAt": "6000000000",
    });
    assert_owned_decoder_extracts_edge("separation-lease/v1", lease_body, &binding_digest);
}
