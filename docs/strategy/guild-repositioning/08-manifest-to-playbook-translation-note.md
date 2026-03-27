# Manifest-To-Playbook Translation Note

This note is a translation guide, not a new manifest spec or runtime design.
Use it to map a current repo example into the public playbook framing without
claiming that Guild already ships a first-class playbook engine.

## Translation Anchor

Use [`../../../examples/skills/guild-ops-starter/README.md`](../../../examples/skills/guild-ops-starter/README.md)
as the first repo-grounded anchor.

Guild Ops Starter is a good bridge document because it already presents a small
operator story over real Guild receipts, evidence records, and bounded query
resources while staying explicit about today's trust and runtime boundaries.
It is also the honest entrypoint for a "manifest-to-playbook" translation
because today's manifests still live with the individual skills rather than in
one standalone playbook file or pack manifest.

## What Changes In The Framing

When maintainers describe Guild Ops Starter in playbook terms, the wording can
change in these narrow ways:

- the starter set reads as one operator-facing playbook family rather than just
  a collection of example skills
- the README journey map reads like playbook intent: explain one run, compare
  two runs, scan failures, inspect evidence
- the operator-facing capability names such as `runs:inspect` or
  `failures:query` become the review language a future playbook surface would
  show first
- the installed skills become the reusable execution steps that a future
  playbook layer could point at

## What Does Not Change Underneath

The runtime and trust facts stay exactly where they are today:

- Guild Ops Starter is still a small set of ordinary installed skills
- each referenced skill still resolves to immutable installed executable
  identity before execution
- caller-requested authority is still narrowed by host-owned policy before
  guest start
- the live grant JSON still uses the current internal family names such as
  `read-resource` and `invoke-skill`
- durable execution receipts and evidence records still live at the underlying
  skill execution layer
- `render-report` remains a bounded zero-authority formatter child, not a new
  workflow engine or orchestration runtime

## Current Example To Playbook Reading

| Current repo artifact | Playbook-facing reading | Unchanged underlying fact |
| --- | --- | --- |
| Guild Ops Starter journey map | One small operator playbook family for read-only operational review | Installation still happens skill by skill through `guild install` |
| `incident-brief`, `run-diff`, `recent-failures`, `evidence-summary` | Reusable playbook steps or reference skills | Each one still executes as an ordinary installed skill |
| Operator-facing names like `runs:inspect` | Capability review language shown to the operator | Concrete grant authoring still uses `read-resource` / `invoke-skill` today |
| `guild why`, `guild get`, and stored Guild URIs | The receipt and evidence context around a future playbook run | Stored Guild resources remain the durable host-owned truth |
| `render-report` child alias | Internal helper step hidden behind the operator story | Composition is still the current bounded alias invoke path only |

## One Honest Step Translation

Journey 1 from Guild Ops Starter, "explain one stored execution", can be read in
playbook terms like this:

- operator intent: explain one persisted execution and produce a compact report
- operator input: one `guild://executions/<id>` ref
- operator-facing capability review: `runs:inspect`
- underlying execution unit: `skill://example/incident-brief@^0.1`
- underlying granted authority: bounded `read-resource` on
  `guild://executions/` plus bounded `invoke-skill` for alias `renderer`
- receipts and evidence: the existing stored execution and evidence resources,
  plus the new execution receipt for the `incident-brief` run itself

That translation changes how the workflow is described to operators, but it
does not change what actually executes, what authority is granted, or where the
durable records live.

## How To Use This Note

- Use [`04-playbook-surface-v1.md`](04-playbook-surface-v1.md) for the bounded
  public playbook concept and minimum schema shape.
- Use this note when you need to explain how today's manifest-driven examples
  map into that concept honestly.
- Use [`07-reference-playbooks.md`](07-reference-playbooks.md) when you need
  the next operator stories Guild should eventually support beyond the current
  read-only starter examples.
