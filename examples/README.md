# Examples

Guild examples are ordinary repo-native examples and example skills.

There is no separate pack system here. The examples exist to prove and teach
specific honest slices of the current repo.

The canonical command examples assume `guild` is installed and on `PATH`.
Lower-level `cargo run -p guild-mcp --example ...` commands are developer proof
helpers, not the normal operator workflow.

## Start Here

If you want one compact first-five-minutes workflow, start with the Guild Ops
Starter Pack:

- [`examples/skills/guild-ops-starter/README.md`](./skills/guild-ops-starter/README.md)

That pack is the current user-facing path for trusted local operational
analysis over durable Guild refs. It uses:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the parent
  report skills

## User Journeys

### Install and run a skill

Start with [`examples/skills/hello-inspect/README.md`](./skills/hello-inspect/README.md).
It teaches the normal `guild install`, `guild show`, `guild run`, `guild why`,
and `guild verify` path with the smallest real skill in the repo. Use
`guild grants template emit-evidence` when you want the concrete JSON starting
point before editing `--grants-json`.

### Explain what happened

Use the current primary CLI first:

- `guild why` for the compact persisted-execution explanation path
- `guild why -v` when you need requested-versus-granted diff, nearby child/evidence refs, and authority observations
- `guild why --lineage` when you want the native bounded ancestor/descendant view
- `guild get` when you want the raw stored resource
- `guild ls evidence --limit 5` when you need to discover stored evidence first
- `guild grants template read-resource` and `guild grants template invoke-skill` when you need concrete bounded grant JSON before running the reusable analysis skills

Then move to the example skills when you want richer reusable reports:

- [`examples/skills/incident-brief`](./skills/incident-brief)
- [`examples/skills/explain-execution`](./skills/explain-execution)
- [`examples/skills/explain-execution-tree`](./skills/explain-execution-tree)
- [`examples/skills/summarize-execution-query`](./skills/summarize-execution-query)

### Verify trust state and move installed state

Start with the trust and transport flow in the top-level
[`README.md`](../README.md). Then use these example READMEs when you want the
same journey grounded in one concrete skill or closure:

- [`examples/skills/hello-inspect/README.md`](./skills/hello-inspect/README.md)
- [`examples/skills/hello-composite/README.md`](./skills/hello-composite/README.md)

### Debug failures and compare runs

Keep starting with the native CLI:

- `guild why` for the compact stored execution summary
- `guild why -v` for the expanded requested-versus-granted diff and family-aware authority hints

Use the Guild Ops Starter Pack for compact real-path troubleshooting:

- [`examples/skills/guild-ops-starter/README.md`](./skills/guild-ops-starter/README.md)
- [`examples/skills/recent-failures`](./skills/recent-failures)
- [`examples/skills/run-diff`](./skills/run-diff)
- [`examples/skills/incident-brief`](./skills/incident-brief)

For narrower authority and policy debugging after that native CLI path, use:

- [`examples/skills/explain-capability-denial`](./skills/explain-capability-denial)
- [`examples/skills/explain-http-authority`](./skills/explain-http-authority)
- [`examples/skills/diff-execution-authority`](./skills/diff-execution-authority)

## Deeper Proof Fixtures

- Composition and invoke boundary fixtures:
  [`examples/skills/invoke-parent-single-child`](./skills/invoke-parent-single-child),
  [`examples/skills/invoke-child-zero`](./skills/invoke-child-zero),
  [`examples/skills/hello-composite`](./skills/hello-composite)
- Bounded HTTP example:
  [`examples/skills/inspect-http-json`](./skills/inspect-http-json)

Use the top-level README and [`docs/testing.md`](../docs/testing.md) for the
smoke commands that exercise these examples through the real CLI and runtime.
