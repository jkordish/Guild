# 08. Market Context

**Status:** Informing strategy
**Owner:** Founding team
**Last updated:** 2026-03-28

This doc captures the relevant ecosystem context behind the Guild repositioning decisions.

## Current ecosystem facts

### 1. OpenAI now supports skills across multiple products

OpenAI documents skills in ChatGPT and says skills are also supported in Codex and the API, even though they do not yet sync across products. OpenAI also says its skills follow the open Agent Skills standard.

Source: <https://help.openai.com/en/articles/20001066-skills-in-chatgpt>

**Implication for Guild:**

Do not invent a closed distribution format. Keep `SKILL.md` compatibility and focus Guild on authoring, packaging, verification, and trust.

### 2. GitHub already supports agent skills

GitHub documents agent skills for Copilot as folders of instructions, scripts, and resources, with project-scoped and personal-scoped skill locations.

Source: <https://docs.github.com/en/copilot/concepts/agents/about-agent-skills>

**Implication for Guild:**

Install and export flows matter now. A Guild pack should be able to target GitHub-compatible locations cleanly.

### 3. Microsoft documents the same general skills shape

Microsoft Agent Framework documents `SKILL.md`, progressive disclosure, and portable skill packages.

Source: <https://learn.microsoft.com/en-us/agent-framework/agents/skills>

**Implication for Guild:**

Guild is better positioned as a trust and packaging layer on top of the standard than as a competing format.

### 4. skills.sh already covers discovery and installation

skills.sh documents skill discovery, installation, and a leaderboard based on install telemetry. It explicitly notes that users should review skills before installing them.

Source: <https://skills.sh/docs>

**Implication for Guild:**

Generic discovery is already commoditizing. Guild should not try to win on "we also list a lot of skills." Guild should win on **verified packs**, **receipts**, and **replayable trust**.

### 5. Packaging / management tooling is appearing quickly

Tools and docs in the wider ecosystem increasingly focus on installation, browsing, translation, and management of skills.

Example references:

- SkillPad - <https://skillpad.dev/>
- SkillKit - <https://www.skillkit.sh/docs>

**Implication for Guild:**

Packaging is timely, but generic packaging UX alone will not be enough differentiation.

## Strategic conclusions

### Keep the standard, improve the authoring experience

The right move is not to fork the spec. The right move is to make authoring better and verification stronger.

### Package curated outcomes now

Guild should ship starter packs immediately because installable outcomes are now table stakes.

### Do not open a public marketplace yet

Public catalog breadth is easier to imitate than trust. Guild should earn the right to broad distribution by proving curation and verification first.

### Differentiate on trust primitives

The missing category-defining feature is not more catalog surface area. It is:

- declared capabilities
- approval gates
- receipt chains
- replay
- verification reports

## Decision summary

| Decision | Why |
| --- | --- |
| Keep `SKILL.md` canonical | interoperability already matters |
| Add a Guild authoring layer | human authoring needs to be much friendlier |
| Package curated packs now | installability is important right now |
| Delay marketplace | breadth without trust is weak differentiation |
| Invest in replay and verification | strongest durable wedge |
