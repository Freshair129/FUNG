---
version: "0.1.3b"
created_at: "2026-10-20"
last_update: "2026-10-20"
status: "under review"
superseded_by: null
attributes:
  domain: "product-mcp"
  doc_type: "verification"
  scope: "MCP-02 local Product MCP bridge, durable job controls, and source verification"
  language: "Thai-first"
---

# Product MCP implementation — MCP-02

## Outcome

**MCP-02 remains in `REVIEW`.** The initial local candidate passed the source,
frontend, and synthetic workflow checks listed below. The independent review
then found notification dispatch and unbounded job-list reads; both are patched
in the current working tree. Post-fix Rust regression, UI build, and workflow
MCP checks now pass. Packaged-app startup, a real MCP client, microphone
capture, and independent re-review remain open. MCP-03 and later feature
families remain unstarted.

## Evidence identity

| Field | Value |
|---|---|
| Date | 2026-10-20 |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Base HEAD | `1ccf1075120e294c15bae57183adf8af392ed0dd` |
| Candidate state | Uncommitted working tree; no push, PR, merge, or release |
| Contract | MCP-01 `contracts/product-mcp-v2.yaml`, review accepted as v0.1.4b |
| Scope | MCP-02 loopback transport, Rust/Tauri app bridge, settings controls, job operations, and local source tests |

## Implemented source surface

- `src-tauri/src/product_mcp_transport.rs`: IPv4 loopback-only Streamable HTTP,
  bounded request framing/deadlines/concurrency/rate limits, required MCP
  headers, notification response framing, protocol errors, and orderly stop/drain.
- `src-tauri/src/product_mcp.rs`: default-off server lifecycle, explicit local
  grants and project bindings, OS Credential Manager token reference, protocol
  discovery, grant-filtered tools, health/capability/permission reads, bounded
  paginated job list/get/cancel/retry, and Genesis audit/idempotency paths.
- `src-tauri/src/job_engine.rs`: enqueue a retry using the stable job ID
  persisted in its operation intent, making replay safe both before and after
  the job row was committed.
- `src-tauri/src/genesis_adapter.rs`: schema v15 adds nullable retry-intent
  storage after v14 and exposes deterministic bounded relational pages; the
  migration chain remains stepwise and additive.
- `src-tauri/src/lib.rs`, `src/tauri.ts`,
  `src/components/ProductMcpSettings.tsx`, and Settings styles: explicit
  start/stop, ephemeral token display, scope/project grants, and revocation.
- `contracts/product-mcp-v2.yaml` and the feature roadmap/DAG describe only
  the accepted MCP-02 scope; later tool families must define schemas before
  implementation.

The server remains off by default and binds to `127.0.0.1`. The implemented
tool family is limited to `fung.health`, `fung.capabilities.get`,
`fung.permissions.get`, `fung.job.list`, `fung.job.get`, and
`fung.job.control`. It does not start capture or emit audio.

## Pre-remediation candidate verification

The evidence below was collected before the implementation-review fixes in
the working tree. It does not verify the current post-fix source.

| Command/check | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | **647 passed, 0 failed, 6 ignored**, 653 total |
| `cargo test --manifest-path src-tauri/Cargo.toml product_mcp::tests --lib -- --nocapture` | **7 passed** after the final replay-audit change |
| `cargo test --manifest-path src-tauri/Cargo.toml product_mcp_transport::tests --lib -- --nocapture` | **12 passed** |
| Product MCP schema-persistence and v14→v15 migration tests | Passed |
| `cargo check --manifest-path src-tauri/Cargo.toml --lib` | Passed |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Passed |
| `npm run build` | Passed; TypeScript and Vite production build |
| `npm run test:fung-workflow-mcp` | **3 passed** |
| `git diff --check` | No whitespace errors; Git reported CRLF→LF normalization notices for two edited report files |

The first unrestricted full-suite run exposed a schema-chain assertion that
still expected v14; the v14 step was added to the test chain and the migration
test passed. A later full-suite run had one unrelated playback deadline timeout
under high parallel load; the same test passed alone, and the full library suite
then passed with four test workers. The final MCP-specific suite was rerun after
the last MCP source edit.

## Review remediation and remaining gates

The implementation review identified three findings. The dispatcher no longer
rejects a request solely because it is a JSON-RPC notification; successful
notifications now reach the transport's existing `202` empty-body response
path. `fung.job.list` now reads deterministic primary-key pages, scans at most
one Genesis page per request, applies grant filtering while scanning, and binds
opaque continuation cursors to the grant and optional project filter. A scoped
grant may receive fewer than `page_size` items, including an empty page, with a
continuation cursor when the per-call scan budget is reached. The machine
contract status now agrees with the accepted MCP-01 prose contract and DAG.

The earlier retry replay fix and its regression test remain part of the
pre-remediation candidate evidence. After remediation, the complete Rust
library suite passed, including the notification-dispatch and bounded-list
regression cases. Frontend build and developer workflow MCP acceptance also
passed. An independent re-review has now found one remaining P2 pagination
performance issue; MCP-02 remains `REVIEW` until it is resolved or explicitly
accepted.

The installed/native Desktop package and a real MCP SDK client have now been
tested in an isolated profile (details below). The pinned synthetic CPU
acceptance workflow passed on the current source HEAD. A short microphone plus
system-loopback capture completed and wrote WAV chunks. The loopback track
contains the generated test tone; the microphone chunks are effectively silent
because no one spoke during that run, so real microphone signal remains
unverified. Windows restart/recovery UAT, hosted CI, and release/signing remain
unrun. Do not claim native live-capture acceptance or MCP-02 release acceptance
from this report.

## Next gate

Resolve or explicitly accept the independent P2 pagination-performance
finding before MCP-02 leaves `REVIEW`. Only after MCP-02 is accepted may the
DAG advance to MCP-03 capture-source discovery.
Package/client interoperability is reserved for MCP-10/MCP-18 as specified in
the roadmap.

## Post-remediation checks

| Command/check | Result |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=4` | **649 passed, 0 failed, 6 ignored**, 655 total; includes new notification and bounded-pagination regression tests. |
| `cargo check --manifest-path src-tauri/Cargo.toml --lib` | Passed after the notification and bounded-pagination fixes. |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Passed. |
| `npm run build` | Passed; TypeScript and Vite production build. |
| `npm run test:fung-workflow-mcp` | **3 passed, 0 failed.** |
| `git diff --check` | Passed; Git emitted CRLF-to-LF normalization notices for three edited Markdown files. |

## Independent re-review and packaged UAT (2026-10-21)

The review was performed read-only against the current working tree. One P2
finding remains: `fung.job.list` caps the number of rows it processes per
request, but each page is queried using SQL `OFFSET` and the unsigned
base64-encoded cursor accepts a client-supplied `u32` offset. A deep or edited
cursor can therefore make SQLite walk many rows even though the returned page
is bounded. See `src-tauri/src/product_mcp.rs` around the page queries at
lines 757-768 and 897-909 and cursor decoding at lines 1860-1884. The current
Genesis relational adapter exposes offset pagination, so this finding remains
open for MCP-02 review and is not represented as a security bypass.

| Runtime check | Result |
|---|---|
| UAT installer | Built unsigned package `FUNG Product MCP UAT_0.1.1_x64-setup.exe`; SHA-256 `19765B518C41C0C65D530DEC0758B68CAFE8B64F208A187751DCBCC2C7B32F16`. |
| Installed application | Installed and opened `dev.fung.uat.productmcp20261021` from `G:\AI-Studio\verification\fung-product-mcp-uat-20261021\installed`; isolated profile is `C:\Users\suanr\AppData\Roaming\dev.fung.uat.productmcp20261021`. App rendered the Home and Live pages. |
| MCP client | `@modelcontextprotocol/client` 2.3.1 connected to `127.0.0.1:49175`, negotiated `2026-07-28`, listed five read-only tools, and passed `fung.health`, `fung.capabilities.get`, `fung.permissions.get`, and `fung.job.list`. The empty UAT workspace returned zero jobs. `ping` is unsupported by the SDK for this negotiated wire era and was recorded as informational. |
| Grant and server cleanup | Used a temporary `workspace.read` grant in the empty isolated workspace; `job.control` was not granted. The grant was revoked, its Windows Credential Manager entry was removed, the MCP server was stopped, and port 49175 was verified closed. |
| Audio capture | Live showed participant notice checked, ONIKUMA M530 selected, and system loopback enabled. The recording wrote three mic chunks (8.00 s, 8.00 s, 6.29 s) and one system chunk (3.04 s) under the isolated G: UAT output folder. Mic chunks were 48 kHz mono 16-bit but near-silent (peak 3, 0, 0; RMS 0.01, 0, 0), so this run does not prove speech reached the mic input. The system chunk contained the 440 Hz test tone (peak 142, RMS 99.39, 97.87% nonzero samples), confirming loopback capture and WAV output. FUNG returned to “หยุดบันทึกแล้ว” after finalization. |
| Synthetic CPU workflow | PASS on source HEAD `1ccf1075120e294c15bae57183adf8af392ed0dd`: loopback import, CPU transcription, transcript edit, SRT/VTT/WAV export, playback readback, and Genesis database close/reopen all passed. Audio/export/readback bytes matched the fixture, edited transcript persisted after reopen, and the job completed. Run `workflow-2026-10-21T045918729Z-efe6aef9`; receipt and evidence are under `G:\AI-Studio\verification\fung-workflow-mcp\workflow-2026-10-21T045918729Z-efe6aef9`. Keyword score is diagnostic only. |

These results establish native-app startup, read-only MCP client
interoperability, system-loopback WAV capture, and the synthetic CPU workflow
through output readback and database reopen. They do not yet establish a live
microphone signal from ONIKUMA or native live-capture transcript output. The
mic input needs one short spoken-signal run to distinguish an idle/quiet room
from a device or routing failure.

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.5b | Added current-HEAD synthetic CPU workflow PASS; retained the independent P2 finding and explicit native mic-signal limitation. |
| 0.1.4b | Recorded read-only independent re-review finding, installed-app/real MCP SDK UAT, and short loopback capture; mic stream opened but produced silence without speech. |
| 0.1.3b | Recorded post-remediation full Rust, frontend build, and workflow MCP acceptance; independent implementation review and packaged/client gates remain open. |
| 0.1.2b | Recorded post-remediation Rust compile, format, and whitespace checks; test suites and independent review remain open. |
| 0.1.1b | Recorded implementation-review remediations and marked prior test results as pre-remediation; post-fix verification remains open. |
| 0.1.0b | Initial MCP-02 source and local verification report. |
