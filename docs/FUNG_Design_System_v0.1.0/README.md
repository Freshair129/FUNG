# FUNG — Quiet Archive Design System

**Version 0.1.0 · Beta · token authority and board updated 16 October 2026**

เริ่มอ่านที่ [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) เอกสารภาษาไทยพร้อมชื่อ component/token/API ภาษาอังกฤษ ครอบคลุม Desktop, Mobile, Web dashboard, Landing, Phone page และข้อเสนอ Companion Overlay

## ไฟล์หลัก

| ไฟล์ | การใช้ |
| --- | --- |
| [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) | หลักการ, logo, tokens, component contracts, flow/state, surface mapping, overlay, QA และ implementation plan |
| [LIQUID_GLASS_DESKTOP_ADAPTATION.md](LIQUID_GLASS_DESKTOP_ADAPTATION.md) | Approved beta addendum สำหรับแปลง `Downloads\fung-new-ui` เป็น desktop Liquid Glass; current shell refresh มี local implementation/build/browser evidence แล้ว แต่ native click-through รอบนี้ยัง `BLOCKED_ENVIRONMENT` และ installer/clean-install/production ยังเปิดอยู่ |
| [fung.tokens.css](tokens/fung.tokens.css) | Semantic tokens แบบ Light/Dark/System ที่ generate แล้ว |
| [fung.tokens.json](tokens/fung.tokens.json) | Canonical token source ตาม DTCG Format Module 2025.10; FUNG metadata อยู่ใน namespaced `$extensions` |
| [build_tokens.py](tokens/build_tokens.py) | ตรวจ token types ที่ FUNG ใช้ และสร้าง CSS, contrast report และ Brand Kit compatibility export |
| [Shared design board](../design-system/index.html) | HTML board ที่สร้างจาก token source เดียว มี searchable catalog, cross-surface examples, theme switch และ print |
| [Brand presentation](../design-system/brand/index.html) | Brand/token presentation ที่สร้างจาก token source เดียว |
| [render_design_system.mjs](../../scripts/render_design_system.mjs) | สร้าง board และ brand presentation; ห้ามแก้ HTML output โดยตรง |
| [CONTRAST_REPORT.md](qa/CONTRAST_REPORT.md) | ผลตรวจคู่สีทึบ 58 คู่ตามเป้าหมายภายในโครงการ |
| [COMPONENT_COVERAGE.md](qa/COMPONENT_COVERAGE.md) | Generated source inventory ของการอ้าง canonical token ใน stylesheet ของแอป |
| [ACCESSIBILITY_REVIEW.md](qa/ACCESSIBILITY_REVIEW.md) | รายงาน combined UX/accessibility ของ Desktop Home, Live, History, Files และ Appearance พร้อมภาพ preview, skip-link evidence, findings และข้อจำกัด |
| [PACKAGE_CHECK.md](qa/PACKAGE_CHECK.md) | รายงานตรวจความครบของแพ็กและข้อจำกัดที่ยังไม่ได้ทดสอบ |
| [CHANGELOG.md](CHANGELOG.md) | ประวัติฉบับเอกสาร |
| `references/` | Brief ต้นทาง, logo concept และ mock ล่าสุดสองภาพ |

## ฐานข้อมูลและสถานะ

เอกสารฉบับนี้เป็น design authority สถานะ **Beta** โดยอิง `FRONTEND_REDESIGN_BRIEF.md` v1.1.0b และ current shell addendum. ณ 16 ตุลาคม 2026 token source ใช้ DTCG 2025.10 conventions และสร้าง runtime CSS, Brand Kit export, contrast report และ shared board ได้จาก JSON เดียว ส่วน component coverage ยัง partial; `implementation_verified: false` จึงคงไว้สำหรับขอบเขตทั้ง design system ไม่ใช่การปฏิเสธ token integration ที่ตรวจแล้ว

`LIQUID_GLASS_DESKTOP_ADAPTATION.md` เป็นเอกสารลูกสถานะ **Beta** จาก reference `C:\Users\pc\Downloads\fung-new-ui` v0.3.0 ซึ่งเป็น mobile HTML prototype และ Companion concept ที่ไม่มี desktop production integration เดิม เอกสารได้รับ approval สำหรับ implementation แบบจำกัดขอบเขตแล้ว การ implement ที่มีอยู่เป็น local evidence ไม่เปลี่ยนสถานะ Beta ของ `DESIGN_SYSTEM.md` และไม่ถือเป็น production readiness

ภาพ mock เป็น reference ไม่ใช่หลักฐานว่า backend รองรับปุ่มทุกตัวแล้ว เอกสาร §2 มี discrepancy register; §16 แยก native overlay ออกจาก panel ภายในแอป; §21 ผูก UI กับ API ที่ brief ระบุและชี้จุดที่ยังไม่มี contract

## สถานะการ integrate ล่าสุด

`tokens/fung.tokens.json` เป็นแหล่งแก้ไข token เพียงแห่งเดียว; CSS ที่ runtime ใช้, `docs/brand-kit/tokens.css`, contrast report และ board เป็น generated outputs. Mobile legacy URLs redirect ไป shared board. Liquid Glass values ที่ Desktop ใช้อยู่ถูกรวมใน source เดียวกัน โดย public `--fung-*` names ยังคงเดิม

หลักฐานและขอบเขตปัจจุบัน:

- Token CSS integration — verified in repository; generator currently validates 202 tokens and 58 declared solid-color contrast pairs.
- Stylesheet token integration — source-audited; all 24/24 application CSS files reference canonical tokens, no active flat single-color declaration remains, and canonical token fallbacks no longer duplicate the source values. Embedded colors remain in layered effects and illustrations; stylesheet references do not prove every component/property has full semantic mapping or rendered parity. See [coverage inventory](qa/COMPONENT_COVERAGE.md).
- Component implementation status — still partial; token presence and direct-color migration do not prove complete component contracts or accessibility acceptance. `implementation_verified: false` remains accurate.
- Accessibility certification — not completed; current evidence covers five Desktop shell routes in a 1280×720 light-theme preview, plus Home focus/skip-link behavior and Appearance selector tab order. The route review records separate sidebar/main scroll regions; a Live action label mismatch found during review is corrected in the current branch. See the [review and screenshots](qa/ACCESSIBILITY_REVIEW.md).
- Figma/Penpot and complete per-screen exports — not included; the local HTML board is the available review/handoff artifact.
- Native desktop, installer/clean-install, physical-device UAT and production gates remain separate; see [package checks](qa/PACKAGE_CHECK.md) and the [Liquid Glass evidence](LIQUID_GLASS_DESKTOP_ADAPTATION.md).

## ใช้ token

นำ CSS เข้า build pipeline ที่มีอยู่ และกำหนด root ของ surface:

```html
<div data-fung-root data-theme="system" lang="th">...</div>
```

เลือก `light`, `dark` หรือ `system` ได้ แต่การ persist preference, event handling, permissions และ native window behavior ต้อง implement แยก CSS ไม่มี network imports และไม่มี font files แนบมา

Brand Kit เป็น generated compatibility export จาก canonical source; แก้ token ที่ JSON แล้ว rebuild แทนการแก้ CSS output.

## Rebuild / check

รันจาก repository root:

```sh
npm run design-system:render
npm run design-system:check
npm run test:design-system
```

`render` สร้าง outputs ทั้งหมดจาก JSON และสร้าง source coverage inventory; `check` ยืนยัน CSS, report, coverage inventory และ board ตรงกับ source โดยไม่แก้ไฟล์; test ตรวจ token conventions และ publishing contracts. Validator รองรับ DTCG token types ที่ FUNG ใช้ ยังไม่ใช่ validator เต็มรูปแบบของ DTCG.

## ขอบเขตการส่งมอบ

W3C Design Tokens Community Group Format Module 2025.10 เป็น stable community-group specification/report ไม่ใช่ W3C Standard. FUNG ใช้รูปแบบ token/group และ `$extensions` ที่เกี่ยวข้อง โดยคง validator scope ตามที่ระบุข้างต้น. ไม่มี font binaries, native Figma/Penpot source, complete screen exports หรือการรับรอง accessibility ของทั้งแอปแนบมา

อ่านพร้อมภาพและ relative links ให้แตก ZIP ทั้งโฟลเดอร์ ไม่ย้ายเฉพาะ Markdown ออกจากโครงสร้าง
