# Mirroring And Promotion

This page records the current operator workflow for mirroring signed installed
state and promoting reviewed artifacts between Guild roots or OCI locations.

It is not a new command surface. Guild does not yet ship `guild mirror` or
`guild promote`; the current workflow is built from `guild export`, `guild
import`, `guild push`, `guild pull`, `guild trust add`, `guild verify -v`, and
the shipped `--preview` preflight for import and pull.

## Current Rules

Keep these rules fixed when you mirror or promote installed state today:

- start from installed executable state, not from source directories
- treat each target Guild root as its own local trust decision point
- use `guild import ... --preview` or `guild pull ... --preview` before
  mutating a different root
- finish with `guild verify -v <skill-ref>` after import or pull
- keep the reviewed transport artifact stable when you want the same signed
  payload to move forward unchanged

One contract point matters for planning: `guild export ...` and `guild push ...`
are publication steps over installed state. They require a signer and produce a
fresh signed transport artifact. Treat them as new publication events, not as
silent copy or retag primitives.

Guild also does not yet ship:

- registry-to-registry copy
- retag-only promotion
- remote trust-store sync
- automatic environment promotion workflows

## Current Install Review Surface

Today the install surface for reviewed transported state is still the existing
`preview` plus `verify -v` loop, not a separate package browser or pack
contract.

Keep that boundary explicit:

- `guild import ... --preview`, `guild import oci-layout ... --preview`, and
  `guild pull ... --preview` are the current read-only admission review steps
- preview should be the first place an operator sees the transport shape they
  are reviewing: bundle path, OCI-layout path, or OCI registry reference
- preview should keep publisher identity, verification result, trust tier,
  bundle digest context, and bundled closure scope visible before any
  installation happens
- `guild verify -v <skill-ref>` remains the first installed-state explanation
  path after import or pull

If Guild later gains a more curated install view, it should stay a presentation
layer over those existing surfaces and their host-owned truth:

- resolved skill identity and the concrete installed-state digest context
- transport shape and reviewed source reference
- publisher identity, signature status, verification result, and local trust
  tier
- closure scope and resulting installed-state classification
- manifest/runtime compatibility facts already derived from the shipped
  contracts and runtime checks

That later presentation must not become a new pack type, a second metadata
contract, or a bypass around target-root trust review. It also must not drift
into marketplace or hosted-control-plane language while the current repo still
ships a local-first trust and transport model.

## Bundle Mirror Between Roots

Use the native signed bundle directory when you want one explicit local artifact
that can be copied through shared storage, artifact storage, or removable media.

```bash
guild --registry-root /srv/guild/dev export bundle \
  skill://example/hello-inspect@^0.1 \
  --signer /srv/guild/publishers/local.example.json \
  --output /srv/guild/mirror/hello-bundle

guild --registry-root /srv/guild/stage import bundle \
  /srv/guild/mirror/hello-bundle \
  --preview

guild --registry-root /srv/guild/stage trust add \
  --identity-file /srv/guild/publishers/local.example.json

guild --registry-root /srv/guild/stage import bundle \
  /srv/guild/mirror/hello-bundle \
  --preview

guild --registry-root /srv/guild/stage import bundle \
  /srv/guild/mirror/hello-bundle

guild --registry-root /srv/guild/stage verify -v \
  skill://example/hello-inspect@^0.1
```

What this means operationally:

- the bundle directory is the transport unit you copy around
- the first preview can honestly refuse on trust without mutating the target
  root
- `guild trust add ...` is still a local operator decision in the target root
- the final `guild verify -v` confirms the installed verification state after
  import

## OCI Layout Mirror Without A Registry

Use `guild export oci-layout` when you want the same signed installed-state
payload in an OCI-shaped directory without introducing a live registry into the
workflow.

```bash
guild --registry-root /srv/guild/dev export oci-layout \
  skill://example/hello-inspect@^0.1 \
  --signer /srv/guild/publishers/local.example.json \
  --output /srv/guild/mirror/hello-layout

guild --registry-root /srv/guild/stage import oci-layout \
  /srv/guild/mirror/hello-layout \
  --preview

guild --registry-root /srv/guild/stage import oci-layout \
  /srv/guild/mirror/hello-layout
```

This is still the same trust model. The OCI layout is another transport shape
for the signed installed bundle, not a bypass around local publisher review or
bundle verification. If preview refuses on trust, review the target root, add
the publisher intentionally, and rerun preview before import.

## OCI Publication And Promotion

Use `guild push` and `guild pull` when the promotion boundary is an OCI registry
reference.

```bash
guild --registry-root /srv/guild/dev push \
  skill://example/hello-inspect@^0.1 \
  --reference registry.example.com/guild/hello-inspect:0.1.0 \
  --signer /srv/guild/publishers/local.example.json

guild --registry-root /srv/guild/prod pull \
  registry.example.com/guild/hello-inspect:0.1.0 \
  --preview

guild --registry-root /srv/guild/prod trust add \
  --identity-file /srv/guild/publishers/local.example.json

guild --registry-root /srv/guild/prod pull \
  registry.example.com/guild/hello-inspect:0.1.0 \
  --preview

guild --registry-root /srv/guild/prod pull \
  registry.example.com/guild/hello-inspect:0.1.0

guild --registry-root /srv/guild/prod verify -v \
  skill://example/hello-inspect@^0.1
```

Use that workflow when the registry reference itself is the promotion artifact.

Two current limits are important:

- Guild does not yet ship a registry-to-registry mirror command, so exact
  cross-registry copying is still an external operator workflow
- pushing again from another root is a fresh signed publication step, not a
  no-touch promotion of the original registry artifact

If you need the same signed payload to survive unchanged across environments
today, keep the reviewed bundle directory, OCI layout directory, or stable OCI
reference as the artifact of record and move that artifact forward directly.

## Proof And References

For repo-local proof flows of the current transport story, use:

- `cargo run -p guild-mcp --example export_import_local`
- `cargo run -p guild-mcp --example export_import_oci_local`
- `cargo run -p guild-mcp --example push_pull_oci_registry_local`

Read next:

- [`README.md`](../README.md) for the shorter trust-and-transport operator story
- [`command-language.md`](command-language.md) for the public CLI wording
- [`testing.md`](testing.md) for the full proof commands and smoke flows
