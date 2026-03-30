# Harness Abstraction

## Definition

Harness is the first-class isolated execution abstraction that Guild admits,
brokers, and receipts.

A harness is not merely “a runtime.” It is the bounded execution environment
that packages:

- isolation boundaries
- the executable payload or entrypoint
- the tool and capability envelope
- runtime configuration that matters to admission or rehydration

## Relation To Existing Concepts

- `skills`: today’s executable unit. Skills may remain one way a harness is
  packaged or invoked, but Harness is the broader product abstraction.
- `tools`: host-mediated capabilities or attached surfaces the harness may use.
- `capabilities`: the policy-gated authority granted to a harnessed session.
- `artifacts`: the packaged content or installed state used to materialize a
  harness.
- `runtimes`: implementation choices used to realize the harness, not the
  product abstraction itself.

## Minimal Proposed Spec Shape

The minimum future harness spec should describe:

- harness identity
- executable payload reference
- isolation profile or runtime class
- declared capability requirements
- persistence expectations
- rehydrate/resume compatibility expectations

This pass does not make that spec normative. It records the minimum shape that
future contract work must address.

## User-Facing Vs Internal

User-facing:

- harness identity
- requested session intent
- declared capabilities
- resulting receipts and evidence
- execution mode outcome: warm, resumed, rehydrated, or cold

Internal:

- exact sandbox primitive
- process/container/VM details
- snapshot format
- mount implementation
- internal runtime pooling strategy
