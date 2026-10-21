---
version: "0.1.0b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "accepted"
superseded_by: null
attributes:
  domain: "product-mcp"
  doc_type: "verification"
  scope: "MCP-01 product MCP v2 contract review and schema validation"
  language: "Thai-first"
---

# Product MCP contract review

## Outcome

**MCP-01 accepted; v0.1.4b passed independent review.** Independent review
accepted v0.1.2b, after which a protocol audit found the mandatory
`server/discover` method was missing. Review rejected v0.1.3b for incomplete
discovery wire details. The v0.1.4b revision added exact request/response
examples, a concrete `serverInfo`, and valid cache hints. The independent
read-only reviewer accepted v0.1.4b with no blocking findings. MCP-02 source
implementation is now authorized by the execution DAG and has begun.

## Evidence identity

| Field | Value |
|---|---|
| Date | 2026-10-20 |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| HEAD | `1ccf1075120e294c15bae57183adf8af392ed0dd` |
| Scope | Contract and roadmap consistency; machine-readable schema validation; protocol discovery requirement |
| Runtime implementation at review time | None; MCP-02 implementation began after MCP-01 acceptance. Current source/test evidence is recorded in the MCP-02 implementation report. |

## Review disposition

The read-only review of v0.1.0b rejected acceptance for missing Streamable HTTP
wire requirements, unclear local Bearer/OAuth boundaries, incomplete MCP tool
schemas, ambiguous idempotency and error handling, project-scope gaps, and a
roadmap gate conflict. The v0.1.1b and v0.1.2b revisions now record:

- POST framing, both required `Accept` media types, protocol metadata headers,
  header mismatch errors, JSON responses, and HTTP behavior for invalid
  origins, unknown RPC methods, and unsupported HTTP methods;
- a local-only Bearer profile explicitly marked as a deviation from the MCP
  OAuth profile, with loopback-only exposure;
- exact workspace/project grant bindings and caller-grant-only permissions;
- complete MCP `inputSchema`, `outputSchema`, descriptions, and annotations for
  the six MCP-02 tools; v0.1.4b also defines the mandatory protocol-level
  `server/discover` request and response, concrete server identity, and cache
  hints for discovery and grant-private tool lists;
- idempotency key uniqueness, request canonicalization, durable intent, retry
  reconciliation, and unknown-outcome handling;
- standard MCP tool error results with serialized JSON in the error text block,
  omitting `structuredContent` so successful-output schemas remain valid;
- rate limits, provider egress rechecks, retry idempotency, and job output
  fields aligned with FUNG's existing `Job` model;
- rate-limit buckets keyed by authenticated grant ID, with JSON-RPC 429 bodies
  that echo request IDs and notification-specific error responses; and
- separate synthetic core workflow acceptance (MCP-10) from real MCP-client
  interoperability (MCP-18).

The independent reviewer accepted v0.1.2b before the discovery gap was found.
The v0.1.3b review rejected missing wire details. The v0.1.4b review accepted
the discovery request/response, server identity, caching, prior security and
error decisions, DAG gate, and roadmap alignment. No runtime verification is
implied by this contract acceptance.

## Local validation

- PyYAML parsed `contracts/product-mcp-v2.yaml` successfully.
- Python `jsonschema` Draft 2020-12 checked all six MCP-02 input and output
  schemas successfully.
- PowerShell parsed the execution DAG JSON; at the review snapshot MCP-01 was
  `IN_PROGRESS`. MCP-01 is now accepted and the DAG has advanced to MCP-02.
- `git diff --check` found no whitespace errors; Git reported CRLF-to-LF
  normalization warnings for the report index and progress file.

These checks validate documentation syntax and schemas only. No MCP server,
app, package, microphone, or client was run for this review. No MCP token was
issued. No external provider, data egress, destructive operation, merge, or
release was attempted.

## Normative references

- [Streamable HTTP, protocol revision 2026-07-28](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
- [MCP authorization](https://modelcontextprotocol.io/specification/2026-07-28/basic/authorization)
- [MCP server tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- [MCP server discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)
- [MCP result caching](https://modelcontextprotocol.io/specification/2026-07-28/server/utilities/caching)

## Next gate

MCP-01 is accepted. MCP-02 is in implementation review; its acceptance requires
the loopback server, bounded tool dispatch, async job primitives, protocol and
rate-limit verification, durable interruption recovery, and proof that tool
calls preserve existing FUNG and Genesis persistence paths. Packaged-app and
third-party client UAT remain separate acceptance gates.
