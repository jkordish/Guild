# Capability Taxonomy V1

## Summary

This document proposes the first external capability taxonomy for Guild.

It is an operator-facing naming layer, not a runtime-contract rename. The current internal families and typed constraints remain the implementation truth until a later implementation phase explicitly changes them.

## Design Principles

- Capability names should read like approvals an operator can understand.
- Names should describe intent, not transport or implementation detail.
- The capability surface should be smaller than the possible implementation detail behind it.
- Scope belongs in policy, admission, or parameters, not in the capability name itself.
- External names can map to current internal families, future families, or multiple underlying checks.

## Naming Rules

- Format: `domain:action`
- Use lowercase kebab-case.
- Use verbs operators already use in runbooks.
- Prefer the smallest action that still has clear operational meaning.
- Do not include environment, namespace, cluster, or target identifiers in the name.
- Do not encode transport details such as HTTP, URI prefixes, or child aliases into the external name.

## Capability Families

| External Capability | Meaning | Likely Current / Future Mapping |
| --- | --- | --- |
| `k8s:restart` | Restart a workload or pod set | likely future runtime work; may compose `invoke-skill` plus service-specific execution today |
| `k8s:scale` | Change replica count | likely future runtime work |
| `k8s:cordon` | Mark a node unschedulable | likely future runtime work |
| `k8s:drain` | Evict work from a node | likely future runtime work |
| `metrics:query` | Query health or performance metrics | current `http-request` in some environments, future dedicated metrics capability possible |
| `logs:query` | Read logs or structured operational events | current `read-resource` or `http-request` depending on backend, future dedicated logs capability possible |
| `chat:post` | Post to an operator chat destination | current `http-request` in some environments, future dedicated chat capability possible |
| `incident:create` | Create or annotate an incident record | current `http-request` in some environments, future dedicated incident capability possible |
| `deploy:rollback` | Roll back a deployment | likely future runtime work |
| `secrets:rotate` | Rotate a secret and record evidence | likely future runtime work |
| `cache:purge` | Purge or invalidate cache content | likely future runtime work |

## Scoping Guidance

- Put target scope in playbook inputs or policy:
  - cluster
  - namespace
  - deployment name
  - service name
  - environment
- Put safety boundaries in admission:
  - environment allowlist
  - time window
  - actor approval
  - evidence requirements
- Put transport and protocol details in the implementation layer:
  - URI prefixes
  - hosts
  - aliases
  - timeout and byte limits

## Examples

- Good:
  - `k8s:restart`
  - `metrics:query`
  - `chat:post`
- Too low level:
  - `http-request`
  - `invoke-skill`
  - `read-resource`
- Too specific:
  - `k8s:restart-prod-api-deployment`
  - `chat:post-slack-webhook`

## Migration Implications

- Current internal `CapabilityId` values remain canonical in `crates/guild-types`, `crates/guild-runner`, manifests, and CLI plumbing for now.
- External names become the preferred docs, playbook, and approval vocabulary first.
- A later CLI phase may accept external names in templates and playbook tooling, but that is not part of the first wave.
- A later runtime/manifest phase may introduce first-class mappings if the implementation work justifies it.

## What V1 Does Not Do

- It does not rename current manifest capability identifiers.
- It does not claim that every named external capability is runnable today.
- It does not collapse all capability checks into one generic permission blob.
- It does not remove the need for typed internal constraints or family-aware policy.

## Recommended Rollout

1. Use these names in strategy docs, example playbooks, approval flows, and roadmap language.
2. Add mapping notes to docs and examples so operators can relate external names to current Guild mechanics.
3. Introduce CLI and policy affordances only after the vocabulary is stable and the playbook surface is defined.
