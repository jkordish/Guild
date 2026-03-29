# Verification Matrix And Curated Labels

This page defines the first honest labeling story for future curated install
views, starter sets, and reference playbooks built on Guild's current trust
signals.

It is not a runtime-contract source and it does not create a new pack
contract. Guild still ships skills, receipts, evidence, trust review, and
transport review on today's local-first surfaces. These labels are a
presentation layer over those existing host-owned signals, not a bypass around
them.

For the current project framing, use [`project-positioning.md`](project-positioning.md).
For the follow-on issue sequence that led to this doc, use
[`roadmap/epics/portable-skill-receipts-and-reference-apps-execution-guide.md`](roadmap/epics/portable-skill-receipts-and-reference-apps-execution-guide.md).
For the current install-review loop and transport boundary, use
[`mirroring-and-promotion.md`](mirroring-and-promotion.md) and the trust review
sections in [`../README.md`](../README.md) and
[`how-guild-works.md`](how-guild-works.md).

The short rule is simple: label only what the repo can currently prove, and
keep future scoring ideas out of current status claims.

## Working Rules

- Start from signals the repo already emits on the current CLI, transport, and
  checked-truth paths.
- Keep `current` and `future` fields explicit so later ideas do not get
  smuggled into today's labels.
- Treat `verified-import` as an installed-state fact for one skill in one
  target root, not as a whole-asset guarantee by itself.
- Do not turn labels into automatic ranking, safety scoring, or mutation
  readiness claims.

## Current Signal Inventory

| Signal | Current source today | Class today | What it proves now | What it does not prove |
| --- | --- | --- | --- | --- |
| Exact executable identity | `guild show -v`, `guild show -vv`, `guild verify -v` | current | requested ref, resolved ref, digest, and installed-state context are explicit and reviewable | cross-host compatibility by itself, or a broader pack guarantee |
| Target-root transport verification | `guild import ... --preview`, `guild pull ... --preview`, `guild verify -v` | current | signed transport can be reviewed against one target Guild root, including verification result, trust tier, and refusal reason | that every claim made by docs or examples is therefore verified |
| Installed-state classification | `guild verify`, `guild verify -v` | current | whether one installed skill is `local-source`, `verified-import`, `local-dev`, `trusted-imported`, or `restricted` in the selected root | broader labeling for a starter set, playbook, or curated view |
| Publisher review and trust tier | `guild trust list`, `guild trust show`, preview output, `guild verify -v` | current | a local operator reviewed a publisher in the selected root and what trust tier applies there | global trust, org-wide promotion state, or remote trust sync |
| Durable receipt and evidence chain | `guild why`, `guild get`, `guild://executions/...`, `guild://objects/records/...`, evidence metadata URIs | current | a runnable slice can point to stored execution and evidence refs after the fact | replay execution, mutation readiness, or universal correctness |
| Proof-backed support frontier | `docs/testing.md`, `docs/schemas/draft-v1/family_support_matrix.json`, `docs/schemas/draft-v1/benchmark_matrix.json`, checked `xtask` truth flows | derived from current truth | which capability slices are supported, bounded, unsupported, or `not_proven` on the checked path | a general "works everywhere" guarantee or support outside the named slices |
| Docs-first boundary and deferrals | roadmap epic docs, examples docs, quickstarts | derived from current truth | whether a playbook or starter progression is real now, docs-first, or deferred | transport verification, runtime proof, or publisher trust on its own |

## Current Verification Matrix

Use this matrix when deciding what a future curated view may claim today.

| Review question | Current field or signal | Current use today | Future field, not current | Honest reading today |
| --- | --- | --- | --- | --- |
| Can we name the exact thing being reviewed? | requested ref, resolved ref, digest, installed path | yes | richer pack manifest summaries | every label starts from exact executable identity, not a fuzzy title |
| Can we review signed transport into one target root? | preview `decision`, preview `verified`, `trust_tier`, refusal reason, `guild verify -v` output | yes | org-wide promotion lanes or remote trust sync | labels may talk about reviewed import or pull state only when that root-local review actually happened |
| Can we explain what happened after execution? | durable execution receipt, evidence records, `guild why`, `guild get` | yes | first-class replay execution semantics | executable review paths can claim stored receipts and evidence, not replay mutation |
| Can we show proof-backed support for claimed capability surfaces? | support matrix, benchmark report, checked proof flows, `supported` / `bounded` / `not_proven` vocabulary | yes | eval pass rate, mutation-risk scoring, broad runtime-general minimization scores | a label may only claim the currently checked slices it can point to explicitly |
| Can we score mutation safety or blast radius? | none | no | approval quality score, mutation-risk score, retry-health score | do not include this in current labels |
| Can we auto-rank compatibility across hosts or environments? | none beyond current manifest/runtime compatibility facts and target-root verification review | no | compatibility rank, promotion rank, install confidence score | do not present a ranking story today |

## Label Semantics

Use a small, honest vocabulary:

| Label | Minimum current bar | What it still cannot imply |
| --- | --- | --- |
| `experimental` | The asset is visible on purpose, but one or more current surfaces are still docs-first, local-source-only, or explicitly outside the current proof frontier. Current versus future claims stay visible. | trusted import, proof-backed support for every claimed surface, mutation readiness, or broad compatibility |
| `curated` | The asset was reviewed against the current signal inventory. Its claims stay inside today's trust, transport, receipt, evidence, and support-frontier language. Docs-first pieces are marked as docs-first instead of being blended into current support. | automatic safety, universal installability, or a guarantee that every step is already proof-backed |
| `verified` | The asset is already curated, and every claimed current surface is backed by current signals: exact executable identity, target-root trust/verification review where installable, and explicit proof-backed or shipped support for each named capability slice. If it claims executable review paths, it must also point to durable receipt/evidence behavior that exists today. | correctness scoring, eval ranking, mutation safety, replay execution, or any future-only field |

## Promotion Bar

Treat label changes as a promotion bar, not as marketing language.

1. `experimental` -> `curated`

- review the asset against the current signal inventory
- remove or clearly mark any future-only claims
- keep transport, trust, and support-frontier language tied to today's repo truth

2. `curated` -> `verified`

- confirm exact executable identity for the current runnable slice
- confirm target-root signed import or pull review where installability is part of the claim
- confirm every claimed current capability surface is already shipped or proof-backed on the checked path
- confirm any executable review path can actually point to durable receipts or evidence today

## What Does Not Qualify As `Verified` Yet

- A local-source example or starter slice with no target-root signed import or
  pull review. It may still be curated, but it is not verified on transport
  grounds alone.
- A docs-first playbook progression such as `service-recovery review pack`.
  It is intentionally visible, but it is not yet installable or transport
  verified, and its action steps remain future.
- Any artifact that claims broad `http-request`, broad `invoke-skill`, broader
  `emit-evidence`, or mutation-oriented capability support outside the current
  checked slices.
- Any asset whose strongest argument is a future idea such as eval pass rate,
  mutation-risk scoring, or compatibility ranking. Those are future fields, not
  current verification inputs.
- Any asset that is merely `verified-import`. That installed-state result is a
  necessary signal for one skill in one root, but it is not sufficient to label
  a broader starter set, playbook, or curated view as `verified`.

## Working Rule

If a future curated view wants the label `verified`, ask one question first:

Can every current claim it makes be traced back to today's exact executable
identity, target-root trust review, and proof-backed or shipped support
signals?

If the answer is no, the honest label is `experimental` or `curated`, not
`verified`.
