---
version: "0.1.2b"
created_at: "2026-07-21T06:10:00+07:00,ATHER"
last_update: "2026-07-21T06:42:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "mobile-design-system"
  doc_type: "publishing-architecture-proposal"
  scope: "FUNG Mobile design system source and interactive reference"
  language: "Thai"
---

# FUNG Mobile Design System — Single Source of Truth Proposal

## Decision requested

ใช้ Markdown เพียงไฟล์เดียวเป็นแหล่งความจริง (SOT) สำหรับ FUNG Mobile design system และสร้าง HTML แบบ interactive จากไฟล์นั้นอัตโนมัติ

| Layer | Canonical path | Ownership | Editing rule |
| --- | --- | --- | --- |
| Design-system source | `docs/Mobile/DESIGN_SYSTEM.md` | Product/design | แก้โดยตรงได้; เป็น SOT เดียว |
| Interactive reference | `docs/Mobile/design-system/index.html` | Generated artifact | ห้ามแก้โดยตรง; สร้างใหม่จาก SOT เท่านั้น |
| Renderer | `scripts/render_mobile_design_system.mjs` | Tooling | แปล Markdown block ที่กำหนดเป็น HTML |
| Visual references | `docs/Mobile/references/clony-ai-voice-face-ui-kit/` | Immutable input | ห้ามแก้/ทับ; ใช้เพื่ออ้างอิง composition เท่านั้น |
| Brand presentation | `docs/Mobile/design-system/brand/index.html` | Generated artifact | HTML เรียงแบบ case study; export ภาพยาวจาก HTML เดียวกัน |

## Why this structure

- **MD เป็น SOT:** review ใน Git, diff, version history, product rationale และ token อยู่ด้วยกันในที่เดียว
- **HTML มีหน้าที่สื่อสาร:** เปิดดูบน browser เพื่อสลับ light/dark, ตรวจ component state, portrait/landscape และ share ให้ผู้เกี่ยวข้องทดลองได้
- **ไม่เกิด two-way drift:** HTML ไม่มี token ที่เขียนซ้ำเอง; ทุก colour, type, spacing และ component sample ถูกอ่านจาก Markdown block เดียว
- **เหมาะกับ FUNG:** รักษา product truth เช่น local-first, evidence, runtime provenance และ MCP consent ควบคู่กับ visual token ไม่ใช่เก็บแต่สี/spacing

## Source format contract

`docs/Mobile/DESIGN_SYSTEM.md` จะมีส่วนที่อ่านง่ายสำหรับคน และมี structured block สำหรับ renderer ดังนี้:

````md
<!-- fung-mobile-design-system:data:start -->
```json
{
  "version": "…",
  "themes": { "light": {}, "dark": {} },
  "type": {},
  "spacing": [],
  "radii": {},
  "components": {}
}
```
<!-- fung-mobile-design-system:data:end -->
````

ข้อมูลใน JSON block นี้เป็นส่วนหนึ่งของ Markdown SOT ไม่ใช่ไฟล์ token ชุดที่สอง. Renderer ต้อง fail หาก block หาย, JSON ไม่ valid หรือ version ของ HTML ไม่ตรงกับ SOT.

## Interactive HTML scope

หน้า generated HTML จะต้องมี:

1. theme switcher: porcelain / dark work canvas;
2. token swatches และ semantic usage ที่ copy value ได้;
3. type, spacing, radius, elevation reference;
4. interactive state ของ button, chip, search field, voice control, waveform/player, note row และ runtime/provenance badge;
5. viewport preview ที่เปลี่ยน portrait / responsive landscape โดยไม่ใช้ desktop UI;
6. visual-reference panel ที่ link ภาพต้นฉบับและระบุ “adapt grammar, not brand/assets”; และ
7. source version, generation timestamp และคำเตือนว่าไฟล์ถูก generate จาก `DESIGN_SYSTEM.md`.

HTML นี้เป็นเอกสารสื่อสารและ QA visual contract; ไม่ใช่ production UI, ไม่ import runtime, และไม่ใช้เป็น source ของ CSS แอป.

## Brand / CI presentation format

ให้ทำ **HTML vertical case study เป็นรูปแบบหลัก** คล้ายจังหวะการเล่าเรื่องของ reference: ผู้อ่าน scroll จาก rationale ไปสู่ logo construction, token และ product application ได้ในหน้าเดียว. จาก HTML เดียวกันจึง export `brand-presentation.png` (ภาพยาว) และ PDF สำหรับส่งต่อแบบนิ่งได้.

การเลือก HTML เป็นหลักดีกว่าภาพยาวที่แก้มือ เพราะ:

- เปลี่ยน logo clear-space, colour หรือ type token จาก Markdown แล้ว component preview และ export ปรับพร้อมกัน;
- เปิดสลับ light/dark และตรวจ contrast, scale, logo variants ได้จริง;
- URL/ไฟล์เดียวกันใช้ review กับทีมได้โดยไม่ต้องตีความจากภาพนิ่ง; และ
- ยังคงมีภาพยาวสำหรับ stakeholder ที่ต้องการไฟล์เดียว.

### Required narrative sequence

1. **Cover / Brand idea** — FUNG คือ local-first voice intelligence; ไม่มี claim clone voice หรือ cloud-first.
2. **Primary logo** — wordmark + mark, black/porcelain/dark-canvas variants.
3. **Construction** — grid, geometry, optical correction, clear space, minimum size และ misuse examples.
4. **Colour system** — porcelain, ink, indigo focus, sage local, warm-metal inference, signal red; ระบุ semantic meaning และ contrast.
5. **Typography and icon voice** — Thai-first fallback, numeric/timer behaviour, rounded icon rules.
6. **Tactile material and motion** — pressed state, quiet elevation และ reduced-motion behaviour.
7. **Product application** — voice control, capture, note evidence, DAW timeline, Mobile portrait และ responsive landscape.
8. **Reference boundary** — source image, what was adapted, what is prohibited, and FUNG-specific deviation.

`07-brand-presentation-structure.png` is retained as the presentation-structure reference (SHA-256 `A924F9E5986D368654230850CC4C2F019A834FA8900B01762225A2A31C164432`). FUNG may adapt the grid-construction and sequential-case-study approach only. It must not copy the depicted logo geometry, mark, type treatment or brand identity.

The actual FUNG logo mark has not been approved yet. The HTML will therefore render a labelled **brand-mark proposal** only after the logo concepts and their selected option receive a separate visual approval; it must not silently invent a production logo.

## Reference interpretation boundary

ภาพที่ผู้ใช้ให้มาถูกเก็บครบแล้วที่ `docs/Mobile/references/clony-ai-voice-face-ui-kit/` พร้อม SHA-256 เพื่อป้องกันความสับสนของต้นฉบับ:

| File | SHA-256 | What FUNG may adapt |
| --- | --- | --- |
| `01-overview.png` | `C12CDC3CC01A10D324F7B52AAD1C0D8F14119C8E0C3C570237B69EF34E7B27DB` | dark surface hierarchy, bold focal action, capsule dock |
| `02-voice-library.png` | `E52E2F2A41BCC17A0F67873ED7F6DBB52823A21B7C3838A7CD2858A4C41E82CB` | search/filter cadence, compact voice cards, waveform player |
| `03-workflow-montage.png` | `FB6616019E9CA7A7127F6D98F152D90CAE93C5B9BC25B3A903F9287666254CD2` | one-next-action workflow, full-width primary CTA |
| `04-design-system.png` | `8EADBA6EB4BEF63A33353F08707587A84817DB1D5F70422DEE934D9C5990F0FB` | documented tokens, type scale, component-state presentation |
| `05-home-assistant-device-shot.png` | `1F150CB59F137C9B9F73D94C6DB31C3DD37A03BE46E9E4C862758956ED944DF3` | mobile dark-canvas density and owned navigation |
| `06-voice-output-device-shot.png` | `15080ACCA9D03CA5BA6ADD51F9733C7E072ED05174F7FB2AD44C6F7CA1E18BD5` | selection/result flow and clear success state |
| `07-brand-presentation-structure.png` | `A924F9E5986D368654230850CC4C2F019A834FA8900B01762225A2A31C164432` | logo-grid and vertical case-study presentation structure |

FUNG may reuse only the interaction grammar: dark forest/ink canvas, clear primary voice focus, rounded work surfaces, touch-safe capsules, waveform playback and owned mobile navigation. It must not reuse the reference’s logo, wording, avatars/faces, robot, subscription model, cloning claim, lime/neon palette or image-generation/video workflow.

FUNG CI remains the semantic system in `CLONY_INSPIRED_MOBILE_TOKEN_PROPOSAL.md`: indigo for focus, sage for local/confirmed, warm metal for inference, and signal red for recording/destructive actions.

## Existing-document disposition

| Existing document | Role after approval | Change required |
| --- | --- | --- |
| `docs/Desktop/DESIGN_SYSTEM.md` | Cross-product/desktop visual intent | No rewrite in this phase |
| `docs/Mobile/CONCEPT_REVIEW.md` | Approved concept rationale and workflow coverage | Link to new Mobile SOT |
| `docs/Mobile/CLONY_INSPIRED_MOBILE_TOKEN_PROPOSAL.md` | Candidate reference-boundary and token input | Merge its approved rules into Mobile SOT; mark superseded only after review |

## Implementation plan after approval

1. Create `docs/Mobile/DESIGN_SYSTEM.md` as the canonical Mobile SOT and migrate approved Mobile tokens/rules into it.
   Verify: one authoritative token block and clear version/changelog.
2. Build the renderer and generated interactive HTML from that Markdown only, including the brand/CI case-study route and long-image export.
   Verify: changing a token in the MD changes its HTML sample and export; generated HTML contains the same source version.
3. Add a deterministic validation command that rejects malformed source blocks, stale generated output or an unapproved production logo mark.
   Verify: valid source passes; deliberate invalid/stale fixture fails.
4. Review portrait and landscape presentation against the six preserved references and FUNG CI.
   Verify: no copied reference brand/assets/green palette and no desktop UI in landscape.

## Acceptance criteria

1. There is exactly one editable canonical Mobile token source.
2. The HTML preview is regenerated, not manually maintained.
3. A reviewer can inspect light/dark, component states, responsive portrait/landscape and the full brand story without running the Android app.
4. A long static image/PDF can be exported from the reviewed HTML without creating a second manually maintained layout.
5. Reference images are preserved, traceable and bounded to inspiration rather than copied product identity.
6. Existing FUNG functional/product specs remain authoritative over visual presentation.

## Scope boundary

This proposal does not alter Android UI, mobile runtime, GenesisBlockDB, recording, model packages, MCP permissions or application CSS. Those changes remain blocked until this document is approved.

## Version Diff

| Version | Change |
| --- | --- |
| `0.1.0b` | Proposes Markdown SOT, generated interactive HTML, immutable visual references and anti-drift validation for FUNG Mobile design system. |
| `0.1.1b` | Adds brand/CI vertical case-study HTML and export-from-one-source rule; preserves the supplied presentation reference. |
| `0.1.2b` | Approved and implemented the SOT publishing contract; generated artifacts are validated from Markdown. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| `0.1.2b` | 2026-07-21 | beta | Approved and implemented Markdown SOT, interactive HTML, exports and validation. | pending | ATHER |
| `0.1.1b` | 2026-07-21 | candidate | Added HTML-first brand presentation and derived long-image/PDF export contract. | pending approval | ATHER |
| `0.1.0b` | 2026-07-21 | candidate | Proposed SOT/publishing architecture and recorded six supplied visual references. | pending approval | ATHER |
