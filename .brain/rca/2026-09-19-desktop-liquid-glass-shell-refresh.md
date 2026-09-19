---
version: "0.2.0b"
created_at: "2026-09-19T00:00:00+07:00"
last_update: "2026-09-20T01:11:50+07:00"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "rca"
  scope: "FUNG desktop shell"
---

# RCA — Desktop Liquid Glass Shell Refresh

## Symptom

- Desktop still presents the previous workspace and previous UI tree below the current shell.
- The upper shell contains a broad command-deck/header area that duplicates navigation and consumes vertical space.
- `LIQUID_GLASS` does not read as genuinely translucent in reader surfaces, and there is no visible ambient motion/live background.
- `QUIET ARCHIVE` is not consistently locked under `FUNG` at the requested brand scale.
- Navigation/actions are distributed across the header, legacy rail, and old power dock instead of one collapsible sidebar.
- The top-right area does not consistently expose the current user state; signed-out users only reach account actions through Settings.
- The shell falls back to page scrolling at smaller widths instead of keeping navigation in tabs and reserving scrolling for long content regions.

## Evidence

The current implementation was inspected before this RCA was written:

- `src/components/desktop/DesktopShell.tsx` renders the current shell and also wraps `mainContent` in `desktop-shell__legacy-workspace`, with the old workspace title `พื้นที่ทำงานเดิม`.
- `src/App.tsx` still renders the legacy `.app-shell`, `.ambient-grid`, `.stage-wrap`, `.stage`, `.panel-glass`, `HomeScreen`, `.fab-topbar`, `InstrumentRail`, and the old power dock alongside the new Live/Review surface stack.
- `src/components/desktop/DesktopShell.css` keeps a two-column shell with independently scrolling sidebar/main regions and switches the shell to `overflow: auto` at small widths.
- `src/components/desktop/LiquidGlass.css` uses high-alpha reader surfaces and static radial/linear layers; it has no ambient motion keyframes and its reduced-transparency branch intentionally removes backdrop treatment.
- `src/components/desktop/contracts.ts` has no account/profile state in `DesktopShellProps`; account status is currently isolated in `AccountLoginPanel` under Settings.
- The existing tests assert legacy containment and legacy style selectors, but do not reject the old presentation tree or verify the requested transparency, motion, profile-state matrix, hover expansion, or multi-viewport shell overflow contract.

## Follow-up finding — 2026-09-20

The WebView click-through confirmed a second shell information-architecture problem after the first refresh:

- The persistent sidebar still exposes เริ่มบันทึก even though recording is a task action owned by Home/Live.
- The sidebar expands an inline ลักษณะ disclosure containing theme, material, and transparency controls instead of opening a dedicated page.
- The upper-right shell exposes ย่อหน้าต่าง and ปิดหน้าต่าง, which are app-level window controls duplicated beside the profile.

Evidence came from the rendered accessibility tree at the local desktop surface and source anchors in DesktopShell.tsx: the sidebar action is around line 388, the appearance disclosure around line 436, and the two header buttons around lines 1056–1067.

The root cause is that the first shell contract grouped task commands, presentation preferences, and window lifecycle controls into one persistent chrome layer. The approved follow-up separates those ownership boundaries without deleting the underlying native handlers.

## Root Cause

The desktop UI currently has two presentation systems active at the same time: the newer `DesktopShell` surface layer and the older application/stage layer retained inside `mainContent`. The old tree still owns visible navigation and actions, so the shell cannot provide a single information architecture. The glass adaptation was applied primarily as a material layer around selected chrome while reader cards retain opaque/high-alpha backgrounds, and the ambient field was intentionally static; therefore the visual result cannot satisfy a translucent, living Liquid Glass shell. Account state was not promoted to the shell contract, and responsive behavior was implemented as overflow fallback rather than an explicit desktop/compact/tab layout model.

## Why the issue escaped detection

- Regression tests were written around preserving the old legacy tree during the earlier shell migration, so legacy selectors remained treated as expected behavior.
- Existing native click-through covered theme/material/transparency and surface navigation, but not a visual assertion or source contract for “no legacy workspace”, animated ambient presence, sidebar hover expansion, profile placement, or responsive viewport overflow.
- The account panel was validated as a Settings surface, not as a global shell state with signed-out, pending, authenticated, and refresh-failed states.
- Native launch evidence can diverge when the registered desktop app points at a stale executable; without exact-process launch evidence, a UI verification can inspect the previous binary.

## Proposed prevention

1. Remove the legacy presentation tree only after relocating its real actions (record, stop, review, import, export, pairing, and settings) into the new sidebar/action contract.
2. Add source-level tests that fail when legacy workspace/stage selectors or `พื้นที่ทำงานเดิม` remain in the rendered desktop path.
3. Add contract tests for the Liquid Glass material alpha/fallback branches, deterministic ambient motion, `prefers-reduced-motion`, and the brand lockup.
4. Add a profile-state test matrix backed by `broker_session_status`; never invent a user identity or a separate signup API.
5. Add viewport checks for 1280×800, 1024×768, 768×1024, and 390×844. The shell itself must not scroll; only explicitly long content/list regions may scroll.
6. Repeat native click-through against the exact current executable path and record the SHA-256, separating engineering validation from production/release evidence.
7. Add a shell contract test that rejects recording controls and presentation selectors inside the persistent sidebar.
8. Add an active-surface test for appearance and assert that the main page owns theme/material/transparency controls.
9. Keep app-level window commands callable only through approved native integrations; do not expose duplicate shell buttons without a window-management decision.

## Remediation and current evidence

- Removed the duplicate legacy presentation tree and its unused `HomeScreen`/`InstrumentRail` files; real Live, Review, import, export, pairing, settings and Companion handlers remain connected through the new rail.
- Added translucent glass alpha, deterministic ambient drift, reduced-motion/solid fallbacks, a stacked FUNG/QUIET ARCHIVE lockup, a focus/hover rail and truthful top-right account state.
- Browser click-through reached Home → Appearance → Live → Review, the dedicated appearance controls and the signed-out account panel at the local desktop surface; the sidebar has no recording action and source/build/regression checks pass.
- `npx tauri build --no-bundle` produced the exact release binary and SHA-256 `BED7EFB569BCE7C8BDD4D4716052A3697B7EE41CE50590BB17845E9D8DCEF10E`.
- Native click-through remains `BLOCKED_ENVIRONMENT`: the current Computer Use session cannot bind the elevated process, while a user-session launch exits `101` when the native build/runtime crosses the external `.venv-whisper` path. This is recorded as an evidence boundary, not as a UI pass.

## Validation boundary

This RCA supports the candidate design in:

`docs/design/2026-09-19-liquid-glass-desktop-shell-refresh.md`

The implementation was authorized by the approved design/spec conversation. This RCA does not authorize a production release, auth/database change, or installer claim. Risk remains HIGH because the change removes a visible presentation tree, relocates desktop actions, changes the shell contract, and affects responsive behavior.

## Version diff

| Item | Before | Candidate |
|---|---|---|
| Desktop presentation | New shell plus legacy stage | One shell with one active surface tree |
| Navigation | Header + legacy rail + power dock | Hover/focus-expandable sidebar plus responsive tabs |
| Material | Mostly high-alpha/opaque reader surfaces | Reader-safe translucent glass with explicit fallbacks |
| Ambient field | Static layers | Deterministic motion with reduced-motion compliance |
| Account state | Settings-only account panel | Truthful top-right profile/login state |
| Small widths | Shell-level overflow fallback | Tab/action layout with local content scrolling only |
| Global capture action | Persistent sidebar button | Active Home/Live surface action |
| Appearance controls | Inline sidebar disclosure | Dedicated appearance page |
| Window controls | Shell-level ย่อ / ปิด buttons | Native OS/integration boundary only |

## Approval record

Approval was recorded in the task conversation on 2026-09-19 before the first shell implementation and on 2026-09-20 for this follow-up delta. The remaining native click-through boundary requires an interactive non-elevated desktop session and is not silently treated as passed.
