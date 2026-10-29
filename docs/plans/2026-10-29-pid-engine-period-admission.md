---
version: "0.1.0b"
created_at: "2026-10-29"
last_update: "2026-10-29"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "plan"
  scope: "Read-only physical endpoint engine-period capability before a new PID hypothesis"
  language: "Thai-first"
---

# PID engine-period admission plan

> Use systematic-debugging, writing-plans and independent source review under
> master-plan section9. Root alone performs native/device actions.

**Goal:** determine whether the two existing HDMI endpoints advertise a smaller
shared engine period, before considering a new upstream experiment for #96.

**Architecture:** query normal endpoint IAudioClient3, not the virtual process
loopback client. The external helper performs only COM activation and documented
Get calls. No product source, IPC, capture or media/persistence changes.

**Tech stack:** Rust1.99.1, windows0.62.2, serde_json, existing owned inventory and
read-only mute guards. Existing isolated worktree, baseline `fff68af`.

**Spec:** [Issue96](2026-10-25-issue-96-application-transition.md),
[endpoint routing](../specs/2026-10-25-dedicated-endpoint-routing-design.md),
[Microsoft engine-period API](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclient3-getsharedmodeengineperiod).

## Global constraints

Thai UI labels, named exports, existing CSS convention, anon-key-only configuration,
and surface-param routes ungated. No all-system capture fallback, fabricated PCM,
relaxed timestamp tolerance or reinterpretation of missing active audio as silence.
Keep speakers muted and normal-profile data untouched. Existing failed receipts
and historical acceptance remain intact. No execution of the previously blocked
Microsoft reference executable through another path.

No target keepalive/injection, driver install, default/routing/volume setters,
service restart, unrelated process capture or model/GPU work. No Initialize,
InitializeSharedAudioStream, SetClientProperties, Start, render or capture calls
in this admission helper. Explicit allowlist: LM22 and Mi stable IMMDevice IDs;
never substitute the default or another endpoint. Preserve API HRESULTs/errors.

## Rationale and decisions

The reader timer/MF/flags/direct-acquisition experiments all fail. The new
hypothesis changes the upstream shared-engine quantum through a separate owned
endpoint client, rather than consumer polling. Microsoft documents this capability
as device-dependent; it does not guarantee lossless process-loopback transitions.
The readonly query is not a repair or an audio acceptance test.

An independent raw audit finds no comparator defect. A stopped actual renderer
client hold would discriminate Stop from immediate destruction, but cannot repair
an arbitrary application's object lifetime or close restart loss. Do not present
that altered target fixture as a repair.

Browser-tab mirroring is a genuinely different source architecture but needs its
own user gesture/transport/source contract; it cannot substitute for PID. No
extension installation or browser capture is authorized by this plan.

## Review focus

- Only two explicit native IDs; no default lookup or missing-device fallback.
- COM and format allocations are released even on API errors; format copy bounded.
- Unsupported interfaces/periods remain explicit, never a fabricated minimum.
- Current period is an instantaneous snapshot, not a persistent guarantee.
- Query completion is distinct from smaller-period admission and audio PASS.

## Task1 — query and root-owned admission

External root: `G:\AI-Studio\verification\fung-pid96-new-path-20261029`.

- [x] Implement external `engine-period-query/src/main.rs` and pinned manifest.
  Inputs: exactly `--endpoint <allowlisted ID> --out <fresh JSON file>`.
  Record source hash, endpoint state, GetDevicePeriod, full GetMixFormat bytes,
  IAudioClient3 cast, GetSharedModeEnginePeriod and GetCurrentSharedModeEnginePeriod.
  Each API outcome includes actual HRESULT. No stream initialization.
- [x] Compile offline, format/strict Clippy, independent source review and freeze
  helper binary/source/lock hashes before root native execution.
- [x] Root checks fresh owned inventory, role defaults, idle and muted state;
  performs both read-only queries and retains pre/post state plus exits.
- [x] If unsupported or minimum equals default, retain no-smaller-period outcome;
  no repeated stream/format guesses and no package represented as a repair.
- [ ] If legal minimum is smaller, write/review a separate paired study: identical
  owned silent endpoint helper in both arms, default vs legal minimum only,
  actual engine-period verification, unchanged target and frozen PID capture,
  simultaneous independent endpoint control and strict48frame/leakage criteria.
  This plan alone does not permit that stream-owner experiment.

## Task2 — delivery and gates

- [x] Record results, source/CI state and owning indexes/progress with review.
- [ ] Adopt a production repair only after actual native loss disappears; then
  build/install and qualify both complete modes including source-aware endpoint
  consumers, recovery, UI/MCP and microphone. No standalone probe substitutes.
- [ ] Merge/tag/release only after both full actual installed acceptances pass.

## Actual admission checkpoint

Both explicit HDMI devices returned the native48kHz stereo Float32 mix and
default/fundamental/min/max periods of480frames; current period is480frames.
All pre/post guards and queries completed; no smaller period is advertised in
this tested format with API-default client properties. The conditional stream
experiment is not unlocked and has NOT_RUN status. No Initialize/Start was used.
See [actual results](../verification/implementation-reports/2026-10-29-hdmi-engine-period-admission.md).

No new repair package or PID fix exists at plan creation. Existing installed PID
FAIL and endpoint component-only acceptance remain unchanged.
