# M9 Figure Source Set

These figures are source material for counsel. They are intentionally measured and bounded. They should be read together with [`benchmark_matrix.json`](../schemas/draft-v1/benchmark_matrix.json), [`family_support_matrix.json`](../schemas/draft-v1/family_support_matrix.json), and [m9-non-claims.md](./m9-non-claims.md).

## F1 Architecture Figure Source

```mermaid
flowchart LR
    Caller[Caller] --> Admission[M4 upper-bound admission]
    Admission --> Runtime[Live runtime execution]
    Runtime --> Proof[M5 bounded live proof search]
    Proof -->|proof exists on measured slice| Token[M6 proof-backed token]
    Proof -->|proof exists on measured slice| Witness[M7 proof-linked witness]
    Admission -->|explicit fallback enabled| UpperBound[M6 upper-bound fallback token]
    Admission -->|unsupported or not_proven| Refusal[Refusal]
    UpperBound --> Unlinked[M7 unlinked witness]
    Runtime --> ExecStore[(execution records)]
    Runtime --> EvidenceStore[(evidence and metadata)]
    ExecStore --> Witness
    EvidenceStore --> Witness
    Registry[(installed digest-pinned skills)] --> Runtime
```

## F2 Supported Proof-Backed Path

```mermaid
sequenceDiagram
    participant Caller
    participant Host
    participant Runtime
    participant Proof
    participant Token
    participant Witness
    participant Stores

    Caller->>Host: request + input + requested authority
    Host->>Host: compute upper-bound admission plan
    Host->>Runtime: execute bounded invocation
    Runtime->>Stores: persist execution observations
    Host->>Proof: run bounded live proof on measured slice
    Proof-->>Host: proof_backed result
    Host->>Token: issue proof-backed token
    Host->>Witness: generate proof-linked witness
    Witness->>Stores: persist linked witness material
    Host-->>Caller: output + proof-backed / linked status
```

## F3 Unsupported Or Fail-Closed Path

```mermaid
sequenceDiagram
    participant Caller
    participant Host
    participant Runtime
    participant Proof
    participant Token
    participant Witness

    Caller->>Host: request + input + requested authority
    Host->>Host: compute upper-bound admission plan
    Host->>Runtime: execute or prepare proof search
    Host->>Proof: attempt bounded live proof
    Proof-->>Host: not_proven or unsupported reason code
    alt refusal
        Host-->>Caller: refused
    else explicit fallback enabled
        Host->>Token: issue upper-bound fallback token
        Host->>Witness: generate unlinked witness
        Host-->>Caller: upper-bound / unlinked status
    end
```

## F4 Family Support Frontier

```mermaid
flowchart TD
    A[Measured family frontier]
    A --> RR[read-resource<br/>bounded proof-linked<br/>immutable roots only]
    A --> HTTP[http-request<br/>bounded proof-linked<br/>eight replay-backed slices only]
    A --> INV[invoke-skill<br/>bounded proof-linked<br/>exact single-child and exact two-child same-alias zero-authority only]
    A --> LOG[log-write<br/>exact proof-only<br/>info level only]
    A --> EMIT[emit-evidence<br/>bounded proof-linked<br/>exact single-emission fixed sink only]
    HTTP --> HTTPW[fail-closed walls<br/>redirects unsupported<br/>no replay unsupported]
    INV --> INVW[fail-closed walls<br/>broader multi-child beyond exact checked slice not proven<br/>child authority unsupported]
    RR --> RRW[fail-closed wall<br/>query-root shrink unsupported]
```

## F5 Benchmarking Result Figure Source

```mermaid
flowchart TD
    B[Measured proof search costs]
    B --> RR[read-resource<br/>proof mean 6899.103 ms]
    B --> H1[http-request supported slices<br/>proof mean 7314.193-7635.972 ms]
    B --> INV[invoke-skill supported slices<br/>proof mean 10363.662 ms and 15389.453 ms]
    B --> EMIT[emit-evidence exact supported slice<br/>proof mean 4851.549 ms]
    B --> LOG[log-write proof-only slice<br/>proof mean 8934.525 ms]
    B --> U1[redirect http-request unsupported slice<br/>proof mean 3701.789 ms]
    B --> U3[emit-evidence unsupported slice<br/>proof mean 2975.605 ms]
    B --> W1[fail-closed walls<br/>3742.373 ms, 4200.051 ms, 5977.197 ms]
```

## Figure Use Notes

- F1 is the high-level architecture figure source.
- F2 is the supported proof-backed sequence.
- F3 is the unsupported or fail-closed sequence.
- F4 is the measured family support frontier.
- F5 is the benchmarking and overhead summary.
