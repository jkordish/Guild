//! Pure immutable-store snapshots and structurally atomic transition proposals.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    body::{BodyBatch, BodyGraph, BodyKind, StoredBody, validate_batch},
    event::{
        ChainError, EventEnvelope, EventPayload, EventType, PreviousEvent,
        validate_envelope_identity, validate_event_body_refs,
    },
    scalar::Digest,
};

/// The authenticated head value against which a transition proposes compare-and-swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpectedHead {
    Empty,
    Present(Digest),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum ExpectedHeadWire {
    Empty,
    Present { digest: Digest },
}

impl Serialize for ExpectedHead {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Empty => ExpectedHeadWire::Empty.serialize(serializer),
            Self::Present(digest) => ExpectedHeadWire::Present {
                digest: digest.clone(),
            }
            .serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ExpectedHead {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match ExpectedHeadWire::deserialize(deserializer)? {
            ExpectedHeadWire::Empty => Self::Empty,
            ExpectedHeadWire::Present { digest } => Self::Present(digest),
        })
    }
}

impl ExpectedHead {
    #[must_use]
    pub const fn digest(&self) -> Option<&Digest> {
        match self {
            Self::Empty => None,
            Self::Present(digest) => Some(digest),
        }
    }
}

/// One all-or-nothing transition proposal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionBundle {
    expected_head: ExpectedHead,
    new_bodies: Vec<StoredBody>,
    events: Vec<EventEnvelope>,
    new_head: Digest,
}

impl TransitionBundle {
    #[allow(
        dead_code,
        reason = "used by reviewed transition modules added after Task 8"
    )]
    pub(crate) fn new(
        expected_head: ExpectedHead,
        new_bodies: Vec<StoredBody>,
        events: Vec<EventEnvelope>,
        new_head: Digest,
    ) -> Self {
        Self {
            expected_head,
            new_bodies,
            events,
            new_head,
        }
    }

    #[must_use]
    pub const fn expected_head(&self) -> &ExpectedHead {
        &self.expected_head
    }

    #[must_use]
    pub fn new_bodies(&self) -> &[StoredBody] {
        &self.new_bodies
    }

    #[must_use]
    pub fn events(&self) -> &[EventEnvelope] {
        &self.events
    }

    #[must_use]
    pub const fn new_head(&self) -> &Digest {
        &self.new_head
    }
}

/// An immutable snapshot consumed by pure transition validation.
///
/// Mutation is deliberately absent from the public API. The in-memory application seam exists
/// only in crate unit tests; a real adapter must provide authenticated durable CAS.
///
/// ```compile_fail
/// use guild_effect_kernel::store::ImmutableStore;
/// let _test_only = ImmutableStore::apply_committed_for_test;
/// ```
#[derive(Debug, Clone, Default)]
pub struct ImmutableStore {
    bodies: BodyGraph,
    events: BTreeMap<Digest, EventEnvelope>,
    head: Option<Digest>,
}

impl ImmutableStore {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bodies: BodyGraph::empty(),
            events: BTreeMap::new(),
            head: None,
        }
    }

    #[must_use]
    pub const fn bodies(&self) -> &BodyGraph {
        &self.bodies
    }

    #[must_use]
    pub const fn events(&self) -> &BTreeMap<Digest, EventEnvelope> {
        &self.events
    }

    #[must_use]
    pub const fn head(&self) -> Option<&Digest> {
        self.head.as_ref()
    }

    #[allow(dead_code, reason = "used by exhaustive replay in Task 9")]
    pub(crate) fn from_validated(
        bodies: BodyGraph,
        events: BTreeMap<Digest, EventEnvelope>,
        head: Option<Digest>,
    ) -> Self {
        Self {
            bodies,
            events,
            head,
        }
    }

    #[cfg(test)]
    fn apply_committed_for_test(
        &mut self,
        bundle: TransitionBundle,
    ) -> Result<TrustedCommitOutcome, ChainError> {
        if !expected_matches_current(bundle.expected_head(), self.head()) {
            return Ok(TrustedCommitOutcome::HeadMismatch {
                current_head: self.head.clone(),
            });
        }

        let expected_head = bundle.expected_head.clone();
        let new_head = bundle.new_head.clone();
        let (bundle, combined_bodies) = validate_bundle_inner(self, bundle)?;

        let mut combined_events = self.events.clone();
        for event in bundle.events {
            if combined_events
                .insert(event.digest().clone(), event)
                .is_some()
            {
                return Err(ChainError::InvalidBundle);
            }
        }

        *self = Self {
            bodies: combined_bodies,
            events: combined_events,
            head: Some(new_head.clone()),
        };
        Ok(TrustedCommitOutcome::Committed {
            expected_head,
            new_head,
        })
    }
}

/// The authenticated, one-shot result of an outer durable compare-and-swap.
///
/// This type intentionally does not implement `Clone`: a later head read cannot be substituted
/// for the actual commit result delivered once by the store adapter.
///
/// ```compile_fail
/// use guild_effect_kernel::store::TrustedCommitOutcome;
/// let outcome = TrustedCommitOutcome::Unknown;
/// let _copy = outcome.clone();
/// ```
#[derive(Debug, PartialEq, Eq)]
pub enum TrustedCommitOutcome {
    Committed {
        expected_head: ExpectedHead,
        new_head: Digest,
    },
    HeadMismatch {
        current_head: Option<Digest>,
    },
    Unknown,
}

fn expected_matches_current(expected: &ExpectedHead, current: Option<&Digest>) -> bool {
    match (expected, current) {
        (ExpectedHead::Empty, None) => true,
        (ExpectedHead::Present(expected), Some(current)) => expected == current,
        (ExpectedHead::Empty | ExpectedHead::Present(_), _) => false,
    }
}

/// Proves one transition bundle against an immutable base snapshot without writing storage.
///
/// # Errors
///
/// Rejects a stale base head, noncanonical or unsorted bodies, unresolved typed references,
/// malformed event identity/link/time, illegal empty-head use, a wrong final digest, and event
/// sequence exhaustion including outstanding terminalization reserve.
pub fn validate_bundle(
    base: &ImmutableStore,
    bundle: TransitionBundle,
) -> Result<TransitionBundle, ChainError> {
    validate_bundle_inner(base, bundle).map(|(bundle, _)| bundle)
}

#[allow(
    clippy::too_many_lines,
    reason = "the atomic bundle proof keeps all fail-before-write checks in one visible sequence"
)]
fn validate_bundle_inner(
    base: &ImmutableStore,
    bundle: TransitionBundle,
) -> Result<(TransitionBundle, BodyGraph), ChainError> {
    if !expected_matches_current(bundle.expected_head(), base.head()) {
        return Err(ChainError::HeadMismatch);
    }
    if bundle.events.is_empty() {
        return Err(ChainError::InvalidBundle);
    }
    if bundle
        .new_bodies
        .windows(2)
        .any(|pair| pair[0].digest() >= pair[1].digest())
    {
        return Err(ChainError::InvalidBundle);
    }

    let combined_bodies = validate_batch(&base.bodies, BodyBatch::new(bundle.new_bodies.clone())?)?;

    let mut new_event_digests = BTreeSet::new();
    for event in &bundle.events {
        validate_envelope_identity(event.digest(), event)?;
        if base.events.contains_key(event.digest()) || !new_event_digests.insert(event.digest()) {
            return Err(ChainError::InvalidBundle);
        }
        match combined_bodies.get(event.preimage().installation_digest().digest()) {
            Some(body) if body.kind() == BodyKind::InstallationEnrollment => {}
            Some(body) => {
                return Err(ChainError::Body(crate::body::BodyError::WrongTargetKind {
                    source: body.kind(),
                    expected: BodyKind::InstallationEnrollment,
                    actual: body.kind(),
                }));
            }
            None => {
                return Err(ChainError::Body(crate::body::BodyError::MissingReference {
                    source: event.digest().clone(),
                    target: event.preimage().installation_digest().digest().clone(),
                }));
            }
        }
        validate_event_body_refs(&combined_bodies, event)?;
    }

    let first = bundle.events.first().ok_or(ChainError::InvalidBundle)?;
    let last = bundle.events.last().ok_or(ChainError::InvalidBundle)?;
    if last.digest() != bundle.new_head() {
        return Err(ChainError::InvalidBundle);
    }
    let transition_at = first.preimage().occurred_at();
    if bundle
        .events
        .iter()
        .any(|event| event.preimage().occurred_at() != transition_at)
    {
        return Err(ChainError::InvalidBundle);
    }

    match bundle.expected_head() {
        ExpectedHead::Empty => validate_genesis_bundle(base, &bundle)?,
        ExpectedHead::Present(expected) => {
            let previous = base.events.get(expected).ok_or(ChainError::InvalidBundle)?;
            if first.preimage().installation_digest() != previous.preimage().installation_digest()
                || first.preimage().occurred_at() < previous.preimage().occurred_at()
                || !matches!(
                    first.preimage().previous_event(),
                    PreviousEvent::Previous { digest } if digest == expected
                )
            {
                return Err(ChainError::InvalidBundle);
            }
            let expected_sequence = previous
                .preimage()
                .sequence()
                .checked_add(1)
                .map_err(|_| ChainError::InvalidBundle)?;
            if first.preimage().sequence() != expected_sequence {
                return Err(ChainError::InvalidBundle);
            }
        }
    }

    for pair in bundle.events.windows(2) {
        let [previous, current] = pair else {
            unreachable!("windows(2) always contains two events")
        };
        if current.preimage().installation_digest() != first.preimage().installation_digest()
            || current.preimage().sequence()
                != previous
                    .preimage()
                    .sequence()
                    .checked_add(1)
                    .map_err(|_| ChainError::InvalidBundle)?
            || !matches!(
                current.preimage().previous_event(),
                PreviousEvent::Previous { digest } if digest == previous.digest()
            )
        {
            return Err(ChainError::InvalidBundle);
        }
    }

    let terminal_count = bundle
        .events
        .iter()
        .filter(|event| is_terminal(event.preimage().event_type()))
        .count();
    if terminal_count > 1 {
        return Err(ChainError::InvalidBundle);
    }
    validate_sequence_capacity(base, &bundle)?;
    Ok((bundle, combined_bodies))
}

fn validate_genesis_bundle(
    base: &ImmutableStore,
    bundle: &TransitionBundle,
) -> Result<(), ChainError> {
    if !base.bodies.is_empty()
        || !base.events.is_empty()
        || base.head.is_some()
        || bundle.new_bodies.len() != 2
        || bundle.events.len() != 1
        || bundle
            .new_bodies
            .iter()
            .filter(|body| body.kind() == BodyKind::AuthorityPolicy)
            .count()
            != 1
        || bundle
            .new_bodies
            .iter()
            .filter(|body| body.kind() == BodyKind::InstallationEnrollment)
            .count()
            != 1
    {
        return Err(ChainError::InvalidBundle);
    }
    let event = &bundle.events[0];
    let EventPayload::InstallationEnrolled(payload) = event.preimage().payload() else {
        return Err(ChainError::InvalidBundle);
    };
    if event.preimage().event_type() != EventType::InstallationEnrolled
        || event.preimage().sequence().get() != 0
        || !matches!(event.preimage().previous_event(), PreviousEvent::Genesis)
        || payload.enrollment_digest() != event.preimage().installation_digest()
    {
        return Err(ChainError::InvalidBundle);
    }
    Ok(())
}

fn is_terminal(event_type: EventType) -> bool {
    matches!(
        event_type,
        EventType::EffectVerified
            | EventType::EffectFailed
            | EventType::EffectIndeterminate
            | EventType::SeparationVerified
            | EventType::SeparationFailed
            | EventType::SeparationIndeterminate
    )
}

fn validate_sequence_capacity(
    base: &ImmutableStore,
    bundle: &TransitionBundle,
) -> Result<(), ChainError> {
    let installation = bundle.events[0].preimage().installation_digest();
    let events = base
        .events
        .values()
        .chain(bundle.events.iter())
        .filter(|event| event.preimage().installation_digest() == installation);
    let mut publication_starts = 0_u64;
    let mut publication_terminals = 0_u64;
    let mut separation_starts = 0_u64;
    let mut separation_terminals = 0_u64;
    for event in events {
        match event.preimage().event_type() {
            EventType::EffectStarted => {
                publication_starts = publication_starts
                    .checked_add(1)
                    .ok_or(ChainError::InvalidBundle)?;
            }
            EventType::EffectVerified
            | EventType::EffectFailed
            | EventType::EffectIndeterminate => {
                publication_terminals = publication_terminals
                    .checked_add(1)
                    .ok_or(ChainError::InvalidBundle)?;
            }
            EventType::SeparationStarted => {
                separation_starts = separation_starts
                    .checked_add(1)
                    .ok_or(ChainError::InvalidBundle)?;
            }
            EventType::SeparationVerified
            | EventType::SeparationFailed
            | EventType::SeparationIndeterminate => {
                separation_terminals = separation_terminals
                    .checked_add(1)
                    .ok_or(ChainError::InvalidBundle)?;
            }
            _ => {}
        }
    }
    let publication_pending = publication_starts
        .checked_sub(publication_terminals)
        .ok_or(ChainError::InvalidBundle)?;
    let separation_pending = separation_starts
        .checked_sub(separation_terminals)
        .ok_or(ChainError::InvalidBundle)?;
    let reserve = publication_pending
        .checked_mul(3)
        .and_then(|value| {
            separation_pending
                .checked_mul(2)
                .and_then(|separation| value.checked_add(separation))
        })
        .ok_or(ChainError::InvalidBundle)?;
    let remaining = u64::MAX
        .checked_sub(bundle.events.last().unwrap().preimage().sequence().get())
        .ok_or(ChainError::InvalidBundle)?;
    if remaining < reserve {
        return Err(ChainError::InvalidBundle);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        ExpectedHead, ImmutableStore, TransitionBundle, TrustedCommitOutcome, validate_bundle,
        validate_sequence_capacity,
    };
    use crate::{
        authority::{AuthorityPolicy, BudgetCapacity, InstallationEnrollment},
        body::{
            BodyBatch, BodyError, BodyKind, InstallationEnrollmentRef, SortedUnique, StoredBody,
            validate_batch, validated_body,
        },
        canonical::canonical_digest,
        event::{ChainError, EventEnvelope},
        scalar::{Digest, Identifier, IncarnationId, SafeUInt, U64Decimal, UnixNanoseconds},
    };

    const ONE: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    const THREE: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    fn enrollment_bodies() -> (Vec<StoredBody>, InstallationEnrollmentRef) {
        let capacity = |key: &str| {
            BudgetCapacity::new(Identifier::parse(key).unwrap(), SafeUInt::new(10).unwrap())
        };
        let policy = validated_body(
            AuthorityPolicy::new(
                Identifier::parse("policy").unwrap(),
                U64Decimal::from_u64(0),
                SortedUnique::new(vec![Identifier::parse("proposer").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("approver").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("revoker").unwrap()]).unwrap(),
                SortedUnique::new(vec![Identifier::parse("witness").unwrap()]).unwrap(),
                true,
                SortedUnique::new(vec![capacity("reservation")]).unwrap(),
                SortedUnique::new(vec![capacity("start")]).unwrap(),
                Identifier::parse("clock").unwrap(),
                Identifier::parse("store").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let enrollment = validated_body(
            InstallationEnrollment::new(
                Identifier::parse("installation").unwrap(),
                IncarnationId::parse(ONE).unwrap(),
                policy.reference().clone(),
                UnixNanoseconds::parse("0").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let reference = enrollment.reference().clone();
        let mut bodies = vec![policy.into_stored(), enrollment.into_stored()];
        bodies.sort_by(|left, right| left.digest().cmp(right.digest()));
        (bodies, reference)
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "owned JSON values keep each hostile test fixture self-contained"
    )]
    fn raw_event(
        installation: &InstallationEnrollmentRef,
        sequence: &str,
        previous_event: Value,
        occurred_at: &str,
        event_type: &str,
        payload: Value,
    ) -> EventEnvelope {
        let preimage = json!({
            "schemaVersion": "jidoka.dev/events/v1",
            "sequence": sequence,
            "previousEvent": previous_event,
            "installationDigest": installation,
            "occurredAt": occurred_at,
            "eventType": event_type,
            "payload": payload,
        });
        let digest = canonical_digest(&preimage).unwrap();
        let envelope = serde_json::to_vec(&json!({
            "digest": digest,
            "preimage": preimage,
        }))
        .unwrap();
        EventEnvelope::from_json(&envelope).unwrap()
    }

    fn genesis(installation: &InstallationEnrollmentRef) -> EventEnvelope {
        raw_event(
            installation,
            "0",
            json!({ "state": "genesis" }),
            "0",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        )
    }

    fn successor(
        installation: &InstallationEnrollmentRef,
        previous: &EventEnvelope,
        sequence: &str,
        occurred_at: &str,
    ) -> EventEnvelope {
        raw_event(
            installation,
            sequence,
            json!({ "state": "previous", "digest": previous.digest() }),
            occurred_at,
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        )
    }

    #[test]
    fn valid_genesis_bundle_is_pure_then_applies_atomically_for_test() {
        let (bodies, installation) = enrollment_bodies();
        let event = genesis(&installation);
        let bundle = TransitionBundle::new(
            ExpectedHead::Empty,
            bodies,
            vec![event.clone()],
            event.digest().clone(),
        );
        let store = ImmutableStore::empty();
        let validated = validate_bundle(&store, bundle).unwrap();
        assert!(store.head().is_none(), "validation must not write storage");

        let mut store = store;
        let outcome = store.apply_committed_for_test(validated).unwrap();
        assert!(matches!(
            outcome,
            TrustedCommitOutcome::Committed {
                expected_head: ExpectedHead::Empty,
                ref new_head,
            } if new_head == event.digest()
        ));
        assert_eq!(store.head(), Some(event.digest()));
        assert_eq!(store.events().len(), 1);
        assert_eq!(store.bodies().len(), 2);
    }

    #[test]
    fn empty_event_bundle_is_invalid_not_a_head_mismatch() {
        assert_eq!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    Vec::new(),
                    Vec::new(),
                    Digest::parse(THREE).unwrap(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );
    }

    #[test]
    fn bundle_installation_reference_reports_type_confusion() {
        let (bodies, _) = enrollment_bodies();
        let policy = bodies
            .iter()
            .find(|body| body.kind() == BodyKind::AuthorityPolicy)
            .unwrap();
        let confused = InstallationEnrollmentRef::from_digest(policy.digest().clone());
        let event = genesis(&confused);

        assert!(matches!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    bodies,
                    vec![event.clone()],
                    event.digest().clone(),
                ),
            ),
            Err(ChainError::Body(BodyError::WrongTargetKind {
                expected: BodyKind::InstallationEnrollment,
                actual: BodyKind::AuthorityPolicy,
                ..
            }))
        ));
    }

    #[test]
    fn invalid_proposal_and_cas_mismatch_leave_the_store_unchanged() {
        let (bodies, installation) = enrollment_bodies();
        let event = genesis(&installation);
        let mut store = ImmutableStore::empty();
        store
            .apply_committed_for_test(TransitionBundle::new(
                ExpectedHead::Empty,
                bodies,
                vec![event.clone()],
                event.digest().clone(),
            ))
            .unwrap();
        let original_head = store.head().cloned();
        let original_body_count = store.bodies().len();
        let original_event_count = store.events().len();

        let missing_ref = raw_event(
            &installation,
            "1",
            json!({ "state": "previous", "digest": event.digest() }),
            "1",
            "warrant_proposed",
            json!({ "warrantDigest": THREE }),
        );
        let invalid = TransitionBundle::new(
            ExpectedHead::Present(event.digest().clone()),
            Vec::new(),
            vec![missing_ref.clone()],
            missing_ref.digest().clone(),
        );
        assert!(matches!(
            store.apply_committed_for_test(invalid),
            Err(ChainError::Body(_))
        ));

        let wrong_head_event = raw_event(
            &installation,
            "1",
            json!({ "state": "previous", "digest": THREE }),
            "1",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        let mismatch = store
            .apply_committed_for_test(TransitionBundle::new(
                ExpectedHead::Present(Digest::parse(THREE).unwrap()),
                Vec::new(),
                vec![wrong_head_event.clone()],
                wrong_head_event.digest().clone(),
            ))
            .unwrap();
        assert!(matches!(
            mismatch,
            TrustedCommitOutcome::HeadMismatch {
                current_head: Some(ref current),
            } if current == event.digest()
        ));
        assert_eq!(store.head(), original_head.as_ref());
        assert_eq!(store.bodies().len(), original_body_count);
        assert_eq!(store.events().len(), original_event_count);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one hostile bundle fixture checks each linked canonicality condition independently"
    )]
    fn bundle_requires_sorted_unique_bodies_exact_links_time_and_final_digest() {
        let (mut bodies, installation) = enrollment_bodies();
        let first = genesis(&installation);
        bodies.reverse();
        assert_eq!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    bodies,
                    vec![first.clone()],
                    first.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let (bodies, _) = enrollment_bodies();
        let graph = validate_batch(
            &crate::body::BodyGraph::empty(),
            BodyBatch::new(bodies).unwrap(),
        )
        .unwrap();
        let mut base = ImmutableStore::from_validated(
            graph,
            [(first.digest().clone(), first.clone())]
                .into_iter()
                .collect(),
            Some(first.digest().clone()),
        );

        let second = successor(&installation, &first, "1", "1");
        let different_time = successor(&installation, &second, "2", "2");
        assert_eq!(
            validate_bundle(
                &base,
                TransitionBundle::new(
                    ExpectedHead::Present(first.digest().clone()),
                    Vec::new(),
                    vec![second.clone(), different_time.clone()],
                    different_time.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        assert_eq!(
            validate_bundle(
                &base,
                TransitionBundle::new(
                    ExpectedHead::Present(first.digest().clone()),
                    Vec::new(),
                    vec![second.clone()],
                    Digest::parse(THREE).unwrap(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let wrong_link = raw_event(
            &installation,
            "1",
            json!({ "state": "previous", "digest": THREE }),
            "1",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        assert_eq!(
            validate_bundle(
                &base,
                TransitionBundle::new(
                    ExpectedHead::Present(first.digest().clone()),
                    Vec::new(),
                    vec![wrong_link.clone()],
                    wrong_link.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let wrong_internal_link = raw_event(
            &installation,
            "2",
            json!({ "state": "previous", "digest": first.digest() }),
            "1",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        assert_eq!(
            validate_bundle(
                &base,
                TransitionBundle::new(
                    ExpectedHead::Present(first.digest().clone()),
                    Vec::new(),
                    vec![second.clone(), wrong_internal_link.clone()],
                    wrong_internal_link.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let third = successor(&installation, &second, "2", "1");
        let valid = TransitionBundle::new(
            ExpectedHead::Present(first.digest().clone()),
            Vec::new(),
            vec![second.clone(), third.clone()],
            third.digest().clone(),
        );
        base.apply_committed_for_test(validate_bundle(&base, valid).unwrap())
            .unwrap();
        assert_eq!(base.head(), Some(third.digest()));
    }

    #[test]
    fn empty_head_allows_only_one_genesis_event_and_unique_new_bodies() {
        let (bodies, installation) = enrollment_bodies();
        let first = genesis(&installation);
        let second = successor(&installation, &first, "1", "0");
        assert_eq!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    bodies.clone(),
                    vec![first.clone(), second.clone()],
                    second.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let duplicate = bodies[0].clone();
        assert_eq!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    vec![duplicate.clone(), duplicate],
                    vec![first.clone()],
                    first.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let mut bodies_with_unrelated_descriptor = bodies.clone();
        bodies_with_unrelated_descriptor.push(
            validated_body(crate::schema::descriptor(
                crate::schema::SchemaId::LocalFileObservationV1,
            ))
            .unwrap()
            .into_stored(),
        );
        bodies_with_unrelated_descriptor.sort_by(|left, right| left.digest().cmp(right.digest()));
        assert_eq!(
            validate_bundle(
                &ImmutableStore::empty(),
                TransitionBundle::new(
                    ExpectedHead::Empty,
                    bodies_with_unrelated_descriptor,
                    vec![first.clone()],
                    first.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one boundary fixture contrasts overflow, exact reserve, and exhausted reserve"
    )]
    fn checked_successors_and_terminal_reserve_close_the_sequence_boundary() {
        let (_, installation) = enrollment_bodies();
        let max = raw_event(
            &installation,
            "18446744073709551615",
            json!({ "state": "genesis" }),
            "0",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        let graph = validate_batch(
            &crate::body::BodyGraph::empty(),
            BodyBatch::new(enrollment_bodies().0).unwrap(),
        )
        .unwrap();
        let max_store = ImmutableStore::from_validated(
            graph,
            [(max.digest().clone(), max.clone())].into_iter().collect(),
            Some(max.digest().clone()),
        );
        let wrapped = successor(&installation, &max, "0", "0");
        assert_eq!(
            validate_bundle(
                &max_store,
                TransitionBundle::new(
                    ExpectedHead::Present(max.digest().clone()),
                    Vec::new(),
                    vec![wrapped.clone()],
                    wrapped.digest().clone(),
                ),
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );

        let start_payload = json!({
            "bindingDigest": ONE,
            "leaseDigest": THREE,
            "preparedArtifactDigest": ONE,
            "sourceBeforeObservationDigest": ONE,
            "targetBeforeObservationDigest": THREE,
            "mutationMode": "conditional"
        });
        let enough_base = raw_event(
            &installation,
            "18446744073709551611",
            json!({ "state": "genesis" }),
            "0",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        let enough_start = raw_event(
            &installation,
            "18446744073709551612",
            json!({ "state": "previous", "digest": enough_base.digest() }),
            "1",
            "effect_started",
            start_payload.clone(),
        );
        let enough_store = ImmutableStore::from_validated(
            crate::body::BodyGraph::empty(),
            [(enough_base.digest().clone(), enough_base.clone())]
                .into_iter()
                .collect(),
            Some(enough_base.digest().clone()),
        );
        assert!(
            validate_sequence_capacity(
                &enough_store,
                &TransitionBundle::new(
                    ExpectedHead::Present(enough_base.digest().clone()),
                    Vec::new(),
                    vec![enough_start.clone()],
                    enough_start.digest().clone(),
                )
            )
            .is_ok()
        );

        let late_base = raw_event(
            &installation,
            "18446744073709551612",
            json!({ "state": "genesis" }),
            "0",
            "installation_enrolled",
            json!({ "enrollmentDigest": installation }),
        );
        let late_start = raw_event(
            &installation,
            "18446744073709551613",
            json!({ "state": "previous", "digest": late_base.digest() }),
            "1",
            "effect_started",
            start_payload,
        );
        let late_store = ImmutableStore::from_validated(
            crate::body::BodyGraph::empty(),
            [(late_base.digest().clone(), late_base.clone())]
                .into_iter()
                .collect(),
            Some(late_base.digest().clone()),
        );
        assert_eq!(
            validate_sequence_capacity(
                &late_store,
                &TransitionBundle::new(
                    ExpectedHead::Present(late_base.digest().clone()),
                    Vec::new(),
                    vec![late_start.clone()],
                    late_start.digest().clone(),
                )
            )
            .unwrap_err(),
            ChainError::InvalidBundle
        );
    }
}
