# Project Positioning

This document is now a compatibility bridge for older links that previously
pointed at Guild's canonical repo framing.

## Current Direction

Guild is evolving into the admission controller, session broker, and receipt
engine for isolated harness execution.

Calls target a durable session. The platform resumes if possible, rehydrates if
necessary, and cold-starts if forced. The user-facing abstraction is the
session, not the sandbox. Harness is the new first-class execution
abstraction.

## What Ships Today

The live repo still ships a skill-first, inspect-first trust chain:

- host-mediated admission and capability review
- bounded execution over installed portable skills
- durable execution records
- durable evidence records
- a thin local CLI and stdio MCP surface over those records

That shipped slice remains real and should stay explicit. The new
session-substrate direction does not make durable session resume,
rehydration, or harness-level packaging a finished runtime claim.

## Read Next

- [`strategy/session-substrate/00-umbrella-epic.md`](strategy/session-substrate/00-umbrella-epic.md)
- [`strategy/session-substrate/01-north-star.md`](strategy/session-substrate/01-north-star.md)
- [`strategy/session-substrate/07-roadmap.md`](strategy/session-substrate/07-roadmap.md)
- [`strategy/session-substrate/tasks.md`](strategy/session-substrate/tasks.md)
- [`adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md`](adr/0020-evolve-guild-toward-a-trusted-session-substrate-for-isolated-harness-execution.md)
- [`how-guild-works.md`](how-guild-works.md)

## Historical Note

Older docs may still refer to portable skills, playbooks, or starter sets as
the main repo framing. Treat those as the description of the current shipped
slice and the immediately previous planning wave, not as the new long-term
north star.
