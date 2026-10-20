# Design QA: Settings right drawer

## Source and implementation

- Source visual truth: `C:/Users/suanr/.codex/generated_images/01a13907-1593-7092-937b-87234303a7d9/exec-d926953f-4a42-4281-8bf1-25f74fc238e8.png` (Quiet Glass Settings Rail, 1487 × 1058 px).
- Implementation: FUNG local preview at `http://127.0.0.1:1420/app?surface=desktop`, captured in the active Codex in-app browser (1006 × 806 px screenshot; CSS viewport and device scale factor are not exposed by the capture API).
- State: desktop Home surface, Settings open, Connections tab selected, light appearance, Liquid Glass material.
- Density normalization: unavailable. The source and implementation capture have different dimensions, and the browser screenshot bytes were not exposed as a filesystem artifact for resizing or compositing.

## Visual review

- Full view: the implementation follows the selected right-side drawer structure with a brand header, vertical settings tabs, a content region, and a persistent local-first footer. The home surface remains visible behind the drawer. The capture is not a same-size side-by-side composite, so pixel-level comparison is incomplete.
- Focused regions: reviewed the tab rail and the Connections, TTS, Cloud providers, Link import, Zoom, Runtime, and Account/backup content states in the Codex browser. The inner feature panels now share the drawer surface; cloud/TTS controls remain inside the bounded content scroller.
- Typography: the current IBM Plex Sans Thai / DM Sans stack is preserved; Thai tab labels fit on one line at the captured desktop viewport.
- Spacing and layout: drawer, header, vertical rail, active-tab indicator, content padding, and footer fit within the captured viewport. Long cloud-provider content scrolls within the content region.
- Colors and tokens: FUNG porcelain, ink, border, local-state, and focus tokens are used. The dim layer is lighter than the previous capture, keeping the underlying home screen recognizable.
- Image quality and assets: existing FUNG logo and home hero asset are reused; the tab icons use the existing Lucide icon set.
- Copy and content: existing settings content and local-first disclosure are retained.

## Findings and iteration history

- Initial browser capture showed the account panel with an extra rounded border, and the glass theme made the background too dark. Updated the drawer to flatten embedded feature roots and adjusted the glass overlay opacity. Follow-up browser captures show the account panel without the nested frame and the home hero more visible.
- No actionable P0/P1/P2 visual mismatch was observed in the captures reviewed.
- The Account and backup tab shows an unavailable native bridge/auth endpoint in the browser preview. This is a browser-preview limitation; native desktop behavior was not exercised in this visual pass.
- Blocking evidence gap: the Codex browser produced visible screenshots, but did not expose a local screenshot path. The source and implementation images therefore could not be normalized and placed in one comparison input as required by the Product Design QA workflow.

## Interaction review

- Clicked settings tabs and confirmed the selected state/content changes for the settings sections.
- Arrow-key navigation moved focus and selection through the tab list.
- Escape and the drawer close button both closed Settings and restored focus to the Settings control.
- `npm run build` passed. `git diff --check` passed. No test suite was run.

## Follow-up

- Save the active Codex browser capture to a local image path and compare it side by side with the source at a documented scale, then rerun this QA report.

final result: blocked
