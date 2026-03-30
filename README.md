# Guild

Guild is trusted operational automation for engineering teams.

Guild gives operators one trust chain they can review before a workflow runs
and inspect after it completes: admission -> bounded execution -> receipt ->
evidence -> replay-oriented explanation.

Today, the repo exposes that model through portable skills, bounded
capabilities, durable execution and evidence records, and stable Guild refs.
The broader playbook surface is still being defined, so the docs keep the
target operator story and the current implementation boundaries separate.

Guild Ops Starter is the first operator starter set in the repo. It is a
repo-local release slice built on that trust chain, not the whole product
story.

> Status: pre-alpha.
>
> For the current project framing, start with
> [`docs/project-positioning.md`](docs/project-positioning.md). That doc now
> carries the canonical operator-facing vocabulary and capability language for
> this phase. For the current playbook-facing explanation of how those terms
> map onto today’s skill-driven runtime, use
> [`docs/how-guild-works.md`](docs/how-guild-works.md). Current CLI help,
> manifests, and `guild grants template` still use the live internal family
> names; the positioning doc is the operator-facing approval vocabulary in this
> phase.
>
> Use `guild` for local workflows, `guild mcp serve --stdio` for MCP integration, and the deeper docs for proof, benchmark, and contract details.
>
> If you want the short daily-user model first, start with [`docs/how-guild-works.md`](docs/how-guild-works.md).
> If you want one current end-to-end trust proof path, start with
> [`docs/trust-proof-walkthrough.md`](docs/trust-proof-walkthrough.md).
> If you want the current receipt-chain and replay boundary, use
> [`docs/receipt-chain-and-replay-boundaries.md`](docs/receipt-chain-and-replay-boundaries.md).
> If you want the current verification matrix and the exact meaning of
> `experimental`, `curated`, and `verified`, use
> [`docs/verification-matrix.md`](docs/verification-matrix.md).

Normative runtime sources live in `SPECS.md` section "Source Of Truth", `wit/guild-skill-v1.wit`, and the core Rust runtime/types.
Generated support, compatibility, and benchmark artifacts remain checked outputs, not primary contract definitions.
For the frozen core runtime-contract surfaces in this milestone, see `SPECS.md` section "Contract Surface v1 (core)".

## Trust Chain

Guild's operator-facing trust chain on today's live path is:

- `admission`: review execution identity, declared authority, and the
  caller-requested grants before a run starts
- `bounded execution`: the host narrows authority and runs the guest inside
  explicit runtime boundaries
- `receipt`: Guild persists a durable execution record for what ran and how it
  ended
- `evidence`: Guild keeps durable evidence payloads and metadata that point
  back to that run
- `replay-oriented explanation`: operators can explain or re-check what
  happened from stored refs using `guild why`, `guild get`, and the
  explain/report surfaces that exist today; this is not yet a first-class
  replay engine

For the short daily-user model, start with
[`docs/how-guild-works.md`](docs/how-guild-works.md). For one current
operator proof path, start with
[`docs/trust-proof-walkthrough.md`](docs/trust-proof-walkthrough.md). Use
`SPECS.md` and `ARCHITECTURE.md` when you need the exact contract and
subsystem boundaries behind that story.

## Why Guild

Guild is strict about a few things on purpose because safe operational automation needs them:

- operators should be able to understand what a workflow is allowed to do before it runs
- the host, not the guest, owns trust-sensitive authority and isolation boundaries
- execution should leave receipts and evidence that explain what happened later
- inspect, plan, and apply must stay distinct
- the MCP surface should stay small and boring

Guild is not a generic agent framework or a broad workflow engine. Today it runs skills directly and exposes the trust chain around them; the operator-facing playbook story should lead the product, while mechanism-layer terms remain available where precision matters.

## What Works Today

Guild already ships the trust and evidence backbone behind that operator story:

- a real local `guild` CLI for install, show, grants, run, ls, get, why, verify, trust, transport, and MCP setup
- a local registry root with durable execution and evidence records under `guild://...`
- signed bundle export and import with local trust verification
- OCI image layout and OCI registry transport for installed signed bundles
- a real stdio MCP server with one public tool, `guild.inspect`, plus Guild resources
- bounded live-proof coverage for specific `read-resource`, `http-request`, `invoke-skill`, `emit-evidence`, and `log-write` slices
- Guild Ops Starter, the first operator starter set in the repo, now centered on one `incident-casefile` quickstart over stored executions, bounded query refs, and optional evidence refs

The live-proof envelope is intentionally narrow. The exact current status lives in `SPECS.md`, `docs/testing.md`, and `docs/schemas/draft-v1/family_support_matrix.json`, and the docs below keep those limits explicit instead of smoothing them over.

## CLI

Install the operator CLI with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

After install, the normal workflow is the `guild` binary itself.
Repo-local proof commands and lower-level developer helpers live in
`docs/testing.md`.

Top-level commands are grouped around daily use, distribution, and setup:

- daily use: `guild show`, `guild grants ...`, `guild run`, `guild ls`, `guild get`, `guild why`, `guild verify`, `guild doctor`
- install and publish: `guild install`, `guild export`, `guild import`, `guild push`, `guild pull`, `guild trust ...`
- setup and integration: `guild init`, `guild mcp ...`, `guild codex ...`

Legacy aliases remain available for existing scripts:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

The CLI now also ships focused help topics:

- `guild help refs`
- `guild help inspect`
- `guild help trust`
- `guild help roots`
- `guild help doctor`
- `guild help preview`
- `guild help grants`

Use `guild help inspect` when you want the shipped inspect-first preview
wording for today's `show`/`why`/`get`/`ls` inspection surfaces versus the
target `admit -> exec -> inspect -> replay` flow.
Use `guild help doctor` when you want the shipped read-only diagnostics wording
for the selected Guild root and the current local state that the daily CLI
depends on.
Use `guild help preview` when you want the shipped preflight wording for risky
`import` and `pull` flows before any state change.
Use `guild help refs` when you want the shipped ref-shape wording for canonical
skill refs, Guild resource refs, and the source/install/resolved identity
layers.
Use `guild help trust` when you want the shipped trust-review wording for the
preview/import-or-pull/verify loop and the local trust-store maintenance
surface.
Use `guild help roots` when you want the shipped root-resolution wording for
`--registry-root`, `GUILD_REGISTRY_ROOT`, `~/.guild`, and the `root/setup`
failure label.
Use `guild help grants` when you want the shipped read-only grant-authoring
wording for the current active executable families. That help topic also keeps
the operator-facing capability renderings explicitly presentation-only rather
than widening runtime support claims.

Version note: the current workspace Cargo packages, including the `guild` CLI crate, are `0.1.1`. The checked-in example Guild skill manifests still resolve as `0.1.0` / `@^0.1`, and the OCI transport examples intentionally keep those manifest-driven tags. Cargo package version and Guild skill identity are separate axes.

## Identity Layers

Guild uses three identity layers in normal operator workflows:

- source skill: the local source directory you pass to `guild install`
- installed executable state: the installed record under the selected Guild root
- resolved executable identity: the exact installed executable selected to run, including its resolved ref and digest

Trace those layers together with:

```bash
guild show -v skill://example/hello-inspect@^0.1
```

That verbose view prints the requested ref, the resolved ref, the resolved digest, and the installed path together so you can see what you asked for, what Guild installed, and what exact executable identity Guild selected.

When the question is "why did this request resolve to that digest?", the first explanation surface is:

```bash
guild show -vv skill://example/hello-inspect@^0.1
```

That very verbose view explains how Guild interpreted the request, which installed versions matched it, and why one digest was selected from installed state.

## Authority Lifecycle

Guild also uses one authority lifecycle in normal operator workflows:

- declared authority: capabilities declared by the installed manifest and visible in `guild show`
- requested authority: caller-requested grants passed to `guild run`
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

Guild does not hand the guest ambient authority. The host may reduce or deny caller-requested authority before guest start, and the runtime only exposes the final granted set.

When you want a concrete starting point instead of hand-writing grant JSON from scratch, use `guild grants template <family>` for the current active families, narrow the placeholder values, and pass the result back through `--grants-json` or `--grants-file`.

## Quickstart

Guild chooses a local root in this order:

- `--registry-root <path>`
- `GUILD_REGISTRY_ROOT`
- `~/.guild`

There is no cwd-local `.guild/` fallback. `guild init` is the explicit root-creation workflow, and read-only commands do not silently create a missing root.

## Diagnostics

`guild doctor` is the first read-only Guild-scoped diagnostic command.

Its first checks stay tied to real Guild state:

- selected Guild root resolution and whether the root can be opened read-only
- installed and persisted state needed by the daily CLI under that root
- local trust-store state relevant to `guild verify` and `guild trust`
- Guild-specific runtime or setup checks grounded in real Guild reads

Its non-goals are just as important:

- no root creation, install, config writing, or trust mutation
- no remote registry probing or generic machine-inspector behavior
- no hidden bootstrap or repair side effects

### Review Authority, Execute, Inspect, And Verify

```bash
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

What that flow shows:

- it is today's compatible operator flow: review authority and execution identity -> execute a bounded action -> inspect the stored result -> verify installed trust state
- `install` builds source into installed executable state
- `show` is the primary non-executing summary path
- `show -v` traces requested ref -> resolved ref -> resolved digest -> installed path
- `show -vv` is the first requested-ref explanation path and explains why one digest was selected
- `grants template` is the read-only starting point when you need concrete JSON for an active capability family before a run
- `run` executes a human-facing `skill://...` ref through the real Guild path using caller-requested grants filtered through host policy into final runtime authority
- `ls` shows installed skills and recent persisted activity
- `ls`, `why`, and `get` together are today's concrete inspect surfaces while the broader inspect story is still split across multiple commands
- successful runs return a durable `guild://executions/...` receipt
- `why` explains a persisted execution record, points to nearby child or evidence refs when present, summarizes requested-versus-granted authority, and summarizes stored authority observations
- `get` reads the same resource backend used by MCP and guest `read-resource`
- `verify` reports installed trust and verification state for skill refs only

Default human output is concise and meant for reading, not parsing. It may include low-noise follow-up hints such as `Next: ...` on clear success paths. Use `--json` for structured machine-readable output and `--porcelain` for stable one-line machine-readable output. When a command supports `--json`, failure output stays machine-readable too: stdout carries a JSON `error` envelope, stderr stays empty, and the process exits nonzero.

`guild why` stays compact by default and may include one nearby short execution or evidence ref so you can keep navigating stored work without pasting full URIs first. It also reports a compact `requested vs granted:` summary for the stored execution. Use `guild why -v` when you need the expanded nearby-ref lists, the requested-versus-granted authority diff, and family-aware request hints for that stored execution. Use `guild why --lineage` when you want the native bounded ancestor and descendant view over persisted executions without dropping into an example skill yet.

`guild run` keeps the payload on stdout and writes the human execution summary to stderr. When host policy reduced the granted slice or stored observations show blocked authority, the success path may point you straight to `guild why -v <execution-uri>`. Authority-denial failures may also include one bounded family-aware `hint:` before the usual follow-up commands. `guild get` stays the raw resource-read path and supports `--json`, `--porcelain`, and `--output <path>` when you want machine-stable reads instead of styled summaries.

If you want an explicit non-default root for local proofs or CI, keep passing it:

```bash
guild --registry-root target/dev-local-registry/hello install examples/skills/hello-inspect
```

## Failure Paths

Guild now uses a small set of user-facing failure labels on the human CLI path:

- `root/setup`: the selected Guild root or one of its local config files could not be opened as-is
- `lookup/ambiguity`: the ref you gave Guild was missing or not specific enough
- `resource/read`: the durable execution, evidence, or object ref was not available under the selected root
- `authority denial`: local policy denied the run before guest start
- `runtime/compatibility`: the active runtime could not honor the declared runtime surface
- `trust/verification`: a signed artifact or signed plan check failed against the selected root

The follow-up guidance should stay boring and local:

- use `guild ls ...` to find durable state when a read path is missing
- use `guild show -v ...` before rerunning when the problem is authority or runtime shape
- use `guild why ...` after a rejected run when Guild persisted an execution receipt
- use `guild why -v ...` when you need the stored requested-versus-granted diff or family-aware authority hints
- use `guild trust list` and `guild trust add ...` when a trust check fails closed

Wrong-world manifest drift and broader Guild component imports should surface as
`runtime/compatibility`, not `authority denial`.

Representative failure paths:

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

## User Journeys

If you are deciding where to start, use the user-facing docs in this order:

- Compatible operator flow in today's CLI: review authority and execution identity -> execute a bounded action -> inspect the stored result -> verify installed trust state.
- Install and run a skill: the quickstart above plus [`examples/skills/hello-inspect/README.md`](examples/skills/hello-inspect/README.md)
- Explain what happened: start with `guild why` as the first nearby-ref, requested-versus-granted authority, and authority-observation surface, use `guild why -v` for the expanded stored diff and family-aware request hints, use `guild why --lineage` for the native bounded ancestor/descendant view, use `guild get` for raw durable reads, and use `guild ls evidence --limit 5` when you need to discover stored evidence first; then move to [`examples/skills/explain-execution/README.md`](examples/skills/explain-execution/README.md), [`examples/skills/explain-execution-tree/README.md`](examples/skills/explain-execution-tree/README.md), or the [`Guild Ops Starter quickstart`](docs/guild-ops-starter-quickstart.md) when you want one cohesive casefile over the same stored execution
- Verify trust state and move installed state: use `guild verify` plus the trust and transport flow below
- Debug failures and compare runs: start with `guild why -v` for the stored requested-versus-granted diff and family-aware hints, then use the [`Guild Ops Starter quickstart`](docs/guild-ops-starter-quickstart.md), [`Guild Ops Starter`](examples/skills/guild-ops-starter/README.md), and the surrounding index at [`examples/README.md`](examples/README.md); move to narrower authority and policy example skills only when `guild why -v` is no longer enough, especially [`examples/skills/explain-capability-denial/README.md`](examples/skills/explain-capability-denial/README.md), [`examples/skills/diff-execution-authority/README.md`](examples/skills/diff-execution-authority/README.md), and [`examples/skills/explain-http-authority/README.md`](examples/skills/explain-http-authority/README.md)

The deeper proof and benchmark commands still live in [`docs/testing.md`](docs/testing.md), but they are maintainers' helper paths rather than the main onboarding route.

## Guild Ops Starter

Guild Ops Starter is the first operator starter set in the repo. It is a
repo-local release slice built on that trust chain, not the whole product
story. The current user-facing example set lives at
[`examples/skills/guild-ops-starter/README.md`](examples/skills/guild-ops-starter/README.md).
The shortest current starter path lives at
[`docs/guild-ops-starter-quickstart.md`](docs/guild-ops-starter-quickstart.md).
The surrounding examples index lives at [`examples/README.md`](examples/README.md).
The examples docs now also carry the approved reference playbook set and the
current hero-example boundary without depending on a separate strategy
directory.

It is intentionally ordinary example-skill layout, not a new packaging system.
The starter story now centers one primary read-only artifact plus focused
drill-down skills:

- `incident-casefile` as the primary starter artifact
- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by older parent report skills

The example set is meant to show the current Guild story without broadening
runtime or proof semantics: durable refs, compact terminal output, explicit
capability requirements, and bounded host-mediated reads only where they are
already real.

## Trust And Transport

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

That flow demonstrates the current trust model:

- export and import operate on installed signed bundle semantics, not source directories
- `guild trust ...` manages local trust-store state only
- OCI transport carries the same installed signed bundle contract through another transport shape

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
starter-set labels by themselves. In particular, `verified-import` is one
target-root verification fact for one installed skill; it does not by itself
make a broader curated asset `verified`. Use
[`docs/verification-matrix.md`](docs/verification-matrix.md) for the current
labeling bar.

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

Execution-plan signing stays on the same local trust model:

```bash
guild trust sign-plan \
  --plan docs/schemas/draft-v1/examples/zero-authority.admit.plan.json \
  --identity-file target/dev-local-registry/local.example.json \
  --output target/dev-local-registry/zero-authority.admit.signed.plan.json

guild --registry-root target/dev-local-registry/b trust verify-plan \
  --plan target/dev-local-registry/zero-authority.admit.signed.plan.json
```

Preview direction for risky flows is now shipped for the first slice:

- first preview flag: `--preview`
- first scope: `guild import bundle`, `guild import oci-layout`, and `guild pull`
- preview must report real signed installed-state metadata, publisher identity, combined verification result and trust tier, bundle digest context, and bundled closure scope before any state change
- preview must stay read-only: no root creation, staging, installation, trust mutation, or fake detached summary
- `export` and `push` stay out of the first preview slice

Example:

```bash
guild --registry-root target/dev-local-registry/b import bundle target/dev-local-registry/portable-bundle --preview
guild --registry-root target/dev-local-registry/b pull 127.0.0.1:5000/guild-example-hello-inspect:0.1.0 --allow-http --preview
```

Use `guild help inspect` for the shipped inspect-first preview and `guild help preview` for the risky-flow preflight wording of that contract direction.

For the current operator story around mirroring reviewed installed state and
promoting it between roots or OCI locations, read
[`docs/mirroring-and-promotion.md`](docs/mirroring-and-promotion.md). That
guide keeps one limit explicit: `guild export ...` and `guild push ...` are
publication steps over installed state, not silent copy or retag primitives.
It also now anchors the current install-review surface: `--preview` before
admission, then `guild verify -v` after import or pull. Any future curated
install view should stay layered on those existing trust and compatibility
surfaces rather than introducing a new pack type or marketplace contract.

## MCP And Codex

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

For agent discovery, start with `tools/list` and expect exactly one public tool,
`guild.inspect`. Then use `resources/list`, read the returned URIs through
`resources/read`, move to `resources/templates/list` when you need a custom
query shape or exact URI family, and use `guild.inspect` only when you mean to
execute inspect mode and persist a new execution record.

If a client renders `Tools: (none)` against this server, treat that as a client
compatibility regression rather than the intended Guild MCP surface.

For task-shaped agent workflows, use
[`docs/mcp-agent-recipes.md`](docs/mcp-agent-recipes.md).

For persistent Codex integration, use the explicit setup workflow:

```bash
guild init
guild init --global
```

`guild init` creates the selected Guild root, prints the exact `guild mcp serve --stdio` wiring for the running `guild` binary, and can explicitly update global or project Codex config files with `--global` or `--project`.

For deterministic repo-local scenarios and smoke flows from this repository, Guild also keeps the `guild codex` helper surface:

```bash
guild codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
guild codex print-config --registry-root target/dev-local-registry/codex-local
```

`guild codex` is not the normal setup path. It is the deterministic repo-local helper surface for bootstrap, scenario preparation, and smoke coverage.

## Status

Guild still tracks work in milestone labels, but the practical summary is:

- M3 and M4 are complete as the draft-v1 contract and admission bundle under `docs/schemas/draft-v1/`
- M5, M6, and M7 are complete as bounded draft-v1 minimization, token, and witness flows
- M8a and M8b are complete for the active live runtime vocabulary and canonical family mapping
- M8c is partial and intentionally narrow; the exact supported live-proof slices are documented in `docs/testing.md`
- M8-proper is complete as the checked real-path benchmark under `docs/schemas/draft-v1/benchmark_matrix.json` and `docs/benchmarking/m8-real-path-benchmark.md`
- M9 is complete as the measured patent packet under `docs/patent/`
- M10 is not started

If you need the full milestone-by-milestone detail, start with `docs/roadmap.md`, `docs/testing.md`, and `docs/schemas/draft-v1/README.md`.

## Canonical Docs

- `docs/project-positioning.md` - current narrative, target audience, and language decisions for Guild
- `docs/how-guild-works.md` - short operator model for identity, authority, receipts, evidence, and the main CLI surfaces
- `docs/trust-proof-walkthrough.md` - current end-to-end operator trust proof over review, receipt, evidence, and explanation surfaces
- `docs/mcp-agent-recipes.md` - task-shaped MCP recipes for agent users and integrators
- `docs/command-language.md` - public CLI verbs, grouped workflows, and ref grammar
- `docs/mirroring-and-promotion.md` - current operator guidance for mirroring and promoting signed installed-state artifacts
- `docs/authoring-layer-guardrails.md` - compile-down rules for future authoring ergonomics without creating a second contract surface
- `docs/testing.md` - verification commands, proof workflows, and smoke paths
- `SPECS.md` - normative contract and conformance language
- `ARCHITECTURE.md` - practical system view and trust boundaries
- `docs/adr/README.md` - decision log and ADR backlog
- `AGENTS.md` - contributor guardrails for contract-first changes
- `docs/roadmap.md` - ordered epics and build priorities
- `docs/roadmap/epics/portable-skill-receipts-and-reference-apps.md` - next-phase epic for turning the trust and receipt layer into operator-facing playbooks and starter sets

Compatibility wrappers remain at `docs/contracts.md` and `docs/architecture.md` so existing links keep working.
