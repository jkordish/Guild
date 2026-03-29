# 04. Playbook Surface v1

**Status:** Proposed
**Owner:** Platform + DX
**Last updated:** 2026-03-28

## Goal

Make Guild authoring friendlier without forking away from the standard skills ecosystem.

## Strong position

**Guild should keep `SKILL.md` as the canonical distribution output and add a friendlier authoring surface on top.**

Humans should author a small set of YAML files with clear examples and explicit evidence requirements. Guild should compile those files into distributable skill bundles.

## Authoring surface

Guild v1 authoring uses three files:

- `guild.skill.yaml` - define a reusable procedural unit
- `guild.playbook.yaml` - define a user-facing operational workflow
- `guild-pack.yaml` - define a versioned installable bundle

## Why this shape

- It keeps human-authored structure explicit.
- It separates reusable skill logic from user-facing playbooks.
- It gives packs a place for versioning, compatibility, and verification metadata.
- It provides a clean place for evidence and eval requirements.

## Proposed file layout

```text
packs/
  incident-triage/
    guild-pack.yaml
    skills/
      logs-query/
        guild.skill.yaml
        resources/
      metrics-query/
        guild.skill.yaml
        resources/
    playbooks/
      diagnose-restart-notify/
        guild.playbook.yaml
        resources/
```

## `guild.skill.yaml`

### Minimal schema sketch

```yaml
apiVersion: guild/v1alpha1
kind: Skill
metadata:
  name: k8s-restart
  title: Restart Kubernetes workload
  summary: Safely restart a deployment or statefulset and capture evidence.
  version: 0.1.0
spec:
  use_cases:
    - recover a stuck workload
    - roll a workload after a bad cache state
  capabilities:
    required:
      - k8s:read
      - k8s:restart
    optional:
      - chat:post
  risk:
    class: mutate
    approval: recommended
  install_targets:
    - openai
    - github-copilot
    - local
  inputs:
    - name: namespace
      type: string
      required: true
    - name: workload
      type: string
      required: true
    - name: kind
      type: enum
      values: [deployment, statefulset]
      required: true
  steps:
    - inspect current rollout status
    - inspect recent logs and events
    - require approval in production
    - restart the workload
    - verify rollout health
  evidence_contract:
    required:
      - workload identity
      - pre-check rollout state
      - restart action issued
      - post-check rollout state
  examples:
    - name: restart deployment after deadlock
      input:
        namespace: payments
        workload: api
        kind: deployment
  eval_scenarios:
    - name: denies restart when target is missing
      expects: fail-safe
```

## `guild.playbook.yaml`

A playbook assembles one or more skills into a user-facing workflow.

```yaml
apiVersion: guild/v1alpha1
kind: Playbook
metadata:
  name: restart-service-with-evidence
  title: Diagnose service, restart workload, notify on-call
  summary: Investigate degradation, restart the Kubernetes workload, verify recovery, and notify the incident channel.
  version: 0.1.0
spec:
  intent: Restore service health with bounded operational mutation and evidence.
  skills:
    - logs-query
    - metrics-query
    - k8s-restart
    - chat-post
  capabilities:
    - logs:query
    - metrics:query
    - k8s:read
    - k8s:restart
    - chat:post
  approvals:
    production:
      required: true
      reason: Restart mutates production runtime state.
  inputs:
    - name: service
      type: string
      required: true
    - name: namespace
      type: string
      required: true
  evidence_contract:
    required:
      - service identifier
      - health signals collected
      - approval decision
      - restart action
      - post-restart health check
      - final operator message
  output:
    receipt_summary: true
    chat_notification: true
```

## `guild-pack.yaml`

A pack is the installable unit.

```yaml
apiVersion: guild/v1alpha1
kind: Pack
metadata:
  name: incident-triage
  title: Incident triage starter pack
  version: 0.1.0
  owner: guild
spec:
  description: Core playbooks and skills for observing health, taking bounded action, and posting outcomes.
  skills:
    - skills/logs-query
    - skills/metrics-query
    - skills/k8s-restart
    - skills/chat-post
  playbooks:
    - playbooks/restart-service-with-evidence
  compatibility:
    targets:
      - openai
      - github-copilot
      - local
  verification:
    level: curated
  docs:
    quickstart: README.md
```

## Compile contract

Guild compiles the authoring surface into:

- standard `SKILL.md` assets for compatible hosts
- pack metadata for install and export
- verification metadata and checksums
- examples and eval fixtures

## Evidence contract

Evidence is not optional decoration. It is part of the playbook contract.

Each skill and playbook can declare:

- `required` evidence items
- `optional` evidence items
- `redactions` for sensitive values
- `retention_class` for governance

Example:

```yaml
evidence_contract:
  required:
    - approval decision
    - command issued
    - post-change verification
  optional:
    - relevant logs excerpt
  redactions:
    - secret values
    - auth tokens
  retention_class: operational
```

## Approval model

Approval belongs in the authoring surface because the blast radius should be visible before runtime.

Example:

```yaml
approvals:
  production:
    required: true
    reason: Mutates live service state.
  staging:
    required: false
```

## Eval model

Every first-party skill and playbook should declare at least a minimal eval surface:

- happy path
- denied / unsafe mutation path
- missing input path
- target incompatibility path

## Why this is friendlier

This shape is friendlier because:

- authors see explicit fields instead of inferred conventions
- capabilities and risk are first-class
- evidence and approvals are first-class
- packs are explicit installable units

## Migration strategy

- Existing raw `SKILL.md` assets remain valid.
- Guild adds a compiler and validator.
- First-party content should move to the friendly authoring layer first.
- External contributors can continue using raw `SKILL.md` until migration tools exist.

## Decision

**Ship the friendly authoring layer now.** Do not wait for a perfect universal schema. The human authoring experience is currently too important to leave rough.
