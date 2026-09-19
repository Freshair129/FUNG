---
id: DS-FUNG-GLASS-DESKTOP-001
title: "FUNG — Liquid Glass Desktop Adaptation"
version: "0.4.0b"
status: "beta"
created_at: "2026-09-19T08:04:52+07:00"
last_update: "2026-09-19T17:42:17+07:00"
language: "th-TH / en"
parent: "DS-FUNG-001 v0.1.0"
superseded_by: null
attributes:
  domain: "frontend-redesign"
  doc_type: "design-spec"
  scope: "desktop Tauri/React and in-app companion"
  source_path: "C:\\Users\\pc\\Downloads\\fung-new-ui"
implementation_verified: false
implementation_status: "PARTIAL_LOCAL_VERIFIED"
native_clickthrough: "BLOCKED_ENVIRONMENT"
human_approval_required: false
---

# FUNG — Liquid Glass Desktop Adaptation

**สถานะ:** Beta — approved และ local implementation verified บางส่วน; current native click-through ถูกบันทึกเป็น `BLOCKED_ENVIRONMENT`

**ความเสี่ยง:** MEDIUM–HIGH — เปลี่ยน visual grammar หลาย desktop surfaces และเพิ่ม in-app companion presentation แต่ไม่เปลี่ยน backend, auth, recording, permission หรือ native command contract ในรอบนี้

## 0. วัตถุประสงค์และขอบเขตอำนาจ

เอกสารนี้แปลง reference จาก `C:\Users\pc\Downloads\fung-new-ui` ให้เป็นข้อกำหนดสำหรับ **desktop adaptation** ของ FUNG โดยคง product truth และ capability boundary จาก [DS-FUNG-001 v0.1.0](DESIGN_SYSTEM.md)

เอกสารนี้ได้รับ approval สำหรับ implementation แบบจำกัดขอบเขตแล้ว การเปลี่ยน `status` เป็น `beta` ไม่ใช่หลักฐานว่า UI ถูก implement หรือผ่าน native click-through แล้ว

## 1. แหล่งอ้างอิงและสถานะหลักฐาน

### 1.1 Reference ที่ตรวจจริง

| แหล่ง | SHA-256 | ใช้ตัดสิน | สถานะ |
| --- | --- | --- | --- |
| `LIQUID_GLASS.md` | `A5D923756CAD1597E3BB3B6C5EA53E3A4B7AA2C57B1C9A0A66255C3413C8C28E` | material, tokens, M01–M12, transparency และข้อจำกัด | อ่านและยืนยันแล้ว |
| `FUNG_Mobile_Liquid_Glass_v0.3.0.html` | `5C278B4CFF055D45685B2B09B783D4BB5C9649E68C1C39FCBFEF3323B073B730` | interactive mobile visual prototype | อ่านเป็น reference เท่านั้น |
| `FUNG_Liquid_Glass_Dark_v0.3.0.png` | `59D7CFECA082725A8D5994BA5B7419FAE5285ABDD517715DCECD11964DED4CD3` | dark material direction: Home/Capture/Graph | visual reference |
| `FUNG_Liquid_Glass_Light_v0.3.0.png` | `87C7E82DEB5E580F42D9E5B31EC3F7CC59FC406EF1280BC39E60AE9A8A8AB62B` | light material direction | visual reference |
| `ChatGPT Image Sep 19, 2026, 07_59_52 AM-2.png` | `D2D0D96ECD5F7A1FD5D1EEE88DD4D9A8F62EA3676AF8BD6529C080EBA2E713A6` | Companion Overlay mood and Idle/Peek/Command/Conversation modes | concept reference |

Reference เหล่านี้อยู่นอก repository และยังไม่ได้ถูก vendored เข้า design-system package การ hash ไว้เป็น provenance เท่านั้น ไม่ใช่การประกาศว่าไฟล์ดังกล่าวเป็น production asset

### 1.2 สิ่งที่ reference ยืนยันและไม่ยืนยัน

ยืนยัน: Quiet Archive ยังเป็น brand, Liquid Glass เป็น material layer, reader ต้องอ่านง่าย, theme/material/transparency แยกกัน และภาพ mobile มี 12 screen families

ไม่ยืนยัน: desktop pixel-perfect layout, native OS-level overlay, live microphone waveform, AI/command capability, persistence ของ task/marker/note, provider/auth state หรือ production logo/SVG

`LIQUID_GLASS.md` ระบุเองว่าเป็น `mobile HTML prototype; no production integration` และระบุว่าไม่มี Liquid Glass desktop/web/landing mock ฉบับ production ดังนั้น desktop mapping ด้านล่างเป็นข้อเสนอที่ต้อง review ไม่ใช่การคัดลอกภาพแบบ 1:1

## 2. Design decisions ที่เสนอ

1. **Liquid Glass เป็น material layer ไม่ใช่ brand replacement** — คง Quiet Archive, canonical mark, IBM Plex Sans Thai/DM Sans และ semantic status จาก DS-FUNG-001
2. **Reader มาก่อน chrome** — transcript, summary, error, consent และ recovery ใช้พื้นผิวทึบ/เกือบทึบ ไม่วาง blur หรือ distortion บน glyphs
3. **Glass แยกจาก semantic truth** — ความโปร่ง ขอบเรือง และ wallpaper ห้ามถูกใช้แทน `confirmed`, `inferred`, `disputed`, `ai_proposed`, recording หรือ provider state
4. **Theme และ material เป็น preference ของ presentation** — รองรับ `light`, `dark`, `system` และ `glass/full`, `glass/reduced`, `solid`; persistence ต้องใช้ contract ที่มีอยู่หรือเพิ่ม contract แยกก่อน ไม่สร้าง localStorage behavior เงียบ ๆ
5. **Animation สั้นและมีเหตุผล** — transition ประมาณ 180ms; ไม่ใช้ pulse/waveform ต่อเนื่องเพื่อสื่อว่ากำลังฟังเมื่อไม่มี source event
6. **Windows/Tauri เป็น platform baseline** — ไม่คัดลอก macOS traffic lights, iPhone notch หรือ claim ว่าเป็น native Apple Liquid Glass

## 3. Desktop surface map

การปรับใช้ต้องรักษา flow และ command boundary ใน repository ปัจจุบันไว้ก่อน

| Existing surface | Component/route anchor | Liquid Glass adaptation | ห้ามเพิ่มจากภาพโดยไม่มี contract |
| --- | --- | --- | --- |
| Home / recent work | `src/components/HomeScreen.tsx`, `src/components/desktop/DesktopShell.tsx` | ambient canvas, Quiet Archive lens, glass primary CTA, recent rows และ navigation chrome | global search, task persistence, fake activity หรือ waveform |
| Live Meeting / Capture | `src/components/LiveMeetingPanel.tsx`, `src/components/desktop/LiveWorkspace.tsx` | capture lens, clear recording state, reader-safe status panel, stop action ที่เด่น | pause/marker/clip/AI live result หรือ live waveform ถ้าไม่มี source |
| Transcript / recording review | `src/components/desktop/RecordingReview.tsx` | reader surface tint สูง, speaker/status chips, glass controls เฉพาะ action ที่มีอยู่ | speaker identity, confidence, completion หรือ summary ที่ backend ไม่ยืนยัน |
| Summary / export / recovery | `src/App.tsx`, `src/components/RecoveryNotice.tsx`, export actions เดิม | conversation-like reader panel และ provenance/status disclosure | task checklist ที่แก้ไขได้, cloud success, export success จาก timer หรือ fixture |
| Settings / backup / providers | `src/components/SettingsPanel.tsx`, `src/components/BackupPanel.tsx` | material selector, theme, reduced-transparency presentation และ glass sections | เปลี่ยน auth, provider, backup target หรือ permission semantics |
| In-app companion | **proposed new presentation boundary** | Idle → Peek → Command → Conversation เป็น panel/sheet ภายใน FUNG window ใช้ state จาก existing app | OS-level always-on-top, global hotkey, cross-app capture, secret hiding หรือ native overlay permission |

### 3.1 Companion boundary

รอบนี้ Companion คือ **in-app UI only**: เปิด/ปิดได้, focus และ Escape ทำงาน, background ของ panel inert ตามความเหมาะสม, และการปิด Companion ต้องไม่หยุด recording หรือสร้าง recording ใหม่

Native window ที่ลอยเหนือแอปอื่นต้องทำ ADR แยกตามข้อเสนอเดิม `ADR-DS-003` และต้องมี lifecycle, focus, multi-monitor, screen-share privacy และ permission evidence ก่อน implementation ห้ามใช้ `position: fixed` แล้วเรียกว่า OS-level overlay

## 4. Material contract

ให้สร้าง/ใช้ namespace แยกจาก baseline tokens เดิม โดยยังไม่ลบหรือเปลี่ยน semantic token source of truth:

| Token family | Dark proposal | Light proposal | การใช้ |
| --- | --- | --- | --- |
| canvas | `#111A17` | `#EEF0E9` | ambient desktop background |
| chrome | `rgba(27,40,34,0.76)` | `rgba(252,252,246,0.76)` | rail, nav, control chrome |
| control | `rgba(53,70,62,0.64)` | `rgba(248,251,245,0.68)` | buttons and lenses |
| reader | `rgba(22,32,27,0.96)` | `rgba(255,254,250,0.96)` | transcript, summary, errors |
| sheet | `rgba(28,40,33,0.96)` | `rgba(255,254,250,0.95)` | in-app sheet/panel |
| edge/highlight | `rgba(205,223,213,0.42)` / `rgba(248,255,249,0.70)` | `rgba(88,104,94,0.44)` / `rgba(255,255,255,0.94)` | rim and separation, not focus substitute |

Suggested names use `--fung-glass-*` and remain separate from `--fung-*` baseline until token ownership is reviewed. Blur is limited to chrome/control/sheet; reader surfaces use tint first. Focus-visible remains an independent high-contrast outline

## 5. Interaction and accessibility contract

- Every primary action keeps an accessible name and a target of at least 44 CSS px; primary desktop controls target at least 52 CSS px where layout allows
- Focus-visible must be visible independently of the glass rim; keyboard order follows the existing desktop flow
- `prefers-reduced-transparency`, `prefers-contrast: more`, `forced-colors` and `prefers-reduced-motion` must have a solid/readable fallback
- Theme/material switching must not reset recording, review selection, auth state, pairing state or job state
- Glass is not an accessibility proof; contrast and native click-through must be re-run against the actual rendered surfaces
- Thai copy remains primary where current product copy is Thai; English labels may remain for technical state names where existing contracts use them

## 6. Acceptance criteria for implementation

Implementation follows this approved document. Completion requires all of the following:

| Gate | Expected evidence |
| --- | --- |
| Visual foundation | Current source uses the reviewed Liquid Glass token namespace without deleting baseline tokens |
| Desktop states | Home, Live Meeting, Review/Summary, Settings and in-app Companion states render in light/dark/system as scoped |
| Capability truth | No new button reports success without an existing handler/contract; unsupported reference actions remain hidden/disabled/disclosed |
| Interaction | Keyboard/focus/Escape/material fallback works; Companion close does not affect recording |
| Static validation | `npm run build`, relevant contract tests, token/design-system checks and existing CI pass |
| Browser visual smoke | Desktop light/dark routes render without console errors; this is separate from native proof |
| Native proof | Rebuilt release executable is opened and the bounded desktop click-through is actually observed; process launch alone is insufficient |
| Release boundary | Installer, clean-install, physical-device and production readiness remain separate evidence gates |

## 7. Explicit non-goals

- ไม่เพิ่ม native OS-level overlay, global shortcut, screen-share hiding หรือ always-on-top permission
- ไม่เพิ่ม AI chat/command API, task persistence, note/marker persistence, speaker identity หรือ provider execution
- ไม่ย้าย mobile M01–M12 navigation มาแทน desktop information architecture
- ไม่ถือภาพ PNG, HTML prototype, animation หรือ completion ของ transition เป็น runtime success
- ไม่อ้างว่า Liquid Glass นี้เป็น Apple/native API หรือ GPU/thermal benchmark

## 8. Implementation sequence and approval gate

1. **Document review** — review this mapping and open decisions; no code change
2. **Foundation slice** — add/derive material tokens and scoped visual primitives; run token/static checks
3. **Desktop surfaces** — adapt existing Home → Live → Review/Summary → Settings flows without changing command contracts
4. **In-app Companion** — implement only the bounded panel/sheet state machine; native overlay remains blocked by ADR
5. **Visual and interaction QA** — browser smoke, keyboard/fallback checks, then native release click-through when Computer Use can target the window
6. **Release evidence** — record exact build artifact, commit SHA, test results and untested gates; do not call it production-ready from local evidence alone

### Open decisions before implementation

| ID | Decision | Default for this candidate |
| --- | --- | --- |
| LG-D01 | Desktop ambient wallpaper/geometry | Use CSS/static ambient layers only; no remote asset or video |
| LG-D02 | Companion placement | In-app panel/sheet first; no OS-level window |
| LG-D03 | Preference persistence | Reuse an approved existing settings contract; otherwise document a separate change request |
| LG-D04 | Canonical glass token owner | Keep `--fung-glass-*` as an addendum until token source-of-truth ADR is approved |
| LG-D05 | Reference packaging | Keep external source hashes in this document; vendor assets only after licensing/provenance review |

## 9. Evidence หลัง implementation ที่ได้รับ approval

Evidence นี้เป็น local engineering evidence เท่านั้น ไม่ใช่ production-readiness หรือ native acceptance:

| Gate | ผล | Evidence |
| --- | --- | --- |
| Visual foundation | PASS | `DesktopShell.tsx`, `LiquidGlass.css`, `contracts.ts` ใช้ `--fung-glass-*`, `data-material` และ `data-transparency` โดยคง baseline contracts |
| Desktop implementation | PASS (local source/build) | Home, Live, Review/Summary shell, settings slots และ bounded in-app Companion ถูกต่อเข้ากับ existing handlers |
| Static validation | PASS | `npm run build`, `npm run design-system:check`, `git diff --check` |
| Contract/regression tests | PASS | `test:desktop-bootstrap`, `test:callmd-contracts`, `test:callmd-shell`, `test:callmd-live`, `test:callmd-integration`, `test:design-system`, `test:release` |
| Browser visual smoke | PASS (scoped) | Vite `http://127.0.0.1:1420/app?surface=desktop` ที่ default viewport 1280×720: Home → Live → Review, rail focus expansion, appearance disclosure และ top-right account CTA ผ่าน; dark/solid/Companion และ additional 1024×768, 768×1024, 390×844 runs: NOT RUN |
| Windows engineering build | PASS | `npx tauri build --no-bundle`; `src-tauri/target/release/fung.exe`, size `25,806,848` bytes, SHA-256 `BED7EFB569BCE7C8BDD4D4716052A3697B7EE41CE50590BB17845E9D8DCEF10E` |
| Native desktop click-through | BLOCKED_ENVIRONMENT | Current Computer Use session cannot bind the elevated exact process; user-session launch exits `101` while crossing the external `..\\.venv-whisper` path. No native pass is claimed |
| Installer / clean install / production | NOT RUN | รอบนี้ใช้ `--no-bundle`; ไม่ใช่ installer, clean-VM หรือ production evidence |

การแก้ stale UI: release executable ที่ path มาตรฐานถูก rebuild แล้ว แต่ native click-through รอบนี้ยังติด environment boundary; registered app `dev.fung.local` ที่ชี้ไป path อื่นไม่ใช้เป็นหลักฐานของ UI ใหม่

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.4.0b | 2026-09-19 | beta | Replaced the legacy desktop presentation with the approved hover rail/profile/glass shell; browser and build evidence pass, native recheck remains environment-blocked | UNCOMMITTED | RWANG |
| 0.3.2b | 2026-09-19 | beta | Recorded bounded native click-through on the exact current executable and retained installer/release limitations | UNCOMMITTED | RWANG |
| 0.3.1b | 2026-09-19 | beta | Recorded approved implementation, local/browser/build evidence, and explicit native/release gaps | 694607f9ecd67bbc2075f4bb1cba3021dc8b0da | RWANG |
| 0.3.0b | 2026-09-19 | candidate | Added Liquid Glass desktop adaptation scope and approval gates; no code implementation | UNCOMMITTED | RWANG |

Approval recorded in the task conversation on 2026-09-19. Code implementation and browser/build evidence are current; native click-through is environment-blocked, and installer, clean-install, production and full accessibility gates remain separate evidence gates.
