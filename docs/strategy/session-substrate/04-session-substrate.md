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

Proposed states:

- `pending-admission`
- `admitted`
- `active`
- `suspended`
- `rehydration-required`
- `terminated`
- `failed`

Proposed transitions:

- invoke -> `pending-admission`
- admission allow -> `admitted`
- materialized -> `active`
- disconnect or host suspension -> `suspended`
- invalid or missing runtime materialization with durable state intact ->
  `rehydration-required`
- unrecoverable failure -> `failed`
- explicit close or terminal completion -> `terminated`

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
- Cold-start fallback is an expected safe path, not a platform failure.
- Session wake decisions may require fresh policy checks before reusing secrets,
  mounts, network access, or runtime placement.
