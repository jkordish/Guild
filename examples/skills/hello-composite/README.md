# hello-composite

This example is the first real composite skill in Guild's inspect-only slice.

It is a real composition and portability example, but it is not the bounded M8c
`invoke-skill` live-proof fixture. Its child, `hello-inspect`, uses
`emit-evidence`, so this path stays outside the current proof-backed
`invoke-skill` envelope on purpose.

It proves:

- composite source manifests declare dependencies by alias
- local install resolves those dependencies into digest-pinned installed records
- requested same-version multi-digest resolution fails closed instead of silently picking a child artifact
- the Wasm guest invokes a child through the host-managed dependency boundary
- the host executes the child through the normal registry + runner + Wasmtime path
- the parent receives child `SkillOutput`
- the parent `ExecutionRecord` retains host-owned child execution metadata
- parent and child durable execution IDs are both host-minted
- the child execution record and child evidence are durable local resources
- typed `invoke-skill` alias scope and nested capability reduction stay fail-closed

Implementation notes:

- source runtime kind: `wasm-component`
- mode support: `inspect` only
- declared dependency alias: `hello`
- guest source: `skill-rust/`

Run it locally from the repository root:

```bash
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/hello-composite install examples/skills/hello-inspect
cargo run -q -p guild-mcp --bin guild -- --registry-root target/dev-local-registry/hello-composite install examples/skills/hello-composite
cargo run -p guild-mcp --example inspect_composite_local
```

Run the signed portable closure-bundle proof:

```bash
cargo run -p guild-mcp --example export_import_composite_local
cargo run -p guild-mcp --example export_import_composite_oci_local
```

That command:

1. installs `hello-inspect`
2. installs `hello-composite`
3. resolves the public `skill://example/hello-composite@^0.1` ref to installed executable state
4. executes it through `guild.inspect`
5. reads back the parent execution, child execution, and child evidence resources

Both execution records carry host-stamped timestamps, and the child lineage is preserved through host-owned durable metadata rather than guest-authored IDs.

`export_import_composite_local` proves composite portability through installed dependency closure:

1. installs `hello-inspect` and `hello-composite` into registry A
2. generates a local publisher identity
3. exports `hello-composite` together with its transitive installed dependency closure as a signed bundle
4. trusts that publisher in fresh registry B
5. imports the verified bundle into registry B
6. resolves `skill://example/hello-composite@^0.1`
7. executes the parent and child entirely from imported installed records

`export_import_composite_oci_local` proves the same dependency-closure portability contract through an OCI image layout:

1. installs `hello-inspect` and `hello-composite` into registry A
2. generates a local publisher identity
3. exports `hello-composite` together with its transitive installed dependency closure as an OCI image layout
4. trusts that publisher in fresh registry B
5. imports the verified OCI layout into registry B
6. resolves `skill://example/hello-composite@^0.1`
7. executes the parent and child entirely from imported installed records

`push_pull_composite_oci_registry_local` proves the same dependency-closure portability contract through a real local OCI registry:

1. installs `hello-inspect` and `hello-composite` into registry A
2. generates a local publisher identity
3. publishes `hello-composite` together with its transitive installed dependency closure to a local OCI registry
4. trusts that publisher in fresh registry B
5. pulls the verified artifact into registry B through the normal local trust/signature gate
6. resolves `skill://example/hello-composite@^0.1`
7. executes the parent and child entirely from pulled installed records

The working example uses:

- `invoke-skill` with the declared alias `hello`
- `emit-evidence` so the child can still emit its bounded evidence under reduced grants

The active Wasm inspect slice does not expose broader capability families such as cache, secret, or network-style surfaces. Unsupported families are rejected before the composite starts running.
