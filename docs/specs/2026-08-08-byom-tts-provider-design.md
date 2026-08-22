# BYOM TTS Provider Registration

> Design spec สำหรับเพิ่ม Text-to-Speech เข้า FUNG แบบ BYOM —
> user ลงทะเบียน provider เอง ไม่ฝัง model ใดๆ เป็นค่าเริ่มต้น

**Date:** 2026-08-08  
**Status:** Approved  
**Scope:** Phase 1 — registration, validation, test, execution at P3 recap

---

## 1. Context

FUNG มี voice synthesis pipeline วางไว้แล้ว:

- `voice_profiles` → `model_providers` (FK `provider_id`)
- `agent_voice_grants` → MCP tool `fung.voice.speak`
- `agent_voice_sessions` → `delegated_jobs` (operation `voice.synthesize`)

แต่ยังขาด:

- `model_providers.kind` ไม่มี `'tts'`
- ไม่มี UI ลงทะเบียน TTS provider
- ไม่มี executor จริงที่รับ `delegated_jobs` แล้ว synthesize เสียง

Design นี้ต่อท่อให้ครบ โดยยึดหลัก BYOM — user เลือกและตั้งค่า
TTS engine เอง ผ่าน Settings panel

---

## 2. Provider Types

TTS provider ที่รองรับแบ่งตาม `runtime_type` ใน `config_json`:

| runtime_type   | ตัวอย่าง              | config_json fields                                       |
| -------------- | --------------------- | -------------------------------------------------------- |
| `python_script`| F5-TTS-THAI, XTTS     | `venv_path`, `script_path`, `model_path`, `device`       |
| `rest_api`     | Self-hosted TTS on LAN| `endpoint`, `auth_header?`                               |
| `local_binary` | Piper TTS             | `binary_path`, `model_path`, `args_template`             |
| `cloud_api`    | (อนาคต — ไม่ทำเฟสนี้) | `endpoint`, `api_key_ref`, `model_id`                    |

เฟสแรก implement ทั้ง 3 ประเภทแรก ส่วน `cloud_api` วาง interface ไว้แต่ไม่ implement

---

## 3. Schema Changes

### 3.1 Migration: `007_tts_provider_support.sql`

```sql
-- 1) เพิ่ม kind = 'tts' — recreate table (SQLite ไม่รองรับ ALTER CHECK)
CREATE TABLE model_providers_new (
  id               TEXT PRIMARY KEY,
  label            TEXT NOT NULL,
  runtime_location TEXT NOT NULL
    CHECK(runtime_location IN ('local', 'lan', 'cloud')),
  kind             TEXT NOT NULL
    CHECK(kind IN (
      'transcription', 'diarization', 'cleanup',
      'separation', 'summary_intent', 'tts'
    )),
  enabled          BOOLEAN NOT NULL DEFAULT 1,
  config_json      JSON NOT NULL DEFAULT '{}',
  created_at       TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO model_providers_new SELECT * FROM model_providers;
DROP TABLE model_providers;
ALTER TABLE model_providers_new RENAME TO model_providers;

-- 2) ตาราง tts_test_results
CREATE TABLE tts_test_results (
  id               TEXT PRIMARY KEY,
  provider_id      TEXT NOT NULL
    REFERENCES model_providers(id) ON DELETE CASCADE,
  status           TEXT NOT NULL CHECK(status IN ('ok', 'error')),
  latency_ms       INTEGER,
  sample_audio_path TEXT,
  error_message    TEXT,
  tested_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tts_test_provider
  ON tts_test_results(provider_id, tested_at DESC);
```

### 3.2 ไม่ seed TTS provider ใดๆ ตอน startup

ตาราง `model_providers` จะไม่มี row ที่ `kind = 'tts'` จนกว่า user จะลงทะเบียนเอง

### 3.3 ตารางที่มีอยู่แล้ว — ไม่เปลี่ยน

- `voice_profiles.provider_id` FK → `model_providers.id` — ใช้ได้เลย
- `agent_voice_grants` — ไม่เปลี่ยน
- `agent_voice_sessions` — ไม่เปลี่ยน
- `delegated_jobs` — ไม่เปลี่ยน

---

## 4. Tauri IPC Commands

### 4.1 Provider Management (ใหม่ 4 commands)

| Command                 | Input                                    | Output                | หน้าที่                           |
| ----------------------- | ---------------------------------------- | --------------------- | --------------------------------- |
| `tts_provider_register` | `label`, `runtime_type`, `config_json`   | `{ provider_id }`     | สร้าง provider + validate config  |
| `tts_provider_update`   | `provider_id`, `label?`, `config_json?`  | `{ ok: true }`        | แก้ไข config                      |
| `tts_provider_toggle`   | `provider_id`, `enabled: bool`           | `{ ok: true }`        | เปิด/ปิด provider                 |
| `tts_provider_test`     | `provider_id`, `test_text?`              | `{ status, latency_ms, audio_path?, message? }` | ทดสอบ synthesize |

### 4.2 Synthesis Execution (ใหม่ 1 command)

| Command               | Input                                       | Output                | หน้าที่                          |
| ---------------------- | ------------------------------------------- | --------------------- | -------------------------------- |
| `tts_synthesize_text`  | `text`, `provider_id?`, `ref_audio?`, `ref_text?` | `{ audio_path }` | Synthesize จริง → return WAV path |

ถ้าไม่ระบุ `provider_id` → ใช้ TTS provider ตัวแรกที่ `enabled = true`
+ ผ่าน test แล้ว ถ้าไม่มีเลย → return error
`"ยังไม่ได้ลงทะเบียน TTS provider"`

### 4.3 Existing command — ไม่เปลี่ยน signature

`list_model_providers` — return ทั้งหมดรวม `kind = 'tts'`
ฝั่ง frontend filter เอาเฉพาะ `kind === 'tts'` ตอนแสดงใน Settings

---

## 5. Validation Rules

### 5.1 Registration validation (ตาม runtime_type)

**python_script:**
- `venv_path` ต้องมีจริง + มี `python.exe` (Windows) หรือ `python` (Linux/Mac)
- `script_path` ต้องมีจริง + นามสกุล `.py`
- `model_path` ต้องมีจริง (ถ้าระบุ)
- `device` ∈ `{ "cuda", "cpu" }`

**rest_api:**
- `endpoint` ต้องเป็น URL ที่ถูกต้อง (http/https)
- default restrict เฉพาะ `127.0.0.1` / `192.168.*` / `10.*` / `172.16-31.*`
- ถ้าเป็น public URL → return warning (ไม่ block แต่ UI แสดง warning)

**local_binary:**
- `binary_path` ต้องมีจริง + executable
- `model_path` ต้องมีจริง (ถ้าระบุ)
- `args_template` ต้องมี placeholder `{text}` และ `{output}`

### 5.2 Output validation (หลัง synthesize)

- Output file ต้องมีอยู่จริง
- ต้องเป็น WAV format (validate RIFF header)
- ขนาดไฟล์ > 0 bytes

---

## 6. Python Script Contract

FUNG คาดหวังให้ script ของ user รับ arguments แบบนี้:

```bash
python synthesize.py \
  --text "ข้อความที่ต้องการ" \
  --output /path/to/output.wav \
  --ref-audio /path/to/reference.wav   # optional — สำหรับ voice cloning
  --ref-text "ข้อความใน reference"      # optional
  --model /path/to/model               # optional
  --device cuda                        # cuda | cpu
```

**ข้อตกลง:**
- Exit code 0 = สำเร็จ → output file ต้องมี
- Exit code ≠ 0 = ล้มเหลว → stderr = error message
- Output format: WAV, 16-bit, mono
- Timeout: 30 วินาที (FUNG จะ kill process หลังหมดเวลา)

---

## 7. TTS Executor

Module ใหม่ `tts_executor.rs` ใน `src-tauri/src/`:

```
tts_executor.rs
├── dispatch(provider, text, ref_audio?, ref_text?) → Result<PathBuf>
│   ├── exec_python_script(config, text, ...) → Result<PathBuf>
│   ├── exec_rest_api(config, text, ...) → Result<PathBuf>
│   └── exec_local_binary(config, text, ...) → Result<PathBuf>
├── validate_output(path) → Result<()>
└── cleanup_temp(path) → ()
```

**Integration กับ delegated_jobs:**

```
delegated_jobs (operation = 'voice.synthesize')
    ↓
job_runner ตรวจเห็น job ใหม่
    ↓
อ่าน voice_profile → provider_id → config_json
    ↓
tts_executor::dispatch(provider, text, ref_audio)
    ↓
สำเร็จ → session state = 'completed', return audio_path
ล้มเหลว → session state = 'failed', log error
```

---

## 8. UI Design

### 8.1 Settings Panel — "Voice Synthesis Providers"

ตำแหน่ง: Settings panel (ขวาของ Desktop app) → หัวข้อใหม่

**สถานะว่าง (ยังไม่มี provider):**

```
🔊 Voice Synthesis Providers

  ยังไม่ได้ตั้งค่า TTS provider
  เพิ่ม provider เพื่อใช้งานเสียงสังเคราะห์

  ＋ เพิ่ม TTS Provider
```

**Registration form (ตัวอย่าง Python Script):**

```
ประเภท:   ○ Python Script  ○ REST API  ○ Local Binary

ชื่อ:      [ F5-TTS-THAI            ]
Venv:     [ D:\tts\.venv           ] 📁
Script:   [ D:\tts\synthesize.py   ] 📁
Model:    [ D:\tts\models\v1       ] 📁
Device:   ○ CUDA   ○ CPU

[ ทดสอบพูด ]   [ บันทึก ]
```

**Provider card (หลังลงทะเบียนแล้ว):**

```
┌──────────────────────────────────────┐
│  🟢 F5-TTS-THAI                     │
│  Python Script · CUDA               │
│  ทดสอบล่าสุด: ✅ 2.3 วินาที          │
│                                      │
│  [ ทดสอบ ]  [ แก้ไข ]  [ ปิดใช้งาน ]  │
└──────────────────────────────────────┘
```

สถานะ indicator:
- 🟢 = enabled + test ผ่าน
- 🟡 = enabled + ยังไม่เคย test / test เก่ามาก
- 🔴 = test ล้มเหลวล่าสุด
- ⚫ = disabled

### 8.2 ปุ่ม "ทดสอบพูด"

1. ส่งข้อความ `"ทดสอบระบบเสียง"` ไปที่ provider
2. แสดง spinner ระหว่างรอ
3. สำเร็จ → เล่นเสียงให้ฟัง + แสดง ✅ latency
4. ล้มเหลว → แสดง ❌ + error message (ตัด 500 chars)
5. บันทึกผลลง `tts_test_results`

### 8.3 ปุ่ม 🔊 ที่ P3 Recap (เฟสแรก)

- ข้าง AI recap text → ปุ่ม 🔊
- กด → เรียก `tts_synthesize_text` ด้วย recap text
- ระหว่างรอ: ปุ่มเปลี่ยนเป็น spinner
- สำเร็จ: เล่นเสียง + ปุ่มเปลี่ยนเป็น ⏸ (pause)
- ไม่มี provider: toast "ยังไม่ได้ตั้งค่า TTS → ไปที่ Settings" + link

### 8.4 Voice Profile → เลือก TTS Provider

ที่ CreativeStudio voice tab:
- Dropdown แสดง TTS providers ที่ `enabled = true` + ผ่าน test
- เลือก provider → บันทึก `provider_id` ลง `voice_profiles`
- ปลดล็อค agent voice grant button

---

## 9. Security

| ประเด็น | มาตรการ |
| ------- | ------- |
| Path traversal | Validate ว่า venv/script/binary/model path มีจริง ไม่ resolve symlink ออกนอก user space โดยไม่ warn |
| REST endpoint scope | Default restrict localhost / private IP ถ้า public URL → UI แสดง warning |
| Process timeout | Kill child process หลัง 30 วินาที |
| Temp files | Output WAV ลบหลังเล่นเสร็จ หรือหลัง session จบ |
| Config secrets | `auth_header` เก็บใน `config_json` ใน SQLite ที่อยู่ local — ไม่ส่งออก ไม่ sync |
| TTS ขณะ recording | บล็อค — ไม่ให้ synthesize ขณะ P1 capture active เพื่อกัน feedback loop |

---

## 10. Error Handling

| สถานการณ์ | พฤติกรรม |
| --------- | ------- |
| Provider ถูกปิด/ลบ ขณะ job ทำงาน | Job fail gracefully → session `failed` → UI แสดง "provider ไม่พร้อม" |
| Python process crash / timeout | Kill process → return error + stderr ตัด 500 chars |
| REST endpoint ไม่ตอบ | Timeout → error "endpoint ไม่ตอบสนอง" |
| Output WAV format ผิด | Validate RIFF header → error "ไฟล์เสียงไม่ถูกรูปแบบ" |
| venv/binary path หายไป | Health check fail → card แสดง 🔴 + ปุ่ม "ตรวจสอบอีกครั้ง" |
| ไม่มี provider ลงทะเบียน | Toast + link ไป Settings |

---

## 11. Testing Strategy

| Layer | วิธีทดสอบ |
| ----- | -------- |
| Migration | Unit test: migrate up → verify schema + constraints → migrate down |
| Validation (Rust) | Unit test: config_json ถูก/ผิด สำหรับแต่ละ runtime_type |
| tts_executor dispatch | Integration test: mock python script ที่ echo WAV header |
| UI registration flow | Manual test: เพิ่ม / แก้ไข / ปิด / ทดสอบ provider |
| End-to-end | กด 🔊 ที่ P3 recap → ได้ยินเสียงไทย |

---

## 12. Scope

### ✅ ทำในเฟสนี้

- Settings UI: เพิ่ม / แก้ไข / ปิด / ทดสอบ TTS provider
- Rust: register + validate + test + execute (`tts_executor.rs`)
- SQLite migration: เพิ่ม `kind = 'tts'` + ตาราง `tts_test_results`
- เชื่อม `voice_profiles` → TTS provider (dropdown)
- ปุ่ม 🔊 ที่ P3 recap (จุดแรกจุดเดียว)
- Python script contract documentation
- Health check + test synthesize flow
- เชื่อม `delegated_jobs` → `tts_executor` (ปลด MCP voice.speak)

### ❌ ไม่ทำ (เฟสถัดไป)

- Auto-download model (user ลง model เอง)
- Streaming TTS (real-time output ทีละ chunk)
- `cloud_api` runtime_type implementation
- ปุ่ม 🔊 ที่ P2 Transcript, Notes, Pitching Assist
- ตัวอย่าง wrapper script สำหรับ F5-TTS-THAI
- Periodic auto health check
- Usage analytics / tracking

---

## 13. File Changes Summary

| File | Change |
| ---- | ------ |
| `src-tauri/src/tts_executor.rs` | **ใหม่** — dispatch + exec + validate |
| `src-tauri/src/lib.rs` | เพิ่ม 5 commands + migration + register module |
| `src/tauri.ts` | เพิ่ม TypeScript wrappers สำหรับ 5 commands ใหม่ |
| `src/App.tsx` | Settings panel: เพิ่มหัวข้อ TTS Providers + ปุ่ม 🔊 ที่ P3 |
| `src/mobile/model.ts` | เพิ่ม `TtsProviderConfig` type |
| `src/mobile/CreativeStudio.tsx` | Voice tab: provider dropdown + ปลดล็อค grant |
