# Admission Controller

## Admission Inputs

The future session-aware admission controller should evaluate:

- requested session or harness target
- requested action or invocation intent
- declared and requested capabilities
- identity and trust state of the executable artifacts
- secrets, mounts, and network requirements
- runtime or isolation requirements
- current session state when waking an existing session

## Admission Outputs

- allow
- deny
- ask-human
- elevate-isolation
- a narrowed or recomputed capability envelope
- a receipt-ready explanation of the decision

## Policy Decisions

- `allow`: requested action may proceed under the computed envelope
- `deny`: request is rejected before execution or wake
- `ask-human`: explicit escalation is required before proceeding
- `elevate-isolation`: the request may proceed only under a stricter isolation
  posture or different runtime class

## Relation To Key Controls

- `capabilities`: define the allowed authority envelope
- `secrets`: may require fresh authorization at invoke or wake time
- `mounts`: host-owned and policy-gated, never ambient
- `network policy`: explicit host-owned outbound policy, not guest choice
- `runtime selection`: may affect isolation guarantees and admission outcome

## Invoke-Time Vs Wake-Time Checks

Invoke-time:

- artifact trust and identity
- requested intent
- baseline capability requirements
- initial isolation posture

Wake-time:

- whether the current session materialization is still valid
- whether secrets, mounts, or network policy must be reauthorized
- whether the previous isolation posture is still acceptable
- whether a warm resume is allowed or rehydration/cold-start is required
