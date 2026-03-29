# Authoring Layer Guardrails

This page bounds any future Guild authoring ergonomics work so it does not
turn into a second contract surface.

It is not a runtime-contract source. For normative runtime ownership, use
[`../SPECS.md`](../SPECS.md), [`../wit/guild-skill-v1.wit`](../wit/guild-skill-v1.wit),
[`../ARCHITECTURE.md`](../ARCHITECTURE.md), and the core Rust runtime/types.
For the current project framing and the follow-on issue sequence that led to
this doc, use [`project-positioning.md`](project-positioning.md) and
[`roadmap/epics/portable-skill-receipts-and-reference-apps-execution-guide.md`](roadmap/epics/portable-skill-receipts-and-reference-apps-execution-guide.md).

The short rule is simple: ergonomic authoring can exist, but runtime truth
stays contracts-first.

## Source-Of-Truth Matrix

Docs are not one undifferentiated class. Some repo surfaces are normative,
some are derived from those normative surfaces, and some are advisory only.

| Surface | Current class | What it decides | Trusted directly by runtime? |
| --- | --- | --- | --- |
| `crates/guild-types` and `crates/guild-runner` | normative | Host-owned execution model, capability evaluation, durable record shape, and rejection behavior | yes |
| `crates/guild-manifest` plus checked `manifest.json` files | normative | Installed skill declaration, requirements, dependency snapshots, and manifest validation | yes |
| `wit/guild-skill-v1.wit` | normative | Guest ABI, active inspect world, and import/export boundary | yes |
| `SPECS.md` | normative | Human-facing runtime contract and frozen vocabulary | yes, as the repo's normative contract text paired with code |
| Generated matrices, compatibility tables, benchmark outputs, install reports, and similar checked outputs | derived | Regenerated or host-produced views over normative truth | no |
| `ARCHITECTURE.md`, `README.md`, `docs/project-positioning.md`, roadmap docs, and examples | advisory | Explanation, planning, onboarding, and product framing | no |
| Repo-scoped `.agents/skills/**/SKILL.md` files | advisory | Coding-agent workflow helpers and repo-local task instructions | no |
| Future authoring inputs such as `guild.skill.yaml`, `guild.playbook.yaml`, or `guild-pack.yaml` | advisory until explicitly compiled down | Human-friendly source material for generation or linting | no |

The runtime may read generated or installed artifacts that an authoring layer
produces, but it still trusts the compiled manifest, WIT, Rust types, and
runtime checks directly rather than the authoring source itself.

## Classification Rubric

Use this rubric for any future authoring metadata discussion:

| Class | Meaning | Current examples | Allowed effect |
| --- | --- | --- | --- |
| normative | The runtime or installer validates and depends on this data directly. | manifest identity, dependency aliases, required capabilities, WIT world shape, frozen runtime vocabulary | May affect install, resolution, execution, or verification because the runtime already checks it |
| derived | Produced from normative truth by the host, tooling, or checked generators. | support matrices, compatibility reports, trust labels from current signals, install reports, verification summaries | May inform review or labeling, but is never accepted as primary input |
| advisory | Helpful to humans or generators but not trusted as execution truth. | `use_cases`, `risk`, `examples`, `eval_scenarios`, prose approval notes, `.agents/skills/**/SKILL.md` | May drive docs, linting, or generation hints only |

Current default classification for candidate authoring metadata is narrow:
`use_cases`, `risk`, `examples`, and `eval_scenarios` are advisory today.
They do not become executable semantics unless a later change gives them an
explicit compile-down target in the current manifest, WIT, or Rust contract.

## Compile-Down Boundary

Any future authoring layer may help authors write and organize Guild content,
but it must compile down into the current trusted surfaces instead of
competing with them.

What an authoring layer may generate:

- `manifest.json` and companion checked schema/example files
- repo docs, examples, and starter templates
- lint output about missing evidence, approval, or review guidance
- compile-time warnings about metadata that has no runtime effect yet

What the runtime must still verify directly:

- manifest identity, versioning, requirements, and dependency wiring
- capability requirements and granted capability evaluation
- WIT compatibility and active inspect-world imports
- execution, evidence, trust, and verification behavior in the Rust host

The compile-down rules stay fail-closed:

1. If a field changes runtime behavior, it must compile down into current
   manifest, WIT, or Rust truth that the runtime already validates.
2. If a field cannot compile down exactly, the authoring layer should fail
   closed instead of inventing hidden semantics.
3. Generated normative files must stay reviewable, diffable, and subordinate
   to the current installed/runtime truth.
4. Advisory-only fields may guide docs or linting, but they must not grant
   authority, relax policy, or widen runtime support on their own.

## Failure Modes To Keep Explicit

When evaluating a future authoring layer, treat these outcomes as the honest
ones:

- an unmappable runtime claim is a hard error, not a best-effort guess
- a docs-only hint with no compile-down target stays advisory and has no
  runtime effect
- generated output that conflicts with the checked manifest, WIT, or Rust
  model must be rejected or regenerated, not silently merged
- `SKILL.md` may explain how a coding agent should work in this repo, but it
  does not become the canonical execution contract for Guild

## Anti-Goals

- Do not create a second normative contract surface alongside Rust, manifests,
  WIT, and `SPECS.md`.
- Do not let `SKILL.md` or future YAML authoring inputs become runtime truth by
  inertia.
- Do not let prose metadata imply capabilities, approvals, or evidence
  semantics that the runtime cannot verify directly.
- Do not add a friendlier authoring layer by teaching the runtime to accept new
  unchecked source formats before the compile-down boundary is explicit.
- Do not use authoring ergonomics to smuggle in a new pack type, workflow
  engine, or marketplace layer.

## Working Rule

When a future authoring-layer proposal comes up, ask one question first:

Can this compile down cleanly into today's manifest, WIT, Rust, and spec truth
without changing what the runtime trusts?

If the answer is no, the proposal is still planning input, not Guild contract
surface.
