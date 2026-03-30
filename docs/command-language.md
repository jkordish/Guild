# Command Language

This document is the source of truth for Guild's public command and URI grammar only.

It is not the runtime-contract source of truth; see `SPECS.md` section "Source Of Truth".
For the frozen runtime URI roots and support vocabulary in this milestone, see `SPECS.md` section "Contract Surface v1 (core)".
For the current long-term direction, see
[`strategy/session-substrate/00-umbrella-epic.md`](strategy/session-substrate/00-umbrella-epic.md).
For the bridge from the prior framing to the current direction, see
[`project-positioning.md`](project-positioning.md).
The current command surface still uses the live internal family names in help
and `guild grants template`; the strategy docs do not rename the current CLI.

## Install And Run

Install the operator CLI with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

After install, the command language uses `guild ...` directly.
Repo-local proof commands and lower-level developer helpers live in
`docs/testing.md`.
If you want the short daily-user mental model first, read
[`docs/how-guild-works.md`](how-guild-works.md).

The default help is task-oriented:

- `guild --help`
- `guild help refs`
- `guild help inspect`
- `guild help trust`
- `guild help roots`
- `guild help doctor`
- `guild help preview`
- `guild help grants`
- `guild <command> --help`

`guild help inspect` is the shipped inspect-first preview help topic for
today's `show`/`why`/`get`/`ls` inspection surfaces versus the target
`admit -> exec -> inspect -> replay` flow.
`guild help doctor` is the shipped read-only diagnostics help topic for the
selected Guild root and the current local state that the daily CLI depends on.
`guild help preview` is the shipped preflight help topic for risky `import` and
`pull` flows before any state change.
`guild help refs` is the shipped ref-shape help topic for canonical skill refs,
Guild resource refs, and the source/install/resolved identity layers.
`guild help trust` is the shipped trust-review help topic for the
preview/import-or-pull/verify loop and the local trust-store maintenance
surface.
`guild help roots` is the shipped root-resolution help topic for
`--registry-root`, `GUILD_REGISTRY_ROOT`, `~/.guild`, and the `root/setup`
failure boundary.
`guild help grants` is the shipped read-only grant-authoring help topic for the
current active executable families. It also keeps the operator-facing
capability renderings explicitly presentation-only instead of widening runtime
support claims.

## Target Operator Flow

Guild's target operator journey is:

1. `admit`
2. `exec`
3. `inspect`
4. `replay`

That sequence describes the target UX, not the fully shipped top-level CLI in
this milestone. Use the target verbs in planning and migration language, but
keep today's binary surface explicit and use `guild help inspect` for the
shipped inspect-first preview:

- `guild admit`: target-state preflight for capability review, policy
  narrowing, and execution readiness. There is no first-class command today.
  The closest current surfaces are `guild show`, `guild grants template`, and
  the read-only preview/help direction.
- `guild exec`: target-state execution surface. Today this is `guild run`.
- `guild inspect`: target-state inspection surface for receipts, evidence, and
  execution history. Today that work is split across `guild show`, `guild why`,
  `guild get`, and `guild ls`.
- `guild replay`: target-state rerun or re-check surface from stored receipt
  context. There is no first-class command today.

Keep these boundary notes visible:

- current commands remain the source of truth for what the binary actually
  accepts today
- `guild inspect` currently exists only as a compatibility alias for
  `guild run`; it is not yet the target-state inspect surface
- `guild verify` remains a trust-specific review command and is not absorbed
  into the target inspect surface

Conceptual target flow:

- `admit`: review requested authority, policy narrowing, and readiness before execution
- `exec`: perform the bounded action
- `inspect`: review the stored receipt, evidence, and execution history
- `replay`: rerun or re-check from stored receipt context when that contract lands

### Command Mapping

| Today Surface | Target Stage | Status | Migration Notes |
| --- | --- | --- | --- |
| `guild show` | `inspect` | current surface today | primary non-executing summary path for skills, receipts, objects, and evidence |
| `guild why` | `inspect` | current surface today | primary persisted-execution explanation path for authority outcomes and nearby refs |
| `guild get` | `inspect` | current surface today | raw durable read path for Guild resources |
| `guild ls` | `inspect` | current surface today | discovery path for installed skills and persisted Guild state |
| `guild run` | `exec` | current surface today | actual execution entrypoint today |
| `guild inspect` | `exec` | alias-preview today | compatibility alias for `guild run`, not the target inspect surface |
| `guild grants template` | `admit` | helper-preview today | read-only grant-authoring helper for current active families before a run |
| `guild verify` | trust review / verify | current surface today | stays trust-specific and separate from the target inspect surface |
| no first-class command today | `replay` | future only | keep replay descriptive until bounded replay semantics exist |

Current compatible flow:

```bash
# Review execution identity and declared authority before running.
guild show -v skill://example/hello-inspect@^0.1

# Start from a concrete active-family grant template.
guild grants template emit-evidence

# Execute the bounded action on today's shipped surface and note the
# `where` execution URI from the JSON wrapper.
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json

# Inspect the stored result and evidence trail with today's inspect surfaces.
guild why exec:<execution-id-prefix-from-where>
guild get guild://executions/<execution-id-from-where>

# Verify installed trust state separately.
guild verify skill://example/hello-inspect@^0.1
```

## Command Groups

Guild's first-class local verbs today are:

### Daily Use

- `guild show`
- `guild grants ...`
- `guild run`
- `guild ls`
- `guild get`
- `guild why`
- `guild verify`
- `guild doctor`

### Install And Publish

- `guild install`
- `guild export`
- `guild import`
- `guild push`
- `guild pull`
- `guild trust ...`

### Setup And Integration

- `guild init`
- `guild codex ...`
- `guild mcp ...`

Legacy aliases remain supported for compatibility:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

Do not read those aliases as proof that the target `admit` / `exec` /
`inspect` / `replay` journey has already landed in code. They are compatibility
bridges only.

## Ref Forms

### Skill Refs

Canonical public skill syntax is:

- `skill://<namespace>/<name>@<version-or-range>`

The CLI also accepts:

- `<namespace>/<name>@<version-or-range>`
- `<name>@<version-or-range>` when unambiguous across installed skills

Docs and scripts should prefer the canonical `skill://...` form.

### Guild Resource Refs

Guild uses these public resource families:

- `guild://executions/<id>`
- `guild://objects/records/<id>`
- `guild://objects/records/<id>/metadata`
- `guild://objects/sha256/<digest>`
- `guild://queries/executions/...`

The CLI also accepts short resource refs:

- `exec:<execution-id-prefix>`
- `evidence:<evidence-record-id-prefix>`
- `obj:<sha256-prefix>`

### OCI Refs

Transport and publication use standard OCI references:

- `<registry>/<repo>:<tag>`
- `<registry>/<repo>@<digest>`

Guild intentionally does not use `guild://` for transport publication. Installed transport units move through signed bundle directories, OCI image layouts, and OCI registry references.

## Identity Layers

Guild uses three identity layers in day-to-day CLI flows:

- source skill: the local source directory passed to `guild install`
- installed executable state: the installed record stored under the selected Guild root
- resolved executable identity: the exact installed executable selected for use, identified by resolved ref plus artifact digest

The fastest way to trace those layers for one skill is:

```bash
guild show -v skill://example/hello-inspect@^0.1
```

That verbose view shows the requested ref, resolved ref, digest, and installed path together. Use it when you need to answer "what did I ask for?" versus "what exact executable did Guild select?"

When the question is "why did this request resolve to that digest?", the first explanation surface is:

```bash
guild show -vv skill://example/hello-inspect@^0.1
```

That very verbose view explains how Guild interpreted the request, which installed versions matched it, and why one digest was selected from installed state.

## Authority Lifecycle

Guild uses one authority lifecycle in day-to-day CLI flows:

- declared authority: capabilities declared by the installed manifest and visible in `guild show`
- requested authority: caller-requested grants passed to `guild run`
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

Guild does not hand the guest ambient authority. The host may reduce or deny caller-requested authority before guest start, and the runtime only exposes the final granted set.

Use `guild grants template <family>` when you need a read-only concrete JSON starting point for an active capability family before narrowing it into `--grants-json` or `--grants-file`.

## Root Resolution

Guild chooses a root in this order:

1. `--registry-root <path>`
2. `GUILD_REGISTRY_ROOT`
3. `~/.guild`

There is no cwd-local `.guild/` fallback.

`guild init` is the explicit root-creation workflow. Read-only commands do not initialize a missing root. Write-oriented commands may create the selected root when they are already doing real work.

## Diagnostics

`guild doctor` is the first read-only diagnostic command for the selected Guild root.

Current scope:

- selected Guild root resolution and whether the root can be opened read-only
- installed and persisted state needed by the daily CLI under the selected root
- local trust-store state relevant to `guild verify` and `guild trust`
- Guild-specific runtime or setup checks grounded in real Guild reads

Non-goals:

- no root creation, install, config writing, or trust mutation
- no remote registry probing or generic machine-inspector behavior
- no hidden bootstrap or repair side effects

## Primary Workflows

### Quickstart

```bash
# Create the local Guild root and install the example skill.
guild init
guild install examples/skills/hello-inspect

# Review execution identity and declared authority before running.
guild show skill://example/hello-inspect@^0.1
guild grants template emit-evidence

# Execute the bounded action.
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json

# Inspect the stored result and nearby durable state.
guild ls runs --limit 5
guild why exec:<execution-id-prefix>
guild get guild://executions/<execution-id>

# Verify installed trust state separately.
guild verify skill://example/hello-inspect@^0.1
```

What this flow teaches:

- `install` is source-to-installed, not source-to-runtime bypass
- `show` is the primary non-executing summary path
- `show -v` traces requested ref -> resolved ref -> resolved digest -> installed path
- `show -vv` is the first requested-ref explanation path and explains why one digest was selected
- `grants template` is the read-only authoring helper for current active-family grant JSON
- `run` executes a `skill://...` ref through the real Guild runtime path after host policy computes the final granted authority for that run
- success produces a durable `guild://executions/...` receipt
- `ls`, `why`, and `get` together are today's concrete inspect surfaces while the broader inspect story is still split across multiple commands
- `why` explains one stored execution record, points to nearby child or evidence refs when present, summarizes requested-versus-granted authority, and summarizes stored authority observations
- `get` reads the same backend used by MCP and guest `read-resource`
- `verify` reports installed trust and verification state for skill refs only

### Journey Map

Use the examples and docs in this order when you want the current practical path rather than the full maintainer proof surface:

- Compatible operator flow in today's CLI: review authority and execution identity -> execute a bounded action -> inspect the stored result -> verify installed trust state.
- Install and run a skill: the quickstart above plus [`examples/skills/hello-inspect/README.md`](../examples/skills/hello-inspect/README.md)
- Explain what happened: start with `guild why` as the first nearby-ref, requested-versus-granted authority, and authority-observation surface, use `guild why -v` for the expanded stored diff and family-aware request hints, use `guild why --lineage` for the native bounded ancestor/descendant view, use `guild get` for raw durable reads, and use `guild ls evidence --limit 5` when you need to discover stored evidence first; then move to [`examples/skills/explain-execution/README.md`](../examples/skills/explain-execution/README.md), [`examples/skills/explain-execution-tree/README.md`](../examples/skills/explain-execution-tree/README.md), or the [`Guild Ops Starter quickstart`](guild-ops-starter-quickstart.md) when you want one cohesive casefile over the same stored execution
- In operator-facing capability language, those current read-only examples are `runs:inspect`, `runs:compare`, `failures:query`, and `evidence:inspect`, while the concrete grant JSON still uses bounded `read-resource` and, where present, bounded `invoke-skill`.
- Verify trust state and move installed state: use `guild verify` plus the trust and transport flow below
- Debug failures and compare runs: start with `guild why -v` for the stored requested-versus-granted diff and family-aware hints, then use the [`Guild Ops Starter quickstart`](guild-ops-starter-quickstart.md), [`Guild Ops Starter`](../examples/skills/guild-ops-starter/README.md), and the surrounding examples index at [`examples/README.md`](../examples/README.md); move to narrower authority and policy example skills only when `guild why -v` is no longer enough, especially [`examples/skills/explain-capability-denial/README.md`](../examples/skills/explain-capability-denial/README.md), [`examples/skills/diff-execution-authority/README.md`](../examples/skills/diff-execution-authority/README.md), and [`examples/skills/explain-http-authority/README.md`](../examples/skills/explain-http-authority/README.md)

`docs/testing.md` remains the place for deeper proof commands, smoke coverage, and maintainer-oriented verification.

## Failure Language

The human CLI path now uses a small stable set of failure labels:

- `root/setup`: the selected Guild root or one of its local config files could not be opened as-is
- `lookup/ambiguity`: the provided ref was missing or not specific enough
- `resource/read`: the requested durable execution, evidence, or object ref was not available under the selected root
- `authority denial`: local policy denied the run before guest start
- `runtime/compatibility`: the active runtime could not honor the declared runtime surface
- `trust/verification`: a signed artifact or signed plan check failed against the selected root

The default follow-up guidance should stay local and honest:

- use `guild ls ...` to discover stored state
- use `guild show -v ...` before rerunning after authority or runtime failures
- use `guild why ...` after a rejected run when Guild persisted a receipt
- use `guild trust list` and `guild trust add ...` when a trust check fails closed

Wrong-world manifest drift and broader Guild component imports should surface as
`runtime/compatibility`, not `authority denial`.

Representative failure examples:

```text
$ guild ls --json
root/setup: Guild registry root `~/.guild` does not exist yet
detail: read-only commands do not initialize a new root
Next: run `guild install <source-dir>` to create it, or pass `--registry-root <path>` / set `GUILD_REGISTRY_ROOT` to use an existing root

$ guild verify missing-skill@^0.1
lookup/ambiguity: short skill ref `missing-skill@^0.1` did not match any installed skill
Next: run `guild ls skills` to inspect installed skills

$ guild import bundle /tmp/bundle
trust/verification: signed bundle publisher was not trusted by the target Guild root
reason: bundle-publisher-untrusted
Next: run `guild trust list` to inspect the target root, then add the publisher with `guild trust add --identity-file <identity.json>` or `guild trust add --record-file <record.json>`
```

### Guild Ops Starter

The current user-facing example set lives at
[`examples/skills/guild-ops-starter/README.md`](../examples/skills/guild-ops-starter/README.md).
The surrounding examples index lives at
[`examples/README.md`](../examples/README.md).
The shortest starter path lives at
[`docs/guild-ops-starter-quickstart.md`](guild-ops-starter-quickstart.md).

Guild Ops Starter is the current operator starter set in the repo. It remains
a repo-local release slice and still uses ordinary example skills under
`examples/skills/`, not a new packaging system.
Use it when you want a compact real-path walkthrough over persisted Guild refs.
The starter story now centers:

- `incident-casefile` for one cohesive casefile over a subject execution and optional nearby refs

The focused drill-down skills remain:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the report skills

The honest story is narrow on purpose:

- durable Guild refs and resources
- compact markdown output on `guild run`
- explicit capability grants
- exact zero-authority composition only where the runtime already supports it
- no broad HTTP showcase
- no `emit-evidence` proof claims
- no claim that the starter is the whole product story

### Trust And Transport

```bash
guild trust generate \
  --publisher-id local.example \
  --display-name "Local Example" \
  --output target/dev-local-registry/local.example.json

guild --registry-root target/dev-local-registry/a export bundle \
  skill://example/hello-inspect@^0.1 \
  --signer target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/bundle

guild --registry-root target/dev-local-registry/b trust add \
  --identity-file target/dev-local-registry/local.example.json

guild --registry-root target/dev-local-registry/b import bundle \
  target/dev-local-registry/bundle
```

This flow stays within the current trust model:

- export and import operate on installed signed bundle semantics
- `guild trust ...` manages local trust-store state only
- OCI transport uses the same installed signed bundle contract through another transport shape

The first local bundle workflow is:

- `guild export bundle ...` writes one signed installed-state bundle directory
- `guild trust add ...` records the reviewed publisher in the target root
- `guild import bundle ...` installs that signed state into the target root
- default human output for these commands should answer the transport shape, the source or output location, the publisher, and the next likely command

The OCI workflow is the same installed-state contract through a registry-shaped transport:

```bash
guild --registry-root target/dev-local-registry/a push \
  skill://example/hello-inspect@^0.1 \
  --reference localhost:5000/guild/hello-inspect:0.1.0 \
  --signer target/dev-local-registry/local.example.json \
  --allow-http

guild --registry-root target/dev-local-registry/b trust add \
  --identity-file target/dev-local-registry/local.example.json

guild --registry-root target/dev-local-registry/b pull \
  localhost:5000/guild/hello-inspect:0.1.0 \
  --allow-http
```

- `guild push ...` publishes the same installed signed state to an OCI reference
- `guild pull ...` reads that OCI transport artifact, verifies it against the selected root, and installs the imported state
- default human output for `push` and `pull` should make the registry reference and installed-state outcome obvious

The current trust review loop is:

- `guild trust list`
- `guild import ... --preview` or `guild pull ... --preview`
- `guild import ...` or `guild pull ...`
- `guild verify -v <skill-ref>`

Use `guild verify -v <skill-ref>` as the first installed-state verification explanation path after import or pull. That view keeps the publisher and combined trust status visible and adds signing-scheme and short bundle-digest detail when verification metadata exists.

Current installed-state terms:

- `local-source`: installed from local source in the current Guild root
- `verified-import`: installed from a signed import or pull that verified successfully
- `local-dev`: local source state in the current Guild root
- `trusted-imported`: imported publisher trusted for normal imported use
- `restricted`: imported publisher trusted only under restricted local policy posture

Those installed-state terms are current trust signals, not higher-level pack or
starter-set labels by themselves. `verified-import` is one target-root
verification fact for one installed skill, not a blanket `verified` label for a
broader curated view. Use [`verification-matrix.md`](verification-matrix.md)
for the current labeling bar.

Keep these terms distinct:

- `guild verify` reviews installed trust and verification state for a skill.
- `guild why` explains one persisted execution, including policy outcomes for that run.
- `authority denial` means local policy denied required authority before guest start.
- `runtime/compatibility` means the active runtime could not honor the declared runtime surface.
- `trust/verification` means Guild could not verify a signed artifact or signed plan against the selected root.

Trust-store maintenance stays local and explicit:

- `guild trust add --identity-file <identity.json>` trusts one local publisher identity directly
- `guild trust add --record-file <record.json>` trusts one reviewed publisher record without secret key material
- `guild trust list` reviews trusted publishers and their current tiers
- `guild trust show <publisher-id>` inspects one reviewed publisher record under the selected local root
- `guild trust remove <publisher-id>` removes one local trust record when a publisher should no longer be trusted

### Preview Direction

The chosen first preflight direction is `--preview`, and the first slice now
ships as a read-only preflight for import and pull.

First scope:

- `guild import bundle`
- `guild import oci-layout`
- `guild pull`

Preview output must stay grounded in the real installer and trust model:

- inspect the signed installed-state metadata the import or pull path would use
- report publisher identity, combined verification result and trust tier, and bundle digest context
- report the top-level skill ref plus bundled dependency closure scope
- report whether Guild would import or refuse under the selected root

Non-goals:

- no root creation, staging, installation, or trust-store mutation
- no fake preview detached from signed bundle and trust verification semantics
- no preview contract for `export` or `push` in the first slice

Examples:

```bash
guild --registry-root target/dev-local-registry/b import bundle target/dev-local-registry/portable-bundle --preview
guild --registry-root target/dev-local-registry/b import oci-layout target/dev-local-registry/portable-layout --preview
guild --registry-root target/dev-local-registry/b pull 127.0.0.1:5000/guild-example-hello-inspect:0.1.0 --allow-http --preview
```

For the current operator workflow that mirrors reviewed installed state or
promotes it between roots and OCI locations, read
[`docs/mirroring-and-promotion.md`](mirroring-and-promotion.md). That guide
keeps the current boundary explicit: `guild export ...` and `guild push ...`
are publication steps over installed state, not silent registry-copy or retag
primitives. It also keeps the current install-review surface explicit:
`--preview` before admission and `guild verify -v` after import or pull. Any
future curated install view should stay layered on those existing trust and
compatibility surfaces instead of becoming a new pack type or marketplace
contract.

### Execution Plan Signing

```bash
guild trust sign-plan \
  --plan docs/schemas/draft-v1/examples/zero-authority.admit.plan.json \
  --identity-file target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/zero-authority.admit.signed.plan.json

guild --registry-root target/dev-local-registry/b trust verify-plan \
  --plan target/dev-local-registry/zero-authority.admit.signed.plan.json
```

What this flow teaches:

- M4 execution plans are still unsigned by default
- plan signing reuses the same local publisher identity and trust-store model as bundle signing
- verification is fail-closed against the local Guild trust store

## Output Contract

Default human output is compact and status-forward:

- one screen by default for common `show`, `run`, `why`, and `ls` cases
- short refs and short ids by default rather than full digest and URI dumps
- default human output is for reading, not parsing, and may gain low-noise hints such as `Next: ...`
- `guild why` may include one nearby short execution or evidence ref when related stored refs exist
- `guild why` also reports a compact requested-versus-granted authority summary and summarizes stored authority observations
- `guild why -v` expands the requested-versus-granted authority diff and any family-aware request hints
- `guild why --lineage` adds a bounded read-only ancestor/descendant view without changing machine-readable output modes
- stable vocabulary across commands:
  - `proof-backed`
  - `upper-bound`
  - `linked`
  - `unlinked`
  - `bounded`
  - `not_proven`
  - `refused`

Shared output controls for the human-summary commands (`show`, `run`, `ls`, `why`, and `verify`):

- `--json` for structured machine-readable output when you want named fields
- `--porcelain` for stable one-line machine-readable output when you want short script-friendly lines
- `-v` for important ids, digests, and installed-state details
- `-vv` for deeper technical detail
- `--debug` for full internal detail
- `--color auto|always|never`
- `NO_COLOR` disables ANSI color even when the terminal would otherwise allow it

When a command supports `--json`, failure output stays machine-readable too:
stdout carries a JSON `error` envelope, stderr stays empty, and the process
still exits nonzero.

Human-only hints and extra readability text do not belong to `--json` or `--porcelain`.

`guild get` stays a raw resource-read path rather than a styled summary view. It supports `--json`, `--porcelain`, and `--output <path>`.

`guild run` keeps payload and human status separate:

- stdout carries the payload or structured result
- stderr carries the human execution summary and any low-noise next-step hints
- successful runs may point to `guild why -v <execution-uri>` when granted authority was reduced or blocked during the run
- authority-denial failures may include one bounded family-aware `hint:` before the follow-up `Next:` lines
- `--json` and `--porcelain` keep those machine surfaces free of human hint text

## Trust Scope

`guild trust ...` uses the current local trust model only:

- generate local publisher identities
- add, list, and remove local trusted publisher records
- sign and verify execution plans against that same local publisher and trust-store model
- no remote trust distribution
- no transparency-log semantics
- no remote publisher policy management

## Codex And MCP

Guild ships a real stdio MCP server through the same CLI:

```bash
guild mcp serve --stdio
```

The public MCP surface is intentionally small:

- one public tool: `guild.inspect`
- `resources/list` is a bounded discovery catalog: the first entries are canonical recent-query URIs, followed by recent execution and evidence-metadata URIs
- `resources/templates/list` describes the parameterized Guild URI families for execution, evidence, object, and query reads
- `resources/read` fetches the durable execution, evidence, object, and bounded query resources behind those URIs
- cursor-based pagination on `tools/list`, `resources/list`, and `resources/templates/list`

For agent-facing workflows, use the MCP surfaces in this order:

- `tools/list` to confirm the one current public tool, `guild.inspect`
- `resources/list` to discover the first useful URIs under the selected Guild root
- `resources/read` to inspect those durable resources directly
- `resources/templates/list` when you need a specific query URI family or a URI you do not already have
- `guild.inspect` when you actually want to execute inspect mode and persist a new execution record

If a client renders `Tools: (none)` against this server, treat that as a client
compatibility regression rather than the intended Guild MCP surface.

For the concrete task-shaped versions of those flows, read
[`docs/mcp-agent-recipes.md`](mcp-agent-recipes.md).

For persistent Codex integration, use:

```bash
guild init
guild init --global
```

`guild init` creates the selected Guild root, prints the exact `guild mcp serve --stdio` launch command, and can explicitly update global or project Codex config.

For deterministic repo-local scenario prep and smoke coverage from this repository, Guild also ships:

- `guild codex bootstrap`
- `guild codex print-config`
- `guild codex scenario`
- `guild codex smoke`

Those helpers are for repo-local deterministic flows. The normal setup path remains `guild init`.
