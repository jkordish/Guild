# Team Governance Boundaries

This note scopes the later-phase private-pack, policy, and governance surface
for Guild in team-review terms.
It is a docs-first planning artifact, not a runtime-contract source.

Normative runtime truth still lives in `SPECS.md`, `ARCHITECTURE.md`, WIT, and
the Rust runtime/types. Use this document to keep governance language anchored
to Guild's current trust chain and fail-closed posture instead of drifting into
an implied hosted control plane.

## Governance Problem Statement

If Guild moves from repo-local operator workflows toward team adoption, teams
will need a clear way to answer four review questions:

- who is allowed to publish, trust, install, review, approve, or run a
  sensitive workflow?
- what durable record exists for those decisions and later actions?
- how long do execution and evidence records stay available?
- what evidence may be retained, shared, or redacted without breaking the
  trust chain?

The problem is governance in the narrow Guild sense:

- reviewable team boundaries over signed distribution and trust
- reviewable policy over who may run or approve risky work
- durable auditability for those decisions
- retention and redaction choices that do not silently destroy explanation or
  evidence integrity

## Boundary Buckets

Keep the future work separated into these buckets so planning does not blur the
concerns together.

### Policy

Policy concerns answer:

- who may trust a publisher in one Guild root
- who may install or admit a risky workflow
- which approvals are required before future mutation
- which capability families or blast-radius classes need tighter default review

Policy is about admission and allowed behavior, not about long-term storage.

### Audit

Audit concerns answer:

- who approved, denied, installed, trusted, or ran something
- which durable receipt or trust decision records prove that later
- how rejections, narrowed grants, and follow-up explanations stay visible

Audit is about proving what decision happened and when, not about deciding
whether the underlying bytes remain stored forever.

### Retention

Retention concerns answer:

- how long execution receipts, evidence payloads, metadata, and query resources
  should remain available
- which records may be garbage-collected only after policy allows it
- how later explanation changes when an object ages out

Retention is about lifecycle and availability, not about permissioning by
itself.

### Redaction

Redaction concerns answer:

- what evidence detail can be hidden, summarized, or withheld for a given
  audience
- which redaction classes preserve enough fact to keep a claim honest
- when removing detail would break the receipt or evidence chain needed for
  explanation

Redaction is about visibility and disclosure boundaries, not about whether the
underlying action was admitted in the first place.

## Future Runtime Dependencies, Planning-Only

These are believable later prerequisites, not shipped promises:

- richer policy profiles than today's local bounded capability evaluation
- approval or interruption semantics for future apply-mode actions
- durable trust-decision and approval records that can be reviewed alongside
  execution receipts
- retention and garbage-collection controls for receipts, evidence payloads,
  and metadata
- clearer private distribution views layered on current signed bundle and OCI
  transport rather than a new pack contract
- bounded sharing or synchronization surfaces only if they preserve host-owned
  trust review instead of bypassing it

None of those should be described as current runtime truth until code and proof
paths exist.

## Private Distribution Boundary

If Guild later grows a more private or team-scoped distribution story, it
should stay anchored to the current transport model:

- signed bundle and OCI transport remain the substrate
- target-root trust review remains local and host-owned
- private visibility is a governance and distribution concern, not a reason to
  invent a second package contract

That means future private-pack language should read as a presentation or policy
layer over current signed transport and trust review, not as a new pack type.

## Anti-Goals

Keep these non-goals explicit:

- no hosted-control-plane promise by wording alone
- no marketplace or discovery story that outruns local trust review
- no remote trust synchronization implied as already shipped
- no claim that governance planning means multi-tenant SaaS features are near
  term
- no second contract surface beside current manifests, WIT, Rust runtime/types,
  and `SPECS.md`

## Done-When Restatement

Issue `#134` is done when the repo can describe later private-pack, policy,
audit, retention, and redaction concerns in sensible Guild terms without
implying that team governance runtime support already exists.
