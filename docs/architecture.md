# Architecture

Guild has four core jobs:

1. store and resolve skills
2. evaluate capability grants and policy
3. run skills in a controlled runtime
4. expose a stable MCP-facing façade

## Major components

### 1. Registry
The registry stores:

- skill manifests
- artifact digests
- publisher identity
- trust tier
- visibility
- dependency snapshots
- verification metadata

The registry resolves a human-friendly reference such as `infra.aws/inventory-eks-cluster@^0.2` into a concrete artifact digest.

### 2. Policy engine
Policy decides whether a skill can run with a requested set of capabilities for a given tenant, actor, and execution mode.

Effective permission is the intersection of:

- skill-required capabilities
- tenant-allowed capabilities
- runtime-supported capabilities
- execution-mode constraints
- any approval or trust gating

### 3. Runner
The runner takes a resolved skill plus execution request and runs it in the correct runtime.

Preferred path:
- WASM component
- explicit host imports
- bounded budgets
- evidence-aware execution

Later paths may include external process or container adapters, but those should remain secondary.

### 4. MCP façade
Guild should present a small, stable set of tools:

- `guild.search`
- `guild.describe`
- `guild.inspect`
- `guild.plan`
- `guild.apply`

The catalog of skills is data behind the façade, not a direct explosion of MCP tools.

## Request flow

1. Client asks Guild to inspect, plan, or apply a skill.
2. Registry resolves a version requirement to an immutable artifact digest.
3. Policy calculates granted capabilities.
4. Runner starts execution with budgets and trace context.
5. Skill runs using host imports only.
6. Skill emits structured result, evidence, effects, and diagnostics.
7. Guild stores execution metadata and returns a normalized response to the client.

## Security boundaries

Guild assumes skills are not fully trusted, even when published by smart people who mean well.

Security boundaries:

- host capability boundary
- runtime sandbox boundary
- policy boundary
- artifact verification boundary
- tenant visibility boundary

## Why WASM-first

WASM gives Guild a cleaner default for:

- portability
- isolation
- explicit imports
- stable distribution
- policy-scoped host access

It is not magic. It just makes the default path less reckless.

## Why not one MCP tool per skill

A large skill catalog should not become a giant MCP tool list.

Reasons:

- model context gets noisy
- tool discovery gets expensive
- review and policy become harder
- clients become tightly coupled to catalog churn

A stable façade avoids that.

## Early non-goals

- rich workflow DSL
- ambient host access
- mutation-heavy automation
- broad external runtime support before the WASM path is stable
