---
version: "0.1.0b"
created_at: "2026-10-22"
last_update: "2026-10-22"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "visual-evidence"
  scope: "FUNG Desktop meeting workflow MVP mockups"
  language: "Thai-first"
---

# Desktop meeting workflow MVP mockups

ภาพในโฟลเดอร์นี้เป็นหลักฐานภาพของแนวทางที่ใช้เริ่ม implement ใน FUNG Desktop
สร้างด้วย ImageGen เมื่อ 2026-10-22 และเก็บต้นฉบับ PNG ไว้เพื่อเทียบ visual QA
ภาพเป็น design reference เท่านั้น ไม่ใช่ภาพจากแอปที่ build แล้ว และไม่ยืนยันว่า
feature หรือการจับเสียงทำงานจริง

## ภาพหลัก

| หน้าหรือสถานะ | ภาพ | SHA-256 |
| --- | --- | --- |
| Home | [home.png](home.png) | `A451C97E9DB6823068C80268F17319C2FC11CEEF0A2641427515B5CC10B02308` |
| Live setup: เลือกเสียงระบบหรือแอป | [live-setup-source-routing.png](live-setup-source-routing.png) | `CC435136A1B487F74E5541FCA7498A52F15120B16FF428E0D0BF6F7731F27BD7` |
| Live recording: waveform แยกไมค์และเสียงที่เลือก | [live-recording-channel-waveforms.png](live-recording-channel-waveforms.png) | `6FABA4E5BB5183F5D43F9D7C273941DE6017BCC84CD999A8CDA3503698C4893C` |
| Review | [review.png](review.png) | `A4A2BFC92B8AAB2570A2FF2E4CC7E94E8F4A5B1EE53790B046B2E403F92BB627` |
| Files / library support view | [files-library.png](files-library.png) | `41B7D05B614B08398E5B1DB171FE92709FDF19EF13E66FE8AF990337BDE04A3F` |
| Companion support panel | [companion.png](companion.png) | `A8093CF18509BE48F1C1BBFC465228799B944E51B010A1C2A209DC18A0293389` |
| Settings right drawer with tabs | [settings-source-tabs.png](settings-source-tabs.png) | `A0394B6C63E4142FE5240EC61181F218E10C28D8DC3922BC07BC05F5767A505D` |

## Iterations

ภาพใน [iterations](iterations/) เก็บทางเลือกและร่างก่อนหน้าเพื่อรักษาที่มา
ภาพเหล่านั้นไม่ใช่ spec ปัจจุบัน; ใช้ภาพในตารางหลักร่วมกับ
[Studio flow contract](../../2026-10-16-desktop-navigation-and-flows.md)
และ token ใน [Design System](../../../FUNG_Design_System_v0.1.0/README.md)
เป็นแนวทาง implement

| ภาพร่างที่เก็บไว้ | SHA-256 |
| --- | --- |
| [exploratory-home-settings-grid.png](iterations/exploratory-home-settings-grid.png) | `5AECC9250D4AF333EAD5441062CED7D3AB4541C807C6EB2E0ADD5622027A4E07` |
| [settings-initial-panel.png](iterations/settings-initial-panel.png) | `8F8040F25A25ADF6DE62FEF1D2284C62976DE993D022BDDDBC33D656EDC4AF2E` |
| [settings-tab-variant.png](iterations/settings-tab-variant.png) | `C12A5489BDC5A7B75658AF13F6C32D1A9554D2FDA79F4D084060C84CFC692F5A` |
| [settings-vertical-navigation-variant.png](iterations/settings-vertical-navigation-variant.png) | `0E2E06EF2A9884270610D64DF8BCC50E0006D26E4FA21A813A1ED265F14A8B5C` |

## ขอบเขตของภาพ

- คง navigation หลัก Home, Live และ Archive; Files เป็น action/support view และ
  Companion เป็น utility panel ไม่เพิ่ม primary route ตามภาพเพียงอย่างเดียว
- ข้อความประชุม วันเวลา ผู้เข้าร่วม งานและระดับเสียงในภาพเป็นตัวอย่างประกอบ
  ไม่ใช่ข้อมูลจริง ห้ามย้ายเป็น hard-coded production data
- นัดหมายที่ยังไม่มี calendar provider ต้องแสดง empty state ตามข้อมูลจริง
- waveform ระหว่างอัดต้องมาจาก sample ที่ capture จริง แยกไมค์กับ source ที่เลือก;
  เมื่อไม่มี sample ให้แสดงสถานะรอสัญญาณแทนกราฟตกแต่ง
- การเลือกเสียงรายแอปต้องแสดงเฉพาะ session ที่กำลังส่งเสียง และ native capture
  ต้องตรวจ source เดิมซ้ำเมื่อเริ่มอัด
