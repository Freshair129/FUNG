---
version: "0.3.0b"
created_at: "2026-10-16T00:00:00+07:00,RWANG"
last_update: "2026-10-16T00:00:00+07:00,RWANG"
status: "superseded"
superseded_by: "docs/FUNG_Design_System_v0.1.0/DESIGN_SYSTEM.md"
attributes:
  domain: "mobile-design-system"
  doc_type: "compatibility-entry"
  scope: "FUNG Mobile references into the shared design system"
  language: "Thai"
---

# FUNG Mobile Design System — compatibility entry

This page remains so older Mobile documents and bookmarks keep a stable destination. It no longer owns editable tokens or generated previews.

## Current shared design system

- **Canonical token source:** [fung.tokens.json](../FUNG_Design_System_v0.1.0/tokens/fung.tokens.json), using the Design Tokens Format Module 2025.10 with FUNG data under the namespaced extension.
- **Interactive board:** [Shared design-system board](../design-system/index.html).
- **Brand presentation:** [Quiet Archive presentation](../design-system/brand/index.html).
- **Mobile behavior and capability contract:** [PRODUCT_UX_SPEC.md](PRODUCT_UX_SPEC.md).
- **Quiet Archive SVG masters:** [Mobile design-system assets](design-system/assets/).

Run node scripts/design_system.mjs render from the repository root after editing the canonical JSON. The renderer also refreshes the CSS and contrast report. Run node scripts/design_system.mjs check to detect stale generated files.

The old Markdown token payload and Mobile-only palette were removed when the shared source was established. The former HTML paths under docs/Mobile/design-system/ redirect to the shared board.
