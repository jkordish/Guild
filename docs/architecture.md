# Architecture

Guild treats architecture as product surface, not implementation trivia.

The canonical system view now lives at [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

This file remains in place as a compatibility path for older links. If this page ever disagrees with the root architecture document, the root document wins.

Current architecture highlights worth knowing before you follow older notes:

- execution IDs and evidence-record IDs are host-minted
- evidence blob storage is separate from per-emission evidence records
- evidence payload reads and evidence metadata reads now use distinct companion URIs under the same object-record scope root
- source installs stage and move atomically instead of pre-deleting installed state
- inspect-mode Wasm guests now instantiate against `guild-skill-inspect-v1`, so unsupported capability imports are absent from the active inspect ABI, host-side unsupported families are still rejected before execution if they appear in grants/manifests, and broader Guild component imports are rejected as host-owned `unsupported-runtime-surface` before instantiation
- the shared host-side capability surface now includes a typed deferred `filesystem` family, and the active inspect slice still rejects it before guest start rather than pretending runtime file access exists
- bounded `http-request` execution is now part of that active inspect slice through the same Wasmtime runtime path, including host/domain/path enforcement, explicit redirect policy, and fail-closed loopback/private-network blocking unless policy grants those destinations
- the runner now uses one explicit host-to-guest inspect projection boundary rather than incidental field dropping when mapping durable host grants into the active guest ABI
- that projection keeps guest `ExecutionContext` as a bounded subset, projects the current five active family grant shapes fully, and leaves policy/provenance/evidence-record truth in durable host records
- caller-requested capabilities now flow through a local host-owned policy evaluator before execution starts
- that evaluator now derives a host-owned local trust tier and selects named profiles by actor and/or tenant before deciding grants
- read-resource auth uses canonical parsed Guild URI scopes
- `guild-mcp-server` is now a real stdio MCP façade over the same inspect/runtime/resource path
- installed-state portability now includes the native signed bundle directory, a local OCI image layout mapping, and OCI registry transport, all of which still import through the same trust/signature checks
- bounded execution-query resources now derive from the same persisted execution backend seen by guest `read-resource` and MCP `resources/read`

Read next:

- [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`../SPECS.md`](../SPECS.md)
- [`adr/README.md`](adr/README.md)
