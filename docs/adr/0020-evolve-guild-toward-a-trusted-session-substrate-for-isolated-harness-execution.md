# ADR 0020: Evolve Guild Toward A Trusted Session Substrate For Isolated Harness Execution

Status: accepted

## Context

Guild's current shipped slice is centered on portable skills, host-mediated
capability review, durable execution and evidence records, and a thin CLI/MCP
surface over that trust chain. That has been a useful implementation framing,
but it is too close to mechanism to carry the long-lived product story.

## Previous Framing

Guild was framed primarily as a local-first, capability-bounded skill runtime
with an operator-facing trust chain layered on top. The repo also used
playbook- and starter-set language to explain how users might consume that
runtime.

## New Framing

Guild should evolve into the admission controller, session broker, and receipt
engine for isolated harness execution.

Calls target a durable session. The platform resumes if possible, rehydrates if
necessary, and cold-starts if forced. The product abstraction is the session,
not the sandbox. Harness becomes the first-class isolated execution
abstraction.

## What Stays

- Rust remains the platform boundary.
- Host-owned capability policy and explicit grants remain core.
- Durable execution and evidence records remain core trust surfaces.
- Portable packaging, digest-pinned resolution, and current skill execution
  remain useful and continue to ship.
- The current inspect/plan/apply execution-mode split remains intact.

## What Changes

- Repo strategy and entrypoint docs now optimize for session and harness
  language instead of skill/runtime language alone.
- `session` becomes the primary user-facing abstraction.
- `harness` becomes the named isolated execution abstraction above raw runtime
  details.
- `sandbox lifecycle` is treated as internal implementation detail, not product
  surface.
- Future design and scaffolding work should prefer session/admission/receipt
  seams over broader runtime-plumbing narratives.

## What Is Deferred

- Any real session lifecycle manager
- Snapshotting or runtime-general rehydration
- A stable harness manifest or WIT contract
- A session-aware wake path in the live runtime
- Session-level receipt aggregation beyond docs and small shared scaffolding

## Why This Is An Evolution, Not Random Thrash

The existing Guild substrate already owns the right kinds of truth: host-issued
execution identifiers, capability-gated execution, durable records, and
evidence refs. The new framing does not reject that work. It clarifies what the
work is for.

Portable skills remain useful. They are just no longer the whole product story.
The new framing raises the abstraction to the durable session while preserving
the trust chain and the existing packaging/capability ideas that made the
current slice honest.
