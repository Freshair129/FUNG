---
version: "0.2.0b"
created_at: "2026-09-24T00:06:35+07:00,RWANG,2c2559fdd907cf94a6f38b34bbb2c6650e68a0e1"
last_update: "2026-09-24T00:31:51+07:00,RWANG"
status: "beta"
superseded_by: null
attributes:
  domain: "programme-readiness"
  doc_type: "implementation-report"
  scope: "Approved gap closure: status reconciliation, operational Whisper staging and bounded qualification"
  complexity: "C-2"
  risk: "MEDIUM - runtime/model staging and approved shared-worker resolver correction"
---

# Gap closure — baseline and Whisper runtime

## Authority and baseline

ผู้ใช้อนุมัติลำดับปิดงานใน [Gap Analysis](../2026-09-23-fung-gap-analysis.md)
ด้วยข้อความ “ลุยตามนั้น” และเลือก “ใช้ fixture ที่มีใน repository ก่อน” สำหรับ qualification.
ฐานซอร์สคือ `2c2559fdd907cf94a6f38b34bbb2c6650e68a0e1` บน branch
`codex/gap-closure-runtime-20260923`. ก่อนเริ่มมีรายงาน Gap Analysis ที่ยังไม่ commit
จากงานก่อนหน้า ไม่มี source changes อื่น

Parent/peer contracts: master plan §0/§10; Desktop architecture/progress;
Mobile status; Whisper model profiles v0.3.0b. ใช้ staging script เดิมโดยไม่แก้
default selection, runtime dependencies หรือ security contracts. หลังพบ native
routing regression ผู้ใช้อนุมัติ remediation spec ด้วย “approve”; source delta
มีเฉพาะ resolver และ focused tests ใน `src-tauri/src/lib.rs`.
ไม่มีการเปิด M1–M5 lane, deploy, external-send, recording หรือ release ในรอบนี้

## Current acceptance table

| Gap | Status | Evidence / remaining boundary |
| --- | --- | --- |
| GAP-12 status reconciliation | RECONCILED for the identified conflicts | Master header/current CI reconciled; Desktop job/runtime claims reconciled; Mobile distinguishes historical programme Phase 2 acceptance from current APK requalification |
| GAP-01 runtime | PARTIAL — operational staging/worker checks PASS | Turbo and medium staged; GPU/CPU worker/decoder checks pass. Thai speech/reference qualification and packaged acceptance remain open |
| GAP-02 Desktop native/package | PARTIAL — routing regression FIXED / native fixture PASS | Approved shared-resolver correction passes turbo GPU and medium CPU native inject probes. Real speech, device/UI/output-path/restart/installer acceptance remains open; [RCA](../../../.brain/rca/2026-09-24-operational-live-worker-routing.md) retains the original failure |
| GAP-03 cloud controller | BLOCKED for this execution context | OPENAI_API_KEY and ANTHROPIC_API_KEY absent in current process; app keyring not inspected. Real-provider run not performed |
| GAP-04 restore/U9 | OPEN | Clean-install restore not performed; historical adapter/fixture evidence remains bounded |
| GAP-05 Mobile | BLOCKED | ARTEMIS required key missing; no connected device/emulator; no installed AVD. ADB host check is empty after self-heal |
| GAP-06 release | OPEN | No signing, installer execution, integrity waiver or release approval evidence added |
| GAP-07–09 M1–M5 | OPEN / gated | Existing contract/schema/provider/independent review gates unchanged |
| GAP-10 speaker/Thai candidate | OPEN | Optional providers/weights and speech-quality corpus not qualified here |
| GAP-11 additional capability | SCOPE DECISION PENDING | No speculative feature implementation added |

## Runtime provenance

| Model | Repository | Pinned revision | Model binary bytes |
| --- | --- | --- | --- |
| turbo | `mobiuslabsgmbh/faster-whisper-large-v3-turbo` | `0a363e9161cbc7ed1431c9597a8ceaf0c4f78fcf` | 1,617,884,929 |
| medium | `Systran/faster-whisper-medium` | `08e178d48790749d25932bbc082711ddcfdfbc4f` | 1,527,906,378 |

Both model cards report MIT. Revision/license/file metadata was read via
Hugging Face without an authentication token. Existing Python 3.11.9,
faster-whisper 1.2.1 and `small` were preserved. Staging uses
`scripts/stage_whisper_runtime.ps1 -Model <name> -ModelRevision <revision>`
without `-Clean`. `large-v3` and the separate Thai candidate were not staged.
The runtime manifest lives in ignored `.venv-whisper/manifest.json` and
records per-file SHA-256 plus model provenance; it is not a committed release artifact.

## Qualification and evidence

The selected repository fixture is
`tests/transcribeConcatOnly.test.py::write_silent_wav`, 16 kHz mono PCM16.
No approved Thai speech/reference pair was found in the inspected repository.
Silence proves protocol and execution behavior only; Thai WER/CER, speaker
accuracy and live latency SLO remain NOT_RUN.

Execution artifacts are under ignored `.runtime-cache/gap-closure-20260924/`:
staging logs, `probe.py`, fixture, batch/live stdout/stderr, and
`runtime-probe.json`. The probe uses offline model settings and local paths,
checks CUDA DLL hashes, batch JSON, live readiness/correlation, missing-file
error recovery and a VAD-disabled decoder call to exercise compute kernels.
Elapsed smoke durations are single-run diagnostics, not throughput/SLO claims.

| Check | Result |
| --- | --- |
| Host import of existing runtime | PASS — Python 3.11.9, faster-whisper 1.2.1, one CUDA device |
| Restricted-shell import | Environment failure; same host runtime works without reinstall. See [RCA](../../../.brain/rca/2026-09-24-whisper-sandbox-import-boundary.md) |
| Turbo staging CPU/int8 load probe | PASS |
| Medium staging CPU/int8 load probe | PASS |
| Batch/live/decoder matrix | PASS: turbo GPU/float16, medium CPU/int8, medium GPU/int8_float16; all batch/live protocol checks and forced decoder calls completed |
| CUDA DLL custody | PASS: all 11 DLL SHA-256 values match the existing CUDA manifest |
| Runtime and model custody | PASS: 43,267 manifest entries verified with zero mismatches; both model binaries match upstream LFS SHA-256 at pinned revisions. [Evidence JSON](2026-09-24-gap-closure-runtime.json) |
| Existing Python fixture suite | 6/6 PASS under ordinary Python 3.12.14 with offline mode. Initial embedded-runner attempt was 5 pass / 1 fail because PYTHONPATH mock injection was ignored; see RCA |
| Main source CI | Prior-turn verified run [35783285300](https://github.com/Freshair129/FUNG/actions/runs/35783285300), exact base SHA; not a new branch CI run |
| Native inject before correction | BUILD PASS; EXECUTION FAIL before worker ready. Preserved log `native-inject.log` documents the batch-entry-point regression |
| Native inject after correction | PASS for turbo GPU and medium CPU: ready, 1 chunk, 1000 ms ledger duration, 0 segments and successful snapshot/shutdown. Summary/export unavailable because the silence fixture produces no transcript |
| Resolver red/green regression | Before: 1 failed / 1 passed, showing both ordinary live profiles resolve the batch path. After: worker/profile cluster 13/13 passed, including candidate and custom batch-path preservation |
| Packaging/release contracts | 16/16 passed |
| Rust fmt / clippy | Both passed; clippy `--lib --all-targets -- -D warnings`, offline. Full Rust library suite not rerun after this bounded change |
| Physical device/GUI/provider/package | NOT_RUN |
| Document validation | `git diff --check` and new-document links verified. Production/test source changes are confined to `src-tauri/src/lib.rs`; no worker Python script, schema or dependency change |

The VAD-disabled diagnostic generated a segment for silent input on the two
GPU profiles. This confirms compute execution, not transcript correctness.
Production batch/live workers keep VAD enabled and returned no segments for
the same silence fixture. No Thai-accuracy or model-quality improvement claim
is made. The initial wrong-interpreter fixture run attempted a download into
the host Hugging Face cache; that cache was retained, not bundled as evidence.

## Approved routing correction and exit assessment

The explicit approval covers
[remediation spec v0.1.1b](../../specs/2026-09-24-operational-live-worker-routing-remediation.md).
The seven-line production correction restores ordinary live selection to the
sibling `transcribe_live.py`; ordinary batch retains the exact runtime script,
including fake-worker fixtures. Candidate paths and profile/default behavior
are unchanged. Two focused tests were added after the approval.

The scoped routing defect is closed by red/green tests and actual native
worker/Genesis fixture runs. New evidence is in `routing-red.log`,
`routing-green.log`, `routing-clippy.log`, `routing-contracts.log`,
`routing-native-build.log`, and `native-fixed-{turbo,medium}/execution.log`
under the same ignored execution-artifact directory. Source, binary and log
hashes are recorded in the companion evidence JSON.

The native harness returned exit 0, but its report explicitly states summary
and export failed for lack of transcript. That outcome is expected for this
silence fixture and was checked, not hidden by the exit code. No LLM summary
or output artifact acceptance is claimed. GAP-01/GAP-02 remain partial at
product level; native real capture, Thai accuracy and packaged end-to-end
acceptance are still open. New branch CI, commit, push and release are NOT_RUN.

## External blockers

ARTEMIS `mobile_diagnose(attempt_fix=true)` restarted ADB successfully; keys
were valid, no tasks were running, and diagnosis remained BLOCKED on missing
multimodal credentials and missing hardware/emulator. Host `adb devices -l`
also returned an empty list. No mobile test was authored or launched.

Doctor guidance: “Connect your Android phone via USB cable and enable Developer
Options -> USB Debugging.” Configure the tester provider key in `F:\artemis\.env`
or the MCP server env block and restart the MCP server afterward. Do not put
keys in chat. There is no installed `Pixel_8_API_34` AVD; physical-device
acceptance still needs a selected physical device even if an emulator is later added.

## Version Diff

| Document | Before → After | Change |
| --- | --- | --- |
| Gap analysis | 0.1.0b → 0.1.1b | Linked execution results while retaining the initial audit snapshot |
| Master plan | 1.8.0b → 1.8.1b | Header and exact-HEAD integrated CI reconciliation |
| Desktop progress | 0.2.36b → 0.2.38b | Current acceptance index, runtime staging and approved native routing correction |
| Mobile status | 0.4.3b → 0.4.4b | Historical programme versus current APK/device evidence |
| Closure ledger | 0.1.0b → 0.2.0b | Added approved source correction, red/green tests and native fixture results |
| Import RCA | none → 0.1.0b | Compared host/sandbox import without dependency repair |
| Routing RCA / repair specification | 0.1.0b → 0.1.1b | Recorded approval and bounded correction while retaining original regression evidence |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.2.0b | 2026-09-24 | beta | Approved resolver corrected; worker tests, contracts, fmt/clippy and native fixture matrix passed; product acceptance remains partial | base 2c2559f; uncommitted | RWANG |
| 0.1.0b | 2026-09-24 | need review | Staging/worker/custody passed; native routing failure confirmed; bounded code proposal awaits approval | base 2c2559f; uncommitted | RWANG |
