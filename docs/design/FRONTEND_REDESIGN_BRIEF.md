---
version: "1.0.1"
created_at: "2026-09-16T18:30:00+07:00,Claude"
status: "brief"
attributes:
  domain: "frontend-redesign"
  doc_type: "design-brief"
  scope: "FUNG — all user-facing surfaces (desktop, mobile, web, landing, phone page)"
  language: "Thai/English"
---

# FUNG — Frontend Redesign Brief

Scope correction (2026-09-17): Google Drive is canceled. The redesign covers
local encrypted backup/restore and must not add Drive controls, Drive OAuth
copy, Drive commands, or Drive deployment assumptions.

เอกสารชุดเดียวสำหรับทีมออกแบบ UI ใหม่: ผลิตภัณฑ์คืออะไร, ฟีเจอร์ที่มีจริง (และที่ยังไม่มี), หน้าจอ/สถานะที่ต้องรองรับ, โลโก้และ token ที่ใช้อยู่, API ที่ frontend ใหม่ต้องเรียก, และข้อจำกัดที่ทดสอบบังคับไว้ ทุกข้อชี้ไฟล์ต้นทางในโค้ด (`file:line`) ณ `main` วันที่ 16 ก.ย. 2026 (เวอร์ชันแอป 0.1.1)

## 0. ลิงก์และไฟล์ที่ต้องมีติดมือ

| สิ่งที่ต้องดู | ที่อยู่ |
| --- | --- |
| Landing page (production) | https://fung-seven.vercel.app |
| Web dashboard (production, ต้อง Google sign-in) | https://fung-seven.vercel.app/app |
| Repo | https://github.com/Freshair129/FUNG |
| โลโก้หลัก (SVG, `currentColor`) | `docs/brand-kit/logo/fung-mark.svg` |
| ชุด mark "Quiet Archive" (ink / white / app-icon) | `docs/Mobile/design-system/assets/quiet-archive-mark.svg`, `-ink.svg`, `-white.svg`, `quiet-archive-app-icon.svg` |
| Concept sheet โลโก้ | `docs/brand-kit/logo/Quiet Archive.png` |
| Brand tokens ของจริง | `docs/brand-kit/tokens.css`, `docs/brand-kit/README.md` |
| โลโก้ที่ render ในแอปตอนนี้ (React) | `src/components/FungLogo.tsx` |
| ไอคอนแอป (Windows/Android) | `src-tauri/icons/` — **ยังเป็นตัวอักษร "F" placeholder** ไม่ใช่ mark |
| Favicon เว็บ | `index.html:14-17` — inline SVG คนละแบบกับ FungLogo |
| Mobile design system (source of truth + generator) | `docs/Mobile/DESIGN_SYSTEM.md`, `npm run design-system:render` |
| Desktop wireframes P1–P4 | `docs/Desktop/wireframes/` |

## 1. ผลิตภัณฑ์ใน 1 หน้า

FUNG คือแอปบันทึกการประชุม/เสียงแบบ **local-first**: เสียง, transcript, โน้ต, กราฟความรู้ อยู่ในเครื่องผู้ใช้ (GenesisBlockDB ฝังใน desktop) ไม่ขึ้น cloud เว้นแต่ผู้ใช้เลือกเอง Cloud (Supabase) ใช้แค่ auth + metadata ของอุปกรณ์ Tagline: **"Local by default. Connected by choice."** / "ฟังสิ่งที่ไม่ได้พูด"

5 surface ที่ frontend ใหม่ต้องรองรับ (ทั้งหมด boot จาก `index.html` → `src/main.tsx` → `src/lib/bootstrap.ts` เลือกเส้นทาง):

| Surface | เมื่อไร | Component ราก | หมายเหตุ |
| --- | --- | --- | --- |
| **Desktop** (Windows, Tauri) | รันในแอป desktop | `src/App.tsx` | หน้าต่าง 1280×800 **ไม่ resize** ไม่มี OS decorations, stage คงที่ 1304×744 แล้ว scale ด้วย CSS transform |
| **Mobile** (Android, Tauri) | รันในแอปมือถือ | `src/mobile/MobileApp.tsx` | `max-width 520px`, bottom nav 5 ช่อง, dark/light/system |
| **Web dashboard** | เบราว์เซอร์ `/app` (ต้อง login) | `src/web/Dashboard.tsx` | คุยกับ desktop เครื่องเดียวกันผ่าน loopback API |
| **Landing** | เบราว์เซอร์ path อื่น ๆ | `src/landing/LandingPage.tsx` | scroll-narrative 5 sections |
| **Phone page** (เสิร์ฟจาก desktop) | มือถือเปิด `http://<desktop>:PORT/#TOKEN` | `src-tauri/assets/local_recordings.html` | HTML ไฟล์เดียว ไม่มี build, ไม่มี external fetch |

## 2. Brand และ visual ที่ใช้อยู่

**Identity:** "Quiet Archive" — แผ่นพอร์ซเลนม้วนเป็นช่อง อ่านเป็นตัว O (เปิด/ฟัง) และ archive (เก็บ/พับ/ส่วนตัว) เลือกแล้วเป็น beta identity (`docs/Mobile/QUIET_ARCHIVE_IDENTITY_SELECTION.md`)

**กฎโลโก้** (`docs/brand-kit/README.md`): mark ใช้ `currentColor` — ink `#171918` บนพื้นสว่าง, porcelain `#FAF8F3` บนพื้นมืด; ไม่ใส่กล่องพื้นหลัง (ยกเว้น OS app icon บน tile ink); clearspace ≥ 25% ของความกว้าง; ขั้นต่ำ 24px ใน UI / 16px favicon; ห้ามย้อมสีอื่น/ยืด/บิด Wordmark `FUNG` DM Sans Medium, uppercase, tracking `0.34em`, sage บนสว่าง / porcelain บนมืด

**Brand tokens** (`docs/brand-kit/tokens.css`):

| Token | Hex | บทบาท |
| --- | --- | --- |
| porcelain | `#FAF8F3` | พื้น/วัสดุหลัก |
| ink | `#171918` | ตัวอักษร, พื้นมืด, tile ไอคอน |
| sage | `#6F897E` | accent, wordmark, สถานะ local/ยืนยัน |
| slate | `#4A5B8B` | interactive: ปุ่มหลัก, ลิงก์, focus |
| metal | `#9A8260` | caution / pending (semantic) |
| clay | `#B0553F` | error / destructive เท่านั้น |

Type: Fraunces (display), **IBM Plex Sans Thai** (UI ไทย นำ), DM Sans (Latin + wordmark), IBM Plex Mono (ตัวเลข/timer/token) · Spacing ฐาน 4px (4·8·12·16·24·32) · Radius 8/12/16/pill · Elevation: soft shadow (mobile/web), beveled porcelain (desktop)

**ปัญหาที่ redesign ควรแก้ (ข้อเท็จจริงจากโค้ด):**
- 4 stylesheet ไม่แชร์ token กัน และ **ค่าสีไม่ตรงกัน**: sage desktop `#6e897d` / landing `#71847c` / brand `#6f897e`; porcelain desktop `#f4f1ea` / web+landing `#f5f2eb` / brand `#faf8f3`; **mobile ทิ้ง palette ไปใช้ navy `#28374c` + ส้ม `#fe6a3c`** (ตัวแปรชื่อ `--m-indigo` แต่ค่าเป็นส้ม) — `src/styles.css`, `src/mobile/mobile.css`, `src/web/Dashboard.css`, `src/landing/landing.css`
- Desktop ไม่ประกาศฟอนต์ไทยเลย (`styles.css:3-9` เป็น SF Pro/Segoe) ทั้งที่ copy เป็นไทย
- ไอคอนแอปเป็นตัว "F", favicon เป็น SVG อีกแบบ, `FungLogo.tsx` เป็น re-creation 200×200 อีกแบบ — มี 3 โลโก้ที่ไม่เหมือนกันในโปรดักต์
- Theme: desktop toggle light/dark **ไม่ persist** (รีเซ็ตทุกครั้งที่เปิด); web/landing/phone page มีแค่ light
- Icon library เดียว: `lucide-react` (~70 ไอคอน, ขนาด 12–61px)

## 3. Feature spec (ของจริง vs ยังไม่มี)

สถานะ: ✅ ใช้ได้จริง · 🟡 ทำงานแต่มีข้อจำกัด/ต้องพึ่ง desktop · ⛔ ปุ่มมีแต่ปิดไว้พร้อมเหตุผล · 🧪 UI ยังใช้ fixture

| โดเมน | ฟีเจอร์ | Surface | สถานะ | คำสั่ง/API เบื้องหลัง |
| --- | --- | --- | --- | --- |
| Capture | Live meeting: ไมค์ + เสียงระบบ, ถอดสด, topic ทุก ~45 s, ถาม-ตอบจากคลัง | Desktop `LiveMeetingPanel` | ✅ | `live_meeting_start/stop/status`, events `live-status/segment/topic/summary`, `meeting_ask` |
| | อัดเสียง native บน Android (segment, level meter) + fallback MediaRecorder | Mobile Capture | ✅ | `mobile_native_recorder_*`, `mobile_capture_*` |
| | อัดเสียงในเบราว์เซอร์ → IndexedDB → list/เล่น/ดาวน์โหลด/ลบ | Web tile "เริ่มบันทึก" | ✅ | `useWebRecorder`, `webRecordings.ts` |
| | กู้คืนการอัดที่ค้าง | Desktop `RecoveryNotice` | ✅ | `recovery_scan/recover` |
| Import | เลือกไฟล์เสียง/วิดีโอแล้วถอด | Desktop | ✅ | `import_and_transcribe` (wav mp3 m4a mp4 mov mkv webm ogg flac) |
| | ดึงจาก URL (yt-dlp) ต้องกดยินยอมก่อน | Desktop Settings › Fetch | 🟡 | `media_fetch_status/consent_set`, `fetch_and_transcribe` |
| | นำเข้า Zoom cloud recording | Desktop Settings › Zoom | 🟡 ต้อง `FUNG_ZOOM_CLIENT_ID` | `zoom_*` |
| | ส่งไฟล์จากเว็บไปถอดที่ desktop | Web | ✅ (เครื่องเดียวกัน) | `POST /recordings/import` → `/jobs/{id}` → `/recordings/{id}/transcript` |
| Transcription | Whisper CPU/GPU (faster-whisper) | Desktop | ✅ | job `transcript.transcribe`/`transcript.retry` |
| | Diarization (แยกผู้พูด) | Desktop P2, Mobile Timeline | 🟡 ต้อง runtime/model | `diarization_status`, job `speakers.diarize`, `mobile_diarization_*` |
| | Mobile ส่งงานถอดให้ desktop (FUNGWIRE LAN) หรือ cloud ผ่าน desktop | Mobile ProcessingStudio | 🟡 ต้องจับคู่ก่อน | `fungwire_delegate_transcription`, `fungwire_job_poll` |
| Intelligence | สรุปหลังประชุม 3 แบบ (ภาพรวม/timeline/decisions) ผ่าน Ollama | Desktop | ✅ | `generate_meeting_summary`, `meeting_summaries` |
| | กราฟความรู้ | Desktop retry, Mobile Graph | ✅ | `graph_build_start`, `mobile_graph_query` |
| | โน้ต | Mobile Notes | ✅ | `mobile_note_upsert` |
| | TTS อ่านสรุป (BYOM provider) | Desktop P3 + Settings › TTS | 🟡 ต้องตั้ง provider | `tts_*` |
| External tools | MCP stdio connector: preview → อนุมัติ → รัน, grant ≤ 15 นาที | Desktop Live panel | 🟡 flag `VITE_FUNG_EXTERNAL_MEETING_TOOLS=1` | `external_connector_*`, `meeting_tool_*` |
| Backup | สำรอง/กู้คืนไฟล์ในเครื่อง เข้ารหัส + รหัสกู้คืน 24 คำ | Desktop Settings | ✅ | `backup_*`, `filesystem_backup_select_root` |
| Pairing | จับคู่ desktop↔mobile ด้วยรหัส 6 หลัก (TTL 5 นาที, ผิดได้ 5 ครั้ง) | Desktop `DevicePairingPanel`, Mobile Devices | ✅ | `broker_pairing_*`, RPC `confirm_pairing` |
| | FUNGWIRE server บน LAN | Desktop toggle | ✅ | `broker_fungwire_status/set_enabled` |
| Local API | เว็บ/มือถืออ่าน-เล่นไฟล์ของ desktop (loopback, LAN opt-in, หรือ USB `adb reverse`) | Desktop Settings › Runtime, Web, Phone page | ✅ | `start_local_api`, `set_local_api_lan` |
| Auth | Google ผ่าน Supabase — 3 flow ต่าง surface | ทุก surface | ✅ | ดู §5.5 |
| Cloud providers | คีย์ Anthropic/OpenAI/custom, ลำดับ tier, ขีดจำกัดต่อวัน (default ปิด) | Desktop Settings › Cloud, Mobile อ่านอย่างเดียว | ✅ | `cloud_config_*`, `tier_policy_*` |
| Jobs | คิวงาน 5 ชนิด + ยกเลิก | Desktop | ✅ | `create_job`, `cancel_job`, `list_jobs` |
| Export | `.srt` / `.vtt` / สรุป `.txt` | Desktop | ✅ | job `export.render`, `list_export_artifacts` |
| Story/Creative | ตัด/ย้าย/แยกคลิป, effect chain, agent voice | Mobile StoryEditor/ProcessingStudio | 🧪 บางส่วน fixture | `mobile_story_*`, `mobile_effect_chain_update` |
| Mobile MCP server | MCP บนมือถือ (ปิด default) | Mobile Devices | ✅ toggle | `mobile_mcp_set_enabled` |

**ปุ่มที่มีอยู่แต่ปิดพร้อมเหตุผล** (desktop tiles, `src/lib/jobActions.ts:53-60`) — redesign ต้องเลือกว่าจะซ่อนหรือคงเป็น roadmap:
`capture.marker` "ยังไม่มีที่เก็บ marker", `speakers.lock` "ยังไม่มีการยืนยันผู้พูดแบบถาวร", `review.evidence` "ยังไม่มีการทำเครื่องหมายหลักฐาน", `summary.compare` "ยังเทียบสรุปข้ามครั้งไม่ได้", `export.queue` "ยังไม่มีคิวส่งออกแยก", `archive.project` "ใช้แผงสำรองข้อมูลแทน" · ปุ่ม Search บน topbar **ไม่มี handler** · ปุ่ม Play บน rail **disabled ถาวร** ("No local playback in this desktop build") · VU meter บน rail **ตั้งใจ inactive** (ไม่มี level source) · Notes filter chips บนมือถือ **inert**

## 4. หน้าจอและสถานะที่ต้องรองรับ

### 4.1 Desktop (`src/App.tsx`, 1549 บรรทัด, ไม่มี router)

**Home** (`src/components/HomeScreen.tsx`): wordmark, CTA หลัก `เริ่มบันทึกประชุม`, รอง `นำเข้าไฟล์เสียง`, รายการ `การประชุมล่าสุด` (สูงสุด 5, สถานะ Live/Queued/Saved คำนวณจาก job), empty `ยังไม่มีการประชุมที่บันทึกไว้`

**Meeting workspace** — 4 anchor (`App.tsx:86-98`): P1 Capture · P2 Transcript (default) · P3 Summary · P4 Runtime แต่ละ anchor มี 3 focus tile (eyebrow/title/detail/action/status/tone sage|indigo|metal) + zone: score header, stats bar (4 pill), focus workbench, agent card, sector log (Activity + events), signals (4 การ์ด toggle) Label nav/tile เป็น **อังกฤษ** ทั้งที่ body เป็นไทย

Chrome คงที่: topbar (Search, segmented nav, Home, theme toggle, `New`), **InstrumentRail** ซ้าย (VU meter, Record, Import, Play, Export, Pair device, Settings), power dock (`พับจอ` / `ปิด`)

Overlay: `RecoveryNotice`, `SettingsPanel` (7 tab: External Connections, Sign In and Backup, TTS Providers, Cloud Providers, Fetch from URL, Zoom import, Runtime), `LiveMeetingPanel`, `DevicePairingPanel`

**LiveMeetingPanel** (หน้าจอสำคัญสุด): phase `พร้อมเริ่มประชุม / กำลังเริ่ม... / กำลังฟังอยู่ / อัดต่อเนื่อง (ถอดสดมีปัญหา) / กำลังปิดเซสชัน... / จบการประชุมแล้ว / เกิดข้อผิดพลาด`; ปุ่ม `● เริ่มประชุม` / `■ จบประชุม`; option ก่อนเริ่ม: checkbox จับเสียงระบบ, เลือกภาษา (auto/ไทย/อังกฤษ), consent line; feed `Transcript สด` (cap 200 segment, หน่วง ~10-20 s); การ์ด topic; การ์ดถาม FUNG (คำตอบ + แหล่งอ้างอิง [n]); สรุปหลังประชุม 3 ส่วน + `ลองสรุปใหม่`; ExternalMeetingToolsPanel (flag)

**Panel อื่นที่มี copy/สถานะเฉพาะ**: BackupPanel (phrase 24 คำแสดงครั้งเดียว), GoogleDrivePanel, CloudProvidersPanel (5 slot คงที่, daily cap), TtsProviderPanel (3 runtime type), MediaFetchPanel (3 สถานะแยกด้วย `blockerCode`), ZoomPanel, ExternalAccountPanel (**อังกฤษล้วน**, ปุ่ม disabled ถาวร), DevicePairingPanel (รหัส 6 หลัก + นับถอยหลัง, FUNGWIRE switch), AccountLoginPanel (ชื่ออุปกรณ์, สถานะรออนุมัติ)

รายละเอียด copy ทุกปุ่ม/สถานะดู `docs/UI_INTERFACE_INVENTORY.md` และไฟล์ component โดยตรง

### 4.2 Mobile (`src/mobile/MobileApp.tsx`, 1215 บรรทัด)

Bottom nav 5 ช่อง: `หน้าหลัก` · `ไฟล์` · **`พูด`** (ปุ่มกลมยกขึ้น 74px) · `โน้ต` · `อุปกรณ์` + 2 หน้าไม่มีช่อง nav: Timeline (จากการ์ดอัดเสร็จ/โน้ต), Graph (จาก header โน้ต)

| หน้า | หน้าที่ | สถานะสำคัญ |
| --- | --- | --- |
| Home | ปุ่ม orbit `กดค้างเพื่อพูด`, `เริ่มบันทึก`, `สร้างโน้ต`, `งานล่าสุด` | chip `พร้อมใช้งานบนมือถือ`; empty |
| Files | รายการไฟล์ในเครื่อง + player inline (waveform จาก peaks จริง, playhead) | loading/error/empty; ไฟล์ยังไม่เสร็จ disabled |
| Capture | timer HH:MM:SS, `LiveWaveform` จาก level จริง, pause/resume, `หยุดและบันทึก` | `พร้อมบันทึกบนอุปกรณ์` / `กำลังบันทึกบนอุปกรณ์` / `หยุดชั่วคราว`; `บันทึกปลอดภัยถึง {clock}`; 5 error stage |
| Notes | ค้นหา, filter chip (inert), sheet สร้างโน้ต, detail มี `หลักฐานต้นทาง` + `สถานะความรู้` | |
| Graph | node/edge legend `ยืนยันแล้ว / อนุมาน / ขัดแย้ง / ข้อเสนอจากระบบ` | |
| Devices | บัญชี (avatar/ชื่อ/อีเมล/ออกจากระบบ), มือถือเครื่องนี้ (ลงทะเบียน), Desktop ที่เชื่อถือ, ระดับคลาวด์ของ Desktop (อ่านอย่างเดียว), รูปแบบหน้าจอ, MCP, sheet จับคู่ (รหัส 6 หลัก) | trust chip `จับคู่แล้ว / ไม่ตอบสนอง / ถูกยกเลิก / ยังไม่จับคู่` |
| Timeline | speaker turns, zoom 1×–8×, `เปลี่ยนชื่อ/แยกช่วง/รวมผู้พูด/ยืนยัน` | 🧪 มี `previewData()` fixture เมื่อไม่มีข้อมูลจริง |
| StoryEditor / ProcessingStudio | ตัดต่อคลิป; 4 tab ถอดเสียง/ปรับข้อความ/เอฟเฟกต์/เสียง Agent, ปุ่มส่งงานให้ desktop/cloud | 🧪 model list hard-coded; inline style สีมืดไม่ตาม theme (`CreativeStudio.tsx:410-427`) |

Landscape mode: nav กลายเป็น rail ซ้าย 68px (`mobile.css:404+`)

### 4.3 Web dashboard (`src/web/Dashboard.tsx`)

Topbar (logo, badge `Web`, avatar → `ตั้งค่าบัญชี` / `ออกจากระบบ`) + 3 tile:
1. **เริ่มบันทึก** — อัดในเบราว์เซอร์; ปุ่ม `เริ่มอัด` → live dot + timer + level meter → `หยุดและบันทึก`; รายการไฟล์: `ถอดเสียงที่ desktop` (เปิดเมื่อเชื่อม desktop) → progress `desktop กำลังถอดเสียง… n%` → transcript inline, `ดาวน์โหลด`, `ลบ`, `<audio>`; error ไมค์ภาษาไทยตาม `DOMException.name`
2. **ไฟล์ล่าสุด** — 4 สถานะ `unconfigured` (ช่องวางลิงก์ `http://127.0.0.1:PORT/#TOKEN`) / `loading` / `error` (แยกจากว่าง) / `ready` (chip `ไมค์` `เสียงระบบ` `ไฟล์` + `<audio>`)
3. **อุปกรณ์ที่จับคู่** — loading / error (`โหลดรายการอุปกรณ์ไม่สำเร็จ — ไม่ใช่ว่าไม่มีอุปกรณ์`) / empty / rows + `ยกเลิก`

Modal `ตั้งค่าบัญชี`: โปรไฟล์ (ชื่อแสดง), บัญชีที่เชื่อมต่อ, BackupPanel (disabled บนเว็บ), อุปกรณ์ · Responsive: คอลัมน์เดียวที่ ≤ 640px

### 4.4 Landing (`src/landing/LandingPage.tsx`)

Header (nav Product/How it works/Demo/Privacy, chip APK "เร็ว ๆ นี้", `เข้าสู่ระบบ`, `เปิด FUNG`) → Hero (`FUNG ในสิ่งที่เค้าไม่ได้พูด / แล้วเราจะได้ยินสิ่งที่เค้าต้องการ`, Google sign-in) → 02 Knowledge (3 step `บันทึก/เข้าใจ/เชื่อมโยง` + กราฟ 5 node) → 03 Architecture (`Local by default. Connected by choice.`) → 04 Demo (3 การ์ด, ปุ่มโหลด Windows + SmartScreen notice) → Closing → Footer (`All Systems Local`) Scroll progress ผ่าน CSS var `--landing-scroll` / `--progress`

### 4.5 Phone page (`src-tauri/assets/local_recordings.html`)

การ์ดเชื่อมต่อ (สแกน QR/วางลิงก์) → รายการไฟล์ของ desktop + `<audio>` ต่อรายการ + chip ช่องเสียง → footer `ลืมลิงก์นี้` token เก็บใน `sessionStorage` และลบออกจาก address bar

## 5. API spec ที่ frontend ต้องเรียก

### 5.1 Desktop: Tauri commands (130 ตัว, `src-tauri/src/lib.rs:3269-3401`)

เรียกผ่าน `invoke("name", { camelCaseParams })` — wrapper พร้อม type อยู่ใน `src/tauri.ts` (ทุกฟังก์ชันมี fallback เมื่อไม่ได้อยู่ใน Tauri ยกเว้น TTS/live/ask/external tools/Zoom ที่ throw) กลุ่มหลัก:

| กลุ่ม | คำสั่ง |
| --- | --- |
| Core | `app_health`, `create_project(name)`, `list_projects`, `create_job(jobType, projectId?, recordingId?)`, `cancel_job(jobId)`, `runnable_job_types`, `list_jobs`, `list_model_providers`, `list_transcript_segments(projectId, recordingId)`, `import_and_transcribe(filePath, projectId?)`, `fetch_and_transcribe(url, projectId?)`, `media_fetch_status`, `media_fetch_consent_set(enabled)`, `audio_integrity_check(projectId)`, `recovery_scan`, `recovery_recover(recordingId)`, `start_local_api`, `set_local_api_lan(enabled)`, `list_export_artifacts(projectId)`, `diarization_status` |
| Live/Intel | `live_meeting_start(projectId?, captureSystem?, language?)`, `live_meeting_stop`, `live_meeting_status`, `meeting_ask(question, projectId?)`, `meeting_summaries(projectId, recordingId)`, `generate_meeting_summary(projectId, recordingId)`, `graph_build_start(projectId, recordingId)` |
| Session broker (`src/lib/desktopSessionBroker.ts`) | `broker_session_login_begin/cancel/status/logout`, `broker_enrollment_request/status`, `broker_device_list/revoke/audit_list/endpoint_publish`, `broker_pairing_create/poll/reconcile`, `broker_fungwire_status/set_enabled` |
| Local Backup | `backup_status/list_archives/generate_recovery_phrase/run/restore/restore_select_target`, `filesystem_backup_select_root` |
| Cloud/Zoom/TTS | `cloud_config_set/clear/status`, `tier_policy_get/set`, `cloud_call_counts_today`, `zoom_connect/connection_status/disconnect/list_recordings/import_recording`, `tts_provider_register/update/toggle/test`, `tts_synthesize_text` |
| External MCP | `external_connectors_list`, `external_connector_register/disconnect`, `meeting_tool_suggest/execute/cancel/revoke/runs_list` |
| FUNGWIRE | `fungwire_desktop_reachable`, `fungwire_desktop_status_probe`, `fungwire_delegate_transcription`, `fungwire_job_poll`, `device_identity_ensure`, `device_public_key` |

Type หลัก (verbatim จาก `src/tauri.ts`):

```ts
type Health = { app; version; databasePath; sqliteWal; genesisPath; genesisStableFrontier;
  storageAuthority; localApi: { running: boolean; bind: string|null; lanBind: string|null }; pendingJobs: number };
type Project = { id; name; storagePath; activeRecordingId: string|null; createdAt; updatedAt };
type Job = { id; projectId; type; status; progress: number; inputRefs: string[]; outputRefs: string[];
  providerId; errorCode; errorMessage; startedAt; finishedAt; createdAt; updatedAt };
type TranscriptSegment = { id; projectId; recordingId; speakerId; speakerName; startMs; endMs; text;
  confidence: number|null; createdAt };
type TranscriptView = { segments; capped: boolean; cap: number; cappedRecordingIds: string[] };
type LocalApiInfo = { bind; token; connectUrl; lanEnabled; lanBind; lanUrl };
type CancelOutcome = "cancelled" | "requestedWhileRunning" | "notPending";
type DiarizationReadiness = { available; blocker: "runtimeMissing"|"workerMissing"|"dependenciesMissing"|"modelNotFetched"|null; detail; … };
type MediaFetchReadiness = { available; blocker; blockerCode: string|null /* branch on this, never on detail */; … maxDurationS };
type LiveStartOutput = { projectId; recordingId; jobId; micDevice; systemDevice: string|null; warning: string|null };
type AskAnswer = { answer; sources: { n; kind; projectName; text; startMs; recordingId }[]; model; searchedRowsCapped: boolean };
type MeetingSummaries = { rows: { id; kind: "whole_story"|"timeline"|"decisions_actions"; content; evidenceCount; createdAt; recordingId; superseded }[];
  otherRecordings: number; unattributable: number; attributionComplete: boolean };
type SessionStatus = { state: "signed_out"|"login_pending"|"authenticated"|"refreshing"|"refresh_failed"|"logout_pending"|"credential_cleanup_failed"|"shutdown";
  userId; email; accessExpiresAtMs };
type PairingResult = { pairingId; displayCode; expiresAtMs; status: "waiting" };
type FungwireStatus = { enabled; bind; activeJobs; connectedPeers };
```

Job types ที่รันได้: `summary.generate`, `transcript.retry`, `graph.build`, `speakers.diarize`, `export.render` (`src/lib/jobActions.ts`)

**Events** (มีแค่ 4, ฟังด้วย `listen<T>` จาก `@tauri-apps/api/event`): `live-status {recordingId, state, detail, micDevice, systemDevice}`, `live-segment {recordingId, segmentId, channel, speaker, startMs, endMs, text, confidence}`, `live-topic {recordingId, topic, openPoints[], actionItems[], model, windowStartMs, windowEndMs}`, `live-summary {recordingId, state: running|ready|failed, detail, exportPath}` ที่เหลือใช้ **polling**: jobs 1 s, pairing 2 s, broker session 500 ms, Zoom 2 s, delegated job 1.5 s, endpoint publish 60 s

### 5.2 Mobile bridge (`src/mobile/bridge.ts`, 32 ฟังก์ชัน, คืน null/[] เมื่อไม่อยู่ใน Tauri)

Capture: `startCapture`, `appendCaptureSegment`, `finishCapture`, `startNativeRecorder`, `nativeRecorderStatus`, `nativeRecorderLevel`, `controlNativeRecorder`, `reconcileNativeCapture`, `playbackManifest`, `loadPlaybackSegment` · Data: `queryRecordings`, `queryTimeline`, `queryGraph`, `persistNote`, `queryStory`, `createStory`, `moveStoryClip`, `trimStoryClip`, `splitStoryClip`, `storyHistory` · Speakers: `startDiarization`, `renameSpeaker`, `splitSpeakerTurn`, `mergeSpeakers`, `confirmSpeakerTurn` · Devices: `deviceIdentityEnsure`, `devicePublicKey`, `pairingComplete`, `desktopEndpoint` (อ่าน Supabase), `desktopReachable`, `delegateTranscription`, `pollDelegatedJob`, `desktopCloudEnabled` (read-only, fail-closed), `setMcpEnabled`, `onDeviceAiStatus`

```ts
type CaptureState = { state: "idle"|"preparing"|"recording"|"paused"|"finalizing"|"completed"|"recovery_required";
  levelPercent: number|null /* null = ไม่มีค่าจริง ห้ามวาดคลื่นปลอม */; backend: "web"|"android-native"|null;
  error: { stage: "session"|"native-start"|"web-permission"|"native-sync"|"native-stop"; detail }|null; … };
type EpistemicStatus = "confirmed"|"inferred"|"evidence"|"superseded"|"disputed"|"ai_proposed";
type DeviceState = { …; trustState: "unpaired"|"paired"|"unreachable"|"revoked" };
type OnDeviceAiStatus = { probeAvailable; tier: "core"|"ai_lite"|"ai_standard"|"ai_pro"; reason; totalRamMb; … };
```
Domain types ทั้งหมดใน `src/mobile/model.ts`

### 5.3 Web loopback API (desktop เสิร์ฟ, `src-tauri/src/local_api.rs`)

| Route | Auth | ตอบ |
| --- | --- | --- |
| `GET /health` | ไม่ต้อง | `{app, version, databasePath, storageAuthority, stableFrontier}` |
| `GET /` | ไม่ต้อง | phone page |
| `GET /recordings` | token | `{recordings: LocalRecording[]}` |
| `GET /recordings/{id}/audio?channel=mic\|system\|file` | token (`?token=` ได้) | bytes, รองรับ `Range` |
| `GET /recordings/{id}/transcript` | token | `{projectId, recordingId, transcript: {segments}}` |
| `GET /jobs/{id}` | token | `{job: LocalJob}` |
| `POST /recordings/import` (body = ไฟล์, `Content-Type`, `X-Fung-Filename`, ≤ 512 MB) | token | `202 {jobId, projectId, recordingId}` |

Token: สร้างใหม่ทุกครั้งที่เปิด desktop, ไม่บันทึก, อยู่ใน fragment ของ connect URL `http://127.0.0.1:PORT/#TOKEN`; ส่งเป็น `Authorization: Bearer` หรือ `?token=` (เฉพาะ `<audio src>`) · CORS: `https://fung-seven.vercel.app` + localhost/127.0.0.1 ทุกพอร์ต (+ env `FUNG_WEB_ORIGINS`) · Client: `src/web/localApiClient.ts` (`parseConnectUrl`, `fetchRecordings`, `audioUrl`, `importRecording`, `fetchJob`, `fetchTranscript`) — **ไฟล์เดียวใน `src/` ที่ใช้ `fetch` ได้**

```ts
type LocalRecording = { id; projectId; projectName; source; status; durationMs; createdAt; language; channels: string[]; chunkCount };
type LocalJob = { id; projectId; type; status; progress; errorCode; errorMessage };
type LocalTranscriptSegment = { id; startMs; endMs; text; speakerName; confidence };
type LocalImportReceipt = { jobId; projectId; recordingId };
```

### 5.4 Supabase (auth + metadata เท่านั้น)

ตารางที่ frontend แตะ: `profiles` (อ่าน/แก้ `display_name`), `devices` (**อ่านอย่างเดียว** — เขียนผ่าน edge function `device-enrollment` actions `pending|pairing_only|revoke` ใน `src/lib/deviceAuthority.ts` เท่านั้น), `device_audit_events` (insert), `oauth_connections` (อ่าน historical metadata เท่านั้น), `pairing_sessions` (อ่าน) · RPC: `confirm_pairing(session, code, device)` → `confirmed|already_confirmed|wrong_code|locked|expired|not_found`, `publish_device_endpoint` · Edge function: `device-enrollment`

### 5.5 Auth flow ต่อ surface (ต่างกันจริง ออกแบบหน้า login ต้องรู้)

| Surface | Flow |
| --- | --- |
| Web | supabase-js PKCE: `signInWithOAuth({provider:"google", redirectTo: origin+"/auth/callback"})` → `/auth/callback` → `/app` |
| Mobile | PKCE ที่ TS ทำเอง → เปิด **system browser** (Google บล็อก webview) → deep link `fung://auth/callback?code=` → แลก token native (`auth_exchange_google_code`) → `supabase.auth.setSession`; รองรับ cold start เพราะ Android อาจ kill แอประหว่างเปิดเบราว์เซอร์ |
| Desktop | Rust broker ถือ token ทั้งหมด (webview ไม่เห็น): `broker_session_login_begin` → loopback `127.0.0.1/auth/callback` → poll `broker_session_status` ทุก 500 ms; error เป็น code เช่น `auth_request_in_progress` |

### 5.6 FUNGWIRE (mobile → desktop job) ที่มีผลกับ UI

LAN TCP + Noise KK เฉพาะอุปกรณ์ที่จับคู่; ต้องมี `ownDeviceId` (`localStorage["fung.device.id"]`) ไม่งั้น `ยังไม่พบตัวระบุอุปกรณ์นี้ — เข้าสู่ระบบก่อน`; `executor: "local"|"cloud"` เป็น **คำขอ** desktop ตัดสินเองได้ (`Error{code:"cloud_disabled"}`) ดังนั้น badge `☁ คลาวด์` แสดงเมื่อ `completed` เท่านั้น; job resume กลางทางได้ → progress ต้องรับค่าย้อน; keepalive progress ทุก 20 s

## 6. ข้อจำกัดที่ test บังคับ (ทำผิด CI แดง)

1. **ห้าม `fetch`/`WebSocket`/`XMLHttpRequest` ใน `src/` ทุกไฟล์ ยกเว้น `src/web/localApiClient.ts`** และในนั้นทุก `fetch(` ต้องมี `if (!isLoopbackBaseUrl(` นำหน้าภายใน 400 ตัวอักษร (`tests/egressRegister.test.mjs`) → ข้อมูลใหม่ทุกอย่างต้องมาทาง Tauri command, loopback client หรือ supabase-js
2. CSP desktop `connect-src` มีได้แค่ `ipc:`, `http://ipc.localhost`, `'self'`, `http(s)://127.0.0.1(:*)`; Android เพิ่มได้แค่ origin Supabase หนึ่งเดียว
3. Bootstrap/lazy boundary (`tests/desktopBootstrap.test.mjs`): `main.tsx` import `App` แบบ static เท่านั้น และห้าม static import `LandingPage`/`MobileApp`/`AuthGuard`; `App.tsx` ต้อง lazy `DevicePairingPanel`/`SettingsPanel`; `SettingsPanel` ต้อง lazy `AccountLoginPanel`/`BackupPanel`; ปุ่ม record ต้องเรียก `setLiveMeetingOpen(true)` + `enterMeetingWorkspace("P1")`
4. Viewport มือถือบนเว็บ: `(pointer: coarse) and (max-width: 760px), (pointer: coarse) and (orientation: landscape) and (max-height: 760px)` → ไป dashboard ไม่ใช่ mobile shell
5. `public.devices` ห้ามเขียนตรง (`tests/deviceAuthority.test.mjs`); การเพิกถอนเป็น soft (`revoked_at`)
6. ชื่อ job ต้องตรงกับ Rust `JobKind` (`tests/jobActions.test.mjs`); สรุปต้องขอเป็น `(projectId, recordingId)` เสมอ (`tests/summaryScoping.test.mjs`)
7. Dependency ปัจจุบัน: react, react-dom, lucide-react, qrcode, @fontsource/*, @supabase/supabase-js, @tauri-apps/* — **ไม่มี** CSS framework / component lib / router / state manager / animation lib การเพิ่มต้องผ่าน review
8. Desktop window ไม่ resize (1280×800) + stage scale คงที่ — ถ้าจะทำ responsive ต้องแก้ `tauri.conf.json` และถอด `useStageScale`

## 7. หลักการ copy ที่ต้องคง (product truth)

- **ไทยนำ** สำหรับ copy ผู้ใช้ (ยกเว้นที่ยังเป็นอังกฤษ: ExternalAccountPanel, tab label ของ Settings, nav/tile ของ desktop, stat pill — redesign ควรตัดสินใจให้เป็นระบบเดียว)
- **ห้ามสร้างข้อมูลปลอม**: "ตรวจไม่ได้" ≠ "ไม่มี" (error กับ empty แยกกันเสมอ); ไม่มีค่าจริง → แสดงว่า inactive ไม่ใช่ศูนย์ (VU meter, `levelPercent: null`); ปุ่มที่ยังทำไม่ได้ → disabled พร้อมเหตุผล ไม่ใช่ no-op เงียบ
- ต้องเปิดเผยความไม่ครบ: `TranscriptView.capped` แสดง **ก่อน** เนื้อหา (`transcript ไม่ครบ — อ่านได้สูงสุด {cap} ท่อน…`), `AskAnswer.searchedRowsCapped`, `MeetingSummaries.otherRecordings/unattributable`
- ข้อมูลที่เป็น inference ต้องมีป้าย (`ai_proposed` ไม่ปนกับ `confirmed`; `ป้ายผู้พูดเป็นการจัดกลุ่มเสียง ไม่ใช่การยืนยันตัวบุคคล`)
- ประโยค privacy ที่ต้องอยู่ต่อ: `เสียงทั้งหมดถูกบันทึกและประมวลผลในเครื่องนี้เท่านั้น — โปรดแจ้งผู้ร่วมประชุมก่อนเริ่มอัด`, `ไม่มีเสียงขึ้น cloud`, `ใช้งานแบบ Local ได้โดยไม่ต้องมีบัญชี`, `ต้นฉบับเสียงยังคงเดิม`
- สีแดงสงวนไว้สำหรับ "กำลังอัด" และ destructive เท่านั้น; touch target ≥ 44dp (mobile)

## 8. UX debt ที่ควรใช้ redesign แก้ (เรียงตามผลกระทบ)

1. ภาษาผสม (label อังกฤษ / body ไทย) ทั่ว desktop
2. โลโก้/ไอคอน 3 แบบไม่ตรงกัน + palette 4 ชุดไม่ตรง brand tokens + mobile ใช้ navy/ส้มนอก brand
3. Desktop ใช้ metaphor "instrument/score" (P1–P4, tiles, signals, agent card) ที่ซ้อนกันหลายชั้น มี dead action 6 จุด + Search/Play/VU inactive → ควรลดเหลือ flow จริง: อัด → ถอด → สรุป → ส่งออก
4. ไม่มีฟอนต์ไทยบน desktop; theme ไม่ persist; web/landing ไม่มี dark
5. Web dashboard เป็น 3 tile ซ้อนกันแนวตั้ง ยังไม่มี IA (ไม่มี design doc ของ web เลย)
6. Mobile: Timeline/ProcessingStudio ยัง fixture, filter chip inert, inline style ไม่ตาม theme
7. Live meeting เป็น modal ใหญ่ทับ workspace — ควรเป็นหน้าหลักของการใช้งานจริง
8. การเชื่อม web↔desktop ต้องวางลิงก์ด้วยมือ (QR มีเฉพาะ LAN) — ออกแบบ onboarding การเชื่อมต่อให้ชัด (loopback / LAN QR / USB)
9. Empty/error state ทำได้ดีแล้วเรื่องความซื่อสัตย์ แต่รูปแบบไม่เหมือนกันข้าม surface

## 9. สิ่งที่ต้องการจากทีมออกแบบ

- Design system เดียวสำหรับ 3 surface + landing: token (สี/type ไทย-Latin/spacing/radius/elevation), dark+light, component set (ปุ่ม, chip สถานะ, list row, player, level meter, transcript row, progress, sheet/modal, bottom nav/rail)
- Flow หลักครบทุกสถานะ: (a) desktop live meeting เริ่ม→ฟัง→จบ→สรุป→ส่งออก, (b) mobile อัด→ไฟล์→ส่งถอดที่ desktop→timeline, (c) web อัด/เชื่อม desktop→ถอด→transcript, (d) จับคู่อุปกรณ์ + sign-in ทั้ง 3 flow, (e) settings (runtime/cloud/backup)
- ระบุ mapping ของทุกหน้าจอกับ command/API ใน §5 (ชื่อ command เดิม; เพิ่มได้แต่ต้องขอ)
- ส่งเป็นไฟล์ที่ทีม dev ใช้ต่อได้: token เป็น CSS variables, ไอคอนจาก lucide, โลโก้ SVG `currentColor` ตามกฎ §2

### 9.1 Wireframe ทุกหน้า + Mockup ทุกหน้า (deliverable บังคับ)

ส่ง 2 ชั้นต่อทุกหน้าจอด้านล่าง: **wireframe** (low-fi, โครง/ลำดับข้อมูล/จุดกด, ไม่ต้องสี) และ **mockup** (hi-fi ตาม design system ใหม่, light + dark) ทุกหน้าต้องมีสถานะ **ปกติ / ว่าง (empty) / กำลังโหลด / ผิดพลาด (error)** และ mobile ต้องมีทั้ง portrait และ landscape สำหรับ Home, Capture, Files

| Surface | หน้าจอที่ต้องมี (อ้างอิง §4) |
| --- | --- |
| **Desktop** (1280×800 หรือ responsive ถ้าเสนอ) | 1 Home · 2 Meeting workspace (แสดงทั้ง 4 anchor: Capture / Transcript / Summary / Runtime หรือ IA ใหม่ที่เสนอแทน) · 3 Live meeting (idle / listening / degraded / stopped + สรุปหลังประชุม) · 4 Transcript review + rename ผู้พูด · 5 Summary + TTS · 6 Export · 7 Recovery notice · 8 Settings ทุก tab (Sign In & Backup, TTS, Cloud, Fetch from URL, Zoom, Runtime/Local API + QR, External Connections) · 9 Device pairing (รหัส 6 หลัก + FUNGWIRE) · 10 External tools (preview → approve → result) · 11 Sign-in (pending / authenticated / error) · 12 Local Backup (phrase 24 คำ) |
| **Mobile** (Android, 360–430dp) | 1 Home · 2 Capture (idle / recording / paused / finalizing / completed / recovery_required) · 3 Files + player inline · 4 Notes (list / create sheet / detail) · 5 Graph + inspector · 6 Timeline (speaker turns + inspector) · 7 Devices (signed-out / signed-in / จับคู่แล้ว / ยกเลิก) · 8 Pairing sheet · 9 Story editor · 10 Processing studio (4 tab + delegate to desktop/cloud) · 11 Sign-in ผ่าน system browser + กลับแอป · 12 Theme/MCP settings |
| **Web dashboard** | 1 Landing → sign-in → callback (loading / error) · 2 Dashboard (3 ส่วน: อัดในเบราว์เซอร์, ไฟล์จาก desktop, อุปกรณ์) · 3 Recorder (idle / requesting mic / recording / saving / mic denied) · 4 รายการไฟล์ + ถอดที่ desktop (uploading / running % / completed transcript / failed) · 5 เชื่อมต่อ desktop (unconfigured / loading / error / ready) · 6 Account settings modal · 7 Mobile-width (≤ 640px) ของทุกหน้า |
| **Landing** | Hero, How it works, Architecture, Demo/Download, Closing, Footer — desktop + mobile width |
| **Phone page** (desktop-served) | Connect card (สแกน/วางลิงก์) · รายการ + player · error (ติดต่อไม่ได้ / ลิงก์หมดอายุ) |

รูปแบบส่งมอบ: Figma (หรือ Penpot) 1 ไฟล์ แยก page ต่อ surface, component/token เป็น library ในไฟล์เดียวกัน; export PNG 2× ทุก mockup ลงโฟลเดอร์ `docs/design/mockups/<surface>/` และ wireframe ลง `docs/design/wireframes/<surface>/` ตั้งชื่อ `NN-screen-state.png`; ทุกหน้าใน mockup ระบุ command/API ที่หน้านั้นเรียก (จาก §5) ไว้ใน note ของ frame

## เอกสารอ้างอิงเพิ่มเติม (ในรีโป)

อ่านก่อน: `docs/UI_INTERFACE_INVENTORY.md` (draft 2026-08-22), `docs/superpowers/specs/2026-08-29-desktop-sitemap-redesign-design.md` (IA desktop ปัจจุบัน), `docs/Mobile/PRODUCT_UX_SPEC.md`, `docs/Mobile/IMPLEMENTATION_STATUS.md` (2026-08-23), `docs/Mobile/DESIGN_SYSTEM.md`, `docs/Desktop/02-tokens.md` + `04-components.md` (เจตนา ก.ค. ก่อน redesign), `docs/app/WEB_LANDING_PAGE_PROPOSAL.md`, `docs/WEB_PRODUCTION_DEPLOYMENT.md` (ตารางว่าเว็บทำอะไรได้), `docs/appendices/E-egress-register.md`, `docs/Desktop/08-real-progress.md` (สถานะจริงล่าสุด) · superseded: `docs/Mobile/CLONY_INSPIRED_MOBILE_TOKEN_PROPOSAL.md`, `docs/Desktop/CALLMD_FUNG_SCORECARD_TH.md`
