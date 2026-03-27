# Guild Repositioning North Star

Use [`02-glossary-and-banned-terms.md`](./02-glossary-and-banned-terms.md)
as the canonical operator-facing vocabulary and user-facing language source for
this repositioning pack. It guides wording and review, but it does not rename
runtime contracts, Rust types, or WIT surfaces.

## Target Audience

- Ops engineers running recurring operational procedures
- Platform engineers standardizing safe automation
- SREs who need inspectable execution and replayable recovery paths
- Security engineers reviewing admission, isolation, capabilities, receipts, and evidence

## Product Thesis

Guild is trusted operational automation for engineering teams.

The playbook is the application. The trust chain is the product.

Guild should read as the system that lets operators define an ops playbook, admit it under explicit capability policy, run it in isolation, and keep receipts and evidence that can be inspected and replayed later.

## Narrative Hierarchy

1. Safe operational automation
2. Expressed as playbooks
3. Powered by portable skills
4. Made trustworthy by admission, isolation, explicit capabilities, signed receipts and evidence, and replay

## Product Promises

- Operators can understand what a workflow is allowed to do before it runs.
- Playbooks are legible enough to review like operational procedure, not substrate plumbing.
- Execution leaves receipts and evidence that explain what happened.
- Capabilities are human-readable and scoped to real operator intent.
- Replay and inspection stay grounded in stored receipts instead of chat-only memory.

## Non-Goals

- Do not reposition Guild as a generic agent framework or orchestration engine.
- Do not rename the internal runtime contract in the first wave.
- Do not imply that every future capability or playbook shape is implemented today.
- Do not hide current bounded or not-yet-supported runtime surfaces behind marketing language.
- Do not make the first wave depend on a hosted control plane or marketplace story.

## Decision Summary

- Lead the product story with trusted operational automation, not portable artifacts.
- Raise playbooks above skills in the operator story while keeping skills as the reusable execution unit.
- Introduce an external capability taxonomy that is operator-readable and maps to current internal mechanics.
- Tighten the target CLI story around `admit`, `exec`, `inspect`, and `replay`, but take an aliases-first migration path.
- Use concrete ops workflows as the primary examples for the next narrative wave.

## Constraints From Current Repo Truth

- Current public docs still lead with portable artifacts, trust layers, and receipt chains.
- Current CLI is shaped around `show`, `grants`, `run`, `ls`, `get`, `why`, and `verify`.
- Current active runtime families are `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`.
- `SkillCategory::Playbook` already exists in the type system, but playbooks are not yet a first-class user-facing concept.
- This strategy doc is explanatory planning, not a runtime-contract source.
