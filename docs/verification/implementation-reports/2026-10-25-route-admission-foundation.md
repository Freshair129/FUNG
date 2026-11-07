---
version: "0.1.0b"
created_at: "2026-10-25"
last_update: "2026-10-25"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Read-only native route inventory and admission source integration"
  language: "Thai-first"
---

# Read-only route admission — Task 1

## Disposition

**Source/read-only native integration verified; multi-source capture and installed
acceptance remain open.** This follows the
[bounded endpoint feasibility](2026-10-25-dedicated-endpoint-feasibility.md),
[design](../../specs/2026-10-25-dedicated-endpoint-routing-design.md) and
[implementation plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md).

User decision: keep the existing PID mode and do not release until both PID and
endpoint modes pass. Original #96 remains a release blocker. No new installed
package, tag or release is claimed here.

## Source changes

Base `a883973` on the existing `fix/issue-96-app-loopback-transition` worktree.
`capture_routing.rs` provides typed native endpoint/process/session observations,
pure admission policy and fresh native observation on a dedicated MTA worker.
No audio stream is initialized or started and no OS routing/settings are mutated.

Two Tauri commands are registered in `lib.rs`:

- `capture_route_inventory(processIds?)`: read-only endpoint/session and requested
  process identity observation; at most 128 requested process identities.
- `capture_route_validate(plan)`: takes route intent only and calls fresh native
  `admit_route_plan`. It does not accept a supplied inventory or deserialize a
  previously validated token. Returned readiness may be WaitingForTarget.

Both commands are async and use `spawn_blocking`, keeping the internal synchronous
MTA/join and native filesystem/session inspection off the UI thread. The public
Rust module is a read-only developer foundation, not a capture executor or Product
MCP tool registration. No source-selection UI is added by these commands.

Windows crate features add read-only process/file/snapshot metadata APIs. Existing
capture, device preferences and normal-profile schema are unchanged.

## Policy and reviewed corrections

- One or two routes, distinct opaque endpoint IDs and nonnil source UUIDs; default
  role endpoints are rejected. Missing/inactive/ambiguous devices and selected
  observation errors fail explicitly without name/default substitution.
- Process identity includes native creation FILETIME, image path and current
  filesystem volume/file ID. 64-bit identifiers are strings for JSON precision.
  File identity is not a loaded-image/signature or an OS security guarantee.
- Explicit child identities require a current ancestry chain and creation-time
  consistency. A precise FILETIME cutoff before ToolHelp prevents a later-born
  opened process from inheriting an older parent-map observation.
- Review calibrated that mixed-generation inventory issue as consistency hardening;
  fixed expected identities plus fresh admission already reject the demonstrated
  PID replacement. No admission-bypass vulnerability closure is claimed.
- Native raw GetProcessId HRESULT preserves non-S_OK success ambiguity and PID-zero
  system/cross-process cases; active ambiguous/foreign owners fail closed.
- App identity authorizes its streams. Ephemeral session instance IDs are native
  observed provenance, not client authorization. Recreating a stream in the same
  exact process remains valid; changing PID/generation or adding undeclared children
  does not silently expand authorization. Active provenance is bounded to 64 sessions.
- No observed target stream yields WaitingForTarget, not Armed. Snapshot freshness
  is at most five seconds; actual capture must revalidate and monitor at execution.

These checks do not atomically freeze OS state or prevent an unobserved short
foreign stream. Continuous source monitoring/quarantine belongs to Task 2, not
this source-only foundation.

## Verification actually run

Host: Windows 11 Pro build 26300; repository
`G:\AI-Studio\development\FUNG`. Evidence root:
`G:\AI-Studio\verification\fung-endpoint-routing-20261025`.

| Command/check | Result | Receipt |
| --- | --- | --- |
| External policy RED→GREEN, inventory consistency and stream-recreation regressions | Final 16/16 pass; immutable earlier versions retained | `task1-draft/README.md`, `draft-identity-v2.json` and RED/GREEN logs |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib capture_routing::tests` | 16 passed, zero failures, 703 filtered | `integrated-policy-tests.log` |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib native_command_inventory_smoke -- --ignored --nocapture` | One read-only native command test passed; 16 endpoints; no capture/settings changes | `integrated-native-command-smoke.log`, `integrated-native-inventory.json` |
| `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --test-threads=2` | 708 passed, zero failures, 11 ignored; 262.68 seconds | `task1-full-library-tests.log` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --lib --all-targets -- -D warnings` | Exit 0; 1m 54s | `task1-integrated-clippy.log` |

Build concurrency was limited to two jobs for the integrated checks. Ignored
tests remain ignored in the full suite; the separate read-only native command
smoke does not qualify ignored audio/provider/device tests.

Independent source review accepted the frozen v2 and the actual copy, async command
wrappers, registration and features with no actionable findings. Its acceptance
is source/read-only scope, not an independent rerun of these integrated commands.

## Remaining implementation

No new route is wired into `live_meeting_start`, the capture coordinator, source
ledger, recovery, playback, export, UI or MCP execution. Those are Task 2/3 work.
The existing native PID path still has its original failure. The endpoint route
must pass the complete rebuilt installed workflow before any release claim.

The additive source-schema design must also preserve both transcript modes,
recording-wide conservative quarantine where text lacks source lineage, and
source UUID identity through backup/recovery; a read-only admission token alone
cannot enforce any of those properties.
