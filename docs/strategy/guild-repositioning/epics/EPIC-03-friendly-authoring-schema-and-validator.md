# EPIC-03. Friendly Authoring Schema and Validator

**Status:** Proposed
**Milestone:** M2
**Owner:** TBD
**Last updated:** 2026-03-28

## Objective

Give authors a humane Guild-native authoring surface that compiles to the standard skill format and validates structure early.

## Why now

Guild needs a better authoring experience now, but should not fork away from the ecosystem.

## In scope

- `guild.skill.yaml`, `guild.playbook.yaml`, `guild-pack.yaml`
- parser and compiler
- validator
- examples and migration docs

## Out of scope

- replacing `SKILL.md` as a distribution target
- designing a perfect permanent schema before shipping

## Deliverables

- `guild/v1alpha1` spec
- parser + compiler
- validator + golden tests
- migration docs
- scaffolding examples

## Dependencies

- EPIC-02 capability model should be stable enough first

## Risks

- over-designing the schema
- letting the compiler output drift from author intent
- generating assets that users must hand-edit

## Exit criteria

- a first-party pack can be authored without hand-writing raw `SKILL.md`
- generated outputs are deterministic
- validation failures are clear and actionable

## Suggested tasks

- GR-009
- GR-010
- GR-011
- GR-012
- GR-013
