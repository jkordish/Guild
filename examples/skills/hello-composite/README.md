# hello-composite

This example is the first real composite skill in Guild's inspect-only slice.

It proves:

- composite source manifests declare dependencies by alias
- local install resolves those dependencies into digest-pinned installed records
- the Wasm guest invokes a child through the host-managed dependency boundary
- the host executes the child through the normal registry + runner + Wasmtime path
- the parent receives child `SkillOutput`
- the parent `ExecutionRecord` retains host-owned child execution metadata
- the child execution record and child evidence are durable local resources
- typed `invoke-skill` alias scope and nested capability reduction stay fail-closed

Implementation notes:

- source runtime kind: `wasm-component`
- mode support: `inspect` only
- declared dependency alias: `hello`
- guest source: `skill-rust/`

Run it locally from the repository root:

```bash
cargo run -p guild-mcp --example inspect_composite_local
```

Run the signed portable closure-bundle proof:

```bash
cargo run -p guild-mcp --example export_import_composite_local
```

That command:

1. installs `hello-inspect`
2. installs `hello-composite`
3. resolves the composite by `RequestedSkillRef`
4. executes it through `guild.inspect`
5. reads back the parent execution, child execution, and child evidence resources

`export_import_composite_local` proves composite portability through installed dependency closure:

1. installs `hello-inspect` and `hello-composite` into registry A
2. generates a local publisher identity
3. exports `hello-composite` together with its transitive installed dependency closure as a signed bundle
4. trusts that publisher in fresh registry B
5. imports the verified bundle into registry B
6. resolves `hello-composite` by `RequestedSkillRef`
7. executes the parent and child entirely from imported installed records

The working example uses:

- `invoke-skill` with the declared alias `hello`
- `emit-evidence` so the child can still emit its bounded evidence under reduced grants
