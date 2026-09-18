---
version: "0.1.1b"
created_at: "2026-09-17T02:20:07+07:00,Codex DOC_UX,c378af9fac3c00db063948f49f9ee857ebad9126"
last_update: "2026-09-17T02:20:07+07:00,Codex DOC_UX"
status: candidate
superseded_by: null
base_sha: "c378af9fac3c00db063948f49f9ee857ebad9126"
branch: "codex/callmd-ui-dag"
attributes:
  doc_type: "implementation-report"
  domain: "FUNG desktop UX"
  scope: "DOC_UX static five-board finalization; documentation only"
  risk: "MEDIUM documentation; no implementation authority"
  model_request: "gpt-5.6-luna/max explicitly requested by user"
  model_runtime_provenance: "UNKNOWN; hidden runtime model is not independently observable"
  previous_resume_provenance: "UNKNOWN; preserved from the prior resumed owner"
  renderer: "sharp 0.35.4 via bundled Node; no install"
---

# FUNG desktop DOC_UX finalization report

## Outcome

The bounded static closeout passes. The existing five editable SVGs and five
rendered PNGs were preserved unchanged; no source defect was found, so no SVG
edit or PNG regeneration was needed. This report is candidate `0.1.1b`.

The UX source remains a candidate documentation package. Workflow and the
documentation wave are approved, while A/B feature selection, product code,
CI repair, and implementation authority remain pending. Parallel document
owner files were inspected only where needed for scope alignment and were not
modified.

## Provenance and authority boundary

- The user explicitly requested a `gpt-5.6-luna/max` DOC_UX finalization. That
  request is recorded; the hidden execution model is not independently
  observable here and is therefore not attested as fact.
- The previous resumed owner's model provenance remains `UNKNOWN`; it was not
  inferred or repaired retrospectively.
- Rendering/metadata inspection used the already-installed bundled Node and
  `sharp` `0.35.4` at
  `C:/Users/pc/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/`.
  No install, build, test, provider, web, or network operation was performed.
- No app, source, test, CI, or implementation file was edited. Feature
  approval, Terra/DOC_REVIEW, and all runtime gates remain pending.

## Static QA performed

| Check | Result | Evidence boundary |
|---|---|---|
| SVG parsing | PASS — 5/5 | PowerShell XML parse; every root is `svg` with one title and one description. |
| Render metadata | PASS — 5/5 | Bundled `sharp 0.35.4` reads every SVG and PNG; all PNG formats are `png`. |
| Exact scale | PASS — 5/5 | Every existing PNG is exactly 2× its editable SVG width and height. |
| Normal boards | PASS — static review | Each light/dark board contains three actual `1280×800` frames: Home, Live, Review. |
| Wireframe board | PASS — static review | Three low-fi `1280×800` frames with H1–H4, L1–L4, and R1–R4 zones. |
| State boards | PASS — static review | Each theme has nine distinct compact panels: three surfaces × empty/loading/error. They are not nine full `1280×800` screens. |
| Thai/fixture/proposal labeling | PASS — static review | Normal boards visibly mark `DESIGN FIXTURE`; proposed capability labels use `PROPOSED`. State panels use `STATIC DESIGN` plus `P1-B PROPOSED`. |
| Dark normal visual check | PASS — observed | `desktop-dark.rendered-mockup.png` visibly contains the Thai `เริ่มประชุม` Start label and the three per-frame scope footers; the Home button is not blank. The earlier blank-label observation was not reproduced in this current PNG. |
| Dark state visual check | PASS — observed | `states-dark.rendered-mockup.png` shows the 3×3 state grid, Thai copy, state labels, and visible static/proposed footers without an obvious clipping or overlap defect at review scale. |

The visual observations are static image evidence only; they do not establish
runtime mounting, interaction, keyboard behavior, accessibility-tree output,
audio output, async ownership, or persistence.

## Scope-alignment spot checks

- A uses the exact `ค้นความรู้ในเครื่อง` label. Its project filter is stated
  as transcript-only, with the unscoped graph/live-tail caveat visible in the
  normal/state boards and aligned with UX §8, contracts §8, and acceptance
  AC-05/T07.
- B is consistently marked `PROPOSED`. The UX and contract boundary is
  compatible native `cpal` output for integer PCM16 WAV, mono/stereo, 8–96 kHz
  with matching output rate; no MP3/float-WAV decoder, resampler, HTTP/mediaURL
  transport or direct fetch, renderer audio buffer, or waveform claim is
  introduced.
- Capture/playback mutual exclusion is stated in the UX and review boards:
  playback is unavailable while capture is active/starting, and capture waits
  for playback close acknowledgement. The native admission proof remains
  implementation-gated.
- Reviewed history remains honest: B proposes real project recording
  enumeration and stable selection; A remains current-recording-only with no
  synthetic older rows. Reopen is read-only and never silently restarts
  capture, inference, or playback.
- Summary/export scopes retain the `{projectId, recordingId}` pair, while the
  project-scoped export inventory remains visibly a project scope. These match
  the UX, contracts, and acceptance scope sections without selecting A or B.

## Artifact inventory and digests

All dimensions below are actual file metadata. Normal and wireframe board
dimensions are the combined-board dimensions; the three embedded frames remain
`1280×800`. State-board panels are intentionally compact as documented in the
format exception.

| Board | SVG source dimensions | PNG render dimensions | Actual scale | SVG SHA-256 | PNG SHA-256 |
|---|---:|---:|---:|---|---|
| `desktop-dark` | `3968×1016` | `7936×2032` | `2×, 2×` | `018bf8c67f6946b81d714d540544efeab6a8d1ae8d125760c1b261c6bbf9dc36` | `e3f9d456cf1f6041f48c8c049aa263ba8ecf4926c6efeef7186326a6ff1917fb` |
| `desktop-light` | `3968×1016` | `7936×2032` | `2×, 2×` | `556c7a85700cb5d30c3be4e2ebf7fdc338f9114aa4109103cab897ffc0ae5e11` | `7bf0aca085b24de38f51e64e1add9e29b82e5d3cea1ab39e41eff34383589d82` |
| `states-dark` | `2168×1530` | `4336×3060` | `2×, 2×` | `584b4f8fc81fe6809f955656031d7a13200bdc23e5f5e88c8a62208d46fb08b2` | `afc22091b45d782e584953a4f62e3448475e4b9c6e29a89b6019a368cdbb9856` |
| `states-light` | `2168×1530` | `4336×3060` | `2×, 2×` | `be0bd749760f90ab0522d466a0d2be61c78307cbbba257a747d0c3ab6d13fb8e` | `e79931d662afed7c5da9b6aaa22d06d3d0d933434725b72ba76c6dc8ad183f0a` |
| `wireframes` | `3968×1016` | `7936×2032` | `2×, 2×` | `6f9725e4863a12712715926add2a682b97f6fc2a08d0201995695312ab6b7745` | `17f19219c4a543ac34c2f4936b3ac00797f59f44db3196badd9ee15ea60517dc` |

Files are under `docs/design/callmd-desktop/`; the PNGs are existing generated
derivatives. The combined editable SVG plus 2× PNG board package is the explicit
scoped exception for this documentation wave; it does not replace the full
Figma/Penpot component library or all-screen/all-state exports. No derivative
was regenerated during this closeout.

## Explicitly not run / still pending

`npm` tests, Rust tests, builds, native launch, packaged runtime, browser
runtime, provider/network calls, hosted CI, keyboard interaction, zoom,
screen-reader/accessibility-tree, contrast runtime checks, native PCM device
compatibility, capture/playback races, async/persistence behavior, and mobile
or web work were **NOT_RUN**. The full Figma/Penpot and all-screen/all-state
brief package was not created. A/B selection, exact feature approval, and
independent DOC_REVIEW remain pending. Static board QA must not be promoted to
product readiness.

## Version diff and changelog

`0.1.0b → 0.1.1b`: added the bounded parse, actual-dimension/2×, label,
static-visual, semantic-scope, digest, and model-provenance report. Existing
SVG/PNG contents were unchanged; product version and implementation authority
were unchanged.

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| `0.1.1b` | 2026-09-17 | candidate | Final bounded DOC_UX static QA and artifact digest inventory; no implementation edits. | `UNCOMMITTED; base c378af9fac3c00db063948f49f9ee857ebad9126` | Codex DOC_UX |
