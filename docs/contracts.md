# Contracts

Guild starts from contracts, not implementation folklore.

The canonical contract now lives at [`../SPECS.md`](../SPECS.md).

This file remains in place as a compatibility path for existing links and habits. If this page ever disagrees with the root specification, the root specification wins.

Current contract highlights worth knowing before you follow older notes:

- durable execution IDs are host-minted; caller IDs are correlation only
- `EvidenceRef` points at a host-issued evidence-record URI, while payload blobs remain digest-addressed
- requested same-version multi-digest resolution now fails closed as ambiguous
- the active Wasm inspect slice only supports `read-resource`, `invoke-skill`, `emit-evidence`, and `log-write`
- `read-resource` scopes are canonical Guild URI roots, not permissive raw string prefixes

Read next:

- [`../SPECS.md`](../SPECS.md)
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md)
- [`spec-delta-guest-abi-host-record-boundary.md`](spec-delta-guest-abi-host-record-boundary.md)
- [`adr/README.md`](adr/README.md)
