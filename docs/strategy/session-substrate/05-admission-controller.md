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

## Boundary By Control Surface

The invoke-time versus wake-time split should be explicit for the main
host-owned control surfaces.

- `artifact trust and executable identity`
  - invoke-time: always checked before first materialization
  - wake-time: rechecked when the wake path depends on rehydration, artifact
    replacement, or a trust-state change
- `secrets`
  - invoke-time: check that the request is allowed to bind the named secret set
  - wake-time: recheck lease freshness, rotation state, and whether reuse is
    still allowed before handing the secret back to a resumed or rehydrated
    materialization
- `mounts`
  - invoke-time: check requested mount classes, scopes, and path policy
  - wake-time: recheck that the mount source still exists, still matches
    policy, and is still safe to reconnect
- `network policy`
  - invoke-time: check the requested egress classes and destination policy
  - wake-time: recheck any policy or credential state that could have changed
    while the session was suspended
- `runtime selection and isolation posture`
  - invoke-time: choose the minimum acceptable runtime class or isolation
    profile for the request
  - wake-time: re-evaluate whether the previous runtime placement is still
    acceptable or whether Guild must elevate isolation, rehydrate elsewhere, or
    cold-start

## Safe Default

If Guild cannot prove that reuse is still safe at wake time, it should
reauthorize, rehydrate, or cold-start. It should not silently inherit stale
secret, mount, network, or placement assumptions from an earlier admission
decision.
