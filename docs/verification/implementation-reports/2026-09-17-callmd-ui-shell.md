---
version: "0.1.2b"
created_at: "2026-09-17T08:49:17.371+07:00,RWANG,Codex gpt-5.6-luna/max,base=376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T08:53:05.353+07:00,RWANG"
status: "beta"
superseded_by: null
base_sha: "376ef30db13670e4dea816ceff440f44ce73fffd"
agent: "RWANG (Codex gpt-5.6-luna/max)"
model: "gpt-5.6-luna/max"
commit: "UNCOMMITTED"
attributes:
  domain: "FUNG desktop"
  doc_type: "implementation-report"
  scope: "P1-B UI_SHELL PHASE FIX2; exact four-file lease"
  complexity: "C-2 bounded shared type and UI semantic correction"
  risk: "MEDIUM bounded UI lifecycle and navigation truth; no native wire change"
---

# UI_SHELL PHASE FIX2 implementation report

## Frozen handoff

Result: **FROZEN_HANDOFF**. The bounded shared-type and Shell lifecycle
correction is complete and released for independent read-only review. This is
not self-acceptance and does not authorize main integration or a new feature.

Base and HEAD are
`376ef30db13670e4dea816ceff440f44ce73fffd`. The exact execution cwd was
`C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30`.
The result is **UNCOMMITTED**: no commit, staging, push, merge, root copy,
integration transfer, deployment, installation, provider, native-app, browser,
device, credential, or tool-network operation was performed.

The source lease is released at this handoff for an independent review. Main
integration must wait for that review and must perform the later approved
`INTEGRATE FIX1` App forwarding separately.

## Root cause and bounded correction

The existing `DesktopShell` mapper treated every `ReadState.loading` value as
capture `starting`. A status read is not a capture-start operation. In the
real-app failure, the initial native-unavailable/loading snapshot therefore
created a false capture strip, active-session sidebar state, and navigation
guard without a native session.

The existing `LivePhase` type is the owner phase already published by the Live
owner. The correction is type-only at the shared boundary and uses this exact
production precedence in `getCaptureLifecycle`:

1. native `data.stopping === true` → `stopping`;
2. explicit owner phase `stopping` → `stopping`;
3. native `data.active === true` → `active`;
4. explicit owner phase `starting` → `starting`;
5. explicit owner phase `listening` or `degraded` → `active`;
6. otherwise no known capture → the existing Shell `inactive` lifecycle.

The optional `DesktopShellProps.livePhase` defaults to `"idle"` only for
backward compatibility. That default is not evidence that native capture is
inactive and does not fabricate a native status payload. Loading, unavailable,
or error without known capture data or a capture-relevant phase cannot create a
capture strip or navigation guard. Existing active/stopping data and explicit
capture-relevant phases survive a read error, while the existing error or
unavailable notice remains visible. The hard `stopAndLeave` inactive
acknowledgement remains unchanged.

Only these four paths were changed by this lease:

- `src/components/desktop/contracts.ts` — added optional type-only
  `DesktopShellProps.livePhase?: LivePhase`.
- `src/components/desktop/DesktopShell.tsx` — consumed the existing phase and
  implemented the bounded precedence; no state machine, poller, listener,
  fetch, native DTO, command, error, or bridge change.
- `tests/callmdDesktopShell.test.mjs` — exercised the actual production
  `getCaptureLifecycle`, `shouldGuardNavigation`, and mounted Shell SSR through
  `renderShellWithProps`; no copied production helper or regex-only substitute.
- `docs/verification/implementation-reports/2026-09-17-callmd-ui-shell.md` —
  this FIX2 handoff report.

`src/components/desktop/DesktopShell.css` was frozen and unchanged. `src/tauri.ts`
was frozen and unchanged. `App.tsx` phase forwarding belongs to the later
integration lease. LiveFIX2's Panel/CSS/test/report paths, native files,
package/CI, schema, Cargo, dependency, lock, auth, cloud, Drive, and unrelated
worktree paths were not edited.

## Verification

The previous six Shell test cases were retained. Two new regression test cases
were added, for a total of **8/8**. Existing loading coverage was corrected to
use explicit `starting`; the new cases prove that ambiguous loading without a
phase stays clean. Coverage includes bootstrap loading, unavailable/error
without phase, explicit starting, listening/degraded fallback, native and
owner stopping precedence, pending stop with `active=true`, authoritative
active/stopping retained through read error, inactive false/false, idle with
no fabricated payload, the real rendered Shell, keyboard guard behavior, and
the existing brand/CSS assertions.

| Command | Exit | Result |
|---|---:|---|
| `node --test --experimental-strip-types tests/callmdDesktopShell.test.mjs` | 0 | PASS — 8/8 (6 retained, 2 new) |
| `npm run build` | 0 | PASS — TypeScript and Vite production build |
| `npm run test:desktop-bootstrap` | 0 | PASS — 10/10 |
| `npm run test:egress` | 0 | PASS — 8/8 |

These are local source/test/build results only. They do not prove browser paint,
real React App/Live phase forwarding, native presence, audio capture, device,
provider, hosted CI, packaged runtime, production data, or production
readiness. Whole-UI isolated-CI and native-presence gaps remain expected
integration work and were not waived.

## Lease artifact hashes

The four source/report paths and the preserved CSS are recorded together here.
The report's own SHA-256 is intentionally emitted separately after this final
write because embedding a self-hash changes the report bytes.

| Absolute path | SHA-256 |
|---|---|
| `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30\src\components\desktop\contracts.ts` | `AA6C05C014E62A56D2F2E88F52157C2D1C19CE82B9861018CDB976CE9883DDF2` |
| `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30\src\components\desktop\DesktopShell.tsx` | `E9859D79D8A93C1EAE48CBD1B553863203D1892A1EC46A1236F89E8A623F6D76` |
| `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30\src\components\desktop\DesktopShell.css` | `C2DE7032B06E92CA8126F1B680C5A26686108CF5C6363587805E9E3F3C911D7E` |
| `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30\tests\callmdDesktopShell.test.mjs` | `1BFE6384BB3EE69D12390C1553AF7E10F34A9970CC4A3F06AE10A0F825FAEC2A` |
| `C:\Users\pc\.codex\worktrees\9000\fung\output\callmd-worktrees\shared-376ef30\docs\verification\implementation-reports\2026-09-17-callmd-ui-shell.md` | reported externally after this final write |

Historical accepted invariants were rechecked before the report write:

- Previous shared-contract hash: `7F2BD4191F804FCA5788684A5891471FDA70D713110C5E75B3B5F67870CBAE81`.
- Current FIX2 shared-contract hash: `AA6C05C014E62A56D2F2E88F52157C2D1C19CE82B9861018CDB976CE9883DDF2`.
- Frozen bridge hash: `816B9E1142B410F73EF780ED8A5328088C3F2D09CD8C05B93E28D20FA1C4E7FE`.
- Preserved CSS hash remains `C2DE7032B06E92CA8126F1B680C5A26686108CF5C6363587805E9E3F3C911D7E`.

No dependency was installed and no package-lock, Package/CI baseline, or
native file was overwritten. Existing unrelated dirty paths remain outside this
lease.

## Scope and release gates

- **Accepted for this handoff:** bounded truthful Shell mapping and type-only
  internal phase handoff; focused tests/build/bootstrap/egress local evidence.
- **Pending independent review:** source correctness, scope, acceptance and
  hash review by a separate reviewer; this worker does not self-accept.
- **Later integration:** App forwards the existing owner phase in the separate
  fresh `INTEGRATE FIX1` lease and updates integration coverage there.
- **Not claimed:** native/runtime/browser/provider/device/data/credential,
  hosted-CI, packaged, production, merge, release, or deployment acceptance.

## Version diff / CHANGELOG

`0.1.1b → 0.1.2b`: separated status-read bootstrap from the existing Live owner
phase, added the exact precedence and truthful no-capture fallback, retained
read-error disclosure and hard stop acknowledgement, and added two production
Shell regression tests. No new feature, native interface, bridge, schema, CSS,
or runtime listener was added.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.2b | 2026-09-17 | beta | UI_SHELL PHASE FIX2 bounded type-only handoff and truthful capture/navigation semantics; frozen for independent review | UNCOMMITTED; base 376ef30 | RWANG / Codex Luna max |
| 0.1.1b | 2026-09-17 | beta | UI_SHELL FIX1 frozen brand-fidelity handoff for fresh independent review | UNCOMMITTED; base 376ef30 | RWANG / Codex Luna max |
| 0.1.0b | 2026-09-17 | beta | Quiet Archive shell, guard/focus behavior, state SSR tests, and bounded evidence | UNCOMMITTED; base 376ef30 | RWANG / Codex Luna max |
