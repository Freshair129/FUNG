# FUNG Desktop Page Wireframes

ชุด wireframe ระดับหน้า Desktop สำหรับ P1-P4 โดยยึด `1280 x 720` stage, fixed HUD, P rail, center workbench, Agent card, Sector C และ signal sector ตาม [`03_LAYOUT.md`](../03_LAYOUT.md), [`04-components.md`](../04-components.md) และ [`05-sitemap-ia.md`](../05-sitemap-ia.md)

## Pages

| Page | Wireframe | Primary job |
| --- | --- | --- |
| P1 | [p1-capture.svg](p1-capture.svg) | Capture and Library |
| P2 | [p2-transcript-review.svg](p2-transcript-review.svg) | Transcript and Speakers |
| P3 | [p3-summary-intent.svg](p3-summary-intent.svg) | Summary and Intent |
| P4 | [p4-runtime-export.svg](p4-runtime-export.svg) | Runtime and Governance |

## Shared zones

- Topbar: project identity, local/runtime status and global controls
- P rail: top-level page selection
- Center workbench: one focused task and three frequent-use actions
- Agent card: active domain guidance and runtime state
- Sector C: activity and evidence events
- Signal sector: D Capture, E Transcript, F Intelligence, G Export

These are structural wireframes, not final visual mocks. They show ownership, information hierarchy, controls and required state coverage. Runtime, UAT and release readiness remain governed by the implementation-status documents.

## State legend

| Marker | Meaning |
| --- | --- |
| Indigo | Current selection or primary action |
| Sage | Confirmed local or safe state |
| Metal | Provenance, export or durable artifact state |
| Red | Data-risk, failure or destructive confirmation |
| Dashed zone | Proposed or unavailable capability |

## Version

`0.1.0b` — first complete P1-P4 page-level wireframe set.
