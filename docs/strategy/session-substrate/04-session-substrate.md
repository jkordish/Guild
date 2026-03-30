# Session Substrate

## Session Identity Above Runtime Identity

A session is a durable host-owned identity that persists above any individual
runtime instance. Runtime identity is replaceable. Session identity is the
stable thing a caller addresses.

## Canonical Session Identity Rule

Guild mints the canonical durable session identifier on the host side. Callers
may name an intent, correlation key, or prior session reference, but they do
not define the canonical durable `SessionId`.

That rule keeps persistence ownership explicit:

- the host owns session identity minting
- the host owns the durable session record keyed by that identity
- the host owns the mapping from that session to resolved harness or artifact
  identity
- the host owns the mapping from that session to any current runtime
  materialization

Sandbox IDs, process IDs, container IDs, VM IDs, and snapshot handles are all
replaceable implementation details. They are never the canonical session
identity.

## Execution Modes

- `warm`: the target session is already materialized and can continue in-place
- `resumed`: the target session can continue from preserved durable session
  state without rebuilding from scratch
- `rehydrated`: the target session cannot resume directly, but Guild can
  rebuild a valid materialization from durable session state and artifacts
- `cold`: Guild must start a fresh materialization because no safe resume or
  rehydration path exists

## Lifecycle States

State kinds:

- transient attempt states: `pending-admission`, `admitted`
- durable session states: `active`, `suspended`, `rehydration-required`,
  `failed`, `terminated`

State meanings:

- `pending-admission`: Guild has minted or targeted a `SessionId` for one
  invoke or wake attempt and is still evaluating admission. This is not a
  durable rest state.
- `admitted`: one specific attempt was allowed and has a computed envelope and
  isolation posture, but no active materialization exists yet. This is also
  transient.
- `active`: the session currently has a live materialization serving work.
- `suspended`: no live materialization exists, but direct resume is still an
  eligible wake path if wake-time checks pass.
- `rehydration-required`: direct resume is already invalid or disallowed; only
  rehydration or an explicitly permitted cold materialization may continue.
- `failed`: Guild cannot continue the same session automatically. This is a
  stop state for automatic wake logic.
- `terminated`: the session lifecycle is closed. The same `SessionId` must not
  reactivate.

## Canonical Transition Rules

There is no durable "admitted but idle" rest state. `pending-admission` and
`admitted` exist only while Guild evaluates or materializes one attempt.

- first invoke for a session: `pending-admission` -> `admitted` -> `active`
  on success, typically with `cold`
- deny before first activation: `pending-admission` -> `terminated`; Guild may
  still persist the denial receipt, but it must not leave a resumable session
- active reuse on a live materialization: `active` -> `pending-admission` ->
  `admitted` -> `active` with `warm`
- clean park, disconnect, or host suspension: `active` -> `suspended`
- invalidated or missing live materialization with durable session truth still
  intact: `active` -> `rehydration-required`
- terminal completion or explicit close: `active` -> `terminated`,
  `suspended` -> `terminated`, or `rehydration-required` -> `terminated`
- unrecoverable continuation failure: `active` -> `failed`,
  `admitted` -> `failed`, or `rehydration-required` -> `failed`

Wake-specific rules:

- suspended wake request:
  `suspended` -> `pending-admission` while Guild reruns wake-time checks
- successful direct resume:
  `pending-admission` -> `admitted` -> `active` with `resumed`
- wake denial against an existing suspended session:
  `pending-admission` -> `suspended` after recording the denial receipt
- wake-time proof that direct resume is no longer safe:
  `pending-admission` -> `rehydration-required`; Guild must not quietly pretend
  the session is still resumable
- rehydration-required wake request:
  `rehydration-required` -> `pending-admission`
- successful rehydration:
  `pending-admission` -> `admitted` -> `active` with `rehydrated`
- successful fresh materialization after resume and rehydrate were ruled out:
  `pending-admission` -> `admitted` -> `active` with `cold`
- wake denial against an existing rehydration-required session:
  `pending-admission` -> `rehydration-required`

Stop-state rules:

- `failed` is not a normal wake source. Any future recovery from `failed` must
  be an explicit host-owned reset path, not an implicit resume or rehydrate
  branch.
- `terminated` is terminal. Guild must not transition a terminated session back
  into `pending-admission`, `admitted`, or `active`.

## Persistence Tiers

- `ephemeral`: runtime-only state that may be lost without breaking the session
  contract
- `durable-session`: host-owned state needed to continue, rehydrate, explain,
  or audit the session
- `durable-artifact`: packaged inputs and immutable artifacts used to rebuild a
  harness

## What Must Survive Resume

Must survive:

- session identity
- admission-relevant requested intent
- granted capability envelope or enough data to recompute it safely
- receipt lineage and evidence refs
- references to required artifacts and runtime class

May be rebuilt:

- concrete sandbox instance
- internal process handles
- transient network connections
- caches that are explicitly non-durable

## Explicit Constraints

- Guild must not claim rehydration is safe if external service reconnect
  requirements are unresolved.
- Invalid snapshots or incompatible persisted runtime state must force
  rehydration or cold-start, not undefined resume.
- A denied wake must restore the prior durable state of an existing session. It
  must not strand the session in `pending-admission` or `admitted`.
- `suspended` means direct resume is still eligible. `rehydration-required`
  means direct resume is already off the table until Guild successfully
  rehydrates or cold-starts a new materialization.
- `cold` is a materialization outcome, not a durable session state. The
  separate question of what durable session truth survives a cold
  materialization stays scoped to the persistence-tier rules.
- Cold-start fallback is an expected safe path, not a platform failure.
- Session wake decisions may require fresh policy checks before reusing secrets,
  mounts, network access, or runtime placement.
