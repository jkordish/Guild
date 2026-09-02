#![allow(dead_code)]

use guild_effect_kernel::{
    authority::{
        AuthorityPolicy, BudgetAmount, BudgetCapacity, BudgetClaim, InstallationEnrollment,
        PrincipalId, PublicationApproval, PublicationWarrant,
    },
    body::{
        BodyBatch, BodyGraph, ExpectedState, LocalFileObservation, OptionalValue, SortedUnique,
        StaticArtifactPublishInput, StaticArtifactPublishPrecondition, ValidatedBody,
        validate_batch, validated_body,
    },
    evidence::{ObservationAttempt, WitnessStatus},
    lease::{AdmissionError, EffectLease, LeaseProjection, derive_resource_key},
    scalar::{
        ArtifactName, ByteLength, Hex256, IdempotencyKey, Identifier, IncarnationId,
        LogicalAddress, RawDigest, SafeUInt, U64Decimal, UnixNanoseconds,
    },
};

pub fn authenticated_attempt(
    observation: ValidatedBody<LocalFileObservation>,
) -> ObservationAttempt {
    ObservationAttempt::Observed {
        observation,
        witness: WitnessStatus::AuthenticatedEnrolled,
    }
}

const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const TWO: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";

pub struct AuthorityFixture {
    graph: BodyGraph,
    policy: ValidatedBody<AuthorityPolicy>,
    enrollment: ValidatedBody<InstallationEnrollment>,
    warrant: ValidatedBody<PublicationWarrant>,
}

impl AuthorityFixture {
    pub fn proposer_id(&self) -> PrincipalId {
        Identifier::parse("proposer").unwrap()
    }

    pub fn approver_id(&self) -> PrincipalId {
        Identifier::parse("approver").unwrap()
    }

    pub fn approve_as(
        &self,
        approver_id: PrincipalId,
    ) -> Result<ValidatedBody<PublicationApproval>, AdmissionError> {
        let approval = PublicationApproval::new(
            &self.warrant,
            &self.policy,
            approver_id,
            UnixNanoseconds::parse("500000000").unwrap(),
        )?;
        validated_body(approval).map_err(AdmissionError::from)
    }
}

pub fn authority() -> AuthorityFixture {
    let reservation_key = Identifier::parse("publish-reservation").unwrap();
    let start_key = Identifier::parse("publish-start").unwrap();
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
                reservation_key.clone(),
                SafeUInt::new(10).unwrap(),
            )])
            .unwrap(),
            SortedUnique::new(vec![BudgetCapacity::new(
                start_key.clone(),
                SafeUInt::new(10).unwrap(),
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
    let warrant = validated_body(
        PublicationWarrant::new(
            &enrollment,
            &policy,
            Identifier::parse("proposer").unwrap(),
            input.reference().clone(),
            precondition.reference().clone(),
            IdempotencyKey::parse("fixture-publication-0001").unwrap(),
            resource_keys,
            BudgetClaim::new(
                reservation_key,
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            BudgetClaim::new(
                start_key,
                BudgetAmount::new(SafeUInt::new(1).unwrap()).unwrap(),
            ),
            UnixNanoseconds::parse("0").unwrap(),
            UnixNanoseconds::parse("20000000000").unwrap(),
            Hex256::parse(&"a".repeat(64)).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let graph = validate_batch(
        &BodyGraph::empty(),
        BodyBatch::new(vec![
            policy.clone().into_stored(),
            enrollment.clone().into_stored(),
            source.into_stored(),
            input.into_stored(),
            precondition.into_stored(),
            warrant.clone().into_stored(),
        ])
        .unwrap(),
    )
    .unwrap();
    AuthorityFixture {
        graph,
        policy,
        enrollment,
        warrant,
    }
}

pub fn publication_lease_at(reserved_at: &str) -> EffectLease {
    let fixtures = authority();
    let approval = fixtures.approve_as(fixtures.approver_id()).unwrap();
    let graph = validate_batch(
        &fixtures.graph,
        BodyBatch::new(vec![approval.clone().into_stored()]).unwrap(),
    )
    .unwrap();
    let mut projection =
        LeaseProjection::new(&graph, &fixtures.enrollment, &fixtures.policy).unwrap();
    projection
        .reserve_publication(
            &graph,
            &fixtures.policy,
            &fixtures.warrant,
            &approval,
            UnixNanoseconds::parse(reserved_at).unwrap(),
        )
        .unwrap()
        .lease()
        .payload()
        .clone()
}

pub fn absent_observation(address: &str) -> ValidatedBody<LocalFileObservation> {
    validated_body(LocalFileObservation::absent(
        LogicalAddress::parse(address).unwrap(),
        Identifier::parse("host-probe").unwrap(),
        UnixNanoseconds::parse("1788210000000000000").unwrap(),
    ))
    .unwrap()
}
