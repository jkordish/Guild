# EPIC-04. Packaging and Install Surface

**Status:** Proposed
**Milestone:** M2
**Owner:** TBD
**Last updated:** 2026-03-28

## Objective

Turn Guild content into versioned installable packs that can be exported to the right targets with one obvious workflow.

## Why now

Packaging is now table stakes in the skills ecosystem. Guild should ship it, but in a curated and compatibility-aware shape.

## In scope

- pack manifest
- build / export commands
- install / import flow
- versioning and lockfile
- quickstart docs

## Out of scope

- public marketplace
- leaderboard features
- broad third-party publishing flows

## Deliverables

- pack bundle format
- `guild pack build`
- export targets
- install / import story
- 5-minute first useful run docs

## Dependencies

- EPIC-03 compiler output

## Risks

- building packaging with no clear user outcome
- export targets becoming host-specific hacks
- docs that still require manual glue work

## Exit criteria

- a pack can be built, exported, installed, and referenced cleanly
- quickstart path succeeds end-to-end
- first-party packs use the exact same flow

## Suggested tasks

- GR-014
- GR-015
- GR-016
- GR-017
- GR-018
