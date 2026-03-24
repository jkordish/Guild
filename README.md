# Guild

Guild is a contracts-first Rust/Wasm runtime and control plane for portable AI skills.

It gives you a local CLI, a small MCP surface, and explicit host-owned execution, trust, and evidence records. The main path today is straightforward: install a skill, run it locally, read back what happened, and move installed state through signed bundles or OCI transport.

> Status: pre-alpha.
>
> Use `guild` for local workflows, `guild mcp serve --stdio` for MCP integration, and the deeper docs for proof, benchmark, and contract details.
>
> If you want the short daily-user model first, start with [`docs/how-guild-works.md`](docs/how-guild-works.md).

## Why Guild

Guild is strict about a few things on purpose:

- requested identity is not executable identity
- the host, not the guest, owns trust-sensitive authority
- evidence is a durable artifact, not a prompt scrap
- inspect, plan, and apply are distinct modes
- the MCP surface stays small and boring

The goal is not a loose agent wrapper. It is a portable skill system where execution, delegation, and witness claims stay tied to real runtime behavior.

## What Works Today

Guild already has:

- a real local `guild` CLI for install, show, run, ls, get, why, verify, trust, transport, and MCP setup
- a local registry root with durable execution and evidence records under `guild://...`
- signed bundle export and import with local trust verification
- OCI image layout and OCI registry transport for installed signed bundles
- a real stdio MCP server with one public tool, `guild.inspect`, plus Guild resources
- bounded live-proof coverage for specific `read-resource`, `http-request`, `invoke-skill`, and `log-write` slices
- a user-facing starter pack of example skills for compact ops analysis over stored executions, bounded query refs, and evidence refs

The live-proof envelope is intentionally narrow. The exact current status lives in `SPECS.md`, `docs/testing.md`, and `docs/schemas/draft-v1/family_support_matrix.json`.

## CLI

Install the operator CLI with:

```bash
cargo install --path crates/guild-mcp --bin guild
```

After install, the normal workflow is the `guild` binary itself.
Repo-local proof commands and lower-level developer helpers live in
`docs/testing.md`.

Top-level commands are grouped around daily use, distribution, and setup:

- daily use: `guild show`, `guild run`, `guild ls`, `guild get`, `guild why`, `guild verify`
- install and publish: `guild install`, `guild export`, `guild import`, `guild push`, `guild pull`, `guild trust ...`
- setup and integration: `guild init`, `guild mcp serve --stdio`, `guild codex ...`

Legacy aliases remain available for existing scripts:

- `guild inspect` -> `guild run`
- `guild read` -> `guild get`
- `guild list` -> `guild ls`

The CLI now also ships focused help topics:

- `guild help refs`
- `guild help trust`
- `guild help roots`
- `guild help doctor`
- `guild help preview`

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

## Quickstart

Guild chooses a local root in this order:

- `--registry-root <path>`
- `GUILD_REGISTRY_ROOT`
- `~/.guild`

There is no cwd-local `.guild/` fallback. `guild init` is the explicit root-creation workflow, and read-only commands do not silently create a missing root.

## Diagnostic Direction

The chosen future read-only diagnostic command is `guild doctor`.
It is not implemented yet as a first-class command, but the direction is now fixed so later implementation does not have to re-decide the product surface.

Its first checks should stay tied to real Guild state:

- selected Guild root resolution and whether the root can be opened read-only
- installed and persisted state needed by the daily CLI under that root
- local trust-store state relevant to `guild verify` and `guild trust`
- Guild-specific runtime or setup checks grounded in real Guild reads

Its non-goals are just as important:

- no root creation, install, config writing, or trust mutation
- no remote registry probing or generic machine-inspector behavior
- no hidden bootstrap or repair side effects

### Install, Show, Run, And Read Back

```bash
guild init

guild install examples/skills/hello-inspect

guild show skill://example/hello-inspect@^0.1

guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json

guild ls runs --limit 5

guild why exec:<execution-id-prefix>

guild get guild://executions/<execution-id>

guild verify skill://example/hello-inspect@^0.1
```

What that flow shows:

- `install` builds source into installed executable state
- `show` is the primary non-executing summary path
- `show -v` traces requested ref -> resolved ref -> resolved digest -> installed path
- `show -vv` is the first requested-ref explanation path and explains why one digest was selected
- `run` executes a human-facing `skill://...` ref through the real Guild path using caller-requested grants filtered through host policy into final runtime authority
- `ls` shows installed skills and recent persisted activity
- successful runs return a durable `guild://executions/...` receipt
- `why` explains a persisted execution record, points to nearby child or evidence refs when present, and summarizes stored authority observations
- `get` reads the same resource backend used by MCP and guest `read-resource`
- `verify` reports installed trust and verification state for skill refs only

Default human output is concise and meant for reading, not parsing. It may include low-noise follow-up hints such as `Next: ...` on clear success paths. Use `--json` for structured machine-readable output and `--porcelain` for stable one-line machine-readable output.

`guild why` stays compact by default and may include one nearby short execution or evidence ref so you can keep navigating stored work without pasting full URIs first. Use `guild why -v` when you need the expanded nearby-ref lists and authority-observation detail for that stored execution. Use `guild why --lineage` when you want the native bounded ancestor and descendant view over persisted executions without dropping into an example skill yet.

`guild run` keeps the payload on stdout and writes the human execution summary to stderr. `guild get` stays the raw resource-read path and supports `--json`, `--porcelain`, and `--output <path>` when you want machine-stable reads instead of styled summaries.

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
- `trust/verification`: a signed plan or trusted publisher check failed against the selected root

The follow-up guidance should stay boring and local:

- use `guild ls ...` to find durable state when a read path is missing
- use `guild show -v ...` before rerunning when the problem is authority or runtime shape
- use `guild why ...` after a rejected run when Guild persisted an execution receipt
- use `guild trust list` and `guild trust add ...` when a trust check fails closed

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

- Install and run a skill: the quickstart above plus [`examples/skills/hello-inspect/README.md`](examples/skills/hello-inspect/README.md)
- Explain what happened: start with `guild why` as the first nearby-ref and authority-observation surface, use `guild why -v` for expanded stored detail, use `guild why --lineage` for the native bounded ancestor/descendant view, use `guild get` for raw durable reads, and use `guild ls evidence --limit 5` when you need to discover stored evidence first; then move to [`examples/skills/explain-execution/README.md`](examples/skills/explain-execution/README.md) or the [`Guild Ops Starter Pack`](examples/skills/guild-ops-starter/README.md)
- Verify trust state and move installed state: use `guild verify` plus the trust and transport flow below
- Debug failures and compare runs: use the [`Guild Ops Starter Pack`](examples/skills/guild-ops-starter/README.md) and the surrounding index at [`examples/README.md`](examples/README.md)

The deeper proof and benchmark commands still live in [`docs/testing.md`](docs/testing.md), but they are maintainers' helper paths rather than the main onboarding route.

## Ops Starter Pack

The current user-facing skill pack lives at [`examples/skills/guild-ops-starter/README.md`](examples/skills/guild-ops-starter/README.md).
The surrounding examples index lives at [`examples/README.md`](examples/README.md).

It is intentionally ordinary example-skill layout, not a new packaging system. The pack installs as five example skills and stays inside current honest repo truth:

- `incident-brief` for one stored execution ref
- `run-diff` for two stored execution refs
- `recent-failures` for one bounded execution-query ref
- `evidence-summary` for one stored evidence ref
- `render-report` as the zero-authority child formatter used by the parent report skills

The pack is meant to show the current Guild story without broadening runtime or proof semantics: durable refs, compact terminal output, explicit capability requirements, and bounded single-child composition only where it is already real.

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
- `guild import ...` or `guild pull ...`
- `guild verify -v <skill-ref>`

Use `guild verify -v <skill-ref>` as the first installed-state verification explanation path after import or pull. That view keeps the trust summary visible and adds signing-scheme and short bundle-digest detail when verification metadata exists.

Current installed-state terms:

- `local-source`: installed from local source in the current Guild root
- `verified-import`: installed from a signed import or pull that verified successfully
- `local-dev`: local source state in the current Guild root
- `trusted-imported`: imported publisher trusted for normal imported use
- `restricted`: imported publisher trusted only under restricted local policy posture

Trust-store maintenance stays local and explicit:

- `guild trust add --identity-file <identity.json>` trusts one local publisher identity directly
- `guild trust add --record-file <record.json>` trusts one reviewed publisher record without secret key material
- `guild trust list` reviews trusted publishers and their current tiers
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
- preview must report real signed installed-state metadata, verification outcome, local trust posture, and bundled closure scope before any state change
- preview must stay read-only: no root creation, staging, installation, trust mutation, or fake detached summary
- `export` and `push` stay out of the first preview slice

Example:

```bash
guild --registry-root target/dev-local-registry/b import bundle target/dev-local-registry/portable-bundle --preview
guild --registry-root target/dev-local-registry/b pull 127.0.0.1:5000/guild-example-hello-inspect:0.1.0 --allow-http --preview
```

Use `guild help preview` for the shipped CLI wording of that contract direction.

For the current operator story around mirroring reviewed artifacts and promoting
them between roots or OCI locations, read
[`docs/mirroring-and-promotion.md`](docs/mirroring-and-promotion.md). That
guide keeps one limit explicit: `guild export ...` and `guild push ...` are
publication steps over installed state, not silent copy or retag primitives.

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

- `docs/how-guild-works.md` - short daily-user mental model for identity, authority, and the main CLI surfaces
- `docs/mcp-agent-recipes.md` - task-shaped MCP recipes for agent users and integrators
- `docs/command-language.md` - public CLI verbs, grouped workflows, and ref grammar
- `docs/mirroring-and-promotion.md` - current operator guidance for mirroring and promoting signed installed-state artifacts
- `docs/testing.md` - verification commands, proof workflows, and smoke paths
- `SPECS.md` - normative contract and conformance language
- `ARCHITECTURE.md` - practical system view and trust boundaries
- `docs/adr/README.md` - decision log and ADR backlog
- `AGENTS.md` - contributor guardrails for contract-first changes
- `docs/roadmap.md` - ordered epics and build priorities

Compatibility wrappers remain at `docs/contracts.md` and `docs/architecture.md` so existing links keep working.
