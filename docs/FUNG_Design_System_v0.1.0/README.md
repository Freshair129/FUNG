# FUNG — Quiet Archive Design System

**Version 0.1.0 · Beta · approved and frozen 19 September 2026**

เริ่มอ่านที่ [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) เอกสารภาษาไทยพร้อมชื่อ component/token/API ภาษาอังกฤษ ครอบคลุม Desktop, Mobile, Web dashboard, Landing, Phone page และข้อเสนอ Companion Overlay

## ไฟล์หลัก

| ไฟล์ | การใช้ |
| --- | --- |
| [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) | หลักการ, logo, tokens, component contracts, flow/state, surface mapping, overlay, QA และ implementation plan |
| [fung.tokens.css](tokens/fung.tokens.css) | Semantic tokens แบบ Light/Dark/System ที่ generate แล้ว |
| [fung.tokens.json](tokens/fung.tokens.json) | Source ของ token ในแพ็กนี้; project-specific schema |
| [build_tokens.py](tokens/build_tokens.py) | Rebuild CSS และตรวจคู่สีด้วย Python standard library |
| [CONTRAST_REPORT.md](qa/CONTRAST_REPORT.md) | ผลตรวจคู่สีทึบ 58 คู่ตามเป้าหมายภายในโครงการ |
| [PACKAGE_CHECK.md](qa/PACKAGE_CHECK.md) | รายงานตรวจความครบของแพ็กและข้อจำกัดที่ยังไม่ได้ทดสอบ |
| [CHANGELOG.md](CHANGELOG.md) | ประวัติฉบับเอกสาร |
| `references/` | Brief ต้นทาง, logo concept และ mock ล่าสุดสองภาพ |

## ฐานข้อมูลและสถานะ

เอกสารฉบับนี้เป็น design source ที่ได้รับอนุมัติและ freeze ในสถานะ **Beta** โดยอิง `FRONTEND_REDESIGN_BRIEF.md` v1.0.0 รอบ implementation วันที่ 19 กันยายน 2026 ได้นำ token และ visual foundation ไปผูกกับ FUNG repository แล้ว แต่สถานะนี้ยังไม่ใช่การรับรองว่า component และ Companion ทุกส่วนในเอกสารถูก implement ครบ หรือพร้อม production

ภาพ mock เป็น reference ไม่ใช่หลักฐานว่า backend รองรับปุ่มทุกตัวแล้ว เอกสาร §2 มี discrepancy register; §16 แยก native overlay ออกจาก panel ภายในแอป; §21 ผูก UI กับ API ที่ brief ระบุและชี้จุดที่ยังไม่มี contract

## สถานะการ integrate ล่าสุด

รอบนี้เชื่อม `tokens/fung.tokens.css` กับ global styles และ surface roots ของ Desktop, Mobile, Web dashboard และ Landing; ใช้ canonical mark แบบ `currentColor`; และเพิ่มสถานะที่แยกข้อมูลจริงออกจาก fixture/ข้อมูลที่ยังอ่านไม่ได้ใน Mobile โดยไม่เปลี่ยน bridge, auth, routing หรือ native command contract

หลักฐาน local ล่าสุด:

- `npm run build` — PASS
- `npm run test:callmd-contracts`, `npm run test:callmd-live`, `npm run test:callmd-shell`, `npm run test:mobile`, `npm run test:audio-viz` — PASS
- `npm run test:release`, `npm run test:desktop-bootstrap` — PASS
- Browser smoke — Landing, Desktop light/dark และ Mobile route render ได้; ไม่มี console warning/error ในรอบตรวจ
- Local release executable launch smoke — `src-tauri/target/release/fung.exe` เปิด main window ชื่อ `FUNG` ได้; graceful-close probe ไม่ถือเป็นหลักฐาน packaged acceptance
- ยังไม่ทำ installer click-through, physical-device UAT หรือ full accessibility audit

## ใช้ token

นำ CSS เข้า build pipeline ที่มีอยู่ และกำหนด root ของ surface:

```html
<div data-fung-root data-theme="system" lang="th">...</div>
```

เลือก `light`, `dark` หรือ `system` ได้ แต่การ persist preference, event handling, permissions และ native window behavior ต้อง implement แยก CSS ไม่มี network imports และไม่มี font files แนบมา

การนำ token ไปไว้ใน repository ต้องรวม ownership กับ brand tokens เดิม ไม่ทิ้งสองแหล่งความจริงให้ drift แยกกัน

## Rebuild / check

รันจากโฟลเดอร์แพ็กด้วย Python 3.9+:

```sh
python tokens/build_tokens.py
python tokens/build_tokens.py --check
```

คำสั่งแรกเขียน CSS และรายงาน contrast ใหม่จาก JSON คำสั่งที่สองตรวจว่า output ตรงกับ source โดยไม่แก้ไฟล์ และคืน non-zero เมื่อคู่สีไม่ผ่านหรือไฟล์ไม่ตรงกัน

## ขอบเขตการส่งมอบ

มีเอกสารและ token files พร้อมภาพอ้างอิง แพ็กเกจนี้ไม่ bundle repository patch, font files, Figma/Penpot source หรือ native overlay implementation; integration patch และ runtime evidence อยู่ใน repository แยกจากแพ็กนี้ และยังไม่ใช่การรับรอง accessibility ของแอป

อ่านพร้อมภาพและ relative links ให้แตก ZIP ทั้งโฟลเดอร์ ไม่ย้ายเฉพาะ Markdown ออกจากโครงสร้าง
