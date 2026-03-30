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

## Current Repo Decision

In this phase, Harness remains a docs-first product abstraction.

Guild does not add a normative `Harness` Rust type, manifest field, or WIT
surface yet. The current executable packaging boundary is still the skill
manifest plus resolved installed artifact state. That is the real contract the
repository can prove today, so this pass does not pretend there is already a
stable harness package contract.

Guild should only introduce a typed or manifest-level Harness contract after
all of the following are true:

- one stable package boundary exists across manifest, registry, runner, and
  transport identity
- admission-relevant fields are precise enough to type without placeholder
  blobs
- the mapping from current skill packaging to future harness identity is
  explicit
- the new contract does not widen the current execution or ABI support frontier
  by prose alone

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

Until that future contract exists:

- skills remain the normative executable unit
- resolved installed artifacts remain the concrete transport identity
- harness is the product and architecture term for the broader isolated
  execution abstraction above those current packaging details

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
