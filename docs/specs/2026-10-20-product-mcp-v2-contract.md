---
version: "0.1.4b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "accepted"
superseded_by: null
attributes:
  domain: "product-mcp"
  doc_type: "technical-contract"
  scope: "Desktop product MCP transport, authorization, data identity, and operation semantics"
  language: "Thai-first"
---

# FUNG Product MCP v2 contract

## Status and authority

This is the accepted target contract for MCP-01 in execution
`FUNG-PRODUCT-MCP-2026-10-20-01`. It follows the accepted
[MCP-00 source inventory](../verification/implementation-reports/2026-10-20-product-mcp-inventory.md)
and the independent MCP-01 review recorded in
[the contract review report](../verification/implementation-reports/2026-10-20-product-mcp-contract-review.md).
MCP-02 bridge code now exists with synthetic loopback and source tests; packaged
application and third-party client acceptance remain unverified. This contract
is not runtime evidence by itself.

## 1. Product boundary

The Desktop MCP server exposes FUNG's local meeting workflow to an explicitly
configured local MCP client. It is a separate inbound server surface. The
developer workflow MCP, Mobile gateway, Desktop external-MCP client, and local
HTTP API keep their own contracts and do not inherit MCP v2 registration.

MCP tools call existing application services and the authoritative
GenesisBlockDB adapter. They must not open, migrate, or mutate the backing
SQLite database directly. A tool is considered available only when the app
registers it and the matching runtime acceptance evidence exists.

## 2. Transport decision

| Decision | Contract |
| --- | --- |
| Binding | MCP Streamable HTTP |
| Protocol revision | `2026-07-28`; support another revision only after compatibility tests are recorded |
| Endpoint | `http://127.0.0.1:49175/mcp` by default; configurable loopback port in Settings |
| Protocol state | Stateless requests; no `Mcp-Session-Id` session dependency |
| Enablement | Off until the user enables the local MCP server in FUNG Settings |
| Port collision | Fail closed and show the occupied port; do not silently choose another port |
| Network exposure | Bind IPv4 loopback only; LAN sharing does not apply to MCP |
| Browser access | No CORS grant; accept an absent `Origin`, otherwise require a loopback origin and reject all other origins |
| Rate limits | Per client: 60 requests/minute with burst 20; at most 5 job-control calls/minute; at most 8 concurrent requests globally |
| Request framing | One JSON-RPC request or notification per HTTP POST; request body limit is 1 MiB |
| Request headers | Require `Accept: application/json, text/event-stream`, `MCP-Protocol-Version`, and `Mcp-Method`; require `Mcp-Name` for `tools/call` |
| Response framing | Return one JSON response with `Content-Type: application/json`; accepted notifications return `202` with no body; SSE is not offered in the first implementation |
| Other HTTP methods | `GET` and `DELETE` return `405 Method Not Allowed`; no standalone event stream or session deletion endpoint exists |

### Discovery and list-cache wire contract

`server/discover` is required by protocol revision `2026-07-28`. The request
has no body parameters beyond standard `_meta`; FUNG accepts the protocol
version plus optional client identity and capability metadata. For example:

```json
{
  "jsonrpc": "2.0",
  "id": "discover-1",
  "method": "server/discover",
  "params": {
    "_meta": {
      "io.modelcontextprotocol/protocolVersion": "2026-07-28",
      "io.modelcontextprotocol/clientInfo": {
        "name": "Codex",
        "version": "1.0.0"
      },
      "io.modelcontextprotocol/clientCapabilities": {}
    }
  }
}
```

The complete response includes `resultType: "complete"`, the supported
protocol versions, declared capabilities, server identity in
`_meta["io.modelcontextprotocol/serverInfo"]`, optional usage instructions,
and both cache hints. FUNG reports package version `0.1.1` in this example;
the implementation must emit the version from the running application build.
Discovery is server-wide and contains no grant-specific data, so its cache
scope is `public` with a 300000 ms TTL:

```json
{
  "jsonrpc": "2.0",
  "id": "discover-1",
  "result": {
    "resultType": "complete",
    "supportedVersions": ["2026-07-28"],
    "capabilities": { "tools": {} },
    "_meta": {
      "io.modelcontextprotocol/serverInfo": {
        "name": "FUNG Desktop",
        "version": "0.1.1"
      }
    },
    "instructions": "FUNG local Desktop tools provide scoped access to approved project data and job controls.",
    "ttlMs": 300000,
    "cacheScope": "public"
  }
}
```

Every complete `tools/list` result includes `resultType`, `tools`, `ttlMs`,
and `cacheScope`. It uses `ttlMs: 0` and `cacheScope: "private"` because the
tool set may vary with the authenticated grant. This avoids sharing cached
tool visibility across authorization contexts. Cache hints do not replace
per-call authorization. These fields follow the official [discovery](https://modelcontextprotocol.io/specification/2026-07-28/server/discover)
and [caching](https://modelcontextprotocol.io/specification/2026-07-28/server/utilities/caching)
requirements.

Every request to `/mcp` requires authentication. The server validates the
loopback peer, `Host`, `Origin` when present, HTTP method, content type, MCP
headers, body size, protocol revision, JSON-RPC method and tool name. Each
POST body contains exactly one JSON-RPC request or notification, never a
JSON-RPC response. A request includes `MCP-Protocol-Version` and its value
must exactly match
`params._meta["io.modelcontextprotocol/protocolVersion"]`; a mismatch or a
missing/mismatched required header returns HTTP 400 with JSON-RPC error code
`-32020` (`HeaderMismatch`). An unsupported protocol revision returns HTTP 400
with the protocol-defined unsupported-version error. `Mcp-Method` mirrors the
body's `method`; for `tools/call`, `Mcp-Name` mirrors `params.name`. Header names
are compared case-insensitively and values are compared exactly after decoding
the protocol's Base64 sentinel form where applicable. Invalid `Origin` returns
HTTP 403. A valid request receives a JSON response; an accepted notification
receives HTTP 202 with an empty body. `GET` and `DELETE` return HTTP 405.
Unsupported RPC methods return HTTP 404 with JSON-RPC method-not-found code
`-32601`. The token-bucket limits use the authenticated `grant_id` as the
per-grant bucket key; untrusted `clientInfo` does not determine the key. The
limits are 60 requests per minute, a burst of 20, and at most 5 `job.control`
calls per minute per grant, with 8 requests in flight globally. Every request
above a per-grant or global limit receives HTTP 429, `Retry-After`, and
`Content-Type: application/json`; the JSON-RPC error echoes the parsed request
ID with code `-32000`, message `Rate limit exceeded`, and
`data.retry_after_seconds`. A rate-limited notification receives HTTP 429 and
the same JSON-RPC error without an `id`, as permitted for rejected notification
POSTs. Responses must not disclose secrets or local absolute
paths by default. FUNG does not offer a standalone GET/SSE stream, session IDs,
or resumable streams in this first product MCP implementation.

The selected revision is stateless and materially changed Streamable HTTP
behavior. Codex and other client interoperability is a dedicated MCP-18 gate;
MCP-10 proves the core workflow through synthetic transport and packaged
microphone UAT. Older client revisions must not be assumed compatible. The
protocol's transport requirements and local endpoint protections are documented in the
[official Streamable HTTP specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http).

## 3. Local client authentication and grants

- The HTTP authorization choice is a deliberate local-only interoperability
  deviation from the MCP OAuth authorization profile. MCP authorization is
  optional; this implementation uses an application-issued Bearer token for a
  loopback Desktop client, with no authorization server, OAuth discovery,
  protected-resource metadata, or remote resource URI. It must never be
  exposed through LAN sharing, a reverse proxy, or a public endpoint. A future
  remote transport must adopt the MCP authorization profile before release.
- FUNG issues a separate opaque random Bearer token for each locally approved
  MCP client. It is not the local API token and is never accepted by another
  service.
- Token material is stored in Windows Credential Manager through the existing
  OS-keyring integration. Only a token hash/identifier and grant metadata are
  used in logs. The Settings UI can copy or rotate the client token and revoke
  a grant; revocation takes effect on the next request.
- The user explicitly enables the MCP listener and approves each client grant
  and its scopes. The default grant contains no write, capture, external, delete,
  or administration scope.
- MCP client name/version metadata is untrusted input. Authorization is bound
  to the issued client grant, not that metadata or possession of an object ID.
- A missing, revoked, expired, or insufficient grant fails closed. The server
  does not accept Supabase, external-provider, or unrelated service tokens.
- Every grant is bound to one opaque local `workspace_id` and contains either
  an exact `project_scope` list or the explicit user-selected
  `all_projects: true` flag. Empty `project_scope` with `all_projects: false`
  grants no project data access. `fung.permissions.get` returns only the
  calling grant's scopes and bindings. `fung.health` and
  `fung.capabilities.get` are global metadata tools and return no workspace
  paths or project content.

### Capability scopes

| Scope | Permitted class |
| --- | --- |
| `workspace.read` | Read projects, recordings, jobs, transcript, summaries, artifacts, and citations in the user's local workspace |
| `workspace.write` | Create or update local project metadata and user-authored organization data |
| `capture.control` | Discover sources and start, pause, resume, stop, or recover a recording through the existing capture state machine |
| `job.control` | Cancel or retry supported local jobs; cancellation does not promise rollback |
| `transcript.edit` | Correct transcript text, speaker labels, and other review data with revision checks |
| `artifact.create` | Generate local summaries, tasks, and exports |
| `provider.use` | Invoke an enabled local model or user-configured provider; network egress remains a separate gate |
| `external.disclose` | Send selected local content to an explicitly named external destination after an operation-specific approval |
| `data.delete` | Delete or irreversibly remove a local object after explicit operation-specific approval |
| `admin` | Change global integration, storage, or MCP server configuration |

MCP annotations such as `readOnlyHint` and destructive-operation hints inform
clients but are not authorization. Tool handlers must check the grant and the
underlying FUNG policy on every call. Capture requires `capture.control`, the
app's participant-notice state, an on-screen recording indicator, and an
available stop control. Tool arguments such as `confirmed: true` cannot replace
the app's consent state. External disclosure and deletion require explicit
operation-level approval and are outside the current core execution grant.

`provider.use` authorizes an enabled local provider only. A provider that sends
meeting content off-device additionally requires `external.disclose` and fresh
operation-specific approval in FUNG. `fung.job.control` retry rechecks the
original job's provider and egress policy; prior approval does not authorize a
new remote request. Provider-backed summary and task tools use the same rule.

## 4. Stable identity, revision, and data scope

- Use the exact project, recording, job, segment, speaker, summary, and artifact
  IDs owned by GenesisBlockDB. Treat IDs as opaque case-sensitive strings;
  do not reconstruct them from names, timestamps, or paths.
- A `meeting_id` is not minted as a wrapper until a canonical meeting entity is
  present in the source-of-truth model. Until then, tools identify a meeting by
  its project and recording IDs.
- Every read or mutation is scoped to the grant's allowed workspace. An ID by
  itself is never proof of access.
- Transcript edits identify a segment and its current transcript revision.
  Mutations include `expected_revision`; a stale revision returns
  `revision.conflict` with the current revision and no partial write.
- For other versioned entities, use their authoritative source revision when
  present. Do not invent one for entities that have no revision model; those
  gaps must be closed by the owning phase before exposing conflicting writes.
- List/search tools are paginated, default to 20 results, cap a page at 100,
  and return stable cursor semantics. Absolute paths and raw audio bytes are
  excluded from ordinary list/read responses.

## 5. Side effects, idempotency, and jobs

- Read operations are side-effect free and do not require an idempotency key.
- Every create/start/retry/export operation and every `fung.job.control`
  operation (cancel or retry) requires a caller-generated
  `idempotency_key` (1–128 printable ASCII characters). The durable uniqueness
  key is `(client_grant_id, tool_name, idempotency_key)`; the record also stores
  a canonical request hash and the original result.
- A repeat with the same key and same request returns the original resource or
  job result. Reusing a key with different input returns
  `idempotency.key_reused`; it never creates a second side effect.
- Canonical request hashing uses RFC 8785 JSON Canonicalization Scheme over
  UTF-8, omitting transport-only metadata (`_meta`, headers, request IDs).
  Durable intent is committed before dispatch, and the idempotency key is
  passed to the owning FUNG service so it can deduplicate and reconcile effects.
  A process restart replays a committed result; a pending call is reconciled
  with its owner before dispatch. If the effect cannot be determined, return
  `idempotency.outcome_unknown` and hold for recovery; never blindly retry.
  Retain terminal key/result records 30 days after completion. Keep active and
  recovery-required records until resolved, then retain for 30 days.
- Long work returns a durable `job_id`, current state, and related object IDs
  without holding a tool call open for the full processing duration. Clients
  read progress/results with `fung.job.get` and request supported cancellation
  with `fung.job.control`.
- Job states use the existing stateful job model. Cancellation is best-effort;
  the final response reports whether the job canceled, completed, or could not
  be canceled. It does not imply rollback of already committed work.
- Transport cancellation ends only the current request. It does not cancel a
  durable job unless the caller separately requests the job cancellation tool.

## 6. Errors and audit

Protocol errors use MCP JSON-RPC error semantics. Domain failures use the
standard `tools/call` result envelope with `resultType: "complete"`,
`isError: true`, and a `content` text block containing a serialized JSON error
object. `structuredContent` is omitted for these errors because each tool's
`outputSchema` describes successful results only:

```json
{
  "resultType": "complete",
  "content": [{ "type": "text", "text": "{\"code\":\"revision.conflict\",\"message\":\"The transcript changed since it was read.\",\"retryable\":true,\"resource_id\":\"opaque-id\",\"current_revision\":3}" }],
  "isError": true
}
```

JSON-RPC error responses are reserved for protocol-level failures such as
invalid requests, unsupported methods or protocol/header validation failures.
An unsupported RPC method returns HTTP 404 with JSON-RPC code `-32601`.
The selected tool's `outputSchema` defines the successful structured result
shape. Clients can parse the JSON error object from the tool error text block;
its fields follow the shared domain error schema.

Stable domain codes include `permission.denied`, `grant.revoked`,
`resource.not_found`, `validation.failed`, `revision.conflict`,
`idempotency.key_reused`, `capture.not_ready`, `job.not_cancellable`,
`idempotency.outcome_unknown`, `rate_limited`, `provider.unavailable`,
`feature.not_implemented`, and `storage.unavailable`.
Error messages are safe to show to the user; stack traces and secrets remain in
local diagnostics only.

Each authorized or denied tool call records a local audit event with the grant
ID, tool, requested scopes, operation outcome, object IDs, source revision,
idempotency-key hash, and timestamp. The audit excludes Bearer tokens, raw
transcript text, audio bytes, provider credentials, and absolute file paths.
Audit writes go through the GenesisBlockDB adapter.

Transcript, meeting titles, imported files, summaries, and external tool output
are untrusted data. Text found in them must never be reinterpreted as system
instructions or used to construct shell commands, file paths, or network
destinations.

## 7. Versioning and tool-family rollout

`contracts/local-mcp-v1.yaml` remains a historical declared contract. MCP v2
uses the separate [machine-readable contract](../../contracts/product-mcp-v2.yaml).
Existing v1 names remain aliases only when their input/output semantics are
compatible. A breaking change creates a new versioned tool or contract entry;
it does not silently change a deployed schema.

Initial tool families follow the execution DAG:

1. `fung.health`, `fung.capabilities.get`, `fung.permissions.get`, and durable
   job list/get/control.
2. Capture-source discovery and capture lifecycle.
3. Project, recording, transcript, summary, and artifact reads.
4. Transcript revision edits, search, citations, grounded summaries, tasks,
   and export jobs.
5. Optional local organization and analytics features.
6. External connectors, sharing, calendar actions, deletion, and administration
   stay disabled until their separate scopes and release gates are accepted.

### Codex connection target

Once the runtime exists and the user has enabled it in FUNG Settings, Codex can
register the local Streamable HTTP endpoint with a token environment variable:

```powershell
codex mcp add fung-local --url http://127.0.0.1:49175/mcp --bearer-token-env-var FUNG_MCP_TOKEN
```

The user supplies the token through `FUNG_MCP_TOKEN` in the Codex process
environment. The value itself must not be stored in `config.toml`, committed
files, shell history, or logs. The Codex CLI installed in the current
environment confirms `--url` and `--bearer-token-env-var` for Streamable HTTP;
the packaged FUNG-to-Codex connection still requires MCP-18 interoperability
acceptance.

Tool names and schemas are deterministic, documented in the machine-readable
contract, and listed by user task domain. Hidden tools and undocumented aliases
are not exposed.

## 8. Acceptance for MCP-01

- An independent reviewer accepts the transport, auth, scope, identity,
  revision, idempotency, job, error, and audit decisions.
- The machine-readable v2 contract agrees with this document, defines valid
  MCP `inputSchema`/`outputSchema` and annotations for every MCP-02 tool, and
  points each later tool family to its owner phase and side-effect scope.
- The Codex client configuration path is documented with secret-safe handling;
  no token is written to tracked configuration or logs.
- No runtime, package, microphone, external-provider, release, or merge claim is
  inferred from this contract review.

This revision defines design decisions only. Implementation begins in MCP-02
after MCP-01 review is accepted.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | Froze proposed Desktop Streamable HTTP, per-client auth, scopes, identity, revision, idempotency, jobs, errors, audit, and rollout boundaries for independent review. |
| 0.1.1b | Specified pinned Streamable HTTP wire headers/framing, recorded the local-token OAuth deviation, bound grants to workspace/project scope, defined replay-safe idempotency, and corrected MCP error envelopes and client UAT gate. |
| 0.1.2b | Bound rate limits to authenticated grants, specified JSON-RPC 429 bodies for requests and rejected notifications, and aligned optional-lane skip wording with the execution DAG. |
| 0.1.3b | Added the mandatory modern `server/discover` method with supported protocol version, tool capability, server identity, and bounded discovery caching. |
| 0.1.4b | Specified exact discovery request/response fields and concrete serverInfo, added required `ttlMs`/`cacheScope` hints for discovery and grant-private tool lists, and aligned the contract with protocol caching rules. |
