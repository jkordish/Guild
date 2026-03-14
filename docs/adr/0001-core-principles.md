# ADR 0001: Core principles

## Status

Accepted

## Context

Guild is intended to be a shared registry and runtime for portable agent skills. Similar systems tend to fail early by conflating tools with skills, trusting plugins too much, and letting convenience outrun contracts.

## Decision

Guild will adopt the following core principles:

1. **Rust is the platform core.**
2. **WASM is the preferred skill distribution format.**
3. **Skills receive host capabilities, not ambient authority.**
4. **Execution resolves to immutable digests.**
5. **Inspect, plan, and apply are separate modes.**
6. **Evidence, diagnostics, and provenance are required outputs.**
7. **The MCP surface remains small and stable.**
8. **Contracts are treated as public product surface.**

## Consequences

Positive:

- better portability
- clearer trust boundaries
- easier auditing
- lower chance of tool-sprawl collapse
- better long-term compatibility discipline

Negative:

- more upfront design work
- slower early demos
- more friction when adding new host capabilities
- stronger pressure to keep examples and docs aligned with code

## Notes

These tradeoffs are intentional. Guild is trying to become reliable infrastructure, not an exciting accident.
