# Examples

Guild examples are ordinary repo-native examples and example skills.

There is no separate pack system here. The examples exist to prove and teach
specific honest slices of the current repo.

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

## Other Example Groups

- Composition and invoke boundary fixtures:
  [`examples/skills/invoke-parent-single-child`](./skills/invoke-parent-single-child),
  [`examples/skills/invoke-child-zero`](./skills/invoke-child-zero),
  [`examples/skills/hello-composite`](./skills/hello-composite)
- Durable execution analysis:
  [`examples/skills/explain-execution`](./skills/explain-execution),
  [`examples/skills/explain-execution-tree`](./skills/explain-execution-tree),
  [`examples/skills/summarize-execution-query`](./skills/summarize-execution-query)
- Evidence-producing primitive runtime example:
  [`examples/skills/hello-inspect`](./skills/hello-inspect)
- Bounded HTTP example:
  [`examples/skills/inspect-http-json`](./skills/inspect-http-json)

Use the top-level README and [`docs/testing.md`](../docs/testing.md) for the
smoke commands that exercise these examples through the real CLI and runtime.
