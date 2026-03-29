# EPIC-06. Receipt Chain, Replay, and Policy

**Status:** Proposed
**Milestone:** M3
**Owner:** TBD
**Last updated:** 2026-03-28

## Objective

Make trust inspectable by emitting structured receipts, supporting replay, and enforcing approval-aware runtime behavior.

## Why now

This is the feature set that turns Guild from installable packaging into trusted operational automation.

## In scope

- receipt schema
- receipt emission
- inspect and replay commands
- approval policy model
- redaction / retention hooks

## Out of scope

- analytics dashboards
- generic observability platform features
- governance UX beyond minimal export / inspect

## Deliverables

- run object model
- receipt emission on admit / exec
- `guild inspect`
- `guild replay`
- production mutation gate
- redaction hooks

## Dependencies

- EPIC-05 reference playbooks need to exist first

## Risks

- receipts that capture too little to be useful
- replay that only reproduces metadata, not behavior
- policy that is too opaque to explain to operators

## Exit criteria

- every first-party playbook produces a meaningful receipt
- replay works for reference playbooks
- risky production mutations require clear policy / approval handling

## Suggested tasks

- GR-027
- GR-028
- GR-029
- GR-030
- GR-031
- GR-032
