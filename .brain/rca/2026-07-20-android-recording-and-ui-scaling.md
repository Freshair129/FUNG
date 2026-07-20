---
version: "0.1.2b"
created_at: "2026-07-20T20:02:00+07:00,ATHER"
last_update: "2026-07-20T20:48:00+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "mobile-runtime"
  doc_type: "root-cause-analysis"
  scope: "FUNG Mobile Android physical-device UAT"
  language: "Thai"
---

# RCA — Android บันทึกเสียงไม่เริ่มและเนื้อหา UI ถูกบัง

## Classification

| Item | Value |
| --- | --- |
| Complexity | C-3 — Architecture-Driven |
| Change risk | HIGH |
| Primary severity | S1 / Blocker — standalone recording ใช้งานไม่ได้ |
| Secondary severity | S2 / Major — เนื้อหาและปุ่มบางส่วนถูก bottom navigation บังและเลื่อนไม่ได้ |
| Device | Samsung SM-A075F, Android 16, 720×1600, density 300 (1.875), font scale 1.0 |
| App | `dev.fung.local` 0.1.0 (1000) |

## Symptom

1. แตะ `เริ่มบันทึก` แล้วตัวจับเวลาอยู่ที่ `00:00:00` และ native recorder ไม่เริ่มทำงาน แม้ Android แสดงว่าอนุญาตไมโครโฟนแล้ว
2. เมื่อไม่มี recording จริง หน้า Timeline อยู่ใน empty state ทำให้ Stories Editor และ Processing Studio เข้าใช้งานไม่ได้ตามลำดับ dependency
3. หน้า Home, Notes และ Graph มีเนื้อหาด้านล่างอยู่ใต้ fixed bottom navigation; การปัดขึ้นไม่เปลี่ยนตำแหน่งเนื้อหา
4. แถบธีมชนกับ status/header action และปุ่มสร้างโน้ต; bottom navigation หกช่องทำให้ข้อความภาษาไทยตัดสองบรรทัดบนความกว้าง 384 logical px

## Evidence

### E1 — Android permission กับ WebView capture ไม่ตรงกัน

- `dumpsys package dev.fung.local` รายงาน `android.permission.RECORD_AUDIO: granted=true` และ `POST_NOTIFICATIONS: granted=false`
- เมื่อแตะ Record เวลา `19:52:28` logcat รายงาน `cr_media: Requires MODIFY_AUDIO_SETTINGS and RECORD_AUDIO. No audio device will be available for recording`
- ทันทีหลังจากนั้น Android เปิด `GrantPermissionsActivity` จาก UID ของ `dev.fung.local`
- ไม่มี log ที่แสดงว่า `FungRecorderService` เริ่ม และไม่มี `files/native-recordings` ใน app-private storage หลัง reproduce
- ภาพหลังแตะยังคง `00:00:00`: `output/uat-android/record-repro.png` (SHA-256 `B6A2C49DA2C6B9EEFEA14639A34D05E24ACDF28238D1ADAE9501A96BF5575817`)

### E2 — ลำดับคำสั่งใน UI ขัดกับ native-ownership contract

`src/mobile/MobileApp.tsx` เรียกตามลำดับนี้ใน `CaptureScreen.begin()`:

1. `navigator.mediaDevices.getUserMedia(...)`
2. `startCapture(...)`
3. `startNativeRecorder(...)`

ดังนั้น WebView media capture ต้องสำเร็จก่อน native recorder เสมอ ทั้งที่ `docs/Mobile/TECHNICAL_DESIGN.md` กำหนดว่า native service เป็นเจ้าของ long-running capture และ WebView ต้องไม่เป็น dependency ของ durable recording

### E3 — Scroll container ไม่เกิดขึ้นจริง

- `.m-app` ใช้ `min-height: 100dvh` และ `overflow: hidden`
- `.m-screen` ใช้ `min-height: 100dvh` กับ `overflow-y: auto` แต่ไม่มีความสูงที่จำกัด จึงขยายตามเนื้อหาแทนที่จะเกิด internal scroll container
- `.m-bottom-nav` เป็น `position: fixed`, สูง 84 CSS px และมี 6 คอลัมน์
- หลังปัดจาก `(360,1220)` ไป `(360,520)` bounds ของแถว `UAT_Note` ยังเท่าเดิม `[45,453][676,656]`; เนื้อหาไม่เลื่อน
- ภาพ `output/uat-android/notes-scroll-after.png` แสดงแถว `Desktop Runtime` ถูกแถบล่างบัง (SHA-256 `006AB54BAA952D111DAFF0E934E732DE31D3DA37BDE5281DB0A04933E484A1A9`)

### E4 — Information architecture drift

- `docs/Mobile/PRODUCT_UX_SPEC.md` กำหนด stable command dock 5 รายการ: Home, Notes, Voice, Graph, Devices
- `docs/Mobile/SPEAKER_TIMELINE_PROPOSAL.md` กำหนดให้เปิด Timeline จาก recording หรือ note detail
- implementation เพิ่ม Timeline เป็นช่องที่ 4 ใน fixed navigation ทำให้ dock เป็น 6 ช่อง และลดความกว้างแต่ละปุ่มเหลือประมาณ 56 logical px จนป้ายภาษาไทยตัดบรรทัด

## Root Cause

### RC-1 — Record blocker

Android native path ถูกวางหลัง WebView `getUserMedia()`. บนเครื่องจริง WebView media layer ไม่สร้าง audio device และเปิด permission flow ซ้ำ แม้ native `RECORD_AUDIO` จะอนุญาตแล้ว ข้อผิดพลาดจึงออกจาก `try` ก่อน `startCapture()` และ `startNativeRecorder()` ทำให้ `FungRecorderService` ไม่เคยเริ่ม

### RC-2 — Error diagnosis ถูกกลบ

catch path แปลงข้อผิดพลาดทุกชนิดเป็น `recovery_required` โดยไม่เก็บ error code/source ที่มองเห็นได้ จึงทำให้ UI สื่อว่าเป็นปัญหาสิทธิ์ทั่วไป ทั้งที่ failure เกิดก่อน native recorder และมี WebView-specific cause

### RC-3 — Content clipping

root container ปิด overflow ขณะที่ child ไม่มี constrained height; fixed navigation จึงทับเนื้อหาที่เกิน viewport และผู้ใช้ไม่สามารถเลื่อนเนื้อหาขึ้นเหนือ navigation ได้

### RC-4 — Header/nav collision

theme toggle ถูกวาง fixed โดยไม่จองพื้นที่ใน header และ Timeline ถูกเพิ่มเป็น global navigation item แม้เอกสารกำหนดเป็น contextual surface จึงเกิด geometric overlap และ label wrapping บนจอ compact

### RC-5 — Stop ตรงรอยต่อ segment ถูกตีความเป็น recovery

หลังแก้ native-first แล้ว physical UAT ยืนยันว่า recorder เริ่มและหมุน segment ทุก 5 วินาทีได้ แต่ Stop ที่มาถึงประมาณ 90 ms หลังเปิด segment ใหม่ทำให้ `MediaRecorder.stop()` โยน `RuntimeException` เพราะ segment สั้นเกินกว่าจะมีข้อมูลที่ valid. `onStartCommand` จึงเขียน journal เป็น `recovery_required` และ `onDestroy` ยืนยันสถานะนั้นซ้ำ แม้ segment ก่อนหน้าทั้งหมดปลอดภัยแล้ว

การแก้ที่อนุมัติอยู่ในขอบเขต stop/finalize gate: หาก stop failure เกิดกับ tail ที่อายุต่ำกว่า 1 วินาที ให้ทิ้งเฉพาะไฟล์ tail ที่ยังไม่เคยเข้า journal แล้ว finalize จาก safe segments เดิม; หาก failure เกิดหลัง 1 วินาทีต้องคง `recovery_required` เพื่อไม่กลบ hardware/storage fault

### RC-6 — ตัวจับเวลาหลัง Resume รวมเวลาที่ Pause

physical UAT รอบ Pause/Resume แสดงว่า UI ค้างที่ `00:00:15` ระหว่าง Pause ตามที่คาด แต่ทันทีที่ Resume กลับกระโดดเป็น `00:00:32` แม้หยุดพักประมาณ 17 วินาทีและ native `safeOffsetMs` ยังสะท้อนเฉพาะเสียงที่บันทึกจริง สาเหตุคือ `CaptureState` เก็บเพียง `startedAt` และ effect คำนวณ `Date.now() - startedAt`; การ Pause หยุด interval แต่ไม่ได้เก็บเวลาเริ่มพักหรือเลื่อนฐานเวลา เมื่อ Resume interval จึงรวมช่วงพักทั้งหมดกลับเข้ามา

การแก้จำกัดอยู่ที่ capture clock state: เพิ่ม `pausedAt`, บันทึกเวลาขณะ Pause และเลื่อน `startedAt` ไปข้างหน้าตามระยะพักเมื่อ Resume ทั้ง native และ Web MediaRecorder โดยไม่เปลี่ยน native journal หรือ safe-offset contract

## Why the Issues Escaped Detection

1. หลักฐานเดิมเป็น browser interaction ที่ 393×852 และ APK build/signature; ยังไม่ได้รัน physical-device record-start gate
2. browser preview ใช้ MediaRecorder fallback จึงไม่พิสูจน์ว่า Android native-first path ทำงาน
3. visual checks เดิมยืนยันภาพแรกแต่ไม่ได้ตรวจ scrollability, hit-region overlap และ Thai wrapping จาก UI hierarchy บน WebView จริง
4. implementation status ระบุ real-device lifecycle เป็น external gate แต่ APK ถูกติดตั้งก่อนปิด minimum record-start UAT

## Proposed Prevention

1. แยก Android native-first branch ออกจาก browser fallback และเพิ่ม test ที่ fail หาก Android เรียก `getUserMedia()` ก่อน native recorder
2. เพิ่ม structured capture error (`stage`, `backend`, `code`, recoverability) และ log แบบไม่บันทึกข้อมูลเสียง/ข้อความส่วนตัว
3. เพิ่ม physical-device smoke gate: permission granted → start ≤2 วินาที → service/journal exists → pause/resume → stop → playback/timeline source visible
4. เพิ่ม layout gate ที่ 360/390/430 logical px, Light/Dark/System, Thai labels, font scale 1.0/1.3 พร้อมตรวจ scroll-to-last-item และ element overlap
5. เพิ่ม IA contract test ให้ global dock มี 5 destinations และ Timeline เปิดแบบ contextual ตามเอกสารที่อนุมัติ

## Proposed Repair Boundary

ยังไม่แก้ code ใน RCA นี้ รออนุมัติเอกสารก่อน โดย repair ที่เสนอจำกัดอยู่ที่:

- native-first capture orchestration และ truthful error state
- constrained scroll shell + safe-area/navigation clearance
- ย้าย Timeline ออกจาก global 6-column dock ไป contextual entry
- ย้าย theme control เข้า header-owned action หรือ Devices/Appearance surface ที่มี touch target ≥44 dp
- UAT ซ้ำบนเครื่องเดิมโดยไม่ล้างข้อมูลผู้ใช้

## Acceptance / Success / Exit Criteria

- Record เริ่ม native service ภายใน 2 วินาทีเมื่อ permission granted โดยไม่เปิด permission dialog ซ้ำ
- มี native segment journal และ safe offset เพิ่มขึ้นจริง จากนั้น pause/resume/stop สำเร็จ
- source audio เปิดเล่นได้ และ Timeline ไม่ถูกบล็อกด้วย record-start failure
- ทุกหน้าที่รายการยาวเลื่อนไปถึงรายการสุดท้ายเหนือ bottom navigation ได้
- ไม่มี bounds overlap ระหว่าง theme/header actions และไม่มีข้อความ dock ตัดสองบรรทัดที่ 384 logical px
- Light/Dark/System, note search/save, graph selection, MCP toggle และ pairing modal ยังผ่าน
- automated tests, production build, Android APK build/install และ physical-device regression ผ่านโดยไม่มี known S1/S2 regression

## Version Diff

### `0.0.0` → `0.1.0b`

- เพิ่ม RCA จาก physical-device UAT สำหรับ record blocker, scroll clipping, header collision และ navigation drift
- ระบุ evidence, root cause, escape path, prevention และ repair acceptance gate

### `0.1.0b` → `0.1.1b`

- เพิ่ม physical-device RCA สำหรับ Stop ที่ชน segment rotation และกำหนด bounded discard เฉพาะ unjournaled tail ต่ำกว่า 1 วินาที

### `0.1.1b` → `0.1.2b`

- เพิ่ม physical-device RCA สำหรับ elapsed timer ที่รวมเวลาช่วง Pause และกำหนดฐานเวลาที่ชดเชย pause duration

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-07-20 | candidate | Initial Android physical-device RCA | N/A — no commit created | ATHER |
| 0.1.1b | 2026-07-20 | candidate | Added short-tail stop/rotation RCA from post-fix UAT | N/A — no commit created | ATHER |
| 0.1.2b | 2026-07-20 | candidate | Added pause/resume elapsed-clock RCA from post-fix UAT | N/A — no commit created | ATHER |
