---
version: "1.0.0b"
created_at: "2026-09-19T15:30:00+07:00,RWANG"
last_update: "2026-09-20T01:11:50+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "design-spec"
  scope: "FUNG Tauri desktop shell"
  parent: "docs/FUNG_Design_System_v0.1.0/LIQUID_GLASS_DESKTOP_ADAPTATION.md"
---

# FUNG — Liquid Glass Desktop Shell Refresh

## Status and approval gate

สถานะเอกสาร: **beta / approved implementation slice** มี code และ local evidence แล้ว โดย native click-through ของ exact executable ยังติด environment boundary ตาม §9

งานนี้เป็น C-3 architecture-driven UI change และมีความเสี่ยง **HIGH** เพราะลบ presentation tree เดิม, เปลี่ยน desktop information architecture, เพิ่ม auth status ที่ header และเปลี่ยน responsive/layout contract แม้จะไม่เปลี่ยน backend command contract

## [ASSUMPTIONS]

1. “พื้นที่ shell ด้านบนที่เดินมา” หมายถึง legacy `fab-topbar` / command-deck ที่มากับ UI เดิม และ full-width header ที่ทำหน้าที่เป็น navigation/control dump; จะคง native titlebar แบบบางไว้เฉพาะ drag region, brand, profile และ window controls เพื่อไม่ตัดความสามารถของ Tauri window
2. “QUIET ARCHIVE ย้ายไปใต้ FUNG ขนาดทั้งสอง เสมอ logo” หมายถึง lockup ใหม่วาง `FUNG` ด้านบนและ `QUIET ARCHIVE` ใต้ wordmark ใน block เดียวกับ mark โดยไม่ยืดหรือบีบ logo mark; ไม่ตีความว่า body text ทั้งสองต้องมีขนาดเท่ากัน
3. “live background” หมายถึง CSS ambient field ที่เคลื่อนไหวช้าและ deterministic ภายในเครื่อง ไม่ใช่ microphone waveform, video, network asset หรือสถานะการฟังปลอม
4. สมัคร/เข้าสู่ระบบต้องใช้ existing native broker และ `AccountLoginPanel`; ยังไม่สร้าง signup API หรือ auth state ใหม่

## 1. RCA summary

### Symptom

- native current build ยังแสดงทั้ง Desktop shell ใหม่และ workspace/command-deck เดิมในหน้าเดียว
- `glass` ดูเกือบทึบ เพราะ reader surfaces ใช้ alpha `0.96` และไม่มี moving ambient layer ที่เห็นผ่านพื้นผิว
- navigation/menu ยังคงอยู่บน header ทำให้พื้นที่ด้านบนสูงและ action กระจาย
- ไม่มี profile/account state ที่มุมขวาบน; account UI อยู่ใน Settings เท่านั้น
- responsive fallback เปลี่ยนทั้ง shell เป็น overflow และให้ sidebar/main มี scroll หลายชั้น แทนการเปลี่ยนเป็น tab surface

### Evidence จาก repository

- `src/components/desktop/DesktopShell.tsx:917` ครอบ `mainContent` ด้วย `desktop-shell__legacy-workspace` และแสดง label “พื้นที่ทำงานเดิม”
- `src/App.tsx:1560` เริ่ม `callmd-legacy-workspace`; ภายในยังมี `.app-shell`, `HomeScreen`, `stage-wrap`, `fab-topbar`, `InstrumentRail` และ power dock
- `src/components/desktop/LiquidGlass.css:15,39,59` ตั้ง `--fung-glass-reader` เป็น alpha `0.96`; `src/components/desktop/LiquidGlass.css:71` มีเพียง static pseudo-element และไม่มี keyframe ambient motion
- `src/components/desktop/DesktopShell.tsx:824-873` วาง surface navigation และ appearance/window actions ทั้งหมดใน header เดียว
- `src/components/AccountLoginPanel.tsx:64-70` แสดง account UI ใน panel ที่เปิดจาก Settings; `DesktopShellProps` ยังไม่มี account summary สำหรับ header
- `src/components/desktop/DesktopShell.css:371,381,505` ใช้ fixed sidebar grid และ scroll containers; `:max-width:820px` เปลี่ยน shell เป็น `overflow:auto` แทน surface tabs

## 2. Target information architecture

```text
┌────────────────────────────────────────────────────────────────────────┐
│ native titlebar: [mark] FUNG / QUIET ARCHIVE                         [profile] │
├────── rail 72px (hover/focus → 248px) ────┬─────────────────────────────┤
│ [Home]                                    │ active surface                │
│ [Live]        primary navigation          │ Home / Live / Review          │
│ [Review]                                  │ one task, one readable pane  │
│ ───────                                   │                              │
│ [Import] [Export] [Pair] [Settings]       │ context sheets only when used│
│ [Theme / Material / Transparency]         │                              │
└───────────────────────────────────────────┴─────────────────────────────┘
```

### 2.1 Remove completely

- `desktop-shell__legacy-workspace` wrapper and “พื้นที่ทำงานเดิม” heading
- `callmd-legacy-workspace` and the old `.app-shell`/`stage-wrap` presentation tree
- old `HomeScreen` presentation in the shell
- old `fab-topbar` / Command deck bar
- old `InstrumentRail` and power dock presentation after its real handlers are moved into the new rail
- legacy-only CSS in `src/styles.css` that is reachable only through the removed tree

### 2.2 Preserve and relocate

- keep `LiveMeetingPanel` and `RecordingReview` as the real active-surface owners
- keep existing project/recording pair scope and capture navigation guard
- move existing `importMedia`, `export.render`, pairing, Settings and window actions into the new rail or a sheet
- keep `RecoveryNotice`, Settings and Pairing as bounded overlays/sheets; do not turn them into fake routes
- do not delete native commands or backend handlers merely because their old visual caller is removed

## 3. Liquid Glass contract

### 3.1 Real transparency

- Add a dedicated `desktop-shell__ambient` layer behind content with two or more CSS gradient fields and a slow 18–24s drift animation
- `glass/full` must expose the ambient field through chrome/control surfaces: no full-mode surface may fall back to `--fung-glass-solid`
- lower chrome/control alpha so `backdrop-filter: blur(...) saturate(...)` has visible input; keep reader/safety surfaces more opaque but visibly layered and readable
- `glass/reduced`, `prefers-reduced-transparency`, `forced-colors` and unsupported backdrop-filter use solid readable fallbacks
- never represent microphone level, recording, AI progress or completion with the ambient animation

### 3.2 Motion

- motion is only ambient decoration plus bounded hover/focus/sheet transitions
- `prefers-reduced-motion: reduce` disables ambient keyframes and nonessential transitions
- no remote image/video, no perpetual waveform, no random layout movement

## 4. Brand lockup

- titlebar/sidebar brand block renders the canonical mark at its approved geometry
- `FUNG` sits above `QUIET ARCHIVE`; the note aligns to the wordmark column and stays visually subordinate
- the lockup is not a card and does not receive a glow or opaque plate

## 5. Sidebar and menu contract

- desktop rail starts collapsed at approximately 72px and expands to approximately 248px on `:hover` or `:focus-within`
- labels remain available to screen readers in collapsed mode; visible labels fade/clip only visually
- keyboard focus must expand the rail without requiring a pointer
- primary menu: `หน้าหลัก`, `ประชุมสด`, `บันทึกย้อนหลัง`
- secondary menu: `นำเข้าไฟล์`, `ส่งออก`, `จับคู่อุปกรณ์`, `ตั้งค่า`
- appearance controls move into the rail’s lower section or Settings sheet; they must not return to the full-width header
- active surface remains `aria-current="page"`; menu actions keep existing handler/capability truth

## 6. Account/profile contract

Use the existing `broker_session_status` read-only path and `AccountLoginPanel` as the source of truth.

| Broker state | Top-right UI | Action |
| --- | --- | --- |
| `authenticated` with email | compact profile chip with initials + email | open Settings → account tab |
| `signed_out`, `login_pending`, `refresh_failed` or unavailable | `สมัคร / เข้าสู่ระบบ` | open existing account login panel |
| loading/unknown | neutral `กำลังตรวจสอบบัญชี` state | no fabricated identity |

No avatar URL, display name, signup endpoint or profile claim may be invented. A signed-out “สมัคร / เข้าสู่ระบบ” label is one entry point to the existing Google broker flow, not a claim that a separate signup flow exists.

## 7. Responsive contract

| Viewport | Layout | Scroll rule |
| --- | --- | --- |
| `>= 1024px` | collapsed/hover-expanded rail + one active surface | shell itself does not scroll; only long reader/list regions may scroll |
| `721–1023px` | compact rail or wrapped surface tab strip; context opens as sheet | no horizontal overflow; one active surface at a time |
| `<= 720px` | `role="tablist"` for Home/Live/Review; bottom/inline action tabs for Import/Export/Settings | do not stack the old desktop shell; long transcript/list scrolls inside its own region |

At narrow widths, create a new surface tab/sheet when content would otherwise require a second dense column. Do not solve overflow by shrinking text or scaling the whole UI.

## 8. Acceptance criteria before implementation is complete

- source and rendered DOM contain no `พื้นที่ทำงานเดิม`, `desktop-shell__legacy-workspace`, `callmd-legacy-workspace`, `fab-topbar`, old `.app-shell`, or old `InstrumentRail` in the desktop shell path
- Home, Live Meeting and Review remain reachable through the new rail/tabs and preserve existing project/recording scope
- native current executable visibly shows ambient background through `glass/full`; reduced/solid fallback remains readable
- ambient motion is observable in full mode and is disabled under reduced-motion/reduced-transparency contracts
- Quiet Archive is below FUNG in the new lockup and does not become a second logo
- rail expands on hover and keyboard focus; all menu actions have accessible names and at least 44px targets
- authenticated and signed-out/unknown account states render truthfully at top right without exposing credentials
- browser checks at 1280×800, 1024×768, 768×1024 and 390×844 show no shell-level horizontal overflow; native click-through is rerun on the rebuilt exact executable
- `npm run build`, relevant DesktopShell/integration/auth tests, `npm run design-system:check`, `git diff --check` and native bounded click-through pass

## 9. Implementation evidence

การ implement ตามเอกสารนี้เสร็จในขอบเขต desktop shell และผ่าน local regression ดังนี้:

| Gate | ผล | Evidence / boundary |
| --- | --- | --- |
| Legacy presentation removal | PASS | ลบ `HomeScreen`, `InstrumentRail`, old `.app-shell`/stage tree, `fab-topbar`, power dock และ `desktop-shell__legacy-workspace`; คง Live/Review owners และ handlers จริง |
| Liquid Glass / motion | PASS (source/build) | `LiquidGlass.css` ลด alpha ของ full glass, เพิ่ม ambient drift keyframes และ reduced-motion/solid fallbacks; ไม่ใช้ animation แทน recording state |
| Brand / sidebar / profile | PASS (browser) | FUNG → QUIET ARCHIVE lockup, rail 72px → 248px เมื่อ focus/hover, Home/Live/Review/actions, top-right `สมัคร / เข้าสู่ระบบ` และ account sheet ทำงานใน browser surface |
| Browser click-through | PASS (local) | `http://127.0.0.1:1420/app?surface=desktop`: Home → Appearance → Live → Review → signed-out profile panel ผ่าน; sidebar ไม่มี recording start/stop และ appearance controls อยู่ในหน้าแยก; additional 1024×768, 768×1024, 390×844 viewport run: NOT RUN |
| Static / regression | PASS | `npm run build`, `npm run test:callmd-shell`, `npm run test:callmd-integration`, `npm run test:desktop-bootstrap`, `npm run test:callmd-contracts`, `npm run test:callmd-live`, `npm run test:callmd-history`, `npm run test:design-system`, `npm run design-system:check`, `npm run test:release`, `npm run test:auth`, `npm run test:recovery`, `git diff --check` |
| Windows engineering build | PASS | `npx tauri build --no-bundle`; `src-tauri/target/release/fung.exe`, 25,806,848 bytes, SHA-256 `BED7EFB569BCE7C8BDD4D4716052A3697B7EE41CE50590BB17845E9D8DCEF10E` |
| Native exact-executable click-through | BLOCKED_ENVIRONMENT | elevated launch could not be bound by the current Computer Use session; user-session launch exits `101` while the build/runtime still reads `..\\.venv-whisper` outside the writable workspace. No native pass is claimed |
| Installer / clean install / production | NOT RUN | `--no-bundle` only; no installer, clean VM, physical-device or production evidence |

The native boundary is an environment/evidence limitation, not a source/build failure. Re-run the exact executable in an interactive non-elevated desktop session with access to the bundled runtime before promoting this document to stable.

## 10. Non-goals

- no backend schema/API/auth contract change
- no OS-level Companion window, global hotkey, always-on-top behavior or cross-app capture
- no real-time microphone waveform or fake “live” telemetry
- no new profile storage, avatar service or separate signup implementation
- no production/installer claim from local evidence

## 11. Version diff

`LIQUID_GLASS_DESKTOP_ADAPTATION.md 0.3.2b → beta 0.4.0b`: removes the duplicated legacy desktop presentation, moves navigation into an expandable rail, adds truthful account/profile presentation, defines actual translucent glass with bounded ambient motion, and replaces overflow-heavy responsive behavior with surface tabs/sheets.

## 12. Approved shell delta — 2026-09-20

This approved follow-up supersedes the earlier placement of capture controls, appearance controls, and app-level window controls.

### UI contract

- The persistent sidebar does not render the recording start/stop action. Existing recording handlers remain available from the active Home/Live surfaces and the capture strip when recording is active.
- Appearance is a dedicated active surface named appearance. The sidebar keeps only a navigation action; theme, material, and transparency controls render in the main surface and are never descendants of the sidebar.
- The FUNG shell header keeps brand, profile, and truthful recording status only. The app-level ย่อ and ปิด buttons are removed. Native OS titlebar controls and the Tauri drag region remain outside this UI delta.
- No Tauri command, recording, pairing, auth, or backend contract is removed. Existing window action handlers may remain available to native integrations even though this shell no longer exposes the buttons.

### Verification gate

- Accessibility tree for the shell contains no sidebar control labelled เริ่มบันทึก, หยุดบันทึก, ย่อหน้าต่าง, or ปิดหน้าต่าง.
- Clicking ลักษณะ navigates to the appearance surface; the page heading is ลักษณะและธีม and all three presentation controls are available in the main content region.
- Home, Live, Review, account/profile, Settings, Pairing, and Companion flows keep their existing handlers and scope.
- Browser click-through covers Home → Appearance → Live → Review → profile/settings, followed by the existing static and regression gates.

## 13. Version diff

0.4.0b → 1.0.0b: moves recording out of the persistent sidebar, promotes appearance to a dedicated page, removes app-level minimize/close controls, and keeps the existing native command boundary unchanged.

## 14. Approval record

Approval was recorded in the task conversation on 2026-09-19 before implementation. This document now records the implementation and its evidence boundary; production/release approval remains out of scope.
