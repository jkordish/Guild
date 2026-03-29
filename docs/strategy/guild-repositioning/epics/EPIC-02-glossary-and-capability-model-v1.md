# EPIC-02. Glossary and Capability Model v1

**Status:** Proposed
**Milestone:** M1
**Owner:** TBD
**Last updated:** 2026-03-28

## Objective

Define the canonical external language and capability taxonomy that Guild uses for policy, docs, and user understanding.

## Why now

Without stable nouns and capability names, schema, policy, and trust work will all drift.

## In scope

- glossary and banned terms
- capability grammar
- capability families and verbs
- alias model
- terminology linting

## Out of scope

- tool-native permission modeling
- deep RBAC replacement
- exhaustive domain coverage

## Deliverables

- published glossary
- capability taxonomy v1
- naming rules
- alias mapping guidance
- terminology checks in docs / CI

## Dependencies

- EPIC-01 should settle the headline narrative first

## Risks

- over-granular capability names
- capability naming by vendor instead of user meaning
- inability to infer blast radius from the name

## Exit criteria

- capability names are stable enough to use in policy and receipts
- top-level docs stop inventing new nouns
- first-party packs can declare capabilities consistently

## Suggested tasks

- GR-005
- GR-006
- GR-007
- GR-008
