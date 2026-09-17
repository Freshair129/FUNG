---
version: "0.1.1b"
created_at: "2026-09-17T04:32:50.0960481+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T04:48:51.5477875+07:00,Codex"
status: "candidate"
superseded_by: null
base_sha: "376ef30db13670e4dea816ceff440f44ce73fffd"
attributes:
  doc_type: "implementation-report"
  domain: "FUNG desktop"
  scope: "SHARED_CONTRACT FIX1: bounded P1-B consumer correction"
  complexity: "C-3"
  risk: "MEDIUM"
  authority: "approved P1-B; controller RCA and current contracts/UX spec"
---

# CallMD desktop shared-contract FIX1 handoff

## Result and exact lease

- Result: `DONE_WITH_CONCERNS`; FIX1 is complete and released to Terra for independent bounded review. This is not self-acceptance and does not create a new approval loop.
- Base/HEAD: `376ef30db13670e4dea816ceff440f44ce73fffd`.
- Revision: uncommitted; no commit, staging, push, merge, install, deployment, or new agent.
- Authorized writes only: `src/components/desktop/contracts.ts` and this report.
- `src/tauri.ts` remained byte-identical at SHA-256 `816b9e1142b410f73ef780ed8a5328088c3f2d09cd8c05b93e28d20fa1c4e7fe`.
- Accepted contract tests, reports, baselines, native/spec/CI/package/lock inputs, and existing legacy bridge behavior remained read-only.

## Bounded implementation correction

1. `LiveWorkspaceActions.ask(selection: RecordingKey, question: string, requestId: string): Promise<RecordingAnswer>` now carries the selected recording and request identity. `LiveWorkspaceProps.ask` is `ReadState<RecordingAnswer>`. The unused `AskAnswer` import was removed; the existing legacy bridge and legacy UI binding were not changed.
2. Exported `normalizeReviewError(error: unknown): ReviewError` preserves a runtime-valid typed `ReviewError`. `Error`, strings, null, and unrecognized shapes become the fixed safe `{ code: "LEGACY_COMMAND_FAILED", message: "Legacy desktop command failed.", retryable: false }`; no raw message, path, token, stderr, or parsing is used.
3. Added the minimal controlled `DesktopSurface = "home" | "live" | "review"`, `DesktopShellProps.selectedProjectId: string | null`, `DesktopShellProps.activeSurface`, and `DesktopShellActions.showHome`. No generic legacy action/store/listener or unnecessary legacy surface was added.

The existing settlement identity comparison, typed DTOs, native command wrappers,
capture/listener ownership, review/player ownership, and backend/native separation
are unchanged. No native interface or backend change is included.

## Verification and provenance

| Exact command | Exit | Result |
|---|---:|---|
| `node --test --experimental-strip-types tests/callmdDesktopContracts.test.mjs` | 1 | 13 total: 12 pass, 1 approved native-presence expected red; 0 harness/setup failures. |
| `npm run test:summary-scoping` | 0 | 6/6 pass. |
| `npm run test:job-actions` | 0 | 17/17 pass. |
| `npm run test:egress` | 0 | 8/8 pass. |
| `npm run build` (`tsc && vite build`) | 0 | TypeScript and Vite production build pass. |

Native-free inline normalizer probe (no new test file):

```text
node --experimental-strip-types --input-type=module -e 'import assert from "node:assert/strict"; import { normalizeReviewError } from "./src/components/desktop/contracts.ts"; const valid = { code: "STORAGE_READ_FAILED", message: "safe storage failure", retryable: true }; assert.strictEqual(normalizeReviewError(valid), valid); const markers = ["TOKEN=secret-token", "C:\\private\\recording.wav", "stderr=provider secret"]; const unknown = [new Error(markers[0]), markers[1], null, { code: "NOT_A_REVIEW_ERROR", message: markers[2], retryable: true }]; for (const value of unknown) { const normalized = normalizeReviewError(value); assert.deepEqual(normalized, { code: "LEGACY_COMMAND_FAILED", message: "Legacy desktop command failed.", retryable: false }); const encoded = JSON.stringify(normalized); for (const marker of markers) assert.equal(encoded.includes(marker), false); } console.log("normalizer probes: 5/5 pass (typed preservation + 4 safe fallbacks; no secret/path/stderr echo)");'
```

Type-contract evidence is the passing `tsc` stage plus source inspection of the
exported props/actions; future UI consumer behavior remains owned by the UI workers.
Controller inputs were the read-first RCA `2026-09-17-callmd-shared-contract.md`,
current `docs/specs/2026-09-17-callmd-desktop-contracts.md`, and current
`docs/design/2026-09-17-callmd-desktop-ui.md`, not isolated worker notes.

Implementation SHA-256 after FIX1: `src/components/desktop/contracts.ts` =
`7f2bd4191f804fca5788684a5891471fda70d713110c5e75b3b5f67870cbae81`.
The single expected red remains the separate missing native modules/commands gate;
no native/runtime/device/packaged/hosted-CI/production readiness is claimed.

## Version Diff / CHANGELOG

- `0.1.0b → 0.1.1b`: corrected the B live ask contract, added the safe shared
  legacy-error normalizer, and added the minimal controlled home/live/review shell
  surface while preserving all existing ownership and legacy bridge behavior.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.1b | 2026-09-17 | candidate | Bounded SHARED_CONTRACT FIX1; released for Terra review | UNCOMMITTED; base 376ef30 | Codex |
| 0.1.0b | 2026-09-17 | candidate | Bounded shared bridge and UI contract handoff | UNCOMMITTED; base 376ef30 | Codex |
