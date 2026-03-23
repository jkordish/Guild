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

In practice, that means the caller can ask for authority, but the guest only receives the final granted set.

## Command Roles

The main daily commands each answer a different question:

- `guild ls`: list installed skills and other objects in the current root
- `guild show`: what is installed or what stored object am I looking at?
- `guild run`: execute one installed skill locally
- `guild why`: explain one persisted execution record
- `guild get`: read one Guild resource directly
- `guild verify`: show installed trust and verification state for a skill

`guild ls`, `guild show`, `guild why`, `guild get`, and `guild verify` are read-only inspection surfaces.
`guild run` is the execution surface.

## Output Modes

Default human output is for reading, not parsing.
Short human summaries may include low-noise follow-up hints such as `Next: ...`
when the follow-up is obvious.

When you need a stable machine surface:

- use `--json` for structured machine-readable output
- use `--porcelain` for stable one-line machine-readable output

## A Normal Daily Flow

```bash
guild init
guild install examples/skills/hello-inspect
guild show -v skill://example/hello-inspect@^0.1
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
- `guild run` executes with caller-requested grants filtered through host policy
- `guild why` and `guild get` explain what happened after the run completes
- `guild verify` is about installed trust state, not execution replay

## Planned Help Topics

Two future-facing directions are fixed now, even though the commands or flags
themselves are not implemented yet:

- `guild help doctor` defines the first read-only diagnostic command direction
- `guild help preview` defines the first read-only preflight direction for risky
  `import` and `pull` flows

## Where To Go Next

- Use `guild help refs`, `guild help trust`, `guild help roots`,
  `guild help doctor`, and `guild help preview` when you want the shipped CLI
  wording first.
- Read [`docs/command-language.md`](command-language.md) for the full public CLI surface.
- Read [`docs/testing.md`](testing.md) for proof commands and smoke flows.
- Read `SPECS.md` if you need exact contract language.
