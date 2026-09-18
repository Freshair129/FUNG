# Changelog

## 0.1.0 — 2026-09-16 — Proposed

ฉบับแรกที่รวบรวม Quiet Archive identity และ mock Desktop/Companion ล่าสุดเป็น design-system specification โดยอิง product baseline จาก frontend redesign brief v1.0.0

เพิ่ม brand/semantic tokens แบบ light/dark/system, typography, geometry, motion, primitive/domain contracts, privacy/data-truth rules, surface inventory, API mapping, accessibility targets, acceptance plan และ phase/ADR proposals

บันทึกจุดขัดแย้งจาก mock อย่างชัดเจน: controls แบบ macOS ใน Windows baseline, waveform/search/pause ที่ยังไม่รองรับ, task persistence และ desktop note/clip actions, speaker identity, wordmark color และ unconditional privacy copy

Companion 4 presentation modes เป็น proposal ที่ต้องผ่าน native capability/ADR review ไม่ระบุว่า implement แล้ว มี token build และตรวจคู่สีทึบ 58 คู่ แต่ยังไม่ได้ทดสอบกับแอปจริง

### Implementation note — 2026-09-19

นำ semantic token layer, canonical currentColor mark, typography foundation และ surface mappings ไปเชื่อมกับ FUNG repository ใน local implementation slice แล้ว ครอบคลุม Desktop, Mobile, Web dashboard และ Landing; เพิ่มการสื่อสารสถานะ fixture/unavailable ใน Mobile เพื่อไม่แสดง waveform หรือผลลัพธ์ที่ไม่มีข้อมูลจริง

หลักฐานที่ผ่านคือ build, desktop/callmd contracts, live workspace, mobile capture, audio visualization และ local browser smoke check สำหรับ Landing/Desktop light-dark/Mobile การตรวจ packaged native, physical device และ full accessibility ยังไม่ run
