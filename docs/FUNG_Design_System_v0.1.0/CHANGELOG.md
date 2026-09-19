# Changelog

## 1.0.0b — 2026-09-20 — Beta

อัปเดต current DesktopShell ให้ตรงกับ implementation: recording อยู่บน Home/Live surface, Appearance เป็นหน้าแยก, sidebar ไม่มี controls ของ theme/material/transparency, header ไม่มีปุ่มย่อ/ปิด และมี profile/login state; commit `285566b` พร้อม build, tests และ WebView evidence แล้ว

## 0.4.0b — 2026-09-19 — Beta

ปรับ desktop shell ตาม approved Liquid Glass refresh: ลบ legacy presentation tree, ย้ายเมนูเข้า hover/focus rail, เพิ่ม FUNG/QUIET ARCHIVE lockup, profile CTA, ambient motion และ responsive surface layout; browser/build/regression ผ่าน แต่ native exact-executable click-through รอบนี้ถูกบันทึกเป็น `BLOCKED_ENVIRONMENT`

## 0.3.2 — 2026-09-19 — Beta

บันทึกผล bounded native desktop click-through บน `src-tauri/target/release/fung.exe`: ตรวจ Liquid Glass controls, Companion, Home → Live → Review, Settings open/close และ stale binary distinction; installer, clean install, production และ full accessibility ยังไม่ run

## 0.3.1 — 2026-09-19 — Beta

บันทึกผล implementation ของ Liquid Glass desktop adaptation: local build, browser visual smoke และ contract checks ผ่าน; native click-through และ installer evidence ยังไม่ run

## 0.3.0 — 2026-09-19 — Beta

อนุมัติ `LIQUID_GLASS_DESKTOP_ADAPTATION.md` สำหรับ implementation แบบจำกัดขอบเขต โดยคง backend/command contract เดิม ใช้ in-app Companion เท่านั้น และแยก native overlay ออกจากรอบนี้

## 0.1.0 — 2026-09-19 — Beta

ได้รับ approval ให้ freeze FUNG Quiet Archive Design System เป็น design authority ระดับ Beta สำหรับ implementation ต่อเนื่อง โดยคง `implementation_verified: false` เพราะ installer click-through, physical-device UAT และ full accessibility audit ยังไม่ run

## 0.1.0 — 2026-09-16 — Proposed

ฉบับแรกที่รวบรวม Quiet Archive identity และ mock Desktop/Companion ล่าสุดเป็น design-system specification โดยอิง product baseline จาก frontend redesign brief v1.0.0

เพิ่ม brand/semantic tokens แบบ light/dark/system, typography, geometry, motion, primitive/domain contracts, privacy/data-truth rules, surface inventory, API mapping, accessibility targets, acceptance plan และ phase/ADR proposals

บันทึกจุดขัดแย้งจาก mock อย่างชัดเจน: controls แบบ macOS ใน Windows baseline, waveform/search/pause ที่ยังไม่รองรับ, task persistence และ desktop note/clip actions, speaker identity, wordmark color และ unconditional privacy copy

Companion 4 presentation modes เป็น proposal ที่ต้องผ่าน native capability/ADR review ไม่ระบุว่า implement แล้ว มี token build และตรวจคู่สีทึบ 58 คู่ แต่ยังไม่ได้ทดสอบกับแอปจริง

### Implementation note — 2026-09-19

นำ semantic token layer, canonical currentColor mark, typography foundation และ surface mappings ไปเชื่อมกับ FUNG repository ใน local implementation slice แล้ว ครอบคลุม Desktop, Mobile, Web dashboard และ Landing; เพิ่มการสื่อสารสถานะ fixture/unavailable ใน Mobile เพื่อไม่แสดง waveform หรือผลลัพธ์ที่ไม่มีข้อมูลจริง

หลักฐานที่ผ่านคือ build, desktop/callmd contracts, live workspace, mobile capture, audio visualization, local browser smoke check สำหรับ Landing/Desktop light-dark/Mobile และ local release executable launch smoke ที่เห็น main window `FUNG` การตรวจ installer click-through, physical device และ full accessibility ยังไม่ run
