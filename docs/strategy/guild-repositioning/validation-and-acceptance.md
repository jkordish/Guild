# Validation And Acceptance

Use these checks to keep the repositioning work honest. Validation should prove that wording, examples, and CLI surfaces changed together without widening Guild's actual trust or runtime guarantees.
For one current end-to-end operator proof story, see
[`../../trust-proof-walkthrough.md`](../../trust-proof-walkthrough.md).
Use [`02-glossary-and-banned-terms.md`](./02-glossary-and-banned-terms.md)
as the canonical operator-facing vocabulary and user-facing language source
when reviewing doc and help-text wording.

## Core Acceptance Rules

- Public-facing wording must stay aligned with the current supported capability and runtime frontier.
- No doc, example, or help text may claim a shipped playbook engine, replay engine, or broader capability coverage than the repo actually supports today.
- Any task that changes the visible user contract must update the help, docs, or examples that teach that contract.
- Any task that touches `docs/project-positioning.md` or the top-level thesis must account for the `project-positioning` guardrail.

## Validation Buckets

## Narrative And Glossary Tasks

Use for `README.md`, top-level docs, issue templates, and terminology sweeps.

Commands:

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
rg -n 'artifact|substrate|reference application|portable skill artifacts' README.md docs examples .github
```

Acceptance checks:

- The changed entrypoints describe Guild as trusted operational automation before mechanism-layer detail.
- Discouraged lead terms are removed, demoted, or clearly scoped.
- The docs do not fight the guardrail text.

## CLI And Help-Text Tasks

Use for help output, CLI examples, and alias-preview changes.

Commands:

```bash
git diff --check
cargo test -p guild-mcp --test guild_cli
cargo run -q -p guild-mcp --bin guild -- --help
cargo run -q -p guild-mcp --bin guild -- run --help
cargo run -q -p guild-mcp --bin guild -- show --help
cargo run -q -p guild-mcp --bin guild -- why --help
cargo run -q -p guild-mcp --bin guild -- verify --help
```

Acceptance checks:

- Docs and help output use the same glossary where possible.
- Any new alias or preview wording is explicitly marked as compatibility-preserving if the command is not primary yet.
- No help text implies a command path that the binary does not support.

## Capability And Playbook UX Tasks

Use for docs that describe external capabilities, playbooks, admission, and replay.

Commands:

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
cargo run -q -p guild-mcp --bin guild -- help grants
cargo run -q -p guild-mcp --bin guild -- grants template http-request
cargo run -q -p guild-mcp --bin guild -- grants template read-resource
cargo run -q -p guild-mcp --bin guild -- grants template invoke-skill
```

Acceptance checks:

- External capability names are presented as operator-facing vocabulary, not as a silent internal rename.
- Playbook docs say clearly that the playbook surface is a planning and UX target unless the code has caught up.
- Capability examples remain traceable to current internal families and trust boundaries.

## Example And Trust Tasks

Use for examples, trust docs, launch copy, and walkthroughs.

Commands:

```bash
git diff --check
cargo run -q -p xtask -- project-positioning check
```

Recommended proof commands when example or trust docs mention the live path:

```bash
cargo run -q -p guild-mcp --bin guild -- codex scenario --registry-root target/dev-local-registry/codex-local --scenario recent-failure-triage --json
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
```

Acceptance checks:

- Trust docs describe admission, isolation, receipts, evidence, and replay in operator terms without overstating current implementation.
- Example copy stays inside today's support frontier.
- Launch copy does not introduce unsupported commands or capabilities.

## Task-File Requirement

Every task file under [`tasks/`](./tasks/) should name:

- the minimum required validation commands
- the user-visible acceptance criteria
- the fallback if a guardrail or implementation-truth mismatch blocks the preferred path
