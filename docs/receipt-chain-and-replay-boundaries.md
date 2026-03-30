# Receipt Chain And Replay Boundaries

This note scopes the current receipt-and-evidence chain and the meaning of
`replay-oriented explanation` on Guild's live path.
It is a docs-first planning artifact, not a runtime-contract source.

Normative runtime truth still lives in `SPECS.md`, `ARCHITECTURE.md`, WIT, and
the Rust runtime/types. Use this document to keep roadmap and operator wording
honest about what Guild can explain today from stored refs versus what a true
future replay-execution feature would still require.

## Current Receipt Chain

Today Guild's durable trust chain can be read as:

1. request and review
2. resolution to immutable executable identity
3. host-owned capability evaluation
4. host-issued execution envelope
5. bounded guest execution
6. durable execution receipt
7. durable evidence records and payloads
8. replay-oriented explanation from stored refs

That chain maps to today's shipped behavior like this:

| Step | Current surface | Host-owned fact |
| --- | --- | --- |
| request and review | `guild show`, `guild grants template`, install/verify surfaces | the operator can review what Guild resolved and what authority is being requested before execution |
| resolution | requested ref -> resolved ref + digest | Guild resolves human-meaningful refs to immutable executable identity before the run starts |
| capability evaluation | requested versus granted authority, rejection outcomes | the host narrows or denies authority; the guest does not self-assert it |
| execution envelope | execution ID, start timestamp, parent linkage, granted slice | the host mints the execution envelope and durable identifiers |
| bounded execution | runtime boundary, host-mediated calls only | the guest runs inside the granted capability slice rather than ambient host access |
| durable receipt | `guild://executions/...`, `guild why`, `guild get` | Guild persists the execution result even for failures and rejections |
| durable evidence | `guild://objects/records/...`, metadata companion resources | evidence payloads and evidence metadata remain durable and tied back to the producing execution |
| later explanation | `guild why`, `guild why --lineage`, explain/report skills | later explanation stays grounded in stored refs instead of chat memory |

## Operator Meaning Of Replay-Oriented Explanation

Today `replay-oriented explanation` means:

- explain or re-check what happened by reading the stored receipt, lineage, and
  evidence refs for a prior run
- compare durable records and evidence again later without needing the original
  chat context
- keep explanations anchored to host-owned stored state instead of guest-only
  narration

Today it does **not** mean:

- rerunning the skill automatically
- replaying mutations against a live environment
- reconstructing hidden provider state that Guild did not store
- claiming deterministic equivalence between a prior run and a future rerun

The safe shorthand is:

- Guild can explain from stored refs today
- Guild does not yet ship first-class replay execution

## Current Durable Inputs That Make Explanation Honest

The current explanation path is believable because Guild already stores:

- resolved executable identity
- requested and granted capability slices
- terminal execution outcome, including rejection records
- parent and child execution linkage where composition exists
- evidence payload references plus evidence metadata such as audience,
  redaction, MIME type, size, and producing execution

That is enough to support grounded explanation of what happened.
It is not enough by itself to authorize or safely drive future mutation replay.

## What Future Replay Execution Would Still Require

A true replay-execution feature should stay deferred until Guild can prove all
of the following in a host-owned way:

- explicit replay admission semantics rather than treating replay as a disguised
  inspect path
- approval references for risky replayed mutations
- idempotency keys or equivalent effect identities tied to the replay request
- durable capture of the inputs and effect descriptors needed to rerun the work
  honestly
- clear handling for external provider drift, partial side effects, and
  ambiguous prior outcomes
- audit records that distinguish explanation from a new effectful execution
- policy controls that can allow explanation while still denying replayed
  mutation

If those controls are missing, replay should fail closed as planning-only.

## Anti-Goals

Keep these boundaries explicit in docs, examples, and future issue bodies:

- do not describe `guild why`, lineage views, or explain/report skills as
  replay execution
- do not imply that durable receipts alone authorize a future rerun
- do not describe a future `guild replay` surface before approval,
  idempotency, audit, and effect semantics are real
- do not use replay language to smuggle mutation claims into read-only docs

## Done-When Restatement

Issue `#138` is done when the repo can talk about today's receipt chain and
replay-oriented explanation precisely, while keeping real replay execution
explicitly subordinate to later approval, idempotency, audit, and mutation
readiness.
