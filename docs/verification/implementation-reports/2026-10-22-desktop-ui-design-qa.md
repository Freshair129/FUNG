---
version: "0.1.0b"
created_at: "2026-10-22"
last_update: "2026-10-22"
status: "beta"
superseded_by: null
attributes:
  domain: "desktop-ui"
  doc_type: "design-qa"
  scope: "Visual QA of FUNG Desktop meeting workflow surfaces against saved mockups"
  language: "Thai-first"
---

# Desktop meeting workflow UI design QA

## Evidence identity

| Field | Value |
| --- | --- |
| Date | 2026-10-22 |
| Repository | `G:\AI-Studio\development\FUNG` |
| Branch | `codex/desktop-workflow-mcp-20261020` |
| Build | `npm run build` production output served at `http://127.0.0.1:4173/app?surface=desktop` |
| Browser | Codex in-app browser, 1280 × 720 viewport |
| Reference | [Saved mockup PNGs and provenance](../../design/mockups/2026-10-22-desktop-mvp/README.md) |

## Surfaces inspected

- **Home:** The rounded shell and dock stay in frame. The hero remains the main
  visual anchor; meeting, local workspace, and task pool cards occupy the right
  grid. With no native bridge, calendar and task data display honest empty or
  unavailable states rather than the example content in the mockup.
- **Live:** The capture form exposes distinct microphone-only, system, and
  application tabs. The browser reports `NATIVE_UNAVAILABLE`, and Start remains
  disabled. The capture form has an internal vertical scroll at this viewport;
  the shell and dock remain fixed in frame. This is a source/layout check, not
  audio acceptance.
- **Archive / Review:** The list and detail panes remain separate. With no
  project selected, both panes present an explicit empty state and do not
  fabricate a recording.
- **Settings:** The right-side drawer keeps all eight categories visible in a
  horizontal two-row tab grid. The content reader scrolls within the drawer;
  the category rail does not become a separate scroll area.
- **Companion:** The utility panel opens as a right drawer and its actions route
  to Live and Settings. It does not begin recording when opened. Its content
  uses an internal scroll at the inspected viewport.

## Findings

No P0/P1 visual blocker was observed in the inspected production preview. The
Home shell/dock and Archive layout fit the viewport; settings categories remain
visible while content scrolls. Live and Companion retain internal scrolling at
1280 × 720, so compact-height layouts still require a native-window pass at the
supported minimum size. Colors and type inherit the canonical Quiet Archive
tokens; `npm run design-system:check` confirms 202 tokens, 58 solid contrast
pairs, and canonical token use in 25/25 stylesheets.

The mockup PNGs are reference evidence only. The rendered preview has no Tauri
bridge and cannot verify microphone enumeration, system loopback, application
capture, recording data, or native window behavior.

---
