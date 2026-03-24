# How Guild Works

This page is the short daily-user model for Guild.

It is not the normative contract source. Use `SPECS.md` when you need the exact platform contract, and use `ARCHITECTURE.md` when you need the fuller implementation view.

## The Short Version

Guild lets you ask for a skill by a human-meaningful ref, resolve that request to installed executable state, run it with host-owned authority decisions, and read back durable execution and evidence records afterward.

The important part is that Guild keeps those boundaries explicit instead of flattening them into "the tool ran somehow."

## Identity Layers

Guild uses three identity layers in normal operator workflows:

- source skill: the local source directory you pass to `guild install`
- installed executable state: the installed record stored under the selected Guild root
- resolved executable identity: the exact installed executable selected for use, identified by resolved ref plus artifact digest

One quick trace path is:

```bash
guild show -v skill://example/hello-inspect@^0.1
```

That one command lets you line up what you asked for, what Guild installed, and what exact executable identity Guild selected.

When you need the current "why did this ref resolve that way?" surface, use:

```bash
guild show -vv skill://example/hello-inspect@^0.1
```

That adds the installed-version and selected-digest reasoning on top of the identity trace.

## Authority Lifecycle

Guild also keeps authority staged instead of ambient:

- declared authority: capabilities declared by the installed manifest and visible in `guild show`
- requested authority: caller-requested grants passed to `guild run`
- granted authority: the final capability slice the host policy allows for that run
- effective at runtime: the authority the guest can actually exercise during execution

Guild does not hand the guest ambient authority. The host may reduce or deny caller-requested authority before guest start, and the runtime only exposes the final granted set.

Use `guild grants template <family>` when you want a read-only concrete JSON starting point for the active families before narrowing that request and passing it to `guild run`.

## Command Roles

The main daily commands each answer a different question:

- `guild ls`: list installed skills and other objects in the current root
- `guild show`: what is installed or what stored object am I looking at?
- `guild grants template`: print read-only grant templates for the active families
- `guild run`: execute one installed skill locally
- `guild why`: explain one persisted execution record, point to nearby stored refs when present, summarize requested-versus-granted authority, and summarize stored authority observations
- `guild get`: read one Guild resource directly
- `guild verify`: show installed trust and verification state for a skill

The `guild grants` command group, `guild ls`, `guild show`, `guild why`, `guild get`, and `guild verify` are read-only surfaces.
`guild run` is the execution surface.

## Output Modes

Default human output is for reading, not parsing.
Short human summaries may include low-noise follow-up hints such as `Next: ...`
when the follow-up is obvious.
`guild why` may also include one nearby short execution or evidence ref when a
stored execution already points at related work. It also reports a compact
requested-versus-granted summary for that run. Use `guild why -v` when you
want the expanded nearby-ref lists, the requested-versus-granted authority
diff, and family-aware request hints for that execution. Use `guild why
--lineage` when you want the native bounded ancestor and descendant view over
persisted executions.

When you need a stable machine surface:

- use `--json` for structured machine-readable output
- use `--porcelain` for stable one-line machine-readable output

When a command supports `--json`, failure output stays on that machine surface:
stdout carries a JSON `error` envelope, stderr stays empty, and the process
still exits nonzero.

## A Normal Daily Flow

```bash
guild init
guild install examples/skills/hello-inspect
guild show -v skill://example/hello-inspect@^0.1
guild grants template emit-evidence
guild run \
  skill://example/hello-inspect@^0.1 \
  --input-json '{"name":"Ada"}' \
  --grants-json '{"grants":[{"id":"emit-evidence","access":"write","constraints":{"max_bytes":65536,"audiences":["user"],"redactions":["none"]}}]}' \
  --json
guild why exec:<execution-id-prefix>
guild get guild://executions/<execution-id>
guild verify skill://example/hello-inspect@^0.1
```

What that flow tells you:

- the source directory becomes installed executable state through `guild install`
- `guild show -v` explains the identity path before you run anything
- `guild show -vv` explains why the requested ref resolved to the selected digest
- `guild grants template` is the read-only starting point when you need current active-family grant JSON before a run
- `guild run` executes with caller-requested grants filtered through host policy
- `guild why` is the first nearby-ref, requested-versus-granted authority, and authority-observation surface after the run completes; `guild why -v` expands that stored diff and any family-aware request hints, `guild why --lineage` adds the native bounded ancestor/descendant view, `guild ls evidence --limit 5` discovers recent evidence refs, and `guild get` is the raw durable read path
- `guild verify` is about installed trust state, not execution replay

## Trust Review

Use the current trust review loop in this order:

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

## Planned Help Topics

Two future-facing directions are fixed now, even though the commands or flags
themselves are not implemented yet:

- `guild help doctor` defines the first read-only diagnostic command direction
- `guild help preview` defines the first read-only preflight direction for risky
  `import` and `pull` flows

## Where To Go Next

- Use `guild help refs`, `guild help trust`, `guild help roots`,
  `guild help doctor`, `guild help preview`, and `guild help grants` when you want the shipped CLI
  wording first.
- Read [`docs/command-language.md`](command-language.md) for the full public CLI surface.
- Read [`docs/mirroring-and-promotion.md`](mirroring-and-promotion.md) when you are moving reviewed installed state between roots or OCI locations.
- Read [`docs/testing.md`](testing.md) for proof commands and smoke flows.
- Read `SPECS.md` if you need exact contract language.
