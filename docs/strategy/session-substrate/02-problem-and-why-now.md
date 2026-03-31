# Problem And Why Now

## Why Portable Skills Alone Are Not Enough

Portable skills are still useful. They give Guild executable identity,
capability declarations, transportability, and durable execution records. But
they are too low-level to carry the longer-lived product story on their own.

The current skill-first framing makes Guild easy to classify as a runtime,
plugin system, or packaging layer. That hides the more durable value: a host
that admits isolated work, keeps durable records, and can continue work across
interruptions without making the caller reason about raw runtime instances.

## Why The Session Substrate Is The More Durable Wedge

A durable session gives Guild a stable unit for:

- admission and re-admission
- reuse of prior context or state
- reconnecting external services
- aggregating receipts across attempts
- deciding whether to resume, rehydrate, or cold-start

That is a stronger wedge than “we can run portable skills” because it maps to a
problem users already have: they need isolated work to continue safely across
time, failures, and reconnects.

## Why “Session, Not Sandbox” Is The Correct UX Abstraction

Users care whether their work continues and whether the platform can account
for it. They do not want to reason about which sandbox process or VM instance
held that work at a given moment.

The sandbox is an implementation detail. The session is the durable contract:

- a caller invokes a session
- the platform decides how to materialize that session safely
- receipts explain what happened

That framing keeps Guild honest about trust boundaries while removing the wrong
primitive from the user story.
