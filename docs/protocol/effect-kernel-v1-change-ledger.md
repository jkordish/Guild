# Effect Kernel v1 Guild Migration Ledger

| Concern | Recovered source | Guild implementation | Wire effect |
| --- | --- | --- | --- |
| Product owner | Jidoka | Guild | None; Jidoka remains v1 protocol provenance. |
| Cargo identity | `jidoka-kernel` / `jidoka_kernel` | `guild-effect-kernel` / `guild_effect_kernel` | None; Cargo names are not protocol fields. |
| Rust toolchain | 1.98.0 | 1.94.0 | None; canonical vectors must prove byte parity. |
| Integration terms | standalone coordinator | future Guild host integration | None; host integration is outside protocol v1 and outside this implementation phase. |

The following values are frozen and are not migration-ledger substitutions:

- event schema version `jidoka.dev/events/v1`;
- all 29 body-kind strings;
- all 26 event-type strings;
- canonical JSON bytes, SHA-256 identities, classification tables, and transition laws.
