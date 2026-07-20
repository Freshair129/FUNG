---
version: "0.3.0b"
created_at: "2026-07-20T20:02:00+07:00,ATHER"
last_update: "2026-07-21T04:18:10+07:00,ATHER"
status: "need review"
superseded_by: null
attributes:
  domain: "mobile-quality"
  doc_type: "uat-report"
  scope: "FUNG Mobile Android physical device"
  language: "Thai"
---

# FUNG Mobile — Android Physical-Device UAT

## Outcome

ผลรวมของรอบทดสอบวันที่ 2026-07-20: **PARTIAL PASS — standalone native capture ผ่าน แต่ audio playback/diarization ยังไม่ครบใน artifact ที่ทดสอบวันนั้น**

Record, Pause, Resume และ Stop ทำงานบนเครื่องจริงแล้ว; journal จบเป็น `completed`, canonical segments ถูก reconcile เข้า project, หน้ายาวเลื่อนได้, dock กลับเป็น 5 รายการ, Timeline/Stories/Processing เปิดจาก contextual flow และ Light/Dark/System ไม่ชน header/FAB อย่างไรก็ตามหลักฐานนี้เป็น historical physical evidence ของ APK ก่อน Genesis cutover เท่านั้น

หลัง UAT รอบดังกล่าว โค้ดได้เพิ่ม GenesisBlockDB cutover และ checksum-verified source-segment playback พร้อม rebuild APK แล้ว แต่ยังไม่ได้ติดตั้งและทดสอบ artifact ใหม่นี้บนเครื่องจริง จึงให้สถานะ playback/Genesis migration เป็น **IMPLEMENTED / RETEST REQUIRED** ไม่ใช่ PASS

## Test Classification

| Item | Value |
| --- | --- |
| Complexity | C-3 — Architecture-Driven |
| Risk | HIGH |
| Date | 2026-07-20 (ICT) |
| Device | Samsung SM-A075F (`R8YY91PX3ZT`) |
| OS | Android 16 |
| Viewport | 720×1600 physical; density 1.875 ≈ 384×853 logical px |
| Font scale | 1.0 |
| App | `dev.fung.local` 0.1.0 (1000), debug APK |
| Theme coverage | System, Light, Dark |
| Interaction method | ADB taps derived from Android UI hierarchy; screenshots used for visual verification |

## Assumptions and Scope Boundary

1. UAT ใช้ข้อมูลเดิมและสร้าง test note หนึ่งรายการชื่อ `UAT_Note`; ไม่ล้าง app data
2. ไม่ส่ง pairing code จริงและไม่เชื่อม Desktop ภายนอก
3. MCP ถูกเปิดเพื่อทดสอบ state แล้วปิดกลับ
4. การทดสอบรอบนี้ครอบคลุมเครื่องจริงหนึ่งรุ่น, portrait, font scale 1.0; compact 360 px, large 430 px, dynamic text, landscape, screen-off และ long-duration capture ยังไม่ผ่านการพิสูจน์

## UAT Matrix

| ID | Flow | Result | Evidence / observation |
| --- | --- | --- | --- |
| UAT-01 | เปิดแอปและ bottom navigation 5 รายการ | PASS | Home, Notes, Voice, Graph, Devices แสดงบรรทัดเดียว; Timeline ย้ายเป็น contextual route |
| UAT-02 | Home visual scaling / scroll | PASS | theme control ไม่อยู่ใน header แล้ว; swipe เลื่อน recent row ให้พ้น dock ได้ |
| UAT-03 | เริ่ม Record เมื่อ permission granted | **PASS** | native `MediaRecorder` เริ่ม, timer/safe offset เพิ่ม, ไม่มี WebView permission dialog ซ้ำ |
| UAT-04 | Pause/Resume/Stop | **PASS** | Pause ค้าง `00:00:24` ตลอดช่วงพักจริงประมาณ 32 วินาที; Resume ไป `00:00:29` โดยไม่รวมเวลาพัก; Stop จบ `completed` |
| UAT-04B | Playback source audio | **IMPLEMENTED / RETEST REQUIRED** | โค้ดใหม่ query ordered segment จาก Genesis, ตรวจ path sandbox + SHA-256 และเล่นต่อ segment; ยังไม่มี physical-device evidence ของ APK ใหม่ |
| UAT-05 | Timeline source + speaker lanes | PARTIAL | recording source/duration และ contextual CTA แสดง; speaker lanes ยังไม่มีจนกว่าจะ import diarization หรือสร้าง turn data |
| UAT-06 | Stories Editor | PASS within metadata scope | เปิดจาก populated Timeline และแสดง editor ได้; audio render/provider ยังเป็น external gate |
| UAT-07 | Processing Studio | PASS within control scope | เปิดและเลื่อนถึง FX/Agent sections ได้; provider execution ยังถูกปิดอย่าง truthful |
| UAT-08 | Notes list | PASS | รายการท้ายและ Desktop Runtime เลื่อนขึ้นมาเหนือ dock ได้ |
| UAT-09 | Create/save typed note | PASS | บันทึก `UAT_Note / physical_device_test` และกลับมาที่ list ได้ |
| UAT-10 | Search note | PASS | ค้น `UAT` แล้วเหลือผล `UAT_Note` ถูกต้อง; keyboard ไม่ทับ sheet |
| UAT-11 | Historical graph propagation | PASS within old prototype | `UAT_Note` ปรากฏเป็น node บน APK เดิม; Genesis transaction/graph cutover ผ่าน automated tests ภายหลัง แต่ต้อง re-run physical create/reopen/graph บน APK ใหม่ |
| UAT-12 | Graph inspector scaling | PASS | shell ใช้ constrained scroll และมี bottom clearance เดียวกับหน้ารายการอื่น |
| UAT-13 | Theme System → Light → Dark → System | PASS | control อยู่ใน Devices/Appearance, target ประมาณ 47.5 dp และ restore เป็น System แล้ว |
| UAT-14 | MCP enable/disable | PASS | เปิดแล้วแสดง local-only `127.0.0.1:44863`; ปิดกลับแล้ว |
| UAT-15 | Desktop pairing modal | PASS within UI scope | modal/input แสดงครบ; ไม่ submit pairing จริง |
| UAT-16 | Scroll to final content | PASS | Home, Notes และ Processing แสดงเนื้อหาท้ายหน้าหลัง swipe โดยไม่ถูก dock บัง |

## Defect Register

| ID | Severity | Defect | User impact | RCA |
| --- | --- | --- | --- | --- |
| MOB-UAT-001 | S1 Blocker | Android Record เรียก WebView `getUserMedia` ก่อน native recorder | **Closed** — native-first และ physical record evidence | RC-1/RC-2 |
| MOB-UAT-002 | S2 Major | root overflow ปิดแต่ child ไม่เป็น constrained scroll container | **Closed** — constrained `100dvh` scroll shell | RC-3 |
| MOB-UAT-003 | S2 Major | global dock มี 6 ช่อง ขัด approved IA 5 ช่อง | **Closed** — 5-item dock + contextual Timeline | RC-4 |
| MOB-UAT-004 | S2 Major | theme toggle fixed ไม่ได้เป็นเจ้าของโดย header | **Closed** — Devices/Appearance owns control | RC-4 |
| MOB-UAT-005 | S2 Major | Timeline empty state ไม่มี actionable Record CTA | **Closed** — empty-state Record CTA + contextual route | peer-flow gap |
| MOB-UAT-006 | S3 Minor | generic capture recovery messageไม่บอก stage/backend/error | **Closed** — structured stage/detail state | RC-2 |
| MOB-UAT-007 | S2 Major | Stop ที่ชน segment rotation เปลี่ยนทั้ง session เป็น recovery | **Closed** — discard เฉพาะ invalid unjournaled tail ต่ำกว่า 1 วินาที | RC-5 |
| MOB-UAT-008 | S2 Major | Resume timer รวมเวลาที่ Pause | **Closed** — pause-adjusted capture clock; unit + device UAT | RC-6 |
| MOB-UAT-009 | S2 Major | Timeline Play ยังไม่มี physical source-audio proof | **Implemented / retest required** | transport gap fixed in code; device evidence pending |

## Scaling Measurements

| Surface | Measured bounds | Logical assessment |
| --- | --- | --- |
| Appearance control | 184×89 physical | ≈98×47.5 dp; ผ่านขั้นต่ำ 44 dp และไม่ชน FAB |
| Notes search input | 523×86 physical | ≈279×45.9 dp; ผ่านขั้นต่ำ 44 dp |
| Bottom dock | 84 CSS px; 5 equal columns | Thai labels แสดงบรรทัดเดียวที่ 384 logical px |
| Notes FAB | 88×88 physical | ≈46.9 dp; target ผ่านขั้นต่ำและไม่มี theme overlap |
| Android insets | cutout/status 64 physical; nav bar 90 physical | safe-area top ทำงานบางส่วน แต่ fixed app dock และ content clearance ยังไม่สัมพันธ์กัน |

## Evidence Artifacts

- `output/uat-android/record-repro.png` / `.xml` — record failure
- `output/uat-android/home.png` / `.xml` — home overlap
- `output/uat-android/notes-scroll-after.png` / `.xml` — clipped, non-scrolling notes
- `output/uat-android/notes-search.png` / `.xml` — search pass
- `output/uat-android/theme-light.png`, `theme-dark.png` — appearance pass
- `output/uat-android/graph-after-note.xml`, `graph-selected.xml` — graph propagation/selection pass
- `output/uat-android/devices-mcp-on.png`, `devices-pair-modal.png` — MCP/pairing UI pass
- `output/uat-android/fix-home-scrolled.png`, `fix-notes-bottom.png`, `fix-processing-scrolled.png` — post-fix scroll proof
- `output/uat-android/fix-theme-light-confirmed.png`, `fix-theme-dark-confirmed.png`, `fix-theme-system-final.xml` — appearance proof
- `output/uat-android/fix-clock-paused-start.png`, `fix-clock-paused-end.png`, `fix-clock-resumed.png`, `fix-clock-completed.png` — pause/resume/stop proof
- Native journal `15f573e4-ef97-4e12-a941-ad017cf2e4c1` — `completed`, safe offset `29,091 ms`, 7 reconciled segments

## Historical Approved-Contract Impact

- รอบแรกละเมิด `R-MOB-001`: record/play flow ไม่ standalone
- รอบแรกละเมิด `R-MOB-002`: native recorder ไม่เริ่มเพราะ WebView เป็น prerequisite
- ไม่ผ่าน technical target `recording start p95 ≤2s`
- ไม่ผ่าน UX minimum touch target และ safe-area/dynamic wrapping intent บางรายการ
- implementation status ที่เคยระบุ Android build/interaction pass ต้องเพิ่ม physical-device failure นี้ก่อนอ้าง readiness ครั้งถัดไป

## Implemented Repair Plan

1. **Capture orchestration** — Android ใช้ `startCapture → startNativeRecorder` ก่อน; เรียก `getUserMedia` เฉพาะ browser/native-unavailable fallback พร้อม stage-specific error
2. **Scrollable shell** — จำกัด app/screen ที่ `100dvh`, ทำ `.m-screen` เป็น scroll container จริง, เพิ่ม navigation clearance และ `scroll-padding-bottom`
3. **Navigation IA** — กลับเป็น stable 5-item dock; เปิด Timeline แบบ contextual จาก recording/note detail ตาม approved proposal
4. **Header ownership** — ย้าย theme control เข้าพื้นที่ header/action ที่จองไว้หรือ Appearance surface; target อย่างน้อย 44 dp
5. **Empty-state recovery** — เพิ่มปุ่ม `เริ่มบันทึก` ที่ Timeline empty state และคงคำอธิบายว่า Story/Processing ต้องมี source audio
6. **Regression gates** — automated ordering/clock tests และ build APK ผ่าน; install ทับของเดิมและ UAT ซ้ำ record→pause→resume→stop→playback→reopen บนเครื่องจริงยัง pending

## Acceptance / Success / Exit Criteria

ถือว่ารอบแก้ผ่านเมื่อ:

- MOB-UAT-001 ถึง MOB-UAT-008 ปิดด้วยหลักฐานบนเครื่องจริง
- Record เริ่มภายใน 2 วินาที, service/journal/safe-offset มีหลักฐาน, pause/resume/stop ผ่าน
- Timeline, Stories และ Processing ไม่ติด dead end จาก record failure; source-audio playback ต้องผ่าน physical retest ก่อนปิด phase
- Home/Notes/Graph/Devices เลื่อนถึงเนื้อหาสุดท้ายเหนือ dock ได้
- ไม่มี header/FAB/theme overlap และ dock label ไม่ wrap ที่ 384 logical px
- Notes create/search, Graph, Theme, MCP และ Pairing regression ผ่าน
- `npm` tests/build, Rust tests, Android build/install และ physical UAT ผ่าน
- เอกสาร Implementation Status และ version diff ถูกอัปเดตตามผลจริง

Current exit: automated gates and signed debug artifact pass; physical installation/retest of the Genesis-enabled APK is still required, so this UAT remains `need review`.

## Version Diff

### `0.0.0` → `0.1.0b`

- เพิ่ม UAT matrix จาก Samsung Android 16 เครื่องจริง
- บันทึก blocker ของ Record, scaling/scroll/nav defects, downstream blocked flows และ remediation gate

### `0.1.0b` → `0.2.0b`

- บันทึกผลหลังแก้ native-first capture, scroll shell, 5-item navigation, appearance ownership, short-tail stop และ pause-adjusted timer
- เพิ่ม physical-device evidence ของ completed journal และแยก source playback/diarization ที่ยังไม่ operational ออกจาก UI ที่เปิดได้

### `0.2.0b` → `0.2.1b`

- แก้คำอ้าง Graph UAT ให้เป็น transitional FUNG-owned SQLite graph-table behavior และห้ามใช้เป็นหลักฐาน GenesisBlockDB integration

### `0.2.1b` → `0.3.0b`

- แยก physical evidence ของ APK เดิมออกจาก implementation/build evidence ของ APK Genesis-enabled ชุดใหม่
- อัปเดต playback จาก not implemented เป็น implemented/retest required โดยไม่ยกเป็น PASS ก่อนทดสอบบนเครื่องจริง
- เพิ่ม requirement ให้ re-run migration, reopen, Graph และ source playback บนอุปกรณ์ Android ที่เชื่อมต่อ

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-20 | candidate | Initial Android physical-device UAT and remediation proposal | N/A — no commit created | ATHER |
| 0.2.0b | 2026-07-20 | need review | Post-remediation Android physical UAT with remaining playback/diarization boundaries | N/A — no commit created | ATHER |
| 0.2.1b | 2026-07-20 | need review | Corrected graph evidence boundary against the GenesisBlockDB unified spec | N/A — no commit created | ATHER |
| 0.3.0b | 2026-07-21 | need review | Genesis cutover/playback implementation delta recorded; fresh physical-device retest pending | N/A — no commit created | ATHER |
