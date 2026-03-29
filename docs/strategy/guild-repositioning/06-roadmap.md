# 06. Roadmap

**Status:** Proposed
**Owner:** Founding team
**Last updated:** 2026-03-28

## Strong position

**Do not open a public marketplace before Guild can prove trustworthy first-party playbooks with receipts, replay, and verification.**

Packaging now is correct. Marketplace now is a distraction.

## Milestones

| Milestone | Goal | Included epics | Rough duration | Exit gate |
| --- | --- | --- | --- | --- |
| **M1** | Make Guild legible | EPIC-01, EPIC-02 | 2 weeks | site, README, glossary, and capability model all align |
| **M2** | Make Guild installable and useful | EPIC-03, EPIC-04, EPIC-05 | 4 to 6 weeks | friendly authoring, packaging, 4 packs, 6 playbooks, quickstart |
| **M3** | Make Guild trustworthy and differentiated | EPIC-06, EPIC-07 | 3 to 4 weeks | every first-party playbook emits receipts, supports replay, and has verification coverage |
| **M4** | Make Guild adoptable by teams | EPIC-08 plus hardening | 3 to 4 weeks | private pack distribution, signatures, audit export, governance docs |

## Sequencing logic

### Why M1 first

If the product story is muddy, every subsequent feature is harder to explain and easier to build in the wrong shape.

### Why M2 before M3 marketplace work

Guild needs installable, useful examples before it needs broad distribution mechanics.

### Why M3 is the real differentiator

Packaging alone will become commodity behavior. Evidence, replay, and verification are where Guild becomes worth choosing.

### Why M4 waits

Team adoption work matters, but it only compounds after the core trust story is real.

## Dependencies

- M2 depends on the canonical nouns and capability model from M1.
- M3 depends on packs and reference playbooks from M2.
- M4 depends on receipt / verification primitives from M3.

## Decision gates

### Gate after M1

Only proceed if:

- the landing page, README, and CLI summary all use the same narrative hierarchy
- capability naming is stable enough to start policy work

### Gate after M2

Only proceed if:

- at least 4 packs build and export cleanly
- at least 6 playbooks are runnable end-to-end
- the 5-minute first useful run path exists

### Gate after M3

Only proceed if:

- every curated pack has a verification report
- every first-party playbook emits receipts
- replay works for at least the reference playbooks

## What not to start early

Do not start these before the matching gate:

- public marketplace
- generic community submission pipeline
- leaderboard mechanics
- memory / agent platform detours
- enterprise sales positioning

## Risks and failure modes

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Schema over-design | burns time before value is visible | keep `v1alpha1`, compile to standard, prove with first-party packs |
| CLI sprawl | confuses users and preserves internal framing | freeze a small top-level surface |
| Pack sprawl | lots of half-useful skills, no proof of value | ship curated packs only |
| Verification theater | labels with no real checks | require reports, evals, and compatibility proofs |
| Trust buried again | product drifts back to substrate talk | narrative review gate on major docs and pages |

## Recommended operating cadence

- Weekly: milestone review and risk review
- Per PR: narrative and glossary compliance check
- End of each milestone: demo the reference playbooks, not just the underlying plumbing

## Resourcing assumption

This roadmap assumes a small team. If capacity is constrained, preserve the sequence and reduce scope rather than parallelizing everything.

The order matters more than the calendar.
