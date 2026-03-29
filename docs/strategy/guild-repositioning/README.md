# Guild Repositioning Strategy Stack

**Status:** Proposed
**Last updated:** 2026-03-28

This folder is the planning stack for repositioning Guild around a simpler and stronger thesis:

> **Guild is trusted operational automation.**
> The playbook is the application. The trust chain is the product.

## Decisions already made

1. **Keep Agent Skills / `SKILL.md` as the distribution target.** Guild should not fork the emerging ecosystem format.
2. **Add a friendlier Guild authoring layer.** Human authors should not have to hand-author raw `SKILL.md` for every serious workflow.
3. **Package curated starter packs now.** Do not wait for a marketplace. Do not open broad community submissions yet.
4. **Differentiate on trust.** Replayable evidence, approval gates, and verification are the moat.
5. **Aim at ops / platform / security teams first.** Generic "AI workflows" messaging is too weak and too crowded.

## How to use this stack

Read in this order:

1. [`00-north-star.md`](./00-north-star.md)
2. [`06-roadmap.md`](./06-roadmap.md)
3. [`milestones/`](./milestones/)
4. [`epics/`](./epics/)
5. [`tasks.md`](./tasks.md)
6. [`11-tracker-crosswalk.md`](./11-tracker-crosswalk.md)

If you only need the short version, read:

- [`00-north-star.md`](./00-north-star.md)
- [`08-market-context.md`](./08-market-context.md)
- [`tasks.md`](./tasks.md)

## Folder map

- [`00-north-star.md`](./00-north-star.md) - final thesis, audience, promises, non-goals
- [`01-messaging-audit.md`](./01-messaging-audit.md) - rewrite plan for site, README, and product copy
- [`02-glossary-and-banned-terms.md`](./02-glossary-and-banned-terms.md) - canonical nouns, phrasing rules, discouraged language
- [`03-capability-taxonomy-v1.md`](./03-capability-taxonomy-v1.md) - external capability model for users and policy
- [`04-playbook-surface-v1.md`](./04-playbook-surface-v1.md) - friendly authoring schema and evidence contract
- [`05-cli-simplification.md`](./05-cli-simplification.md) - target operator and author workflows
- [`06-roadmap.md`](./06-roadmap.md) - milestone sequencing, gates, dependencies, risk register
- [`07-reference-playbooks.md`](./07-reference-playbooks.md) - first-party playbooks to prove the thesis
- [`08-market-context.md`](./08-market-context.md) - ecosystem context and rationale for packaging + trust positioning
- [`milestones/`](./milestones/) - milestone-by-milestone execution docs
- [`epics/`](./epics/) - one doc per epic with scope, deliverables, and exit criteria
- [`tasks.md`](./tasks.md) - PR-sized backlog ordered by dependency
- [`11-tracker-crosswalk.md`](./11-tracker-crosswalk.md) - current mapping from this imported stack to the active GitHub tracker and Guild's contracts-first repo truth

## Assumptions

- Guild already has a repository and website, but its current presentation is still too substrate-first and too abstract.
- The team is small, so the plan prioritizes leverage over breadth.
- The first target users are operators who need bounded, auditable automation rather than creative general-purpose agents.
- Trust, packaging, and replay matter more than breadth of integrations in the next phase.

## Definition of done for the repositioning pass

The repositioning pass is done when all of the following are true:

- A new visitor understands Guild in under 30 seconds.
- A first-party pack can be authored, built, verified, exported, installed, and run with one obvious path.
- Every first-party playbook emits replayable evidence.
- Curated packs have a visible verification state.
- A team lead can explain Guild as "trusted playbooks for operational work" without needing a whiteboard.

## What this stack deliberately does **not** do

- It does **not** open a public marketplace.
- It does **not** attempt to become a general-purpose agent platform.
- It does **not** try to win on raw number of skills.
- It does **not** assume the current repo structure is perfect.

## External references

The market-context doc is grounded in the current skills ecosystem as of 2026-03-28:

- OpenAI: Skills in ChatGPT - <https://help.openai.com/en/articles/20001066-skills-in-chatgpt>
- GitHub: About agent skills - <https://docs.github.com/en/copilot/concepts/agents/about-agent-skills>
- Microsoft: Agent Skills - <https://learn.microsoft.com/en-us/agent-framework/agents/skills>
- skills.sh docs - <https://skills.sh/docs>
