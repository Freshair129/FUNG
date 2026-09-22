---
version: "0.2.0b"
created_at: "2026-07-05T00:00:00+07:00,ATHER"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "product-spec"
  scope: "FUNG"
---

# Product Spec - FUNG Local Audio Intelligence

## Context

FUNG คือ AI native desktop app สำหรับบันทึกเสียงระยะยาว ถอดเทป วิเคราะห์ผู้พูด ตัดเสียงรบกวน แยก layer เสียง และสรุปเนื้อหาโดยเน้น local-first และ BYOM เป็นหลัก เพื่อลดความเสี่ยงด้านกฎหมาย ความเป็นส่วนตัว และการพึ่งพา cloud provider.

ลำดับแพลตฟอร์มคือ Desktop app first, Web app second, Mobile third. Desktop ต้องเป็นแหล่ง truth หลักสำหรับการบันทึกเสียง ประมวลผลไฟล์ local และจัดการ model/runtime. Web app และ mobile จะตามมาในฐานะ companion หรือ remote control เมื่อแกน desktop เสถียรแล้ว.

## Problems We're Solving

- ผู้ใช้ต้องบันทึกเสียงหลายชั่วโมงโดยไม่กลัวไฟล์เสียเมื่อแอปล่มหรือเครื่อง sleep.
- ผู้ใช้ต้องถอดเทปและจำแนกผู้พูดโดยไม่จำเป็นต้องส่งเสียงขึ้น cloud.
- ผู้ใช้ต้อง export ไฟล์เสียงและ transcript ในรูปแบบที่ใช้งานต่อได้จริง.
- ผู้ใช้ต้องวิเคราะห์เนื้อหา เจตนา และบทบาทของแต่ละผู้พูดโดยมีหลักฐานอ้างอิงจาก timestamp.
- ผู้ใช้ต้องควบคุม model เองผ่าน BYOM เพื่อความยืดหยุ่นด้านต้นทุน กฎหมาย และ data governance.

## Scope

### In Scope - V1 Desktop

- Long recording แบบ chunked auto-save.
- Local project library พร้อม SQLite WAL.
- Import audio file และ record จาก microphone.
- Export `.wav`, `.mp3`, transcript `.txt`, `.srt`, `.vtt`, `.json`.
- Transcription ผ่าน local/BYOM provider.
- Speaker diarization พร้อม rename speaker.
- Noise reduction และ speech enhancement.
- Layer view สำหรับ original, cleaned voice, separated voice/music/noise และ selected clips.
- Summary ทั้งเรื่อง และ summary ตามผู้พูด.
- Intent analysis รายผู้พูดแบบ evidence-based พร้อม confidence/uncertainty.
- Local API, MCP endpoint, และ CLI สำหรับ automation.

### Out of Scope - V1

- Mobile recording เป็น primary recorder.
- Cloud sync เป็นค่าเริ่มต้น.
- Realtime collaborative editing.
- Full DAW feature set เช่น MIDI, plugin chain, advanced mixing.
- Legal conclusion อัตโนมัติจาก AI.
- Biometric identity ยืนยันตัวบุคคลแบบ definitive.

### Future Considerations - V2+

- Web companion สำหรับ review/export.
- Mobile companion สำหรับ remote recorder หรือ field notes.
- Optional encrypted sync.
- Team workspace.
- Model marketplace/profile presets.
- Advanced audio editing timeline.

## User Experience Direction

Design direction: Apple Japan, Quiet Luxury, Minimal Cozy.

Product feeling:

- เงียบ เนี้ยบ ไม่รก.
- งานหลักอยู่หน้าแรก ไม่ใช่ landing page.
- สีอุ่น สุขุม อ่านง่าย เหมาะกับงานเสียงและเอกสารยาว.
- interaction ต้องให้ความรู้สึกเป็นเครื่องมือมืออาชีพ แต่ไม่แข็งแบบ enterprise.

## Proposed live meeting and agent features — 2026-09-21

ส่วนนี้เป็น candidate extension ของ baseline เดิม ไม่ใช่สถานะว่าฟีเจอร์พร้อมใช้ ความเสี่ยง HIGH / C-3; ดู [domain map](../architecture/MEETING_INTELLIGENCE_DOMAINS.md) และ [Desktop current truth](08-real-progress.md)

| Feature | Product behavior | Detail |
| --- | --- | --- |
| Live transcript | เห็นข้อความระหว่างประชุม แยกกำลังถอด/บันทึกแล้ว เวลา ผู้พูด ความหน่วง และช่วงข้อมูลขาด | [Live spec](../specs/2026-09-21-live-meeting-transcription-spec.md) |
| API speaker attribution | ใช้ชื่อ/participant-session จาก Google Meet พร้อม source badge; voiceprint ไม่ใช่ prerequisite | [Speaker spec](../specs/2026-08-23-speaker-identification-and-voice-profile-spec.md) |
| Meeting knowledge | เลือกคลัง/บริษัท/ปีงบประมาณ ค้นเอกสารจริงพร้อม citation และสิทธิ์แชร์ | [Knowledge spec](../specs/2026-09-21-meeting-knowledge-evidence-spec.md) |
| Participating Agent | เข้า Google Meet อย่างเปิดเผย ตอบเมื่อเรียก และค้นตามบริบทเมื่อเปิด policy | [Agent spec](../specs/2026-09-21-meeting-agent-participation-spec.md) |
| Same-channel evidence | ส่งคำตอบและลิงก์เอกสารที่มีสิทธิ์ไป Meet chat; native attachment/voice output มี gate แยก | [API strategy](../decisions/2026-09-21-google-meet-agent-api-strategy.md) |

User story: กล่าวถึงยอดขายปีที่แล้ว → resolve บริษัท/นิยามยอดขาย/ช่วงปี → ค้นรายงานฉบับที่อนุญาต → ตอบตัวเลขพร้อมหน้า/เซลล์อ้างอิง → ส่งเฉพาะข้อมูลที่ผู้รับมีสิทธิ์. หากปี/หน่วย/เอกสารขัดกัน ต้องถามหรือแสดงข้อจำกัด ไม่เดาตัวเลข

Default เป็น local-first และ private draft; online meeting-media, cloud inference, voice recognition และ external publication เป็นสิทธิ์แยก เปิด proactive publication ได้ด้วย session policy ที่จำกัดขอบเขต ไม่ใช่ทุกคำพูดเป็นคำสั่ง. Guest/shared-room/unknown speakers ต้องใช้งานได้โดยไม่บังคับบัญชี FUNG

Release acceptance ของ extension: real Meet join, speaker source mapping, live transcript, cited local-knowledge answer, same-room link visible to authorized recipient, stop/revoke, restart/no-duplicate และ provider/packaged evidence. Local copilot alone ไม่ปิดฟีเจอร์ Agent เข้าร่วมประชุม. Mobile/Web rollout ไม่ได้รวมโดยอัตโนมัติ

## Acceptance Criteria

- อัดเสียงต่อเนื่องได้อย่างน้อย 3 ชั่วโมงโดยไม่มี memory growth ผิดปกติ.
- เมื่อ app crash ระหว่างบันทึก สามารถ recover chunk ที่บันทึกแล้วได้.
- Project ทุกตัวมี local state ผ่าน GenesisBlockDB operational boundary และ WAL/custody ที่ระบบนั้นเป็นเจ้าของ.
- ผู้ใช้สามารถถอดเทปไฟล์เสียงหนึ่งรายการและแก้ speaker label ได้.
- ผู้ใช้สามารถ export audio เป็น `.wav` และ `.mp3`.
- ผู้ใช้สามารถ export transcript พร้อม timestamp.
- ผู้ใช้สามารถเลือก local model provider ได้อย่างน้อย 1 แบบ.
- Summary ต้องอ้างอิงช่วงเวลาใน transcript.
- Intent analysis ต้องแสดงว่าเป็น inference ไม่ใช่ fact.

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.0b | Added candidate live transcript, Google Meet speaker attribution, scoped knowledge and participating-agent product requirements. |
| 0.1.0b | Initial product spec for desktop-first local audio AI app with BYOM and legal/privacy boundaries. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.0b | 2026-09-21 | candidate | Added candidate live transcript, Google Meet speaker attribution, scoped knowledge and participating-agent product requirements. | working-tree | RWANG |
| 0.1.0b | 2026-07-05 | beta | Initial product spec. | N/A | ATHER |
