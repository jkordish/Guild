# 03. Capability Taxonomy v1

**Status:** Proposed
**Owner:** Platform
**Last updated:** 2026-03-28

## Goal

Define a user-facing capability model that is:

- simple enough for humans to read quickly
- coarse enough for policy and review
- stable enough to survive underlying tool changes
- expressive enough to gate meaningful operational risk

## Strong position

**Keep the external capability model coarse and human-readable.** Do not expose raw tool permissions, CLI flags, or internal adapter names as user-facing capabilities.

Example:

- user-facing capability: `k8s:restart`
- adapter implementation detail: `kubectl rollout restart deployment ...`

## Capability grammar

Format:

```text
<domain>:<verb>
```

Examples:

- `metrics:query`
- `logs:query`
- `traces:query`
- `chat:post`
- `incident:create`
- `k8s:read`
- `k8s:restart`
- `deploy:rollback`
- `secrets:rotate`
- `dns:update`

## Rules

1. Domain must be user-recognizable.
2. Verb should come from a short approved verb set.
3. Do not encode the concrete tool in the capability name.
4. Do not encode the environment in the capability name.
5. Keep names stable even if adapters change.

## Approved verb set v1

Observation:

- `read`
- `query`
- `list`
- `describe`

Communication / coordination:

- `post`
- `create`
- `annotate`

Mutation:

- `restart`
- `scale`
- `rollback`
- `update`
- `rotate`
- `purge`
- `cordon`
- `drain`
- `dispatch`

## Capability families

| Domain | Typical purpose | Common verbs | Typical risk |
| --- | --- | --- | --- |
| `metrics` | Read time-series health signals | `query` | low |
| `logs` | Read service or platform logs | `query` | low |
| `traces` | Read distributed trace data | `query` | low |
| `chat` | Post operational updates | `post` | low |
| `incident` | Create or annotate incident records | `create`, `annotate` | low to medium |
| `change` | Open or update change records | `create`, `update` | low to medium |
| `ci` | Trigger automation pipelines | `dispatch` | medium |
| `deploy` | Adjust release state | `rollback`, `update` | medium to high |
| `k8s` | Inspect or mutate Kubernetes workloads | `read`, `restart`, `scale`, `cordon`, `drain` | medium to high |
| `cloud` | Interact with cloud resources | `read`, `update` | medium to high |
| `dns` | Read or update edge routing | `read`, `update` | high |
| `cache` | Clear edge or application caches | `purge` | medium |
| `secrets` | Read or rotate secrets | `read`, `rotate` | high |
| `db` | Inspect or mutate database state | `read`, `update` | high |
| `auth` | Read or update identity / access policy | `read`, `update` | high |

## Risk classes

Use capability risk classes for policy defaults:

- **observe** - reads only, no mutation
- **assist** - creates annotations, posts updates, prepares but does not mutate production state
- **mutate** - changes production-adjacent state
- **critical** - changes identity, secrets, routing, or high-impact infrastructure

Suggested defaults:

- `observe` - no approval required
- `assist` - policy dependent
- `mutate` - approval recommended or required in prod
- `critical` - approval required in prod

## Alias model

Capabilities are stable external names. Tool adapters may map multiple tool actions to one capability.

Examples:

| Tool-specific action | Capability alias |
| --- | --- |
| `kubectl get deployment` | `k8s:read` |
| `kubectl rollout restart deployment` | `k8s:restart` |
| Datadog metrics query | `metrics:query` |
| Loki log search | `logs:query` |
| PagerDuty incident create | `incident:create` |
| GitHub Actions workflow dispatch | `ci:dispatch` |
| Cloudflare cache purge | `cache:purge` |

## Policy matching

Policy should match against external capability names first, with optional qualifiers in metadata.

Example:

```yaml
allow:
  - metrics:query
  - logs:query
  - chat:post
require_approval:
  - k8s:restart
  - deploy:rollback
  - secrets:rotate
deny:
  - auth:update
```

## Non-goals

The capability taxonomy is **not**:

- a full RBAC model
- a replacement for tool-native permissions
- a way to encode every possible CLI action

## Decision

**Ship v1 with a coarse capability model and aliases.** If users cannot read the name and infer the blast radius, the taxonomy is too granular.
