# 00. North Star

**Status:** Proposed
**Owner:** Founding team
**Last updated:** 2026-03-28

## One-line product definition

**Guild lets ops, platform, and security teams run trusted playbooks that package steps, permissions, approvals, and evidence.**

## Thesis

Guild should stop presenting itself as a generic agent substrate and instead present itself as the trusted playbook layer for operational work.

The winning shape is:

- **playbooks** as the user-facing product
- **capabilities** as the permissionable action model
- **skills** as the portable procedural building blocks
- **receipts / evidence** as the trust layer

The product is not "agents doing things." The product is **bounded automation that a team can inspect, approve, replay, and trust**.

## Ideal user

Primary:

- platform engineer
- SRE / incident commander
- DevOps engineer
- security engineer
- staff+ engineer standardizing risky operational workflows

Secondary:

- engineering manager who wants repeatable incident / change handling
- internal developer platform team packaging operational knowledge

## Jobs to be done

1. Turn a tribal runbook into a reusable playbook.
2. Let an AI system execute useful work without getting vague or reckless.
3. Require approval before risky mutations.
4. Produce evidence that can be reviewed after the fact.
5. Package the workflow so it can move across products that understand skills.

## Product promises

Guild must make five promises and keep all five:

1. **Legibility** - users can tell what a playbook can do and what it cannot do.
2. **Portability** - packaged skills and playbooks move across compatible hosts.
3. **Control** - risky mutations can be gated by approval and policy.
4. **Evidence** - every serious run produces inspectable receipts.
5. **Verification** - first-party and curated packs are tested, scored, and labeled.

## What Guild is

Guild is:

- a packaging and execution layer for trusted operational playbooks
- a capability model for policy and review
- a way to turn runbooks into reusable, installable artifacts
- a system for replayable evidence and verification

## What Guild is not

Guild is not:

- an "agent operating system"
- a generic workflow engine for every domain
- a marketplace-first product
- a replacement for the underlying tools being called
- a promise of fully autonomous remediation with no human control

## Strategic pillars

### 1. Keep the wire format standard

Guild should compile to the open Agent Skills ecosystem, not fork away from it.

### 2. Make authoring humane

Human authors should write friendly YAML and example files, then let Guild compile that to the canonical distribution format.

### 3. Package outcomes, not fragments

Users should install **starter packs** and run **playbooks**, not collect a bag of disconnected skills.

### 4. Make trust visible

Trust has to be an inspectable artifact, not a marketing claim. Receipt chains, verification reports, and replay are the feature.

## Decision rules

Use these rules whenever a product choice is unclear:

1. Prefer the simpler external noun.
2. Prefer a user-visible playbook over an internal mechanism.
3. Prefer policy and evidence over more power.
4. Prefer compatibility over clever proprietary format changes.
5. Prefer first-party examples that mutate real systems safely over toy demos.
6. Prefer curated packs over open catalog sprawl.
7. Do not ship a generic feature when a trust-centered version would be stronger.

## Success metrics for the next phase

### Narrative success

- README first screen explains Guild without internal jargon.
- Landing page hero and CLI help use the same nouns.
- At least one external reader can summarize the product accurately after a 30 second skim.

### Product success

- A first-party pack can be authored, built, exported, installed, and run in under 15 minutes.
- At least 4 curated starter packs exist.
- At least 6 reference playbooks exist and are runnable.
- Every first-party playbook emits evidence and supports replay.

### Adoption success

- One real team can pilot Guild privately.
- One security reviewer can understand blast radius from capabilities and receipts.
- One manager can audit what happened without reading raw logs.

## Canonical messaging hierarchy

When describing Guild, use this order:

1. **Outcome:** trusted playbooks for ops and security automation
2. **Mechanism:** portable skills + capabilities + policy gates
3. **Proof:** receipts, replay, verification, starter packs

## Short approved copy

### 12-word version

**Trusted playbooks for operational work, with approvals, evidence, and replay.**

### 30-word version

**Guild turns runbooks into installable playbooks that agents can execute with clear capabilities, approval gates, and replayable evidence.**

### 80-word version

**Guild packages operational knowledge into portable skills and playbooks for SRE, platform, and security teams. Instead of vague agent behavior, Guild gives teams a capability model, policy gates before mutation, and evidence after execution. The result is automation that is both useful and reviewable.**
