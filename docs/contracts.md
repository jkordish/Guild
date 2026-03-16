# Contracts

Guild starts from contracts, not implementation folklore.

The canonical contract now lives at [`../SPECS.md`](../SPECS.md).

This file remains in place as a compatibility path for existing links and habits. If this page ever disagrees with the root specification, the root specification wins.

Current contract highlights worth knowing before you follow older notes:

- durable execution IDs are host-minted; caller IDs are correlation only
- `EvidenceRef` points at a host-issued evidence-record URI, while payload blobs remain digest-addressed
- requested same-version multi-digest resolution now fails closed as ambiguous
- the active Wasm inspect slice only supports `http-request`, `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- `read-resource` scopes are canonical Guild URI roots, not permissive raw string prefixes
- `http-request` is now a real bounded host capability behind `guild.inspect`, not a separate MCP tool
- caller-requested capabilities are now local policy input; the host decides the final granted set before execution
- `guild-mcp-server` now exposes one honest stdio MCP tool (`guild.inspect`) plus Guild URI resources and templates
- installed-state portability now has two local transport shapes: the native signed bundle directory and an OCI image layout mapping over the same signed payload

Read next:

- [`../SPECS.md`](../SPECS.md)
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`spec-delta-guest-abi-host-record-boundary.md`](spec-delta-guest-abi-host-record-boundary.md)
- [`adr/README.md`](adr/README.md)
