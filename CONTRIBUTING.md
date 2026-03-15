# Contributing

Thanks for contributing to Guild.

This project is early, so the contribution bar is less about polish and more about architectural honesty. A small, clean change that preserves the trust model is worth more than a large helpful change that smuggles in future regret.

## Ground rules

- contracts first
- least privilege by default
- prefer explicit types over flexible ambiguity
- preserve digest-pinned execution
- keep the MCP facade small
- document invariants when changing them

## Getting started

1. Read:
   - `README.md`
   - `AGENTS.md`
   - `SPECS.md`
   - `ARCHITECTURE.md`
   - `docs/adr/README.md`

2. Make the smallest coherent change that proves the point.

3. Update docs and examples if the change alters contracts or behavior.

## Workspace commands

```bash
make check
make test
make fmt
make clippy
```

## What belongs in an ADR

Add or update an ADR when you change any of the following:

- platform ABI shape
- capability model
- trust or publication model
- execution mode semantics
- crate boundary that affects public structure
- MCP facade semantics

Current ADRs live in `docs/adr/`.

## Pull request expectations

A solid PR usually includes:

- a clear statement of the problem
- the architectural impact
- contract changes, if any
- updated docs
- tests or examples where reasonable
- explicit non-goals when scope is intentionally limited

## Style notes

- keep names boring and precise
- avoid giant abstractions until the second use is obvious
- avoid hidden side effects
- do not treat JSON blobs as a replacement for domain modeling
- do not introduce new capabilities casually

## Early roadmap bias

During the initial phase, contributions are most useful in these areas:

- contract refinement
- WASM execution model
- registry and manifest design
- policy and capability evaluation
- MCP facade design
- example skills and fixtures

Less useful right now:

- UI polish
- broad plugin ecosystems
- mutation-heavy workflows
- long tail integrations without a stable core

## Licensing

A project license has not been selected yet. Do not assume public redistribution terms until that is settled.
