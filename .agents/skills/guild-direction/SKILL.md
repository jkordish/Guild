---
name: guild-direction
description: Quick orientation to Guild's current session-substrate strategy, guardrails, and first-read docs.
---

# Guild Direction

Use this skill when you need a quick orientation to Guild's current strategy.

Read these first:

- `AGENTS.md`
- `docs/strategy/session-substrate/00-umbrella-epic.md`
- `docs/strategy/session-substrate/07-roadmap.md`
- `docs/strategy/session-substrate/tasks.md`
- `docs/adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md`

Current thesis:

Guild is evolving into the admission controller, session broker, and receipt
engine for isolated harness execution. Sessions are the product abstraction.
Harnesses are first-class. Sandbox lifecycle is internal.

Important guardrail:

The repo still ships a skill-first, inspect-first trust chain today. Do not
claim durable session resume or rehydration already exists unless the code and
docs say so explicitly.
