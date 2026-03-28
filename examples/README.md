# Examples

Guild examples are operator workflows and supporting example skills grounded in
current repo truth.

There is no separate pack system here. The examples exist to prove and teach
specific honest slices of the current repo. Today Guild runs skills directly,
and these examples show how the trust chain, receipts, and evidence support
real operational tasks on those current surfaces.
For the current project framing, see [`../docs/project-positioning.md`](../docs/project-positioning.md).

The canonical command examples assume `guild` is installed and on `PATH`.
Lower-level `cargo run -p guild-mcp --example ...` commands are developer proof
helpers, not the normal operator workflow.

## Start Here

If you want one compact first-five-minutes workflow, start with the Guild Ops
Starter quickstart:

- [`docs/guild-ops-starter-quickstart.md`](../docs/guild-ops-starter-quickstart.md)

Then use the full starter README when you want the surrounding drill-down
skills and boundaries:

- [`examples/skills/guild-ops-starter/README.md`](./skills/guild-ops-starter/README.md)

That starter set is the current user-facing path for trusted local operational
analysis over durable Guild refs. It now centers:

- `incident-casefile` for one cohesive casefile over one subject execution and optional nearby refs

The focused drill-down skills remain:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the older parent report skills

In operator-facing capability language, that starter set currently teaches a
read-only review surface that reads like:

- `runs:inspect` for one stored run
- `runs:compare` for two stored runs
- `failures:query` for one bounded failure query
- `evidence:inspect` for one stored evidence record

The concrete runtime path still uses bounded `read-resource` grants and, for
the report-formatting parent skills, a bounded `invoke-skill` alias for the
zero-authority formatter child.

## Reference Playbook Anchors

The approved operator-facing reference playbook set lives in
[`../docs/strategy/guild-repositioning/07-reference-playbooks.md`](../docs/strategy/guild-repositioning/07-reference-playbooks.md).

Use that doc when you want to see which operational workflows Guild should lead
with over time: restart and notify, rollback and verify, cache purge, cert
renewal, node remediation, and secret rotation.

Today's examples are the current runnable bridge into that story, not a claim
that every reference playbook already ships as a first-class workflow:

- Guild Ops Starter is the current strongest bridge because it already teaches
  trusted local operational review over stored receipts, bounded queries, and
  evidence records.
- The current first hero example plan lives in
  [`../docs/strategy/guild-repositioning/09-hero-reference-example-plan.md`](../docs/strategy/guild-repositioning/09-hero-reference-example-plan.md)
  and keeps that future action-heavy story tied to today's support frontier.
- The example inventory is strongest today at read-only review workflows such
  as `runs:inspect`, `runs:compare`, `failures:query`, and `evidence:inspect`.
- The action-heavy playbooks in the reference set stay docs-first until they
  fit the current runtime and capability frontier honestly.

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

In operator-facing capability language, those richer read-only reports are the
current `runs:inspect`, `runs:compare`, `failures:query`, and
`evidence:inspect` examples. The concrete grant JSON still uses the current
internal family names where needed for truth.

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

Use Guild Ops Starter for compact real-path troubleshooting:

When you review those examples through the capability taxonomy, they are still
read-only approvals first: `runs:inspect`, `runs:compare`, `failures:query`,
and `evidence:inspect`. They do not imply broader runtime surfaces than the
current bounded `read-resource` and `invoke-skill` frontier.

- [`../docs/guild-ops-starter-quickstart.md`](../docs/guild-ops-starter-quickstart.md)
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
