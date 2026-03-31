# Admission Controller

## Admission Inputs

The future session-aware admission controller should evaluate:

- requested session or harness target
- requested action or invocation intent
- declared and requested capabilities
- identity and trust state of the executable artifacts
- secrets, mounts, and network requirements
- runtime or isolation requirements
- current durable session state when waking an existing session
- prior granted envelope or enough durable policy input to recompute it safely
- prior materialization and reconnect assumptions when continuity is being
  reclaimed rather than started fresh

## Admission Outputs

- allow
- deny
- ask-human
- elevate-isolation
- a narrowed or recomputed capability envelope
- a receipt-ready explanation of the decision

## Policy Decisions

- `allow`: requested action may proceed under the computed envelope
- `deny`: request is rejected before execution or wake
- `ask-human`: explicit escalation is required before proceeding
- `elevate-isolation`: the request may proceed only under a stricter isolation
  posture or different runtime class

## Relation To Today's PolicyDecision

Today's live runtime still persists `PolicyDecision` with the attempt-local
outcomes `allowed`, `reduced`, and `rejected`.

Future session-aware admission should wrap that model rather than overwrite it:

- `allow` remains compatible with today's concrete-attempt `allowed` and
  `reduced` outcomes because both still mean Guild may proceed under the final
  granted envelope for that attempt
- `deny` remains compatible with today's `rejected`
- `ask-human` is a new host-owned escalation result above the current live
  policy model; it has no current `PolicyDecision` equivalent until the human
  decision resolves into a final concrete attempt decision
- `elevate-isolation` is also a new host-owned routing result above the current
  live policy model; once Guild selects the stricter posture, the resulting
  attempt should still persist a normal `PolicyDecision`

That means current `reduced` should not be overloaded to mean `ask-human` or
`elevate-isolation`. It remains the live-path record that Guild proceeded with
a narrowed envelope for one attempt.

## Admission Surfaces

Guild should keep two host-owned questions separate:

- `invoke-time admission`: may Guild admit this request at all, and under what
  initial capability envelope and isolation posture?
- `wake-time admission`: may Guild reclaim continuity from an existing durable
  session state, or must it reauthorize, rehydrate, cold-start, escalate, or
  deny instead?

That split matters because "reuse the same session" is not the same claim as
"reuse the same materialization." The session may survive even when the old
continuity assumptions do not.

## Invoke-Time Admission Scope

Invoke-time admission applies whenever Guild is evaluating a fresh request:

- the first materialization for a new session
- a request that targets an existing session but changes executable identity,
  capability needs, or runtime requirements
- a request served by an already-live materialization that stays `active` and
  yields the `warm` execution mode outcome

Invoke-time admission must answer:

- what executable identity and trust state are being admitted
- what requested intent is allowed for this attempt
- what capability envelope is allowed before guest execution begins
- what minimum runtime class or isolation posture is acceptable for this
  attempt

Invoke-time admission does not imply a later wake is safe. It only decides the
current attempt.

## Wake-Time Admission Scope

Wake-time admission begins only when Guild is trying to continue a session with
no currently live materialization, typically from `suspended` or
`rehydration-required`.

Wake-time admission must treat prior durable state as policy input, not as
proof that reuse is still safe. It should answer:

- whether direct resume is still valid
- whether rehydration is required before the request may continue
- whether a cold materialization is the only remaining safe path
- whether the prior granted envelope must be narrowed, recomputed, escalated,
  or denied before continuity is reclaimed

Wake-time admission should never silently inherit the previous attempt's
secrets, mounts, network access, or runtime placement just because the same
`SessionId` is being targeted.

## First Session-Targeted Admission Input

The first shared session-targeted caller shape should make session targeting
explicit before admission begins:

- `session = new` means the caller is asking the host to create work against a
  fresh durable session lineage
- `session = existing { session_id }` means the caller is explicitly targeting
  an already-issued host-owned durable session

That shape is intentionally higher-level than wake implementation details.
Admission may use it to choose invoke-time versus wake-time reasoning, but the
request itself must not accept runtime-local identity such as sandbox IDs,
process IDs, container IDs, VM handles, or snapshot handles.

## Relation To Key Controls

- `capabilities`: define the allowed authority envelope
- `secrets`: may require fresh authorization at invoke or wake time
- `mounts`: host-owned and policy-gated, never ambient
- `network policy`: explicit host-owned outbound policy, not guest choice
- `runtime selection`: may affect isolation guarantees and admission outcome

## Invoke-Time Vs Wake-Time Checks

Invoke-time:

- resolve the executable identity and trust basis for this attempt
- evaluate requested intent and baseline capability requirements
- compute the initial capability envelope
- choose the minimum acceptable isolation posture or runtime class

Wake-time:

- evaluate whether the prior continuity claim is still valid for the targeted
  durable session state
- decide whether secrets, mounts, or network policy must be reauthorized before
  reuse
- decide whether the prior isolation posture or placement is still acceptable
- decide whether direct resume is allowed or whether Guild must rehydrate,
  cold-start, escalate, ask-human, or deny

## Boundary By Control Surface

The invoke-time versus wake-time split should be explicit for the main
host-owned control surfaces.

- `artifact trust and executable identity`
  - invoke-time: always checked before first materialization of an attempt,
    including `warm` reuse against an already-active session
  - wake-time: rechecked when the wake path depends on rehydration, artifact
    replacement, compatibility validation, or a trust-state drift that could
    invalidate the prior admit basis
  - consequence: if Guild cannot still prove the executable identity and trust
    basis it must not claim the old materialization is safely reusable
- `secrets`
  - invoke-time: check that the request is allowed to bind the named secret set
  - wake-time: recheck lease freshness, rotation state, caller eligibility,
    and whether reuse is still allowed before handing the secret back to a
    resumed or rehydrated materialization
  - consequence: stale or policy-invalid secret state invalidates direct reuse;
    Guild must rebind through a fresh host-mediated path, narrow the envelope,
    escalate, or deny
- `mounts`
  - invoke-time: check requested mount classes, scopes, and path policy
  - wake-time: recheck that the mount source still exists, still matches
    policy, still matches the admitted mutability/scope assumptions, and is
    still safe to reconnect
  - consequence: a mount that cannot be safely reattached must not be treated
    as ambient continuity; resume becomes invalid if the attempt still depends
    on it
- `network policy`
  - invoke-time: check the requested egress classes and destination policy
  - wake-time: recheck any policy, routing, credential, or destination state
    that could have changed while the session was suspended
  - consequence: open sockets or prior egress success are not proof that wake
    may continue under the old assumptions; Guild must reauthorize or reconnect
    through the host
- `runtime selection and isolation posture`
  - invoke-time: choose the minimum acceptable runtime class or isolation
    profile for the request
  - wake-time: re-evaluate whether the previous runtime placement is still
    acceptable or whether Guild must elevate isolation, rehydrate elsewhere, or
    cold-start
  - consequence: if the prior placement is no longer policy-valid, Guild must
    not resume in place just because the session survived

## Wake-Time Constraints

Wake-time admission should follow a fail-closed continuity rule:

- the prior admitted envelope is reusable only after the control surfaces above
  are reproved for the requested wake path
- wake-time admission may narrow or recompute the envelope, but it should not
  silently widen authority relative to the current request and policy context
- failure to re-prove one continuity assumption does not have to kill the
  session, but it must invalidate the affected reuse path honestly
- denial against an existing session should preserve or restore a safe durable
  rest state rather than leaving the session stranded in a transient
  `pending-admission` or `admitted` state

## Safe Default

If Guild cannot prove that reuse is still safe at wake time, it should
reauthorize, rehydrate, or cold-start. It should not silently inherit stale
secret, mount, network, or placement assumptions from an earlier admission
decision.
