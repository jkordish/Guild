# Repo-Local Launch Copy Pack

This document is the reusable launch copy source for the repo-owned surfaces
that support the current Guild repositioning work. It is copy guidance, not a
runtime-contract source.

Use it for:

- `README.md` summary and trust-story refreshes
- release-note or release-PR blurbs
- GitHub issue-filing language
- explicit handoff notes for website work that lives outside this repo

## Guardrails

- Keep the current trust chain explicit:
  `admission -> bounded execution -> receipt -> evidence -> replay-oriented explanation`.
- Stay honest about today's repo: skills are the shipped execution unit;
  playbooks are the target operator surface.
- Describe replay today as replay-oriented explanation over stored refs, not a
  first-class replay engine.
- Do not claim broader runtime or capability coverage than `SPECS.md`,
  `docs/testing.md`, and the checked proof surfaces support.

## README Summary Blurb

Guild is trusted operational automation for engineering teams. Operators review
and admit a workflow under explicit capability policy, run it in isolation, and
keep receipts and evidence they can inspect later. Today the repo ships that
trust chain through skills, durable Guild refs, and bounded proof-backed
runtime slices, while the broader playbook surface remains a docs-first
direction.

## Release-Note Blurb

This release tightens Guild's operator-facing trust story around one honest
chain: admission, bounded execution, receipt, evidence, and replay-oriented
explanation. The docs now explain why Guild is safer than ad hoc automation
without overstating the current support frontier. Playbooks remain the target
operator surface; skills, receipts, evidence, and explain/report flows remain
the shipped path today.

## Issue-Filing Blurb

Use this when opening repo-local epics or tasks tied to the relaunch:

This issue should improve Guild as trusted operational automation for
engineering teams. Keep the operator story grounded in the current trust chain,
use the approved glossary, and separate shipped behavior from target playbook
or replay surfaces when the repo has not implemented them yet.

## Repo-Owned Launch Surfaces

- `README.md`: lead with operator value, link to the trust walkthrough, and
  keep current-state caveats visible.
- Release notes or release PR description: use the release-note blurb above,
  then cite proof or validation links when needed.
- `.github/ISSUE_TEMPLATE/ux-epic.md` and `.github/ISSUE_TEMPLATE/ux-task.md`:
  use the issue-filing blurb in short form for issue setup language.
- `docs/strategy/guild-repositioning/`: keep strategy docs aligned, but do not
  turn them into a parallel marketing site.

## External Website Follow-Up

There is no in-repo website source today.

Track any external site rollout outside this repository and keep the handoff
scoped to:

- homepage hero and explainer copy derived from the trust-chain summary
- launch-page proof links that point back to repo-owned docs
- any CTA, pricing, or site-navigation work owned by the external website repo,
  not this one

## Do Not Say

- Guild already ships a first-class playbook engine.
- Guild already ships a first-class replay engine.
- Every capability in the operator taxonomy is runnable today.
- Receipts or evidence are compliance guarantees by themselves.
