# ADR 0017: HTTP-request policy family

Status: Accepted  
Date: 2026-03-17

## Context

Guild now has one real outbound network family in the active inspect slice:
`http-request`.

That makes it the highest-risk currently implemented family and the clearest
place where "bounded host capability" must stay sharply distinct from "the
guest can use the network."

## Decision

The `http-request` family authorizes bounded outbound HTTP requests only.
It does not grant ambient network access.

The current request model is intentionally narrow:

- absolute URLs only
- `http` and `https` schemes only
- bodyless `GET` and `HEAD` only
- no arbitrary request headers
- no auth injection
- no request-body streaming

The current typed policy dimensions are:

- `allowed_schemes`
- `allowed_hosts`
- `allowed_host_suffixes`
- `allowed_ports`
- `allowed_methods`
- `allowed_path_prefixes`
- `max_timeout_ms`
- `max_response_bytes`
- `follow_redirects`
- `max_redirects`
- `allow_loopback`
- `allow_link_local`
- `allow_private_networks`
- `allow_ip_literals`

Authorization is host-owned and split into parse, grant, and execution-budget
steps:

1. the host parses the URL and rejects malformed requests
2. the host rejects embedded credentials
3. the host canonicalizes exact-host and domain-suffix matching case-insensitively
4. the host checks method, scheme, host, port, path, and timeout against the
   granted family constraints
5. the host classifies raw IP literals and resolved destinations, then blocks
   loopback, link-local, and private-network targets unless they are explicitly
   granted
6. the host checks `Budget.max_network_requests`
7. the host clamps the effective timeout and response-size ceilings to the
   smaller of the family grant and execution budget
8. redirects stay disabled unless the grant explicitly enables
   `follow_redirects`, supplies a bounded `max_redirects`, and every redirected
   hop passes the same host-owned authorization path before dispatch

The current authorization denial taxonomy is explicit:

- `http-request-url-invalid`
- `http-request-not-granted`
- `http-request-budget-exhausted`
- `http-request-method-not-granted`
- `http-request-scheme-not-granted`
- `http-request-host-not-granted`
- `http-request-port-not-granted`
- `http-request-path-not-granted`
- `http-request-timeout-not-granted`
- `http-request-ip-literal-not-granted`
- `http-request-loopback-not-granted`
- `http-request-link-local-not-granted`
- `http-request-private-network-not-granted`
- `http-request-destination-unresolved`
- `http-request-redirect-not-allowed`
- `http-request-redirect-hop-limit-exceeded`
- `http-request-redirect-location-invalid`
- `http-request-redirect-target-not-granted`

Runtime failures after authorization remain distinct from authorization denials.
The current repository persists those as unsuccessful executions without
reclassifying them as policy denials. The current bounded runtime failures
include:

- `http-request-timeout`
- `http-request-response-too-large`
- `http-request-build-failed`
- `http-request-failed`

Trust-tier and profile interaction is indirect but real:

- policy selects the final `http-request` grant set before guest start
- profiles may deny or cap HTTP authority based on skill key, publisher,
  trust tier, verification state, actor, and tenant
- the current `inspect_policy_local` proof flow uses restricted imported trust
  state to reduce redirect authority before guest start, then proves the host
  still denies the redirected execution honestly at runtime

Nested child behavior is subset-only:

- child `http-request` authority is reduced from the parent grant against the
  child requirement on every typed dimension
- child method, scheme, host, domain-suffix, port, path, redirect, timeout,
  response-size, and risky-destination bounds may narrow only
- a child cannot widen a blocked parent path or host

Guild keeps `http-request` in the active inspect slice now because it is already
implemented end to end through the Wasmtime-backed runtime path and has focused
proof coverage. The public MCP surface still stays at `guild.inspect`; HTTP is
exercised through skill execution, not a new MCP tool.

Safe defaults in the current repository are:

- no `http-request` grant means no outbound HTTP authority
- omitted optional host/path/method/path-scope fields on an existing grant are
  only unbounded within this bounded HTTP family
- omitted risky-destination and redirect allow flags do not imply ambient
  network authority; the host still denies redirects, loopback, link-local,
  private-network, and raw IP-literal targets unless the grant explicitly opts
  into them
- execution budget still caps request count, timeout, and effective response
  size

## Consequences

Positive:

- Guild now has one honest, typed outbound network family instead of implied
  network access
- trust-tier-aware policy reductions have a concrete high-risk family to test
  against
- nested HTTP authority remains auditable and least-authority

Costs and limits:

- the current family is intentionally smaller than general HTTP clients
- request bodies, auth, secrets, streaming, and broader network policy remain
  out of scope
- the family is limited to inspect-mode needs in this milestone

## Explicit invariants

- `http-request` is a bounded host capability, not ambient networking
- authorization denials remain host-owned
- runtime transport failures are not silently relabeled as policy denials
- trust tier and verification state influence grants through policy, not through
  guest logic
- child HTTP authority cannot widen beyond the parent grant

## Explicit non-goals / deferred work

- arbitrary sockets or raw network access
- request bodies and streaming
- auth header injection or secret integration
- broader egress policy platforms
- non-local or distributed policy enforcement

## Cross-references

- `README.md`
- `SPECS.md`
- `ARCHITECTURE.md`
- `docs/adr/0005-capability-schema-and-active-inspect-profile.md`
- `docs/adr/0008-local-policy-evaluator.md`
- `docs/adr/0012-capability-policy-layering-model.md`
- `crates/guild-types/src/lib.rs`
- `crates/guild-runner/src/lib.rs`
- `crates/guild-runner/tests/http_requests.rs`
- `crates/guild-mcp/examples/inspect_http_json_local.rs`
- `crates/guild-mcp/examples/inspect_policy_local.rs`
- `examples/skills/inspect-http-json/README.md`
