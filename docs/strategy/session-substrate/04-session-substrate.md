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

## Lifecycle Invariants

- `active` is the only durable state that implies a live materialization
  exists.
- `suspended`, `rehydration-required`, `failed`, and `terminated` all imply
  that no live materialization currently exists.
- `suspended` is the only durable wake source that may later succeed with the
  `resumed` execution mode.
- `rehydration-required` is proof that direct resume is already invalid for the
  current session lineage. A later successful activation from this state may
  end only as `rehydrated` or `cold`, never `resumed`.
- `pending-admission` and `admitted` are attempt-local only. If an attempt
  stops, denies, or reroutes, Guild must resolve back into a durable state in
  the same receipt lineage instead of persisting a transient rest state.
- `pending-admission` and `admitted` do not by themselves prove whether a live
  materialization currently exists. First invoke uses those states before any
  harness exists, while warm reuse may pass through them while a prior live
  materialization is still running.
- `warm`, `resumed`, `rehydrated`, and `cold` are materialization outcomes for
  successful admitted attempts. They are not durable session states.

## Canonical Transition Rules

There is no durable "admitted but idle" rest state. `pending-admission` and
`admitted` exist only while Guild evaluates or materializes one attempt.

### First invoke and already-active invoke paths

| Source durable state | Trigger | Attempt path | Result | Notes |
| --- | --- | --- | --- | --- |
| no existing durable session | first invoke admitted | `pending-admission` -> `admitted` -> `active` | `active` with `cold` | First materialization of a new durable session |
| no existing durable session | first invoke denied | `pending-admission` -> `terminated` | `terminated` | Guild may still persist the denial receipt, but it must not leave a resumable durable session behind |
| `active` | invoke served by an already-live materialization | `active` -> `pending-admission` -> `admitted` -> `active` | `active` with `warm` | This is invoke-time admission, not wake-time reuse |
| `active` | clean park, disconnect, or host suspension | `active` -> `suspended` | `suspended` | Direct resume remains eligible until wake-time checks prove otherwise |
| `active` | live materialization is missing, invalidated, or otherwise no longer safe to reuse while durable truth still exists | `active` -> `rehydration-required` | `rehydration-required` | Direct resume is already off the table at this point |
| `active`, `suspended`, or `rehydration-required` | terminal completion or explicit close | `source` -> `terminated` | `terminated` | Terminal durable close |
| `active`, `admitted`, or `rehydration-required` | unrecoverable continuation failure | `source` -> `failed` | `failed` | Automatic wake stops here until a future explicit host-owned reset path exists |

### Wake from `suspended`

| Source durable state | Trigger | Attempt path | Result | Notes |
| --- | --- | --- | --- | --- |
| `suspended` | wake begins | `suspended` -> `pending-admission` | `pending-admission` | Guild reruns wake-time admission before any reuse claim |
| `pending-admission` (from `suspended`) | wake-time checks pass and direct resume remains allowed | `pending-admission` -> `admitted` -> `active` | `active` with `resumed` | Only `suspended` may reach `resumed` |
| `pending-admission` (from `suspended`) | wake denied | `pending-admission` -> `suspended` | `suspended` | Denial restores the prior durable state after the denial receipt is recorded |
| `pending-admission` (from `suspended`) | wake-time checks prove direct resume is no longer safe | `pending-admission` -> `rehydration-required` | `rehydration-required` | Guild must not quietly collapse this into same-attempt `cold`; the durable state first records that direct resume is invalid |

### Wake from `rehydration-required`

| Source durable state | Trigger | Attempt path | Result | Notes |
| --- | --- | --- | --- | --- |
| `rehydration-required` | wake begins | `rehydration-required` -> `pending-admission` | `pending-admission` | Fresh wake attempt against an already-non-resumable session |
| `pending-admission` (from `rehydration-required`) | rehydration admitted and succeeds | `pending-admission` -> `admitted` -> `active` | `active` with `rehydrated` | Rebuild from durable truth and compatible artifacts/state |
| `pending-admission` (from `rehydration-required`) | rehydration is unavailable but durable truth plus immutable artifacts still support a safe fresh materialization | `pending-admission` -> `admitted` -> `active` | `active` with `cold` | `cold` is the safe fallback after resume is already ruled out |
| `pending-admission` (from `rehydration-required`) | wake denied | `pending-admission` -> `rehydration-required` | `rehydration-required` | Denial restores the prior durable state |

### Disallowed shortcuts

- `suspended` must not jump straight to `active` with `cold` in the same wake
  attempt that disproves direct resume. Guild first records
  `rehydration-required`, then retries from that durable state on a later
  attempt.
- `rehydration-required` must never land in `active` with `resumed`.
- `failed` is not a normal wake source. Any future recovery from `failed` must
  be an explicit host-owned reset path, not an implicit resume or rehydrate
  branch.
- `terminated` is terminal. Guild must not transition a terminated session back
  into `pending-admission`, `admitted`, `active`, `suspended`, or
  `rehydration-required`.
- A denied attempt must never strand the session in `pending-admission` or
  `admitted`.

## Persistence Tiers

- `ephemeral`: runtime-only state that may be lost without breaking the session
  contract
- `durable-session`: host-owned state needed to continue, rehydrate, explain,
  or audit the session
- `durable-artifact`: packaged inputs and immutable artifacts used to rebuild a
  harness

## Durable Host Truth Versus Rebuildable Harness State

Canonical durable host truth is the minimum continuity contract across
`resumed`, `rehydrated`, and `cold` execution modes.

Canonical durable host truth:

- session identity and current durable lifecycle state
- admission-relevant requested intent and caller correlation data
- granted capability envelope or enough policy input to recompute it safely
- references to required artifacts, runtime class, and harness identity mapping
- receipt lineage, evidence refs, and host-owned audit metadata
- explicit durable session data the host promised to preserve above one runtime
  instance
- reconnect descriptors and rebinding requirements for external services when
  Guild expects a later wake to re-establish them

Rebuildable harness state:

- concrete sandbox, process, container, VM, or placement identity
- in-memory heap, runtime-local caches, temp directories, and file descriptors
- live sockets, opaque client handles, service leases, and other active
  connection state
- snapshot blobs or serialized runtime state used only as optional resume or
  rehydration aids

Snapshot blobs are not canonical session truth. They may help one wake path,
but the durable session contract must still be explainable without pretending a
snapshot handle is the real session.

## Survival Rules By Execution Mode

| Execution mode | Must already survive before Guild chooses the path | What Guild may reuse | What Guild must treat as lost, rebuilt, or freshly proven |
| --- | --- | --- | --- |
| `resumed` | Canonical durable host truth and a still-valid suspended materialization | Preserved runtime-local memory, process state, and active service sessions only if wake-time checks prove they remain safe | Any stale secret, mount, network, placement, or external-service assumption that wake-time checks cannot re-prove |
| `rehydrated` | Canonical durable host truth, durable artifacts, and any explicitly persisted session data needed to rebuild the harness | Validated serialized runtime state or snapshot content as rebuild input only after compatibility checks pass | Prior live handles, sockets, placement-specific state, and any runtime-local data that was never promoted into durable host truth |
| `cold` | Canonical durable host truth plus immutable artifacts needed for a fresh materialization | The same `SessionId`, durable receipts, durable evidence refs, and immutable packaged artifacts | All runtime-local continuity, including snapshots, live connections, and caches, unless that continuity was separately captured as durable host truth |

## External Service Reconnect Boundary

Guild may persist enough host-owned truth to reconnect an external service on a
later wake, but that does not make the live connection itself durable.

- Durable host truth may remember service identity, endpoint references,
  negotiated scopes, and reconnect prerequisites.
- Durable host truth must not pretend an open socket, bearer session, lease, or
  opaque client handle can survive suspension by definition.
- `resumed` may continue using an external service only if wake-time checks
  prove the existing connection state is still valid.
- `rehydrated` must reconnect through a host-mediated path using durable truth
  plus fresh policy checks.
- If Guild cannot safely reconnect the service or rebind its policy-critical
  state, it must fall back to `cold` or fail the wake rather than fake
  continuity.

## Invalid Snapshot And Cold-Start Rules

Snapshots and serialized runtime state are acceleration aids, not a second
source of truth.

- Invalid, missing, incompatible, or policy-stale snapshots must never be
  treated as proof that `resumed` is still safe.
- A broken snapshot may still allow `rehydrated` if Guild can rebuild from
  other durable host truth and artifacts without depending on the invalid data.
- `cold` is the required safe fallback when direct resume is no longer valid
  and no trusted rehydration input remains, but the durable session contract is
  still satisfiable from host-owned truth plus immutable artifacts.
- If the session's promised continuity depended on state that only ever lived in
  rebuildable harness memory, Guild should fail the wake rather than pretend a
  fresh `cold` materialization preserved it.

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
