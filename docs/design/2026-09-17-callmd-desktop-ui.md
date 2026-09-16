---
version: "0.1.0b"
created_at: "2026-09-17T02:07:29+07:00"
last_update: "2026-09-17T02:07:29+07:00"
status: candidate
superseded_by: null
base_sha: c378af9fac3c00db063948f49f9ee857ebad9126
branch: codex/callmd-ui-dag
attributes:
  doc_type: desktop-ux-spec
  scope: "Three desktop surfaces; documentation only"
  complexity: C-3
  documentation_risk: MEDIUM
  proposed_backend_security_risk: HIGH
---

# FUNG desktop: Home, Live, Recording review

## 1. Authority and review decision

Boss's “ap[prove” approves workflow v0.1.0b and this documentation wave.
Feature selection and product implementation remain unapproved.
All B interfaces, navigation behavior changes, and visual changes below are PROPOSED.
This package proposes a reviewable outcome, not proof that B exists.
No cloud/provider/schema addition, Drive work, P2 bookmarks/agenda/metrics,
proactive assists, sentiment scoring, or copied Call.md branding is included.

| Boss choice | Result | Tradeoff |
|---|---|---|
| P1-B — recommended | Existing local UI plus real multi-recording history, native compatible PCM playback, recording-scoped Q&A | Requires reviewed HIGH-risk backend/admission contracts before UI implementation. |
| P1-A — selectable fallback | Existing local/current-recording resurface; playback unavailable; legacy knowledge search accurately labeled | Smaller backend scope; cannot promise historical recording selection or isolated Q&A. |

B is recommended because choosing a past meeting, listening, and asking about
that exact recording form one coherent review journey. A remains a valid explicit
choice; workflow approval selects neither. Reopen means reading a durable recording,
never restarting capture or reconstructing an unverified live-session epoch.

## 2. Evidence and precedence

| Ref | Evidence | Design consequence |
|---|---|---|
| E1 | docs/brand-kit/README.md:14-56; logo/fung-mark.svg:1-5 | Quiet Archive mark, palette, Thai/Latin type, spacing and honest copy. |
| E2 | docs/design/FRONTEND_REDESIGN_BRIEF.md:261-279 | No direct renderer fetch; lazy local access; fixed 1280×800; completeness and scope disclosures. |
| E3 | docs/verification/implementation-reports/2026-09-17-callmd-fung-contract-scan.md:78-115 | Existing project/current-recording, transcript, summary, export and event surfaces. |
| E4 | Same contract scan:119-163 | Recording identity and recovery; missing native desktop list/playback contracts. |
| E5 | docs/specs/2026-09-17-callmd-desktop-contracts.md:53,128-150,190-204,260-285 | Corrected legacy Q&A scope; NEW B native PCM and scoped ask proposals. |
| E6 | docs/verification/implementation-reports/2026-09-17-callmd-source-ui-scan.md, mounted/screenshot/cloud sections | Screenshot hierarchy is inspiration; not current mounted-source or runtime truth. |
| E7 | docs/design/FRONTEND_REDESIGN_BRIEF.md:293-312 | Full design brief requires substantially more screens and Figma/Penpot; exception below. |

Current contract draft E5 refines the earlier scan: legacy meetingAsk's project
argument filters transcript retrieval ONLY. Graph/live-tail context can come from
elsewhere. This UX adopts E5 and never implies fully project-isolated legacy answers.
Four AGENTS entry documents remain governing context; this wave did not rescan backend.

## 3. Artifacts and format exception

All editable sources are in [callmd-desktop](callmd-desktop/).
Each normal board has three actual 1280×800 frames, left-to-right Home, Live, Review.
Board annotations outside frames identify existing/proposed command scopes.

| Source | Contents | Rendered mockup |
|---|---|---|
| [wireframes.svg](callmd-desktop/wireframes.svg) | Numbered H1–H4, L1–L4, R1–R4 interaction zones; intentionally low-fi | [2× PNG](callmd-desktop/wireframes.rendered-mockup.png) |
| [desktop-light.svg](callmd-desktop/desktop-light.svg) | Three normal Quiet Archive light frames | [2× PNG](callmd-desktop/desktop-light.rendered-mockup.png) |
| [desktop-dark.svg](callmd-desktop/desktop-dark.svg) | Three normal dark frames | [2× PNG](callmd-desktop/desktop-dark.rendered-mockup.png) |
| [states-light.svg](callmd-desktop/states-light.svg) | Nine labeled panels: each surface × empty/loading/error | [2× PNG](callmd-desktop/states-light.rendered-mockup.png) |
| [states-dark.svg](callmd-desktop/states-dark.svg) | Same nine distinct states in dark | [2× PNG](callmd-desktop/states-dark.rendered-mockup.png) |

FORMAT EXCEPTION FOR THIS WAVE: static editable SVG + combined 2× PNG boards,
under the allocated callmd-desktop directory, replace neither the brief's
Figma/Penpot component library nor its per-screen NN-screen-state folder exports.
The state panels are compact state specifications, not nine additional 1280×800 screens.
No Figma/Penpot handoff was created. Boss's feature-package review must acknowledge
this limited format/coverage explicitly; it is not a waiver or completion of §9.1.
Fixture rows/transcripts/statuses are visibly marked DESIGN FIXTURE.
The PNG suffix rendered-mockup identifies design rendering, never application evidence.

## 4. Visual system

Use porcelain #FAF8F3 and ink #171918 for primary ground/text; slate #4A5B8B
for primary actions; sage #6F897E sparingly for brand/local state.
Dark surfaces use ink with proposed neutral layers #222624/#2C322F;
interactive text/focus uses the lighter slate companion #BCCAF3.
Muted text: light #536259 / dark #B9C3BD; caution is metal-derived.
Clay/recording red is semantic only, never decorative scoring.
Normal mockups embed the exact path from docs/brand-kit/logo/fung-mark.svg;
ink on light, porcelain on dark, no box, 40px size and at least 10px clearspace.
No Call.md logo, asset, component code, or screenshot image is copied.

Typography declaration: IBM Plex Sans Thai for Thai, DM Sans for Latin/wordmark;
data may use IBM Plex Mono. No font download or embedding occurred.
Fallback stack is Leelawadee UI, Tahoma, sans-serif. Exact brand-font shaping is UNKNOWN.
The dark Start label explicitly uses the available Thai fallback for reliable raster QA.
Body 14–16px, headings 20–26px, annotations 11–14px; 4px spacing rhythm,
8/12/16px radii. Rendered annotations are review notes, not mandatory product jargon.
Desktop remains 1280×800; long content scrolls within its labeled region.
Wireframes are unfilled geometric zones with interaction labels, not recolored mockups.

## 5. Home / Shell — H1–H4

Primary hierarchy: selected project → local status → start/import → reopen/read → tools.
“เริ่มประชุม” opens the existing local capture workflow after the privacy notice;
never starts from a navigation click or silently opens a provider.
Existing listProjects/listJobs/listModelProviders feed the shell; jobs are not history.
A reads the selected project's activeRecordingId and calls the page “บันทึกปัจจุบัน”.
B's “บันทึกย้อนหลัง” uses proposed listRecordings/getRecording; selection is read-only.
The illustrated recent item is a fixture; production must never substitute it for no data.
Settings/account/backup and pairing open lazily. Local access never requires login.
No global search affordance is introduced without an actual search contract.

Home empty: successful zero-project read offers start/import.
Home loading: preserve selection and working navigation, announce pending read once.
Home error: “อ่านโครงการไม่สำเร็จ”; retry/detail, never “ยังไม่มีโครงการ”.
Keep a labeled last-known selection if available, but disable identity-dependent actions.

## 6. Live transcript + insights — L1–L4

Keep one session/event owner across shell navigation; presentation never starts listeners twice.
Live identity is independent of review selection. Mic/system describe channels, not verified people.
Primary column: confirmed transcript and clearly pending text; right: ephemeral topic/summary.
Existing liveMeetingStart/Stop/Status and live-* events remain the lifecycle boundary.
Stop response means requested stop: show stopping until native status confirms completion.
Show transcript cap/incompleteness before text. Do not estimate VU, WPM, talk ratio or health.
Only confirmed transcript corrections use existing correction behavior; preserve original audio.
If the reader scrolls up, retain their position and offer an explicit return-to-latest action.
Topic/open points remain “ข้อเสนอจากระบบ” and ephemeral; do not turn them into durable agenda.
Summary reads/generation always capture the pair (projectId, recordingId).
External tools stay default-off; preview → explicit approval → result/evidence remains reachable.
No proactive “Say this” or screenshot sentiment widget is added to P1.

Active-capture navigation always opens a warning: “ยังบันทึกเสียงอยู่”.
Choices: “อัดต่อและออกจากหน้านี้”, “หยุดแล้วออก”, “อยู่หน้านี้” (default focus).
Continuing retains a global capture strip and reachable Stop on every destination.
Stop-and-leave awaits inactive status; on failure keep the user informed and session reachable.
Closing a view never implicitly stops capture. Window-close behavior retains native policy.
No pause, new recovery action, or live-session replay is invented.

## 7. History / recording review — R1–R4

B selection: choose project → enumerate real recordings → select stable RecordingKey.
Read transcript and summary with that same key; ignore late results after selection changes.
Reopen is a read action; neither capture nor inference nor audio starts automatically.
A has no synthetic history rows: shows current recording or explains that none is available.
Existing correction/speaker-label tools stay reachable through the original review surface;
a renamed channel/group must not be presented as verified identity.

B playback is PROPOSED native PCM16 WAV only: mono/stereo, 8–96 kHz,
matching chunk formats and an output device accepting the exact source sample rate.
No resampler, MP3/other compressed input, float WAV, codec download, HTTP/mediaURL,
renderer audio buffer, direct fetch, or CSP change is implied.
Use a plain time/progress rail; no invented waveform or duration.
Playback open starts paused; explicit Play, Pause, Seek and channel selection.
Changing recording/channel or leaving review closes the old handle; no autoplay.
Unavailable format: “ยังเล่นรูปแบบเสียงนี้ไม่ได้ — รองรับ PCM16 WAV เท่านั้น”.
Output mismatch: “อุปกรณ์เสียงไม่รองรับอัตราสุ่มนี้”; keep transcript/summary usable.
Missing audio/error: explain the actual cause, offer retry/details, never empty-history success.

Native admission blocks playback open during capture/starting.
Show “เล่นเสียงไม่ได้ระหว่างบันทึก” with a link back to live controls; no silent capture stop.
Before starting capture, close the player and await acknowledgement, then request start.
If close fails or capture admission rejects, show the failure and do not bypass the guard.
A Play remains unavailable with “ยังไม่รองรับการเล่นเสียงบน Desktop”.
Reopen after restart does not auto-resume playback or label an interrupted recording active.

## 8. Q&A, summary and export scopes

| UI action | Existing or proposed binding | Required copy/behavior |
|---|---|---|
| A “ค้นความรู้ในเครื่อง” | EXISTING meetingAsk(question, projectId?) | “โครงการที่เลือกกรองเฉพาะบทถอดเสียง; กราฟและบริบทสดอาจมาจากที่อื่น” before submit and beside each answer. |
| B “ถามเฉพาะบันทึกนี้” | PROPOSED askRecording(RecordingKey, question, requestId) | Persisted transcript from BOTH IDs only; graph/live-tail excluded before generation. |
| “สรุปของบันทึกนี้” | EXISTING meetingSummaries/generateMeetingSummary with BOTH IDs | Show other-recording/unattributable exclusions and attribution completeness; never display a foreign summary. |
| “ส่งออกบันทึกนี้” | EXISTING createJob(export.render, projectId, recordingId) | Freeze pair at click time; report job outcome, not assumed file success. |
| “ไฟล์ส่งออกทั้งโครงการ” | EXISTING listExportArtifacts(projectId) | Project scope remains visible; do not claim every artifact belongs to the selected recording. |

B answers cite only validated supplied segment IDs; no relevant evidence shows
“ยังไม่มีหลักฐานเพียงพอในบันทึกนี้”, not an invented answer or empty error replacement.
Provider unavailable/failure and resource limit are separate errors; never select cloud automatically.
Changing recording clears displayed answer immediately; changing scope never relabels cached text.
A is a knowledge-search surface with a transcript filter, not isolation. The older scan's
“project-wide Q&A” shorthand must not become an unqualified user-facing promise.

## 9. Keyboard, focus and state behavior

Tab order: skip-to-content → shell navigation → selected-project control → main actions →
transcript/review content → insights/Q&A → secondary actions. Enter/Space activate buttons.
Use real button/link semantics; no clickable div-only actions. Scope caveats are programmatically
associated with their input and answer. Selected nav gets aria-current.
Focus outline: 2px slate/light-slate plus 2px offset; never rely on color alone.
On page navigation focus the heading; Back restores the initiating list row or nav item.
Dialog traps focus, Escape cancels without side effects, and returns focus to its invoker.
No global Space recording/playback shortcut while text entry is active.
Playback slider arrows seek ±5s; Home/End select bounds only when seekable.
Loading regions use aria-busy; polite announcements do not read every transcript token.
Stop/error status is announced once; moving focus is reserved for explicit action failure.
Do not hide disabled reasons in tooltips alone. Q&A errors preserve the question text.
Runtime keyboard, zoom, screen-reader and contrast compliance are NOT_RUN in this doc wave.

## 10. Preserved and deferred screens

| Existing surface/group | Reachability proposal | Board coverage |
|---|---|---|
| Capture / Transcript / Summary / Runtime anchors | “พื้นที่ทำงานเดิม”; preserve current commands and gates | Linked, detailed legacy screens deferred. |
| Transcript correction/rename; summary + TTS | Existing review/workspace detail actions | Summarized here; full dialogs/TTS deferred. |
| Export / jobs / projects | Shell links + review export pair/project-list distinction | Entry/actions shown; full detail deferred. |
| Recovery notice and interrupted recording flow | Global notice + “นำเข้า / กู้คืน”, no automatic recovery | States specified; full recovery screen deferred. |
| All Settings tabs incl. Sign In, local Backup, TTS, Cloud, URL, Zoom, Runtime/QR, External Connections | Existing lazy Settings boundary | Navigation preserved; all detailed tab mockups deferred. |
| Device pairing/FUNGWIRE and sign-in pending/auth/error | Existing lazy pairing/account surfaces | Entry preserved; trust/auth screens deferred. |
| External tools preview/approve/result | Existing approval flow, default-off | Entry/state label only; full screens deferred. |
| Mobile, web, landing, phone page | Preserve routing/entry behavior unchanged | All detailed redesign/portrait/landscape work deferred. |

This coverage map is a preservation requirement, not runtime verification.
Canceled Google Drive is excluded even where old design material mentions it.

## 11. AC, SC and exit criteria

AC-UX1: five SVG boards; three low-fi and six normal theme frames; eighteen meaningful
theme state panels; every fixture and proposed capability visibly labeled.
AC-UX2: actual FUNG mark and Quiet Archive palette; Thai readable without fetched fonts;
normal frame geometry is 1280×800 and primary actions do not overlap text.
AC-UX3: A/B distinction, legacy-search caveat, summary pair, export project-list scope,
PCM format/output restrictions and capture admission appear in reviewable artifacts/spec.
AC-UX4: keyboard/focus, navigation warning, local no-login and old-surface access specified.
AC-UX5: exact editable/raster outputs and hashes reported; source review/render proof
separated from product tests, native behavior, CI, and full design-brief completion.
SC: 5/5 SVGs parse; 5/5 PNGs render at 2×; visible dark Start label; zero copied Call.md
assets/code, fabricated metrics, qualified-scope contradictions, or product file edits.
Exit: hand off candidate documents and actual QA; retain Boss A/B and format decisions,
independent DOC_REVIEW and all product checks as pending. No code authority follows.

## 12. Verification, unknowns and version diff

Local SVG/PNG results and SHA-256 hashes: [UX report](../verification/implementation-reports/2026-09-17-callmd-doc-ux.md).
Exact IBM Plex/DM Sans rendering, interactive accessibility, native PCM device compatibility,
live capture/navigation behavior, integration tests, builds and hosted CI: UNKNOWN / NOT_RUN.
The five boards do not verify runtime data or complete the global design brief.
Boss choices pending: B versus A; B's native/security contract scope; this wave's format exception.
new → 0.1.0b: five static boards, corrected legacy knowledge-search scope, scoped B review,
native format/admission constraints, preservation map, and review acceptance criteria.

## CHANGELOG

| Version | Timestamp (+07:00) | Status | Change | Commit |
|---|---|---|---|---|
| 0.1.0b | 2026-09-17T02:07:29+07:00 | candidate | Initial bounded desktop UX package; no product implementation | UNCOMMITTED; base c378af9 |

