# Architecture

Guild treats architecture as product surface, not implementation trivia.

The canonical system view now lives at [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

This file remains in place as a compatibility path for older links. If this page ever disagrees with the root architecture document, the root document wins.

Current architecture highlights worth knowing before you follow older notes:

- execution IDs and evidence-record IDs are host-minted
- evidence blob storage is separate from per-emission evidence records
- source installs stage and move atomically instead of pre-deleting installed state
- unsupported capability families are rejected before execution in the active inspect slice
- bounded `http-request` execution is now part of that active inspect slice through the same Wasmtime runtime path
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
