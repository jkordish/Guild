# Guild Ops Starter

Guild Ops Starter is the first operator starter set in the repo. It is a repo-local release slice built on that trust chain, not the whole product story.

This starter now centers one cohesive read-only artifact:
`incident-casefile`. The supporting example skills still exist, but they are
drill-down tools rather than the primary onboarding path.

There is no new pack type here. Guild Ops Starter is still a small set of
ordinary example skills under `examples/skills/`.

## Reference Playbook Fit

This starter is the current read-only bridge into the reference-playbook story,
especially for:

- `diagnose service -> restart pods -> notify on-call`
- `rollback deployment -> verify health -> annotate incident`

Guild Ops Starter currently covers the inspect, compare, query, and
evidence-review side of those workflows. It does not yet execute restart,
rollback, incident-write, or notification steps.

Treat the current hero-example boundary like this:

- operator intent: diagnose and verify one operational incident from durable refs
- current runnable path: `guild why`, `guild why --lineage`, then `guild run incident-casefile@^0.1 ...`
- docs-first future action steps: restart, rollback, and notify flows

That keeps the action-heavy story honest while still letting the docs describe
where the starter path is headed.

## What Comes Next

The next believable progression after this starter is a docs-first
`service-recovery review pack`.

That next pack stays inside the surfaces this starter already proves:

- one subject execution review in `guild why`
- one comparison run for context
- one bounded recent-failures query
- optional evidence review
- one compact casefile or drill-down report before any future action step

Keep the follow-on concepts classified like this:

| Concept | Current status | Boundary |
| --- | --- | --- |
| service-recovery review pack | docs-first next progression | diagnose and verify are real now; restart and notify remain future action steps |
| rollback verification pack | docs-first visible concept | explain, compare, and evidence review fit today's repo; rollback and incident annotation do not |
| cache purge with evidence trail | first plausible next implementation candidate | keep it visible for later mutation planning, not as current starter truth |
| certificate renewal, node remediation, and secret rotation packs | deferred until apply | broader action, approval, and blast-radius support stay later-phase |

## Playbook Translation Boundary

When maintainers describe Guild Ops Starter in playbook terms, the wording can
change in these narrow ways:

- the starter reads as one operator-facing playbook family rather than just a collection of example skills
- the README journey map reads like playbook intent: explain one run, compare two runs, scan failures, inspect evidence
- operator-facing capability names such as `runs:inspect` and `failures:query` become the review language a future playbook surface would show first
- the installed skills become reusable execution steps that a future playbook layer could point at

What does not change underneath:

- Guild Ops Starter is still a small set of ordinary installed skills
- each referenced skill still resolves to immutable installed executable identity before execution
- caller-requested authority is still narrowed by host-owned policy before guest start
- the live grant JSON still uses the current internal family names such as `read-resource` and `invoke-skill`
- durable execution receipts and evidence records still live at the underlying skill execution layer
- `render-report` remains a bounded zero-authority formatter child, not a workflow engine

## Start Here

If you want one current five-minutes-to-first-useful-run path, start with:

- [`../../../docs/guild-ops-starter-quickstart.md`](../../../docs/guild-ops-starter-quickstart.md)

That quickstart keeps the order explicit:

1. `guild codex bootstrap`
2. `guild codex scenario --scenario recent-failure-triage --json`
3. `guild why`
4. `guild why --lineage`
5. `guild run incident-casefile@^0.1`

If you are not using the deterministic Codex dogfood path, install the primary
starter skill directly with:

```bash
guild install examples/skills/incident-casefile
```

## What This Starter Includes

| Skill | Role in the starter | Question it answers | Input |
| --- | --- | --- | --- |
| `incident-casefile` | primary starter artifact | What happened here, what nearby refs add context, and what should I inspect next? | one required `guild://executions/<id>` ref plus optional comparison/query/evidence refs |
| `incident-brief` | execution drill-down | What happened in this one stored execution? | one `guild://executions/<id>` ref |
| `run-diff` | comparison drill-down | What changed between these two stored executions? | two `guild://executions/<id>` refs |
| `recent-failures` | query drill-down | What do the latest failed or refused executions look like inside this bounded query? | one `guild://queries/executions/...` ref |
| `evidence-summary` | evidence drill-down | What is this stored evidence record and why does it exist? | one `guild://objects/records/<id>` ref |
| `render-report` | formatter child used by legacy report skills | Render normalized report JSON as compact markdown | normalized JSON report input |

In operator-facing capability language, the current honest read-only review
surface still reads like:

- `runs:inspect`
- `runs:compare`
- `failures:query`
- `evidence:inspect`

The concrete runtime path remains bounded `read-resource` over
`guild://executions/`, `guild://queries/executions/`, and
`guild://objects/records/`. Only the older report skills still use one bounded
`invoke-skill` alias for the zero-authority formatter child.

## Deterministic Setup

Use the repo-local deterministic setup when you want real stored refs quickly:

```bash
export GUILD_REGISTRY_ROOT=target/dev-local-registry/ops-pack

guild codex bootstrap --registry-root "$GUILD_REGISTRY_ROOT" --reset
guild codex scenario --registry-root "$GUILD_REGISTRY_ROOT" --scenario recent-failure-triage --json
```

That scenario JSON includes:

- `subject_execution_uris`
- `comparison_execution_uris`
- `query_uris`

Use one subject execution ref first. Add one comparison execution ref and one
query ref when you want a fuller casefile. If you already have one concrete
evidence record URI, you can pass that too.

## Journey 1: Inspect The Subject In The Native CLI

Start with the normal CLI before you ask the reusable starter artifact to build
the report:

```bash
guild why <paste one subject_execution_uri>
guild why --lineage <paste one subject_execution_uri>
```

That keeps the native stored-execution explanation path in front:

- `guild why` is the first compact explanation surface
- `guild why --lineage` is the first bounded ancestor/descendant surface
- `guild get` remains the raw resource read if you need the stored JSON

## Journey 2: Build One Incident Casefile

Use `incident-casefile` when you want one compact markdown casefile over the
same durable refs:

```bash
guild run \
  incident-casefile@^0.1 \
  --input-json '{"subject_execution_uri":"<paste one subject_execution_uri>","comparison_execution_uri":"<paste one comparison_execution_uri>","query_uri":"<paste one query_uri>"}' \
  --grants-json '{"grants":[{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://executions/"],"resource_kinds":["execution"]}},{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://queries/executions/"],"resource_kinds":["query"]}}]}'
```

If you also have one evidence record URI you want folded into the report, add
`"evidence_uri":"<paste one evidence_uri>"` to the input JSON and add one more
bounded object-record read grant:

```bash
{"id":"read-resource","access":"read","constraints":{"uri_prefixes":["guild://objects/records/"],"resource_kinds":["object"]}}
```

The casefile stays read-only and host-mediated:

- no new CLI verb
- no new MCP tool
- no new pack manifest
- no mutation hidden inside the report path

## Journey 3: Drill Down With Focused Skills

After the casefile, move to the narrower starter skills only when you need a
more specific read:

- `incident-brief` for one execution-focused markdown report
- `run-diff` for one two-execution comparison report
- `recent-failures` for one bounded failure-query summary
- `evidence-summary` for one concrete evidence-record explanation

Keep using the normal CLI around those follow-ups:

```bash
guild ls runs --limit 5
guild ls evidence --limit 5
guild get <paste one query_uri>
guild show <paste one evidence_uri>
```

## What This Starter Does Not Show

- It does not execute restart, rollback, incident-write, or notification actions from the reference playbook set yet.
- It does not imply `k8s:restart`, `chat:post`, or `incident:create` already ship as runnable capabilities.
- It does not add replay execution support. `guild why` and `guild get` remain the current replay-oriented explanation surfaces.
- It does not broaden `http-request` into the hero path.
- It does not add a workflow engine, pack manifest, or new installer abstraction.
- It does not dump raw JSON by default. The report skills return compact markdown so `guild run` prints useful terminal output directly.
