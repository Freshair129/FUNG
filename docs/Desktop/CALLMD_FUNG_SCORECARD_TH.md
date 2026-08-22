---
version: "0.2.0b"
created_at: "2026-08-09T00:00:00+07:00,CLAUDE"
last_update: "2026-08-11T10:37:54+07:00,Agent: ATHER"
status: "superseded"
superseded_by: "docs/DOC_PREFLIGHT_2026-08-11.md"
attributes:
  domain: "local-first-audio-ai"
  doc_type: "competitive-analysis"
  scope: "Call.md (ตรวจจากซอร์สโค้ดจริง) เทียบ FUNG (ตรวจจากโค้ดจริง) — บทวิเคราะห์ คะแนน และแผนดำเนินการ (ภาษาไทย)"
related:
  - "docs/Desktop/CALLMD_ARCHITECTURE_COMPARISON.md (ฉบับสถาปัตยกรรม, อังกฤษ, v0.1.0b)"
---

# Call.md × FUNG — วิเคราะห์เทียบจากซอร์สโค้ดจริง พร้อมคะแนนและแผนดำเนินการ

> **Historical implementation snapshot:** คะแนนและข้อความสถานะ FUNG ด้านล่างสะท้อนฐานที่ผู้เขียนตรวจ ณ 2026-08-09 และต้องไม่ใช้เป็น current implementation truth หลัง Live Meeting commits วันที่ 2026-08-10 เอกสาร current truth คือ `docs/Desktop/08-real-progress.md` และ `docs/DOC_PREFLIGHT_2026-08-11.md`

## Current-State Correction — 2026-08-11

| Claim in historical snapshot | Current code truth | Remaining boundary |
|---|---|---|
| Desktop อัดเสียงไม่ได้ | `live_meeting.rs` routes microphone plus optional WASAPI system audio and durable live chunks. | Real-device/recovery UAT must be reported separately from code existence. |
| ไม่มี live intelligence | `meeting_intel.rs` routes current topic/open points/actions and manual local Knowledge Base Q&A. | External MCP/CRM lookup and automatic suggestions are absent. |
| ไม่มี summary/export | Post-meeting overview, timeline/key points, decisions/actions, provenance, and Markdown export are routed. | Requires configured local model and current restart/evidence UAT. |
| UI เป็น mock ทั้งหมด | `LiveMeetingPanel.tsx` is wired to Tauri events/commands. | Main dashboard still contains hard-coded demo/status content; fixed microphone rail entry is not wired to the real panel. |
| Desktop MCP ไม่มี | Still true for external meeting tools. | `local-mcp-v1.yaml` is a contract only; controlled external retrieval needs the approved requirements/security design. |

The original comparison remains useful as historical decision evidence, especially its rejection of auto-triggered MCP and its requirement for suggest → preview → approve. Its numeric readiness score must not be reused as a current score.

## 1. วัตถุประสงค์และขอบเขตหลักฐาน

เอกสารนี้ต่อยอดจาก `CALLMD_ARCHITECTURE_COMPARISON.md` (ฉบับอังกฤษ ซึ่งวิเคราะห์จาก README/package.json บน GitHub เท่านั้น) โดยรอบนี้**อ่านซอร์สโค้ดจริงของ Call.md ที่ `D:\call.md-main` (v1.0.4)** และ**ตรวจสถานะ implementation จริงของ FUNG จากโค้ด** ไม่ใช่จากเอกสารออกแบบ

| แหล่งข้อมูล | วันที่ตรวจ | หมายเหตุ |
| --- | --- | --- |
| ซอร์ส Call.md ที่ `D:\call.md-main` | 2026-08-09 | package.json ระบุ v1.0.4, MIT |
| GitHub `video-db/call.md` | 2026-08-09 | ~909 ดาว, 106 forks, 86 commits, **ไม่มี release ใด ๆ** |
| videodb.io/pricing | 2026-08-09 | เครดิตฟรี $20 แล้วจ่ายตามใช้ |
| โค้ด FUNG (`src-tauri/src`, `src/`, `contracts/`) | 2026-08-09 | สำรวจทั้ง 16 ไฟล์ Rust (~12,700 บรรทัด) |

ข้อจำกัด: ไม่ได้รันแอป Call.md จริง (ต้องมี VideoDB key และ macOS) การประเมินพฤติกรรม runtime อิงจากโค้ด ไม่ใช่การทดลองใช้ และนี่ไม่ใช่ security audit อย่างเป็นทางการ

## 2. สรุปผู้บริหาร (TL;DR)

1. **Call.md เป็น "เดโมที่ขัดเงามาก" ของ sales-copilot ที่ผูกกับคลาวด์ VideoDB 100%** — เสียงไมค์ เสียงระบบ และภาพหน้าจอถูกสตรีมขึ้น VideoDB แบบเรียลไทม์ระหว่างประชุม (WebSocket 3 เส้น) LLM ทุกคำสั่งวิ่งผ่าน proxy ของ VideoDB คำว่า "Local-First" ใน README หมายถึง *เก็บสำเนา/ประวัติไว้ในเครื่อง* เท่านั้น ไม่ใช่ประมวลผลในเครื่อง
2. **ตัวเก็บเสียง/จอที่แท้จริงเป็น binary ปิดซอร์สของ VideoDB** (`VideoDBCapture.app` ใน npm package) — สัญญา MIT ครอบเฉพาะเปลือก Electron ไม่ครอบหัวใจของระบบ และ repo ไม่มีไฟล์ LICENSE ที่ราก
3. **ใช้งานจริงได้เฉพาะ macOS** — สคริปต์ติดตั้งเช็ก `uname != Darwin` แล้วปฏิเสธทันที ไม่มี Windows/Linux build ให้ดาวน์โหลด และไม่มี GitHub release
4. **ฝั่ง FUNG ต้องยอมรับความจริง**: เดสก์ท็อปวันนี้ยังอัดเสียงไม่ได้, ไม่มี summary/export, jobs ส่วนใหญ่เป็นแถวค้างคิวที่ไม่มีใครประมวลผล, UI หลายส่วนเป็น mockup — แต่รากฐานที่ลงแรงไว้ (GenesisBlockDB, FUNGWIRE, faster-whisper GPU + ภาษาไทย, วินัยการเก็บ key ใน OS keyring) เป็นของจริงและเป็นจุดที่ Call.md ไม่มี
5. **คะแนนรวม**: มุมมอง "copilot พร้อมใช้วันนี้" Call.md ชนะขาด (7.85 ต่อ 3.00) แต่มุมมอง "พันธกิจ FUNG" (local-first, ภาษาไทย, Windows, evidence) FUNG นำ (5.84 ต่อ 5.20) → **ยืนยันคำตัดสินเดิม: ไม่ fork, ใช้เป็น reference แล้วสร้างบน FUNG** — และลำดับงานถัดไปคือ *ปิด core loop ของตัวเองให้จริงก่อน* ค่อยไล่ทำฟีเจอร์ live

## 3. สิ่งที่ Call.md ไม่ได้บอกใน README (พบจากการอ่านซอร์ส)

### 3.1 เส้นทางข้อมูล: ทุกช่องสัญญาณขึ้นคลาวด์แบบเรียลไทม์

| ข้อค้นพบ | หลักฐาน | ผลกระทบ |
| --- | --- | --- |
| ระหว่างประชุมเปิด WebSocket ไป VideoDB **3 เส้นพร้อมกัน: ไมค์, เสียงระบบ, และหน้าจอ** | `src/main/ipc/capture.ts:39-41` (`micWebSocket`, `sysAudioWebSocket`, `screenWebSocket`) | เนื้อหาการประชุม + ทุกอย่างที่ปรากฏบนจอ ออกนอกเครื่องทันทีที่เริ่มอัด |
| ภาพหน้าจอถูกส่งเข้า "rtstream" ของ VideoDB ให้ AI ฝั่งคลาวด์**บรรยายฉากหน้าจอเป็นข้อความ** (visual index) | `src/main/db/schema.ts:353-366` (`visual_index_items.text` = "AI-generated scene description", `rtstream_id`), widget มี `VisualAnalysisCard` | เอกสาร/แชต/รหัสที่เผลอเปิดบนจอระหว่างประชุม ถูกตีความและเก็บเป็นข้อความ — README พูดแค่ "Screen Recording" |
| ไฟล์บันทึกถูกอัปโหลดเป็น video ในคลังของ VideoDB และได้ `player_url` บน `player.videodb.io` | `schema.ts:16-19`, event `upload:progress/complete` ใน `capture.ts:34-35` | การเล่นย้อนหลังพึ่งคลาวด์; ลิงก์ player ถูกส่งต่อออกไปทาง webhook ด้วย (ระดับการป้องกันลิงก์ยังไม่ได้ตรวจ) |
| LLM ทุกงานวิ่งผ่าน **proxy ของ VideoDB (`api.videodb.io`) ด้วยชื่อโมเดล `ultra`** — ไม่เปิดเผยว่าเบื้องหลังคือโมเดลอะไรของค่ายไหน | `src/main/services/llm.service.ts:98-99` | ผู้ใช้ไม่รู้ว่า transcript ของตนถูกส่งต่อไปยังผู้ให้บริการโมเดลรายใด ภายใต้เงื่อนไขการเก็บข้อมูลแบบใด |
| Webhook หลังประชุมส่ง **transcript ทั้งบทสนทนา** (พร้อม summary, action items, ลิงก์วิดีโอ) ไปยัง URL ที่ตั้งไว้ | `src/main/services/workflow-webhook.service.ts:35,78` | การเปิด workflow หนึ่งครั้ง = เผยแพร่เนื้อหาการประชุมทั้งหมดไปยังระบบภายนอก (n8n/Zapier/CRM) |
| หน้า UI โหลดฟอนต์จาก Google Fonts CDN ตอนรัน | `src/renderer/index.html:8` | รอยรั่วเล็ก (IP/fingerprint ไป Google) และตอกย้ำว่าแอปออกแบบโดยถือว่ามีเน็ตเสมอ |

### 3.2 ความปลอดภัยของ credentials: มีทั้งจุดดีและจุดอ่อน

| ข้อค้นพบ | หลักฐาน | ประเมิน |
| --- | --- | --- |
| **VideoDB API key เก็บ plaintext** ใน SQLite (`users.api_key`) และในไฟล์ `auth_config.json` ใต้โฟลเดอร์ resources ของแอป (ใช้ auto-register ตอนติดตั้ง) | `schema.ts:9`, `src/main/lib/config.ts:27-33,78-92` | จุดอ่อน — ต่างจาก FUNG ที่ใช้ OS keyring |
| กุญแจเข้ารหัส credentials ของ MCP (AES-256-GCM) ถูกเก็บเป็น**ไฟล์ `.mcp-encryption-key` วางข้างฐานข้อมูลเอง** ไม่ใช่ OS keychain | `src/main/utils/encryption.ts:11,18-35` | เข้ารหัสจริงแต่กุญแจอยู่ที่เดียวกับข้อมูล = กันได้แค่การเปิดไฟล์ตรง ๆ ไม่กัน malware/บัญชีเดียวกัน |
| **Google OAuth `client_secret` ถูกฝังมาใน repo สาธารณะ** (scope calendar.readonly) | `resources/google_oauth.json` | ใครก็ปลอมตัวเป็น OAuth client ของเขาได้; เป็น anti-pattern ที่ Google เตือนเอง (แม้ desktop app จะถือว่า secret กึ่งสาธารณะ แต่การ commit ลง GitHub ยิ่งแย่กว่า) |
| จุดที่ทำดี: token ของ Google เข้ารหัสด้วย `safeStorage` ของ OS (มี fallback plaintext บน Linux), **ไม่พบ telemetry/analytics SDK ใด ๆ** (ไม่มี PostHog/Sentry/Amplitude), header webhook ระบุตัวตนชัดเจน | `config.ts:156-180`, grep ทั้ง repo | ยุติธรรมต้องชม — ส่วนนี้สะอาดกว่าที่คาด |

### 3.3 ตัวตนจริงของผลิตภัณฑ์: sales tool ที่แต่งตัวเป็น meeting notes

README ใช้ภาษากลาง ๆ ("meeting intelligence") แต่ schema และเซอร์วิสเผยว่าออกแบบมาเพื่อ**การขายฝั่งเดียวของบทสนทนา**:

- ไฟล์หลักชื่อ `sales-copilot.service.ts` ตรง ๆ
- ทุก segment ถูกวิเคราะห์ **sentiment (บวก/กลาง/ลบ) และ trigger แบบ objection** เช่น `objection:pricing`, `playbook:pain` (`schema.ts:63-64`)
- มีระบบ **"Cue Cards"**: การ์ดสคริปต์ตอบ objection แยกตามประเภท (`pricing`, `competitor`, `authority`, `not_interested`, ...) พร้อม `talk_tracks` (พูดแบบนี้) และ `avoid_saying` (ห้ามพูดแบบนี้) โผล่ระหว่างสนทนา (`schema.ts:94-115`) และ widget มีการ์ด "SayThisCard"
- เก็บ `sentiment_trend` ของคู่สนทนาว่า improving/declining ตลอดสาย (`schema.ts:181`)
- Metrics/nudge ผูกกับ "playbook coverage" ของสคริปต์ขาย (`schema.ts:141-166`)

**นัยต่อ FUNG**: นี่คือสิ่งที่เอกสารสถาปัตยกรรมฉบับอังกฤษเรียกว่า "persuasive coaching defaults" — ยืนยันจากซอร์สแล้วว่าเป็นแกนของผลิตภัณฑ์ ไม่ใช่ฟีเจอร์เสริม จุดยืนของ FUNG (ผู้ช่วยที่เป็นกลาง มีหลักฐานอ้างอิง ไม่ตัดสินคน) เป็น**ความต่างเชิงผลิตภัณฑ์จริง** ไม่ใช่แค่เรื่องเทคนิค

### 3.4 แพลตฟอร์มและการแจกจ่าย: แคบกว่าที่โฆษณา

| ข้อค้นพบ | หลักฐาน |
| --- | --- |
| ตัวติดตั้ง `curl \| bash` **ปฏิเสธทุก OS ที่ไม่ใช่ macOS** ("This installer only supports macOS.") และดาวน์โหลด DMG v1.0.0 จาก artifacts.videodb.io | `scripts/install.sh:27-29,39-44` |
| ไม่มี GitHub release ใด ๆ — Windows/Linux มีแค่ config ใน electron-builder ที่ยังไม่เคย ship | GitHub releases (ว่าง), `electron-builder.config.js:95-102` |
| ตัวเก็บเสียง/จอจริงคือ `VideoDBCapture.app` (binary `capture` + `librecorder.dylib`) ที่มากับ npm package `videodb` — **ปิดซอร์ส และถูกเซ็นแบบ ad-hoc** (`codesign --sign -`) ตอน build | `electron-builder.config.js:120-199` |
| ประกาศขอสิทธิ์ **กล้อง** ด้วย (`NSCameraUsageDescription`) ทั้งที่ README พูดแค่ไมค์+จอ | `electron-builder.config.js:69,173-175` |
| การย้ายไป Windows ไม่ใช่แค่ recompile — ต้องรอ VideoDB ทำ capture binary ฝั่ง Windows ให้ก่อน | โครงสร้าง dependency ข้างต้น |

### 3.5 Automation: ทำงานอัตโนมัติแค่ไหน และเบรกอยู่ตรงไหน

| กลไก | พฤติกรรมจริงในซอร์ส | ความเสี่ยง/ข้อสังเกต |
| --- | --- | --- |
| MCP auto-trigger | regex ~10 กลุ่ม intent (CRM lookup, ราคา, คู่แข่ง, นัดหมาย, เอกสาร, ความจำ) จับจากข้อความสด แล้วเลือก tool อัตโนมัติ + มี LLM ช่วยตรวจชั้นสอง; cooldown 30 วินาทีต่อ intent | `intent-detector.service.ts:24-113` — **pattern เป็นภาษาอังกฤษล้วน** ใช้กับบทสนทนาไทยไม่ได้เลย; คำพูดในห้องประชุมกลายเป็น query ยิงไประบบภายนอก (HubSpot/Notion/Coda คือ template ที่ให้มา) โดยไม่มีขั้นยืนยันรายครั้ง |
| Calendar poller | **poll Google Calendar ทุก 20 วินาที**; โหมด `default_record` = เริ่มอัดเองเมื่อถึงเวลานัด (ค่าเริ่มต้นเป็น `always_ask` — อันนี้ทำถูก) | `calendar-poller.service.ts:9,27,138,292` |
| Widget overlay | หน้าต่างลอย always-on-top ระหว่างประชุม แสดง nudge/การ์ด "Say this"/ผลวิเคราะห์ภาพหน้าจอ | `src/main/windows/widget.window.ts`, `src/renderer/widget/*` — แนวคิด UI ที่ดี ควรจดไว้ (ทำได้บน Tauri multi-window) |
| จุดที่ออกแบบดี | nudge กดปิดได้และเก็บสถานะ dismissed, cue card มีปุ่ม feedback (helpful/wrong/irrelevant), prompt ของ copilot แก้ได้ผ่านตาราง `copilot_settings`, มี `session-recovery.service` กู้สถานะหลัง crash | `schema.ts:130,200,210-219` — แนวปฏิบัติที่ควรลอกเชิงแนวคิด |

## 4. สถานะจริงของ FUNG (ตรวจโค้ด ไม่ใช่เอกสาร)

ส่วนนี้จำเป็นต้องตรงไปตรงมา เพราะคะแนนและแผนถัดไปตั้งอยู่บนความจริงนี้

**ของจริงที่ทำงานได้วันนี้**
- นำเข้าไฟล์เสียง/วิดีโอ → ถอดความด้วย **faster-whisper บน GPU (CUDA DLL ที่ stage เอง)** พร้อมรายงานความคืบหน้า และ**รองรับภาษาไทย** (`lib.rs:1315,1521-1544`, `scripts/transcribe.py`)
- รายการ segment + แก้ชื่อผู้พูด inline (`src/App.tsx:585-639`)
- **Knowledge graph จาก LLM ท้องถิ่น (Ollama)** สกัด Topics/Decisions/ActionItems พร้อม**ลิงก์หลักฐานกลับไปที่ segment** และบันทึก provenance ใน `model_runs` (`graph_build.rs:201-214,407`) — นี่คือ "สรุปเชิงหลักฐาน" เวอร์ชันแรกโดยพฤตินัย
- นำเข้า Zoom cloud recording (OAuth) และเป็นเส้นทางเดียวที่เรียก **diarization (pyannote)** จริง (`zoom_sync.rs:892`)
- TTS หลาย provider + ปุ่มอ่านออกเสียง (`tts_executor.rs`)
- **มือถือ**: อัดเสียงจริง (Android foreground service, แบ่ง segment, SHA-256 reconcile) + **FUNGWIRE** มอบงานถอดความให้เดสก์ท็อปผ่าน LAN เข้ารหัส Noise KK (`fungwire_server.rs`/`fungwire_client.rs` รวม ~3,400 บรรทัด มีเทสต์)
- **GenesisBlockDB ใช้จริง** เป็น storage เดียว (signed WAL, migration ถึง v7) (`genesis_adapter.rs:339`)
- วินัย credentials ดี: `cloud_config.rs` เก็บ key ใน **OS keyring**, redact ใน Debug, มีเทสต์กันรั่วและเทสต์ห้าม key ลง DB (`cloud_config.rs:26-37,67-79,140,171-188`)

**ช่องว่างที่ต้องยอมรับ**
- **เดสก์ท็อปยังอัดเสียงไม่ได้** — ปุ่ม Record เป็นแค่ React state ไม่มี backend (`src/App.tsx:1042-1053`; ไม่มี audio crate ใน `Cargo.toml`) และไม่มี system-audio capture บนแพลตฟอร์มใด
- ไม่มี live transcription — มีแต่ batch หลังนำเข้า
- ตาราง `summaries`/`intent_inferences`/`export_artifacts` เป็น DDL เปล่า ไม่มีโค้ดเขียนลง (`lib.rs:680-706`) → **ยังไม่มี summary, ไม่มี export ทุกรูปแบบ, ไม่มี webhook**
- `create_job` แค่ insert แถว `queued` แล้วจบ — ไม่มี executor/retry/resume; ปุ่ม "Generate recap / Export bundle / Analyze intent" ใน UI สร้างแถวค้างคิวถาวร (`lib.rs:857-893`, `src/App.tsx:1010-1024`)
- Local API มีแค่ `GET /health` (`lib.rs:1597-1632`); **MCP ฝั่งเดสก์ท็อปยังไม่มี** (มีเฉพาะ gateway ฝั่งมือถือ 4 tools ที่ทำถูกแบบ: loopback + Bearer + opt-in, `mobile.rs:1690-1789`)
- Dashboard เดสก์ท็อปส่วนใหญ่เป็น **mockup ตัวเลขปลอม** (~360 บรรทัด hardcode ใน `src/App.tsx:147-511`)
- แผน BYOM Phase 3 เพิ่งเสร็จ 1.5/10 งาน (schema v7 commit แล้ว; `cloud_config.rs` เขียนแล้วแต่ยังไม่ wire เข้า `invoke_handler` — ยังเรียกจาก UI ไม่ได้)

## 5. ตารางเทียบสถาปัตยกรรม (ฉบับย่อ)

รายละเอียดเชิงลึกอยู่ในเอกสารอังกฤษ ตารางนี้สรุปเฉพาะที่ต่างอย่างมีนัย:

| ประเด็น | Call.md (จากซอร์ส) | FUNG (จากโค้ด) |
| --- | --- | --- |
| Shell | Electron 34 + preload IPC + tRPC/Hono ในเครื่อง | Tauri v2 + Rust (52 commands) |
| ตัวเก็บสัญญาณ | binary ปิดซอร์สของ VideoDB (mac เท่านั้น): ไมค์+เสียงระบบ+จอ | เดสก์ท็อปยังไม่มี; มือถือทำเองครบและกู้คืนได้ |
| ถอดความ | คลาวด์ VideoDB แบบสตรีม (dual-channel `me`/`them`) | faster-whisper ในเครื่อง (GPU, batch, ไทยได้) |
| ปัญญาประดิษฐ์ | proxy `api.videodb.io` โมเดลนิรนาม "ultra" | BYOM: Ollama/vLLM ท้องถิ่น + (กำลังทำ) cloud keys ใน keyring |
| ฐานข้อมูล | SQLite + Drizzle ตรง ๆ | GenesisBlockDB (signed WAL, migration v7) |
| งานเบื้องหลัง | event-driven ใน process + poller | สัญญา stateful jobs ครบทั้ง contract แต่ยังไม่มี executor |
| ความน่าเชื่อถือข้อมูล | มี segment/history ละเอียด แต่ไม่มีสัญญา evidence/provenance | evidence spans + `model_runs` provenance เป็น first-class (ใช้จริงแล้วใน graph) |
| การแจกจ่าย | DMG ผ่านสคริปต์ (mac); ไม่มี release สาธารณะ | ยังไม่ release; dev บน Windows |

## 6. ตารางให้คะแนน

หลักการให้คะแนน: 0-10 ต่อมิติ อิงหลักฐานในข้อ 3-4; น้ำหนักรวม 100 คะแนนเต็มถ่วงน้ำหนัก = 10

### 6.1 มุมมอง ก — "ตามพันธกิจ FUNG" (local-first, หลักฐานอ้างอิงได้, ภาษาไทย, Windows-first)

| # | มิติ | น้ำหนัก | Call.md | FUNG | เหตุผลย่อ |
| --- | --- | --- | --- | --- | --- |
| 1 | การจับสัญญาณ (ไมค์/เสียงระบบ/จอ) | 10 | 8 | 3 | เขาครบ 3 ช่องแต่ผูก binary ปิด+mac; เรามีจริงเฉพาะมือถือ + นำเข้าไฟล์ |
| 2 | การถอดความ | 12 | 7 | 8 | เขา live แต่บังคับคลาวด์/ไม่การันตีไทย; เรา batch แต่ local+GPU+ไทย |
| 3 | ไลฟ์อัจฉริยะระหว่างประชุม (metrics/nudge/assist) | 8 | 9 | 0 | เขามีครบและขัดเงา; เรายังไม่มีเลย |
| 4 | หลังประชุม (summary/export) | 8 | 8 | 2 | เขามี summary 3 ชั้น + Markdown export; เรามีแค่ knowledge graph |
| 5 | Integrations (ปฏิทิน/MCP/webhook/Zoom) | 5 | 8 | 3 | เขา: Calendar+MCP+webhook; เรา: Zoom import + MCP มือถือ 4 tools |
| 6 | ความเป็นส่วนตัว & local-first | 15 | 2 | 9 | เขาสตรีมเสียง+จอขึ้นคลาวด์ทันที; เราประมวลผลในเครื่องเป็นค่าเริ่มต้น |
| 7 | ความปลอดภัย credentials & trust boundary | 8 | 4 | 8 | เขา: key plaintext, กุญแจข้างข้อมูล, OAuth secret ใน repo; เรา: OS keyring + เทสต์กันรั่ว |
| 8 | ภาษาไทย | 10 | 2 | 8 | intent regex อังกฤษล้วน ไม่มี multilingual; เราถอดไทยได้จริง UI มีไทย |
| 9 | แพลตฟอร์ม (Windows) & การแจกจ่าย | 8 | 2 | 5 | เขา mac เท่านั้น+ไม่มี release; เรารันบน Windows (dev) แต่ยังไม่มี installer |
| 10 | Data model & provenance | 6 | 5 | 8 | เขาละเอียดแต่ไม่มีสัญญา evidence; เราออกแบบเป็น first-class และเริ่มใช้จริง |
| 11 | ต้นทุนระยะยาว & vendor lock-in | 6 | 4 | 8 | เขา: ~$0.60+/ชม. + โมเดลนิรนาม + รอ VideoDB ทุกเรื่อง; เรา: GPU ตัวเอง + BYOM |
| 12 | ความสมบูรณ์พร้อมใช้เป็นผลิตภัณฑ์ | 4 | 7 | 3 | เขาใช้งานได้จริงบน mac; core loop เรายังไม่ปิด |
| | **รวมถ่วงน้ำหนัก (เต็ม 10)** | 100 | **5.20** | **5.84** | |

### 6.2 มุมมอง ข — "อยากได้ meeting copilot ใช้พรุ่งนี้เช้า" (ไม่สนพันธกิจ)

น้ำหนัก: จับสัญญาณ 20, ไลฟ์ 20, ความพร้อมใช้ 20, ถอดความ 15, หลังประชุม 15, integrations 10

| ผลิตภัณฑ์ | คะแนน (เต็ม 10) | อ่านผล |
| --- | --- | --- |
| Call.md | **7.85** | ถ้าอยู่บน macOS, คุยภาษาอังกฤษ, ยอมรับให้เสียง+จอขึ้นคลาวด์ → เป็นตัวเลือกที่ดีจริง |
| FUNG | **3.00** | ยังไม่ใช่ meeting copilot — เป็นเครื่องมือถอดความ+วิเคราะห์ไฟล์ที่มีรากฐานแข็ง |

### 6.3 บทอ่านคะแนน

คะแนนสองมุมนี้เล่าเรื่องเดียวกันจากคนละด้าน: **Call.md เหนือกว่าเราในทุกมิติที่เป็น "ฟีเจอร์" แต่แพ้ในทุกมิติที่เป็น "สัญญาต่อผู้ใช้"** (ความเป็นส่วนตัว, ความปลอดภัย, ภาษา, แพลตฟอร์ม, ต้นทุน, ความเป็นเจ้าของข้อมูล) และมิติหลังคือสิ่งที่ fork/ลอกโค้ดแล้วไม่ได้มาด้วย เพราะมันฝังอยู่ใน dependency หลัก (VideoDB) ที่ถอดออกแล้วผลิตภัณฑ์เขาแทบไม่เหลืออะไร

## 7. ตารางตัดสินใจรายฟีเจอร์ (ปรับปรุงด้วยหลักฐานจากซอร์ส)

คอลัมน์คะแนน: คุณค่าต่อ FUNG /5, ความเสี่ยง /5 (สูง=เสี่ยงมาก), ต้นทุนทำ /5 (สูง=แพง)

| ฟีเจอร์ของ Call.md | คุณค่า | เสี่ยง | ต้นทุน | คำตัดสิน | เงื่อนไข/หมายเหตุ |
| --- | --- | --- | --- | --- | --- |
| อัดไมค์+เสียงระบบแยกช่อง | 5 | 4 | 4 | **รับ (สร้างเอง)** | Windows ได้เปรียบ: มี WASAPI loopback ให้ใช้ตรง ๆ ไม่ต้องมี binary ปิดแบบเขา; persist chunk ก่อนวิเคราะห์เสมอ |
| Live transcript | 4 | 3 | 4 | **รับ (ภายหลัง)** | เส้นทาง local ก่อน (streaming whisper); cloud ต้อง opt-in ตาม 3-tier policy |
| Metrics (talk ratio/WPM/เงียบ) | 4 | 2 | 2 | **รับ** | คำนวณ deterministic จาก timestamp ไม่ใช้ LLM; โชว์สูตร+ช่วงเวลาอ้างอิง |
| Sentiment รายประโยค + trend ของคู่สนทนา | 1 | 5 | 3 | **ไม่รับ** | ขัดหลัก "ไม่ตัดสินคน"; ความแม่นในภาษาไทยต่ำ; เสี่ยงกฎหมาย/จริยธรรม |
| Cue cards / "Say this" / objection scripts | 1 | 5 | 3 | **ไม่รับ** | DNA sales ฝั่งเดียว ขัด positioning "เครื่องมือบันทึกที่เป็นกลาง" |
| Coaching nudges | 2 | 4 | 3 | **ดัดแปลงเป็นกลาง** | เก็บเฉพาะ nudge เชิงข้อเท็จจริง (เช่น "เงียบนาน 2 นาที") opt-in + dismiss + เก็บเหตุผล |
| MCP auto-trigger | 2 | 5 | 4 | **ไม่รับ as-is** | เปลี่ยนเป็น suggest → preview → approve ตามสัญญา MCP ของเรา |
| MCP results panel | 4 | 3 | 3 | **รับ** | sandbox ผลลัพธ์, แสดงแหล่ง, audit ทุก call |
| Visual index (AI บรรยายหน้าจอ) | 2 | 5 | 5 | **เลื่อนไม่มีกำหนด** | ความเสี่ยง privacy สูงสุดในระบบเขา; ถ้าทำต้องเป็น local model เท่านั้น |
| Summary 3 ชั้น (overview/key points/actions) | 5 | 2 | 2 | **รับทันที** | โครง prompt เขาดี (`summary-generator.service.ts`) แต่รันบน BYOM + evidence spans; เรามีตาราง `summaries` รออยู่แล้ว |
| Markdown export | 5 | 1 | 1 | **รับทันที** | เราไม่มี export ใด ๆ — นี่คือ quick win ที่สุดในลิสต์ |
| Meeting templates/playbooks เป็นข้อมูลผู้ใช้ | 3 | 2 | 2 | **รับ (เฟสหลัง)** | ตัดกลิ่น sales ออก เหลือ "วาระการประชุม + checklist" |
| Bookmark ระหว่างอัด + หมวดหมู่ | 4 | 1 | 2 | **รับ** | เข้ากับ evidence model ของเราโดยตรง |
| Calendar poll 20 วิ + default_record | 2 | 4 | 3 | **เลื่อน** | ต่อเมื่อมี consent UX ต่อปฏิทิน; ห้าม auto-record เป็นค่าเริ่มต้น |
| Workflow webhooks | 3 | 4 | 2 | **เลื่อน** | ต้องมี egress policy + redaction preview ก่อน (payload เขาคือ transcript ทั้งดุ้น) |
| Widget overlay (หน้าต่างลอย) | 4 | 2 | 3 | **รับ (ตอนทำ live)** | Tauri multi-window ทำได้; เป็น UX ที่ดีจริง |
| Session recovery หลัง crash | 5 | 1 | 2 | **รับหลักการ** | ของเรามีอยู่ในสัญญา job แล้ว — ทำให้เกิดจริงใน executor |
| Prompt แก้ได้โดยผู้ใช้ (`copilot_settings`) | 4 | 1 | 1 | **รับ** | เข้ากับ BYOM โดยธรรมชาติ |

## 8. แนวทางการดำเนินการต่อ

### 8.1 หลักคิด

บทเรียนที่แท้จริงจาก Call.md ไม่ใช่ "เราต้องมี live copilot เดี๋ยวนี้" แต่คือ **เขา "ปิด loop ผลิตภัณฑ์" ครบตั้งแต่วันแรก** (อัด → ถอด → สรุป → ส่งออก) แม้จะยืมพลังคลาวด์ทั้งหมดมาทำ ส่วนเรามีรากฐานลึกแต่ loop ยังขาดกลางลำ ดังนั้นลำดับคือ **ปิด loop บนเดสก์ท็อปให้จริงก่อน แล้วค่อยเติม live layer** โดยทุกเฟสอ้าง contract ที่มีอยู่ (`stateful-job-model-v1`, `local-mcp-v1`)

### 8.2 แผนเป็นเฟส (แต่ละเฟสต้องมี design + test gate ของตัวเองก่อนเริ่มโค้ด)

| เฟส | ขอบเขต | นิยาม "เสร็จ" | อ้างอิง |
| --- | --- | --- | --- |
| **P0 — จัดบ้าน (สัปดาห์นี้)** | commit เอกสาร 2 ฉบับนี้ + `cloud_config.rs`; ตัดสินใจ OQ-01..06 ของเอกสารอังกฤษ; เลือกลำดับ P1 กับ P2 ว่าอะไรก่อน | เอกสาร merged, คำตอบ OQ บันทึกเป็นลายลักษณ์ | หัวข้อ 9 |
| **P1 — Job executor จริง** | worker ใน Rust: หยิบ `queued` → รัน → `JobEvent` ครบ transition, retry ตาม contract, กู้คืนตอนเปิดแอป; เริ่มจาก 3 job type ที่มี handler แล้ว | ปุ่มใน UI ไม่สร้างแถวค้างคิวอีก; เทสต์ crash-recovery ผ่าน | `lib.rs:857`, contract jobs |
| **P2 — ปิด core loop** | (a) `summary.generate` บน Ollama ด้วยโครง 3 ชั้นแบบ Call.md + evidence spans ลง `summaries` (b) `export.render`: Markdown + SRT/VTT (c) diarization เข้าเส้นทาง import ปกติ ไม่เฉพาะ Zoom | นำเข้าไฟล์ 1 ชม. → ได้ transcript+ผู้พูด+summary+ไฟล์ .md/.srt โดยไม่แตะเน็ต | ตาราง `summaries`/`export_artifacts` ที่รออยู่ |
| **P3 — Desktop capture (จุดที่เราแพ้ขาดที่สุด)** | mic ก่อนด้วย `cpal` → durable chunks ตาม design มือถือ (ที่พิสูจน์แล้ว) → `recording.capture` job; ตามด้วย **WASAPI loopback** สำหรับเสียงระบบ (ข้อได้เปรียบ Windows — Call.md ต้องใช้ binary ปิดของ VideoDB ทำสิ่งนี้บน mac) | ถอดปลั๊ก/ฆ่าแอปกลางคัน → เปิดใหม่แล้วเสียงไม่หาย ทั้งสองช่อง; consent state ถูกบันทึก | S1 ของเอกสารอังกฤษ |
| **P4 — BYOM Phase 3 ให้จบ** | ทำ 8 งานที่เหลือของแผน: wire `cloud_config` commands, `policy.rs` 3-tier, `cloud_executor.rs`, spend guardrail, UI settings | เลือก local/cloud ต่อ task ได้จริง พร้อมเพดานค่าใช้จ่าย | `docs/plans/2026-08-09-phase-3-byom-cloud-keys.md` |
| **P5 — Live layer แบบ FUNG** | streaming transcript local (chunked whisper) → metrics deterministic → nudge กลางเชิงข้อเท็จจริง (opt-in) → widget overlay | ประชุม 1 ชม. ไม่มี chunk หาย, queue มี bound, ปิด live ได้โดย capture ไม่สะดุด | S2-S5 เอกสารอังกฤษ |
| **P6 — พื้นผิวอัตโนมัติ** | Local API เต็มตาม contract → desktop MCP (ลอกแบบ gateway มือถือที่ทำถูกแล้ว) → MCP suggest-preview-approve → egress policy สำหรับ export/webhook | เทสต์ deny/revoke/redaction ผ่าน | `local-mcp-v1.yaml`, `mobile.rs:1690` |

### 8.3 สิ่งที่ทำได้ "สัปดาห์นี้" (quick wins เรียงตามผลตอบแทน)

1. **Markdown export** ของ transcript+ผู้พูด (ครึ่งวัน, ไม่พึ่งอะไรเลย) — ลบหนึ่งใน gap ที่น่าอายที่สุดเทียบกับเขา
2. **Wire `cloud_config.rs` เข้า invoke_handler + commit** — งานเขียนแล้วแต่ยังตายอยู่ในไฟล์
3. **ถอดตัวเลข mockup ออกจาก dashboard** หรือติดป้าย "ตัวอย่าง" — ตอนนี้ UI โกหกผู้ใช้อยู่ ซึ่งขัดกับจุดขายเรื่องความน่าเชื่อถือของเราเอง
4. ตัดสินใจ **OQ-01** (mic-only หรือ mic+system) เพราะมันกำหนดขอบเขต P3 ทั้งเฟส

### 8.4 สิ่งที่ห้ามทำ (ยืนยันซ้ำ + เพิ่มจากหลักฐานใหม่)

- ห้าม fork/embed โค้ด Call.md — นอกจากเหตุผลสถาปัตยกรรมเดิม ซอร์สยืนยันว่า (ก) repo ไม่มีไฟล์ LICENSE ราก (ข) หัวใจ capture เป็น binary ปิดของ VideoDB ที่เราไม่มีสิทธิ์และไม่มีทางใช้บน Windows
- ห้ามทำ sentiment scoring / objection detection / sales coaching เป็นค่าเริ่มต้น
- ห้ามส่ง transcript เต็มออกนอกเครื่อง (webhook/MCP) โดยไม่ผ่าน redaction preview + approval
- ห้ามให้คำพูดในห้องประชุม trigger การเรียกระบบภายนอกอัตโนมัติโดยไม่มีขั้นยืนยัน
- ห้ามเก็บ API key เป็น plaintext ทุกกรณี (มาตรฐาน keyring ที่ตั้งไว้ใน `cloud_config.rs` คือเส้นต่ำสุด)

## 9. คำถามที่ต้องการคำตัดสินจาก Boss

| ID | คำถาม | ตัวเลือกที่แนะนำ |
| --- | --- | --- |
| Q-A | ลำดับ P1/P2 (executor+core loop) มาก่อน P3 (desktop capture) ตามที่เสนอ หรือสลับ? | แนะนำตามเสนอ — ปิด loop ของข้อมูลที่มีอยู่ก่อน เพราะ capture มี unknown เยอะกว่า |
| Q-B | OQ-01: เฟสแรกของ capture เอา mic อย่างเดียว หรือ mic+system audio เลย? | แนะนำ mic ก่อน แล้ว loopback เป็น P3.5 — แต่บน Windows ช่องว่างเทคนิคแคบกว่าที่เอกสารอังกฤษประเมิน |
| Q-C | Live layer (P5) จำเป็นต่อ v1 หรือเป็น v1.5? | แนะนำ v1.5 — v1 ที่ "อัด-ถอด-สรุป-ส่งออก ได้จริง offline" ก็ต่างจากตลาดพอแล้ว |
| Q-D | จะรับข้อเสนอ "ถอด mockup ออกจาก UI" (8.3 ข้อ 3) เลยไหม? | แนะนำรับ — เกี่ยวกับความน่าเชื่อถือของแบรนด์โดยตรง |

## 10. Version Diff

| Version | การเปลี่ยนแปลง |
| --- | --- |
| 0.2.0b | ทำเครื่องหมาย snapshot เดิมเป็น historical/superseded และเพิ่ม current-state correction จาก Live Meeting code โดยไม่ลบหลักฐานเดิม |
| 0.1.0 | ฉบับแรก: วิเคราะห์จากซอร์ส Call.md v1.0.4 ที่ `D:\call.md-main` + สถานะโค้ดจริงของ FUNG; เพิ่มหมวด "สิ่งที่ README ไม่ได้บอก", ตารางคะแนน 2 มุมมอง, ตารางตัดสินใจรายฟีเจอร์แบบมีคะแนน และแผน P0-P6 |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-08-11 | superseded | Added a current-state correction and redirected implementation truth to the progress/preflight documents. | pending | ATHER |
| 0.1.0 | 2026-08-09 | draft | เอกสารเทียบ Call.md × FUNG ภาษาไทย พร้อมคะแนนและแผนดำเนินการ อิงซอร์สโค้ดจริงทั้งสองฝั่ง | N/A — ยังไม่ commit | CLAUDE |
