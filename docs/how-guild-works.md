# How Guild Works

This page is the short daily-user model for Guild.

It is not the normative contract source. Use `SPECS.md` when you need the exact runtime contract, and use `ARCHITECTURE.md` when you need the fuller implementation view.
For the current project framing, see [`project-positioning.md`](project-positioning.md).
For the bounded playbook concept entrypoint, use
[`strategy/guild-repositioning/04-playbook-surface-v1.md`](strategy/guild-repositioning/04-playbook-surface-v1.md).
For canonical operator-facing vocabulary, use
[`strategy/guild-repositioning/02-glossary-and-banned-terms.md`](strategy/guild-repositioning/02-glossary-and-banned-terms.md).
For the canonical operator-facing capability vocabulary, use
[`strategy/guild-repositioning/03-capability-taxonomy-v1.md`](strategy/guild-repositioning/03-capability-taxonomy-v1.md).

## The Short Version

Guild is being built around an operator flow: understand what a workflow is
allowed to do, run it under explicit host-owned authority, and inspect the
receipts and evidence afterward.

Today, the repo exposes that flow through skills rather than a first-class
playbook engine. You ask for a skill by a human-meaningful ref, Guild resolves
that request to installed executable state, runs it with host-owned authority
decisions, and leaves durable execution and evidence receipts behind.

The important part is that Guild keeps those boundaries explicit instead of
flattening them into "the tool ran somehow."

## Operator Model

Guild's target operator story is simple:

- review what a workflow is allowed to do before it runs
- run it in isolation under explicit capability policy
- inspect receipts and evidence after the run completes
- compare, explain, and replay from stored refs instead of chat-only memory

The current repo does not yet ship a broad playbook engine. It ships the trust,
identity, execution, and evidence surfaces that playbooks are meant to sit on
top of.

## Playbooks And Skills

Guild now has one bounded public playbook concept doc. Use that surface when
you want the operator-facing automation framing rather than the exact CLI or
runtime contract.

The honest translation to today's repo is:

- a playbook is the operator-facing automation unit and review surface
- one or more playbook steps still resolve to ordinary installed skills
- capability review and admission stay host-owned before those skills run
- durable receipts and evidence still come from the underlying skill executions

That is why the main docs now point at the playbook surface explicitly, while
still showing the real CLI, grants, and stored resources that exist today.

## Identity Layers

Guild uses three identity layers in normal operator workflows:

- source skill: the local source directory you pass to `guild install`
- installed executable state: the installed record stored under the selected Guild root
- resolved executable identity: the exact installed executable selected for use, identified by resolved ref plus artifact digest

One quick trace path is:

```bash
guild show -v skill://example/hello-inspect@^0.1
```

That one command lets you line up what you asked for, what Guild installed, and what exact executable identity Guild selected.

When you need the current "why did this ref resolve that way?" surface, use:

```bash
guild show -vv skill://example/hello-inspect@^0.1
```

That adds the installed-version and selected-digest reasoning on top of the identity trace.

## Authority Lifecycle

Guild also keeps authority staged instead of ambient:

- declared authority: capabilities declared by the installed manifest and visible in `guild show`
- requested authority: caller-requested grants passed to `guild run`
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

Guild does not hand the guest ambient authority. The host may reduce or deny caller-requested authority before guest start, and the runtime only exposes the final granted set.

Use `guild grants template <family>` when you want a read-only concrete JSON starting point for the active families before narrowing that request and passing it to `guild run`.

Operator-facing docs may describe that same review in external terms such as
`metrics:query` or `runs:inspect`, but current grant authoring still uses the
live internal family names such as `http-request` and `read-resource`.

## Command Roles

The main daily commands each answer a different question:

- `guild ls`: list installed skills and other objects in the current root
- `guild show`: what is installed or what stored object am I looking at?
- `guild grants template`: print read-only grant templates for the active families
- `guild run`: execute one installed skill locally
- `guild why`: explain one persisted execution record, point to nearby stored refs when present, summarize requested-versus-granted authority, and summarize stored authority observations
- `guild get`: read one Guild resource directly
- `guild verify`: show installed trust and verification state for a skill

The `guild grants` command group, `guild ls`, `guild show`, `guild why`, `guild get`, and `guild verify` are read-only surfaces.
`guild run` is the execution surface.

## Output Modes

Default human output is for reading, not parsing.
Short human summaries may include low-noise follow-up hints such as `Next: ...`
when the follow-up is obvious.
`guild why` may also include one nearby short execution or evidence ref when a
stored execution already points at related work. It also reports a compact
requested-versus-granted summary for that run. Use `guild why -v` when you
want the expanded nearby-ref lists, the requested-versus-granted authority
diff, and family-aware request hints for that execution. Use `guild why
--lineage` when you want the native bounded ancestor and descendant view over
persisted executions.

When you need a stable machine surface:

- use `--json` for structured machine-readable output
- use `--porcelain` for stable one-line machine-readable output

When a command supports `--json`, failure output stays on that machine surface:
stdout carries a JSON `error` envelope, stderr stays empty, and the process
still exits nonzero.

## A Normal Daily Flow

```bash
guild init
guild install examples/skills/hello-inspect
guild show -v skill://example/hello-inspect@^0.1
guild grants template emit-evidence
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
guild why exec:<execution-id-prefix>
guild get guild://executions/<execution-id>
guild verify skill://example/hello-inspect@^0.1
```

What that flow tells you:

- the source directory becomes installed executable state through `guild install`
- `guild show -v` explains the identity path before you run anything
- `guild show -vv` explains why the requested ref resolved to the selected digest
- `guild grants template` is the read-only starting point when you need current active-family grant JSON before a run
- `guild run` executes with caller-requested grants filtered through host policy
- `guild why` is the first nearby-ref, requested-versus-granted authority, and authority-observation surface after the run completes; `guild why -v` expands that stored diff and any family-aware request hints, `guild why --lineage` adds the native bounded ancestor/descendant view, `guild ls evidence --limit 5` discovers recent evidence refs, and `guild get` is the raw durable read path; move to narrower authority and policy example skills only when that native CLI path is no longer enough
- `guild verify` is about installed trust state, not execution replay

## Trust Review

Use the current trust review loop in this order:

- `guild trust list`
- `guild import ... --preview` or `guild pull ... --preview`
- `guild import ...` or `guild pull ...`
- `guild verify -v <skill-ref>`

Use `guild verify -v <skill-ref>` as the first installed-state verification explanation path after import or pull. That view keeps the publisher and combined trust status visible and adds signing-scheme and short bundle-digest detail when verification metadata exists.

Current installed-state terms:

- `local-source`: installed from local source in the current Guild root
- `verified-import`: installed from a signed import or pull that verified successfully
- `local-dev`: local source state in the current Guild root
- `trusted-imported`: imported publisher trusted for normal imported use
- `restricted`: imported publisher trusted only under restricted local policy posture

Keep these terms distinct:

- `guild verify` reviews installed trust and verification state for a skill.
- `guild why` explains one persisted execution, including policy outcomes for that run.
- `authority denial` means local policy denied required authority before guest start.
- `runtime/compatibility` means the active runtime could not honor the declared runtime surface.
- `trust/verification` means Guild could not verify a signed artifact or signed plan against the selected root.

Trust-store maintenance stays local and explicit:

- `guild trust add --identity-file <identity.json>` trusts one local publisher identity directly
- `guild trust add --record-file <record.json>` trusts one reviewed publisher record without secret key material
- `guild trust list` reviews trusted publishers and their current tiers
- `guild trust show <publisher-id>` inspects one reviewed publisher record under the selected local root
- `guild trust remove <publisher-id>` removes one local trust record when a publisher should no longer be trusted

## Planned Help Topics

One inspect-first preview is now shipped, and two future-facing directions are
fixed even where the commands or flags themselves are not implemented yet:

- `guild help inspect` previews the target inspect-first operator surface while keeping today's `guild show`, `guild why`, `guild get`, and `guild ls` explicit
- `guild help doctor` defines the first read-only diagnostic command direction
- `guild help preview` defines the first read-only preflight direction for risky
  `import` and `pull` flows

## Where To Go Next

- Use `guild help refs`, `guild help trust`, `guild help roots`,
  `guild help inspect`, `guild help doctor`, `guild help preview`, and `guild help grants` when you want the shipped CLI wording first.
- Read [`docs/command-language.md`](command-language.md) for the full public CLI surface.
- Read [`docs/mirroring-and-promotion.md`](mirroring-and-promotion.md) when you are moving reviewed installed state between roots or OCI locations.
- Read [`docs/testing.md`](testing.md) for proof commands and smoke flows.
- Read `SPECS.md` if you need exact contract language.
