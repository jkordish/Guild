# Trust Proof Walkthrough

This walkthrough shows the current operator trust chain on a real Guild path
using only shipped repo surfaces. It is not a runtime-contract source; use
`SPECS.md` for normative rules, `ARCHITECTURE.md` for subsystem detail, and
`docs/testing.md` for the broader proof suite.

## What This Proves Today

- admission is visible before execution through execution identity and grant
  review
- bounded execution stays host-owned and capability-scoped
- receipts persist as durable `guild://executions/...` refs
- evidence persists as durable payload and metadata records
- replay-oriented explanation today means explaining or re-checking a run from
  stored refs with `guild why`, `guild get`, and the explain/report paths; it
  is not a first-class `guild replay` command

## 1. Review Identity And Authority Before The Run

Use today's admission-facing review surfaces first:

```bash
guild init
guild install examples/skills/hello-inspect
guild show -v skill://example/hello-inspect@^0.1
guild grants template emit-evidence
```

What to look for:

- `guild show -v` tells you which installed executable identity Guild will use.
- Declared authority stays separate from the caller-requested grants you are
  about to pass into the run.
- `guild grants template emit-evidence` gives you the concrete bounded JSON you
  will narrow and pass to `guild run`.

## 2. Run One Bounded Workflow

Run the smallest real receipt-and-evidence flow in the repo:

```bash
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
```

What to look for:

- The JSON wrapper includes a `where` execution URI for the stored run.
- The granted authority for this run is explicit rather than ambient.
- This is today's bounded execution surface; the host still owns the runtime
  boundary and final grant decision.

## 3. Inspect The Receipt

Use the stored execution URI from the `where` field to review what happened:

```bash
guild why exec:<execution-id-prefix-from-where>
guild get guild://executions/<execution-id-from-where>
```

What to look for:

- `guild why` is the first operator-facing explanation surface for the stored
  run.
- `guild get` is the raw durable read path for the same execution record.
- Together they show that Guild persists a host-owned receipt instead of
  leaving the result in chat-only memory.

## 4. Inspect Evidence Metadata And Payload

Use the nearby evidence ref from `guild why`, or discover one explicitly:

```bash
guild ls evidence --limit 5
guild get guild://objects/records/<evidence-record-id>/metadata
guild get guild://objects/records/<evidence-record-id>
```

What to look for:

- Evidence metadata stays separate from the evidence payload.
- The metadata resource keeps the evidence tied back to the execution that
  produced it.
- This is the current audit trail: durable payload plus durable host-owned
  metadata, not a broader compliance guarantee by wording alone.

## 5. Use Replay-Oriented Explanation On Stored Refs

Stay on the same stored execution path when you need a richer explanation:

```bash
guild why --lineage exec:<execution-id-prefix-from-where>
```

What to look for:

- Guild can keep the explanation grounded in stored refs instead of retelling
  the run from memory.
- The current surface is replay-oriented explanation over durable state, not a
  first-class replay engine.

## Checked Deeper Proof Paths

When you want the deterministic repo-local proof helpers that keep this story
honest, bootstrap the local proof registry first, then run:

```bash
cargo run -q -p guild-mcp --bin guild -- codex bootstrap --registry-root target/dev-local-registry/codex-local --reset
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution
cargo run -q -p guild-mcp --bin guild -- codex smoke --registry-root target/dev-local-registry/codex-local --flow explain-execution-tree
```

The bootstrap step creates the deterministic local registry root and installs
the scenario fixtures those smoke commands depend on. The smoke flows then
exercise the reusable explain surfaces over deterministic local data. Use them
to confirm that the trust-story walkthrough above still matches the current
live proof path.
