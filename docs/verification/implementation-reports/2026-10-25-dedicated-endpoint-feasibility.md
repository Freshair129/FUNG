---
version: "0.1.0b"
created_at: "2026-10-25"
last_update: "2026-10-25"
status: "candidate"
superseded_by: null
attributes:
  domain: "desktop-native-capture"
  doc_type: "verification"
  scope: "Existing-device two-route native feasibility, not installed product acceptance"
  language: "Thai-first"
---

# Dedicated endpoint routing — existing-device feasibility

## Disposition

**Native feasibility: bounded PASS for two owned sources on separate existing
HDMI endpoints. Product integration and installed acceptance: NOT_RUN. Release
remains blocked.** The user selected this alternative after the
[failed PID-route delivery experiments](2026-10-25-issue-96-alternative-capture.md).
Neither those historical failures nor original #96 are relabeled as passed.

The [routing design](../../specs/2026-10-25-dedicated-endpoint-routing-design.md)
and [task plan](../../plans/2026-10-25-dedicated-endpoint-routing-plan.md) define
per-source custody, route verification and separate installed/release gates.
A builder review found the updated design complete for implementation; the user's
original PID-mode release disposition remains pending.

## Host and inventory

Repository baseline: `f7bfdf3`, existing linked worktree
`G:\AI-Studio\development\FUNG`, branch `fix/issue-96-app-loopback-transition`.
Windows 11 Pro build 26300. External tools, references and receipts:
`G:\AI-Studio\verification\fung-endpoint-routing-20261025`.

Read-only native inventory finds 16 render endpoints across all states, four
active. All four active devices have stereo 48-kHz Float32 mix formats. Stable
native IDs round-trip through GetDevice; errors on inactive devices are retained.
Console/multimedia default is Focusrite; communications default is ONIKUMA.
Active Dota PID 13548 was observed on Focusrite, which was excluded from capture.

The owned synthetic routes use these exact nondefault native IDs:

| Source | Device | Native endpoint ID |
| --- | --- | --- |
| A | LM22-B200S (NVIDIA High Definition Audio) | `{0.0.0.00000000}.{c7033d7f-b081-40c6-a5ff-1df83a87d352}` |
| B | Mi Monitor (NVIDIA High Definition Audio) | `{0.0.0.00000000}.{febdb10f-94d3-4af8-af69-b3ac1e887122}` |

These are separate render endpoints on the NVIDIA adapter. Different IDs alone
are not proof of hardware independence; measured PCM separation is the evidence.
No application output policy, role default, driver or normal FUNG data was changed.

Test HDMI endpoints stayed muted. A legacy blanket-mute script at the end of one
batch temporarily muted Focusrite after the user had unmuted it during Dota. The
latest user state was restored to unmuted; receipt
`restore-user-focusrite-state.txt`. No later blanket mute is used. This is an
explicit exception to “no settings change,” not an unchanged-state claim.

## Controlled topology and criteria fixed before capture

Two independent owned player processes explicitly open A and B by IMMDevice ID.
Each submits a different seeded, bandlimited, nonperiodic 30-second PCM16 marker
as native Float32 stereo (`sample / 32768`, duplicated). Each selected renderer
is stopped, destroyed and recreated twice, restarting the source at frame zero.
Capture instances independently read each endpoint mix into source-owned WAVs.
No target-process capture, injection or audio from Dota/Focusrite is used.

`predeclared-analysis-policy.json` precedes the first run: maximum 48-frame timing
uncertainty, and jointly fitted foreign/own transfer gain at most 0.001 (-60 dB).
`predeclared-e06-policy.json` adds an independent endpoint-mix control comparison
without changing those limits; e07 repeats that same policy. Conservative fitted
gain bounds include a half-LSB quantization envelope. They are conditional digital
test bounds, not physical/analog isolation or a privacy security guarantee.

Submitted frames are not the playback oracle: Stop can purge queued audio. Each
Stop records native clock/frequency plus submitted-minus-padding after Stop,
before resetting/destroying the client. The two consumed-prefix estimates agree
exactly in the qualified cases. Independent source correlation checks that prefix.

For e06/e07 a separately compiled **production I16 endpoint-loopback reader** runs
alongside each external Float32 reader. The diagnostic requires a unique exact
device name and logs the resolved native ID; there is no default fallback. Both
controls self-log the intended HDMI IDs. This gives an independent implementation
of delivered endpoint-mix PCM; it does not prove physical audible output beyond
the native clock/padding evidence.

## Results

| Run | Purpose | Result |
| --- | --- | --- |
| e01 | Two routes with separate owned silent keepers | All six process exits 0, capture gaps empty and separation measured; complete played-end claim **NOT_PROVEN** for one 450-frame fading tail beyond the clock/padding prefix. Known consumed prefixes match. Receipt remains unchanged. |
| e02-contamination | Start a foreign owned marker on A during capture, outside its PID allow-list | Capture exit 2 with explicit unknown-session fault; two private diagnostic WAVs retained. Detection is asynchronous, not prevention of every foreign sample. |
| e03-preflight | Foreign owned marker active before capture | Exit 2, no ready marker and zero WAVs. |
| e04-no-keepers | Same two routes without silent keepers | Four process exits 0, no capture faults; all four Stop consumed-prefix ends match exactly; first above-threshold restart references at 39/27 frames, within 48. Bounded native PASS. |
| e05-missing-endpoint | Request nonexistent explicit ID | Exit 1, Element not found, zero WAVs; no default substitution. |
| e06-independent-controls | Two routes plus independent I16 controls, with keepers | Eight process exits 0, all four capture faults empty; all four Stop/restart comparisons against controls differ by 0 frames within numerical fit noise. Bounded endpoint-mix PASS. |
| e07-minimal-controls | Repeat independent controls without keepers | Six process exits 0, all four capture faults empty; all four Stop/restart comparisons differ by 0 frames within numerical fit noise. Bounded endpoint-mix PASS for the minimal candidate. |
| e08-wrong-route | Player A explicitly sends to B while capture requests A | Capture itself exits 0 with silent PCM; target session is absent on A. Root post-run analysis finds wrong routing; proposed native admission must stay WaitingForTarget, not Armed. The prototype does not yet enforce that readiness policy. Endpoint-ready alone is not application verification. |

e06 direct final 40-ms tail comparisons have maximum residual 1 PCM16 unit,
RMS at most 0.507 and cosine at least 0.999999946. e07 tail residual maximum is
also 1 unit, RMS at most 0.490, minimum cosine 0.9999999359; restart comparison
cosine is at least 0.9999997848. The 48-frame criterion is unchanged.

The conservative foreign/own gain bounds in e07 are approximately 0.0002589 for A
and 0.0002394 for B, below the predeclared 0.001 threshold. Digital silent RMS is
0 on both routes. Do not present numerical near-zero OLS coefficients as unlimited
isolation. The bounded conclusion is separation of these two controlled markers.

e01 is not retrospectively converted into PASS: its extra fading tail was not
reproduced in e06/e07. Initial analyzer output treated excess tail as FAIL; retained
revisions correct that oracle ambiguity to NOT_PROVEN, without treating missing
active PCM as silence or changing a loss threshold.

## Custody and reproducibility

Inventory: `inventory-v1.json`, identity JSON and preserved `inventory-v1-source/`.
Markers: `marker-a.wav`, `marker-b.wav` and hashes in the predeclared policy.
Owned tools: `routing-probe-v2.exe`, source under `src/bin/routing_probe.rs`, copied
production clock, and `routing-prototype-v2-identity.json`.

| Frozen executable | SHA-256 |
| --- | --- |
| `routing-probe-v2.exe` | `8A68CC689E706971ADD35427C3BB2720CC1E44F1528DB616B836B82510357F55` |
| `native-endpoint-control.exe` | `F2242C5D7C90D1C6D7CF40B7333FEE978EAFD6C7F74B55A86CFB6EC07E00CB8D` |

Run `run-two-routes.ps1 -Name <fresh>` for keepers or add `-NoKeepers` for the
minimal variant. `-WithControls` adds the frozen exact-endpoint I16 controls.
Latest controller performs fresh native inventory before any audio starts;
capture additionally checks active session PIDs before startup and every 100 ms.
e01/e04 controller snapshots are preserved, including their earlier inventory
precheck behavior. Each native capture still has its own fresh preflight.

Per-run directories preserve controller identities/arguments/exits, render clock
events, exact chunk start/end frames, packet traces, WAVs, stdout and stderr.
Use frame fields for Float32-prototype reconstruction; production-control ledger
milliseconds retain their original rounding and are aligned by marker interiors.
`analyze-routing.py` and `compare-endpoint-control.py` produce independent
receipts; inspect analyzer revision and stated oracle scope, not only PASS text.

Independent review artifacts include `e01-independent-analysis-v3.json`,
`e04-independent-analysis-v3.json`, `e06-independent-control-analysis.json` and
`e07-independent-control-analysis.json`. Native custody/default-role comparison is
in `native-run-custody.json`; default roles match the earlier snapshot. Diagnostic
test-only source hooks were frozen externally and restored from byte snapshots;
there is no production capture change in this evidence-only checkpoint.

## Remaining gates

- The two-channel product pipeline still needs authoritative per-source/chunk
  bindings, separate tracks and source-aware recovery/playback/export/quarantine.
- A read-only inventory/admission draft and policy tests are not shipped capture.
- No arbitrary user application's Windows Volume mixer routing has been changed
  or accepted; owned programs selected endpoints directly. Actual app routing must
  be observed, including stale PID/child/session and device-change conditions.
- Endpoint mix allows other local programs to send audio to it. Polling/session
  notifications detect observed contamination but are not an OS security barrier.
- A rebuilt isolated **installed FUNG** package must execute the complete native
  capture/output/export/reopen/recovery path with source identities intact.
- Original direct-PID mode release disposition, exact-head CI/review/merge and
  versioned release gates remain open. No installer, tag or release was produced.
