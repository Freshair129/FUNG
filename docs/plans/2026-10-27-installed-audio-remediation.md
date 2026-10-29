---
version: "0.1.1b"
created_at: "2026-10-27"
last_update: "2026-10-28"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Installed audio failures: native boundary diagnosis and truthful routed state"
  language: "Thai-first"
---

# Installed audio remediation implementation plan

> **For agentic workers:** Use superpowers:subagent-driven-development, TDD and independent review under master-plan section 9.

**Goal:** Correct demonstrated application defects and establish which boundary
loses PID transition samples before attempting another native backend change.

**Architecture:** Preserve existing process and dedicated-endpoint modes. Add
test-only evidence before PID clock/queue/writer processing, without changing
production capture behavior. Read routed lifecycle alongside legacy lifecycle
when publishing recording state; unsupported routed consumers stay fenced.

**Tech stack:** Rust 1.99.1, Windows WASAPI, Tauri, GenesisBlockDB SDK, existing
owned native render/control fixtures and offline PCM comparison.

**Spec:** [Issue #96 repair contract](2026-10-25-issue-96-application-transition.md),
[routing design](../specs/2026-10-25-dedicated-endpoint-routing-design.md),
[actual installed failure](../verification/implementation-reports/2026-10-27-installed-pid-endpoint-audio-uat.md).
User instruction: fix the demonstrated failures; keep both modes and do not
merge/tag until both installed acceptances pass. Reuse the existing isolated
worktree/branch at baseline `a1fac544ccf99a526e6b89287c067bcf38e94946`.

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

No target injection, retained silent renderer, new driver, user-app capture or
audio-setting mutation. Native actions belong to the root agent only; other
agents edit/review source and read artifacts. No inference/model/GPU work.

## Review focus

- Exact native packet contents must be retained before timeline/queue/writer handling.
- Diagnostic collection must be bounded, opt-in for the ignored owned test only,
  compiled out of production and free of file I/O on the native read path.
- Routed starting/recording/stopping/completed/fault phases must not masquerade
  as inactive recording when their owner is still active or draining.
- Different recording IDs, poisoned locks and legacy/routed lock ordering must
  preserve existing behavior and avoid nested lifecycle locks.
- A faded endpoint tail outside its clock oracle remains NOT_PROVEN; no PCM
  trimming or threshold enlargement can manufacture a pass.

## Task 1 — locate PID loss at the native boundary

Owner: `src-tauri/src/live_application_audio.rs`, test-only ignored native probe
and a bounded trace helper if needed. Existing packet-driver production behavior
and file/event timing remain unchanged.

- [x] Add a failing offline test for exact native payload/flags/index/QPC receipt,
  empty/silent packets and a bounded overrun that invalidates the diagnostic.
- [x] Implement the minimal test-only in-memory packet tap before clock/queue;
  emit owned raw PCM and metadata only after capture stops. No production switch.
- [x] Verify RED/GREEN and focused tests; independently review the source seam.
- [x] Root runs a fresh owned PID probe with simultaneous explicit-endpoint
  control and two renderer transitions, after fresh idle/muted identity checks.
- [x] Reconcile native payload to writer WAV and compare real transition edges;
  adopt a production repair only if an actual application-side cause is shown.
  If missing frames are already absent natively, document that boundary and the
  architectural decision required; do not repeat failed timer/flag/format guesses.

## Task 2 — truthful routed recording metadata and playback admission

Owners: `src-tauri/src/recording_review.rs`, `desktop_playback.rs` and
`routed_media_access.rs` tests. Use the existing routed handle/status contract,
preserving source-aware consumer guards and Genesis SDK ownership. Desktop
playback must apply the same fresh legacy-media admission already used by
export/local API/mobile before touching media paths, including recordings whose
legacy source marker hides authoritative source bindings.

- [x] Regression first: active/stopping routed recording reports active/stopping,
  different recording reports inactive, terminal completed/failed is inactive;
  legacy behavior remains covered. Exercise the actual production projection seam.
- [x] Run and retain RED, implement the smallest correction without nested
  legacy/routed locks, then run GREEN and recording-review/routed regressions.
- [x] Retain a failing desktop-playback regression for pending/recording/
  quarantined/completed routed sources and legacy-marker/source-binding conflict;
  add fresh SDK admission before file inspection, then verify GREEN. Do not
  unlock source-aware playback or weaken quarantine to satisfy the test.
- [x] Independent source/spec review and any bounded corrections.

## Task 3 — package and acceptance checkpoint

- [x] Run exact-toolchain formatting, strict Clippy, relevant/full Rust tests and
  frontend build as appropriate to the final changed tree.
- [x] Build and install a fresh isolated package after source validation;
  preserve all prior evidence. Verify actual image hash, isolated boot/API and
  orderly project persistence without desktop input.
- [ ] Test actual active/stopping routed metadata in the installed package when
  devices return; distinguish it from original audio completeness and full UI UAT.
- [x] Update owning progress/index/report and preserve all failed evidence.
  Commit/push to Draft PR #98 after verification; no merge/tag/release.
- [ ] Both PID and endpoint full installed audio acceptance PASS (still open).

Checkpoint: [source/package evidence](../verification/implementation-reports/2026-10-27-installed-audio-remediation.md)
records source GO, 885 library/17 integration passes and two installed orderly
boots. Native n01 starts no children: both exact HDMI devices are NotPresent.
The original PID FAIL and endpoint NOT_PROVEN remain in force.

Follow-up: [2026-10-28 installed retest](../verification/implementation-reports/2026-10-28-installed-audio-retest.md)
executes n02 after hardware returns. All retained mono bytes reach the writer,
but PID native and installed edges still fail. Two installed endpoint runs pass
bounded PCM/separation components, while routed consumers/custody/recovery remain
unqualified. Actual routed active/completed projection and playback admission are
observed; installed stopping transient remains unobserved, so Task 3's combined
active/stopping item stays unchecked. No production PID repair or release closure.
