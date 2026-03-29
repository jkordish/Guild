# 01. Messaging Audit

**Status:** Proposed rewrite plan
**Owner:** Product + founder
**Last updated:** 2026-03-28

This is a rewrite plan for the repo and site narrative. It focuses on the repeated problems that make Guild feel more abstract than it needs to be.

## Core diagnosis

Guild currently risks sounding like:

- a substrate in search of a job
- a technical framework explained from the inside out
- a packaging idea without a user-facing outcome
- a trust story that appears too late

None of those are fatal. They are just common founder-project gravity. Humans love building a machine and then explaining the pistons first.

## The five messaging problems to fix

### 1. Substrate-first framing

If the site leads with internals, schemas, artifacts, or orchestration mechanics, the reader has to infer the actual user value.

**Fix:** lead with *trusted playbooks for operational work*.

### 2. Too many nouns

If the site rotates through terms like skills, artifacts, workflows, packages, receipts, adapters, and agents with no strict hierarchy, the reader loses the plot.

**Fix:** keep exactly three primary nouns up front:

- capability
- skill
- playbook

"Receipt" and "pack" are supporting nouns, not headline nouns.

### 3. Audience leakage

If the copy sounds like it is for any team doing any kind of AI work, it will feel generic.

**Fix:** talk directly to ops / platform / security teams doing risky, real-world work.

### 4. Trust story is buried

If approval gates, evidence, replay, and verification do not appear immediately, Guild looks like another automation toy.

**Fix:** trust claims need to show up in the hero and in every product explanation.

### 5. Examples are too abstract

A reader should see a concrete operational playbook in the first screen or very close to it.

**Fix:** show one live example immediately.

## Recommended narrative hierarchy

Use this order everywhere:

1. **Category:** trusted playbooks for ops and security automation
2. **User value:** package approvals, steps, permissions, and evidence once
3. **Mechanism:** skills + capabilities + policy + receipts
4. **Proof:** starter packs, replay, verification

## Homepage rewrite

### Recommended hero

**Headline**

> Trusted playbooks for ops and security automation.

**Subhead**

> Guild turns runbooks into installable playbooks that agents can execute with clear capabilities, approval gates, and replayable evidence.

**Primary CTA**

> Run a reference playbook

**Secondary CTA**

> Inspect a receipt

### Recommended three-up value blocks

**Package operational knowledge**
Turn an incident or change workflow into a reusable playbook instead of re-explaining it every time.

**Gate risky mutations**
Capabilities and approval rules make it clear what can happen before anything touches production.

**Replay and verify**
Every serious run produces evidence that can be inspected, replayed, and scored.

## README rewrite

### Recommended opening paragraph

> Guild is a playbook layer for trusted operational automation. It packages runbooks into portable skills and playbooks that agents can use across compatible environments, while keeping policy, approvals, and evidence visible.

### Recommended first example

> Example: diagnose a degraded service, inspect logs and metrics, request approval, restart the Kubernetes workload, verify recovery, and post the outcome to the incident channel with a receipt.

### Recommended quick bullets

- Portable skills and packs
- Human-readable capabilities
- Approval gates before mutation
- Replayable evidence after execution
- Verified first-party playbooks

## Feature-page rewrite anchors

### Packaging

Say:

> Install curated packs for incident response, Kubernetes remediation, safe changes, and secrets or edge operations.

Do not say:

> A flexible artifact model for workflow composition.

### Schema

Say:

> Author in a friendly Guild format, then compile to the standard skill format.

Do not say:

> A novel declarative schema for agentic procedure encoding.

### Trust

Say:

> Inspect what the playbook intended to do, what it was allowed to do, what it actually did, and what evidence it collected.

Do not say:

> Built for trustworthy execution.

The latter is too vague. Trust must be made concrete.

## CLI introduction copy

The CLI should describe itself as:

> Build, verify, run, inspect, and replay trusted playbooks.

## Anti-pattern checklist

Reject copy that does any of the following:

- leads with internal implementation nouns
- says "AI workflows" with no domain anchor
- says "autonomous" without controls or limits
- promises safety without evidence, policy, or verification
- talks about a marketplace before showing a reason to care

## Message review gate

Every net-new top-level page should pass this test:

- Can the first 50 words name the audience?
- Can the first 50 words name the user outcome?
- Does the trust story show up before the fold?
- Is there a concrete playbook example?
- Are the canonical nouns used consistently?
