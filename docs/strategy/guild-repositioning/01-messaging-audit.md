# Messaging Audit

## Top Messaging Problems

1. The repo still leads with mechanism-heavy language instead of operator outcomes.
2. Playbooks are mostly absent, and when they do appear they are framed defensively.
3. Capabilities are exposed as grant JSON and low-level family names before users understand the operator task.
4. Guild Ops Starter carries too much of the product story under the label "reference application."
5. CLI wording is more usable than the top-level docs, but it still sounds like a prototype shell around a substrate.

## Highest-Value Fixes First

1. Replace the README and top-doc hero framing with trusted operational automation and playbooks.
2. Introduce a glossary that turns current internal terms into operator-readable terms.
3. Define the operator-facing capability taxonomy and playbook surface before broad doc rewrites.
4. Reframe Guild Ops Starter and future examples around concrete ops workflows.
5. Publish a staged CLI migration story so docs and examples stop inventing parallel command narratives.

## Concrete Repo Evidence

### README leads with artifacts and trust layer

- Current:
  - `README.md`: "Guild creates portable, capability-bounded skill artifacts and a trust layer..."
  - `README.md`: "Guild turns a skill run into a verifiable receipt chain..."
- Problem:
  - Accurate, but it makes the product read like infrastructure plumbing before the operator value is visible.
- Suggested after:
  - "Guild is trusted operational automation for engineering teams."
  - "Write an ops playbook, admit it with explicit capabilities, run it in isolation, and keep receipts and evidence for inspection and replay."

### Playbooks are positioned as something Guild is not

- Current:
  - `README.md`: "The goal is not ... an ops playbook engine."
  - `docs/project-positioning.md`: "a broad ops playbook runtime" under anti-thesis language.
- Problem:
  - This protects against overclaiming, but it also blocks the operator-facing story the product now wants to tell.
- Suggested after:
  - "Guild is not a generic workflow engine."
  - "Guild is for trusted ops playbooks built on explicit capability and evidence boundaries."

### Guild Ops Starter is overworked as a story vehicle

- Current:
  - `README.md`, `examples/README.md`, and `examples/skills/guild-ops-starter/README.md` repeatedly call it the "first reference application."
- Problem:
  - "Reference application" is accurate but abstract. It says how the repo is organized, not why an operator should care.
- Suggested after:
  - Reframe it as "the first ops playbook starter set" or "the first operator starter set."
  - Keep "reference application" only where the repo needs to explain packaging or planning lineage.

### Capability UX is too implementation-shaped

- Current:
  - CLI help and examples lead with `guild grants template ...`
  - Templates expose `read-resource`, `invoke-skill`, `http-request`, and constraint JSON immediately.
- Problem:
  - This is legible to maintainers, but not to operators who think in terms like "restart pods" or "post to chat."
- Suggested after:
  - Introduce operator-readable names like `k8s:restart` and `chat:post` in docs, playbooks, and approvals.
  - Keep the current internal families for implementation and migration notes.

### CLI wording is useful, but not yet operator-centered

- Current:
  - `guild --help` and `guild run --help` are clear, but the story is still "run a skill locally," "print grant templates," and "explain a persisted execution."
- Problem:
  - Those are honest verbs for today, but they do not tell the future operator story of admit, exec, inspect, and replay.
- Suggested after:
  - Publish the target command story in docs now.
  - Keep current verbs stable, but map them to the future operator flow explicitly.

## Before / After Wording Suggestions

| Surface | Before | After |
| --- | --- | --- |
| `README.md` hero | "portable, capability-bounded skill artifacts and a trust layer" | "trusted operational automation expressed as ops playbooks" |
| Product value | "verifiable receipt chain" | "admitted, isolated automation with inspectable receipts and evidence" |
| Example framing | "first reference application" | "first operator starter set" or "reference playbook starter" |
| Capability story | "grant templates" | "capabilities operators can review and approve" |
| CLI story | `show / grants / run / ls / get / why / verify` | target operator flow of `admit / exec / inspect / replay`, with compatibility mappings |

## Terminology That Is Too Abstract Today

- `artifact` as a lead noun
- `trust layer`
- `reference application`
- `runtime` as a lead-value word
- `capability-bounded` without operator explanation
- `grants` without capability intent

## Messaging Gaps

- There are almost no concrete ops workflows in the repo today beyond incident-analysis examples.
- The repo already has `SkillCategory::Playbook`, but the docs do not use that seam to tell a playbook story.
- The strategy guardrail in `crates/guild-draft-truth/src/project_positioning.rs` currently enforces the older portable-artifact thesis, so the future narrative reset will need an explicit guardrail migration.
- No in-repo website or landing-page source was found, so "site realignment" currently means README, docs, examples, and GitHub metadata unless an external site repo is introduced later.
