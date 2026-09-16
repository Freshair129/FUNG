---
version: "0.2.20b"
created_at: "2026-07-05T13:15:00+07:00,ATHER"
last_update: "2026-09-13T12:00:00+07:00,Claude"
status: "beta"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "real-progress"
  scope: "FUNG"
---

# 08 - Real Progress

## Current Status

FUNG has a working desktop-first foundation and a routed Live Meeting core. Sprint 4 adds an independently default-off connector and operator workflow for controlled read-only document and CRM lookup: local stdio registration, exact evidence/field preview, per-call approval, cancel/revoke, sanitized result provenance, and local history. A Windows relaunch smoke proves the app window can reopen and base Genesis project/recording/transcript rows remain readable; summary/export review after restart is still open. The host `py -3` interpreter cannot import `faster_whisper`, but FUNG's staged `.venv-whisper` runtime imports `faster-whisper` 1.2.1, has the pinned `small` model, and uses the staged CUDA 12/cuDNN 9 bundle. The standalone GPU worker smoke passed; Live Meeting real-capture, device, connector, and visual UAT remain open. Streamable HTTP, vendor-specific production connectors, automated screenshot/keyboard UAT, real-device capture UAT, and real-connector UAT remain open.

This document separates implemented truth from planned capability.

## Current truth sync (2026-09-13)

Supabase (`nqnrvqnijzovkrhxslfp`) is back online (auth API responds; project
settings return 200 with the anon key) after the free-tier pause. Login is
still gated one step earlier: the Google provider is **not enabled** on the
project — `/auth/v1/authorize?provider=google` returns
`validation_failed: provider is not enabled`, and the account has no Google
OAuth client yet (Google Cloud first-run ToS not accepted). So desktop↔mobile
login/pairing UAT stays open on a dashboard-config gate, not a code gate.

PR #44 (merge `f161a1d`) closed REQ-B-08 on the web: the Account-settings
paired-device section was a literal `ยังไม่พร้อมใช้งาน` placeholder while the
Dashboard tile beside it already queried the rows. Both surfaces now read
through one `usePairedDevices` hook — same RLS-scoped query, revoke + audit
write, and reload — and separate load-error from empty (a failed query reads
"โหลดรายการอุปกรณ์ไม่สำเร็จ — ไม่ใช่ว่าไม่มีอุปกรณ์" with retry, not a false
empty list). PR #43 (`98017c7`) added the tracked brand kit under
`docs/brand-kit/` (scalable `fung-mark.svg`, `tokens.css`, README).

Later the same day the Google provider was enabled with a real OAuth client
and web login succeeded end to end — but the callback first landed on
`localhost:3000`, the **ZURI** app: FUNG and ZURI share the Supabase project,
whose Site URL is ZURI's dev server, so a redirect not on the allow-list fell
back there. Adding `https://fung-seven.vercel.app/auth/callback` to the
project's Redirect URLs fixed it (config, not code). Two harder facts surfaced
while checking what the signed-in web could show:

- **The live project's `public` schema is empty.** `information_schema`
  lists no user tables and `select … from public.devices` fails with `42P01`.
  The migrations in `supabase/migrations/` (profiles, devices, pairing,
  OAuth control plane) have never been applied to `nqnrvqnijzovkrhxslfp`.
  Auth works because `auth.users` is built in; every `profiles`/`devices`
  query the web makes fails against live. The deployed bundle predates the
  PR #44 live query, which is why it still shows a static empty-devices
  placeholder rather than the load error. Applying the migrations is an
  owner-directed infra step and remains open.
- **No audio has ever reached the cloud, and no path exists for it.**
  Supabase Storage has 0 buckets / 0 objects. Recordings live only in the
  GenesisBlockDB ledger (`recordings` + `audio_chunks`) as per-channel WAV
  slices on desktop (`live/{id}/chunks/{mic|system}-NNNNN.wav`) or `.m4a`
  segments on the phone; the only off-device copy is the E2E-encrypted Drive
  archive, which the web cannot open. `cloud_config.rs` guard-tests the
  auth-only cloud surface deliberately.

Rather than upload raw meeting audio (which would break that invariant), the
web now reads recordings from the desktop **on the same machine** over
loopback — the owner's chosen direction ("self-host the local GenesisBlockDB
as the storage"):

- `src-tauri/src/local_api.rs` replaces the old `/health`-only listener in
  `lib.rs`. Same `127.0.0.1:0` bind, hand-rolled HTTP like the mobile gateway
  (no new crate). New routes `GET /recordings` (ledger list: project name,
  duration, created, channels present) and `GET /recordings/{id}/audio`
  (stitches the selected channel's WAV slices in filename order via `hound`;
  whole-file imports are served as-is with their MIME; HTTP `Range`/206 for
  seeking; missing slices skipped and counted in `X-Fung-Missing-Chunks`).
  Every route but `/health` needs a 256-bit per-launch bearer token shown as a
  connect URL (`http://127.0.0.1:PORT/#TOKEN`) in Settings › Runtime; CORS
  reflects only the production web origin and `localhost`/`127.0.0.1` dev
  origins and answers Chrome's Private Network Access preflight. Chunk paths
  resolve through `audio_custody::resolve_chunk_path`, so a ledger row cannot
  climb out of the project directory.
- Web: `src/web/localApiClient.ts` (the frontend's one permitted `fetch`,
  refused unless the parsed hostname is loopback — checked on paste, on
  `localStorage` read-back, and on every built URL), `useLocalRecordings`,
  and the Dashboard "ไฟล์ล่าสุด" tile, which was a hard-coded "เร็วๆ นี้" and
  now separates *unconfigured* (paste the link) from *error* (open the desktop)
  from *empty*, lists recordings newest-first with a channel toggle, and plays
  them in `<audio>` (token in the query, since media elements cannot carry
  headers). `tests/egressRegister.test.mjs` now allow-lists exactly that file
  and fails on any other `fetch`, an unguarded one there, or a non-loopback
  host literal; `docs/appendices/E-egress-register.md` §1.9/§2 record the
  boundary.

Later still, on the owner's instruction, the live project was brought up to
the repo: all seven migrations plus a new
`20260913000000_backfill_profiles_for_existing_users.sql` (profiles and Google
`oauth_connections` for the two users who had signed in before the trigger
existed) were applied in order through the management API, and the three Edge
functions were deployed with `verify_jwt` on and no custom secrets. Verified on
live: 11 tables with RLS, 14 functions, the read-only block of
`supabase/tests/w1_authority_schema.sql` passes, the linter shows no critical
findings, and each function answers 401 unauthenticated and grants no CORS to
an unknown origin. Recorded migration versions are apply timestamps, so a CLI
`db push` must reconcile history first.

Applying W1 exposed that three client writes had only ever worked against the
pre-W1 grants: web and mobile revoked with `devices.delete()`, mobile
registered with `.insert()`/`.update()`, and the desktop published its
FUNGWIRE LAN endpoint with a direct `PATCH /rest/v1/devices` — all refused
once `authenticated` is SELECT-only. Fixed in the same change: a new
`src/lib/deviceAuthority.ts` is the one client path (`device-enrollment`
function, `action: "revoke"` / `"pairing_only"`), the mobile liveness check
reads `revoked_at` instead of inferring revocation from a missing row (W1
revocation is soft), and a new owner-scoped `publish_device_endpoint` RPC
(`20260913000001_…`, applied to live) replaces the PATCH in
`auth_session::broker_device_endpoint_publish`. `tests/deviceAuthority.test.mjs`
pins all of it from the client side.

The cross-device case followed on the owner's instruction without giving up
local-first: a browser on a phone cannot fetch `http://<LAN-IP>` from the
HTTPS web (mixed content), so the desktop now **serves the phone page
itself**. `set_local_api_lan` (new command, Settings › Runtime → "แชร์ให้มือถือ
(LAN)") starts a second, opt-in listener on `0.0.0.0:0` — separate from the
loopback one so it can be stopped without ending a same-machine session, and
gone when the app exits — serving the same token-gated routes plus an
embedded, self-contained page at `/` (`src-tauri/assets/local_recordings.html`:
list, channel toggle, `<audio>`; CSP pins it to its own origin; the token is
never in the page, only in the fragment the phone scans). The desktop shows a
QR of `http://<LAN-IP>:<port>/#TOKEN` (`qrcode` npm, audit clean) and the
plain URL. Rust 437/437 (three new: the page carries no token and nothing
off-origin, `lan_url` needs a LAN listener and refuses the wildcard bind, a
stopped listener releases its port); frontend build and Node suites green;
`docs/appendices/E-egress-register.md` §2 records the opt-in bind. Still to
observe on real hardware: the Windows Firewall prompt on first LAN bind and a
phone actually scanning and playing. The stitched WAV is built in memory
(~345 MB per hour of 48 kHz mono) — fine for one or two clients, not a server.

**Mobile capture was never finishing (found on the Galaxy A07, fixed the same
day).** The owner reported "no save button, cannot see the file". Driving the
installed app over `adb` reproduced it and `logcat` named it:
`Uncaught (in promise) Error: native recorder did not finalize safely`. The
shell's `stop()` polled for state `"completed"`, which `RecorderPlugin.kt`
never emits — its terminal state is `"stopped"` — so every native stop timed
out after 1 s, `finishCapture` never ran, the ledger row stayed
`status = "recording"` (that is the "กำลังบันทึก" row from 3 Sep the owner could
not open), and the next "start" resumed that same row instead of creating a
new one. Three fixes, all verified on the device: (1) `nativeRecorderSettled`
accepts the plugin's real terminal state and the poll runs 5 s, a failed stop
now lands in `recovery_required` with the reason instead of leaving the UI on
"กำลังบันทึก…" forever, and `tests/captureOrchestration.test.mjs` reads the
Kotlin `stop()` source so the two strings cannot drift again; (2) playback
was blocked by the webview CSP (`media-src` lacked `blob:`; the segment player
builds blob URLs) — `blob:` added, `http://ipc.localhost` added to
`connect-src` so Android IPC stops falling back to postMessage, and the egress
suite now checks `media-src` too; (3) the UX gap that hid all of this: the
stop control was an unlabelled icon, "งานล่าสุด" rows opened the capture
screen, and there was no place to see or play files. Now the stop reads
"หยุดและบันทึก", a "บันทึกลงเครื่องแล้ว · mm:ss" banner follows, and a new
"ไฟล์" tab lists every recording with in-place playback (shared
`useSegmentPlayback`). Observed on the A07 after the third install: stop →
banner; Files tab → the formerly stuck row now "เสร็จแล้ว" with play enabled;
play → Chromium requested audio focus for `dev.fung.local` and AudioFlinger
showed the active track, no console errors.

The owner then asked for the rest of a real recorder: no overlapping
controls, a player with transport and a playhead, and a recording waveform
that moves with the actual input like a phone voice-memo app. Delivered and
observed on the A07: (1) the plugin gained an amplitude-only `level` command
(Kotlin → `mobile_native_recorder_level` → `nativeRecorderLevel`), polled
every 80 ms while recording — 84 calls in a 7 s take — into a rolling history
drawn by `LiveWaveform` (canvas, newest-right; the web path feeds the same
history from its AnalyserNode); (2) `RecordingPlayer` reads a new
`mobile_capture_playback_manifest` (segment durations, no bytes), keeps one
`<audio>` across the sealed segments on a single global timeline, decodes
peaks per segment with WebAudio once, normalises them to the recording's own
maximum (`audioViz.normalizePeaks`, so a quiet room take still shows its
shape), and offers tap/drag seek, ±10 s, play/pause and elapsed/total — the
playhead was seen advancing 0:03 → 0:04 → 0:05 with the played part tinted;
(3) the completed capture screen shrinks its stage, embeds the player, and
puts the two actions in a grid so "อัดไฟล์ใหม่" sits above the nav; Files rows
expand into the player in place. `tests/audioViz.test.mjs` (wired into CI)
covers the level history, peak reduction/fitting/normalisation, segment
lookup and clock; Rust 438/438 with the manifest test.

**Mobile sign-in had never been possible on a phone.** With Google enabled
on the live project the owner tried it and got `auth_config_invalid`. The
native broker reads `FUNG_SUPABASE_URL` / `_ANON_KEY` from the process
environment, which the desktop fills from the repo's `.env` at startup — a
phone has neither. `native_auth::baked_value` now embeds the two public
values at compile time (`option_env!`, the same publishable pair the web
bundle already ships) as the fallback behind the environment, and
`scripts/mobile_android.ps1` loads `.env` into the build environment so the
Android core actually receives them. `resolve_configured` is unit-tested
(environment wins, blanks count as unset, nothing → the public error).
Two more walls stood behind that one, each found by reading `logcat` after
the owner's next tap: (1) the native PKCE exchange sent a form-encoded
`code=` body, which GoTrue answers with `400 bad_json` — it wants JSON
`{auth_code, code_verifier}` (probed against live: the JSON shape with a
bogus code returns `404 flow_state_not_found`, i.e. parsed and looked up) —
fixed and unit-tested; the PKCE verifier moved from sessionStorage to
localStorage because Android killed the app (`Render process kill (OOM)`)
while Chrome was up and the deep link relaunched it from scratch; (2) the
mobile webview's CSP was the desktop one, so `supabase.auth.setSession` →
`/auth/v1/user` was refused ("Failed to fetch") — `tauri.android.conf.json`
now adds exactly the project origin to `connect-src`, documented in the
egress register and pinned by the egress suite. (3) With all three fixed the browser
finished the sign-in and the app came back to the login card: Android had
killed the app while Chrome was up, the deep link cold-started it, and the
listener only subscribed to `onOpenUrl` (URLs arriving while alive) — the
launch URL sits in `getCurrent()`, which `listenForAuthCallback` now replays
once on startup. The unconsumed `fung.auth.pkce_verifier` still sitting in
the webview's localStorage was the tell. (4) Signed in at last, device registration
failed with "Failed to send a request to the Edge Function": the functions'
CORS default sent no `Access-Control-Allow-Origin`, which is right for the
desktop's native caller but wrong for the mobile webview, a browser caller
whose origin is `http://tauri.localhost`. `_shared/cors.ts` now allows that
origin by default (no ordinary web page can present it) and still requires
`ALLOWED_ORIGIN` for anything else; all three functions redeployed. The
Devices screen also gained the account card (name, e-mail, avatar) and a
sign-out button the owner asked for.

Desktop sign-in then failed in its own way: the loopback callback
`http://127.0.0.1:<port>/auth/callback` was never on the project's Redirect
URLs, so GoTrue fell back to the other app's Site URL with
`bad_oauth_state` (config: add `http://127.0.0.1:*/auth/callback`, now
recorded in `supabase/README.md`); the retry inside `LOGIN_TTL` (120 s) was
refused with `auth_request_in_progress`, which `AccountLoginPanel` rendered
as a generic `auth_start_failed` because the broker rejects `invoke` with a
plain string, not an Error — the panel now shows the broker's code verbatim. With the Redirect URL in place the same `bad_oauth_state` came back, and
probing `/auth/v1/authorize` explained it: GoTrue forwards a client-supplied
`state=` to Google verbatim (the `state` Google receives was our UUID, not
GoTrue's own), so on the way back GoTrue cannot find its flow state. The
desktop authorize URL no longer sends `state`; the loopback callback carries
only the PKCE `code`, which the pending login's verifier binds, and
`parse_callback` accepts that shape (a state, if ever present, must still
match). Unit-tested; the native flow had never been exercised against a real
provider before today.

**Speech recognition is live on this machine again (2026-09-14).** The
staged `.venv-whisper` under `D:\FUNG\` was gone and the repo copy was an
empty directory, so nothing could transcribe. `scripts/stage_whisper_runtime.ps1`
re-staged the pinned set (embedded Python 3.11.9, hash-locked wheels,
`Systran/faster-whisper-small` @ `536b0662…`) with a uv-managed CPython 3.11
as the build interpreter — and exposed a real bug in the script: under
Windows PowerShell 5.1 the model download raised a `NativeCommandError` on
huggingface_hub's stderr progress output and the run died after the wheels
installed; the download step now merges streams and judges by exit code. CPU
`int8` results with `scripts/transcribe.py`: a synthetic English sentence
transcribed verbatim in 2.9 s; the owner's own 50 s desktop recording from
2026-09-13 (7 live-meeting `mic-*.wav` chunks) transcribed in 9 s as Thai
(`languageProbability` 1.0) into 4 timed segments with confidences 0.97 /
0.92 / 0.60 / 0.43. **The GPU profile is staged too (2026-09-16):** the
11 CUDA 12 / cuDNN 9 DLLs came from NVIDIA's own pip wheels
(`nvidia-cublas-cu12` 12.9.2.10, `nvidia-cudnn-cu12` 9.26.0.51,
`nvidia-cuda-runtime-cu12` 12.9.79, downloaded with the host pip, unzipped,
and handed to `scripts/stage_gpu_runtime.ps1 -CudaSource`), so no G-Music /
Torch install is needed any more; `runtime/manifest.json` records their
SHA-256s. On the RTX 5060 Ti the same 50 s Thai recording transcribes in
1.7–1.8 s end to end (`--profile gpu`, float16, ~29× realtime; CPU int8 is
7.0 s on the same run) with segment text identical to the CPU pass. The very
first GPU process took 15 s — one-time kernel/cuDNN warm-up, not repeated.
With language auto-detection the GPU pass once guessed `en` at 0.44 and
rendered the last two short segments in English; the worker is normally
given `--language th`, which pins it. The desktop app reads
`FUNG_TRANSCRIPTION_PROFILE` from the repo `.env` (loaded generically at
startup), so `FUNG_TRANSCRIPTION_PROFILE=gpu` there switches the dev build
to the GPU worker; the key is now documented in `.env.example`. The app's
own Rust path was exercised too: `bin/live_smoke` (headless Live Meeting,
14 s real capture, ONIKUMA mic + Scarlett loopback) spawned
`transcribe_live.py --profile gpu`, the worker showed up in `nvidia-smi` as a
compute process, and the English sample played through the speakers came
back as two verbatim `[system]` segments. The smoke's Ollama summary step
returned `400 Bad Request` — a separate, pre-existing LLM configuration
issue, not part of transcription. **Root-caused and fixed the same day:**
`graph_build::resolve_available_model` falls back from a missing configured
model (`llama3.1:8b` is not installed here) to *the first tag* Ollama lists,
which on this machine is the embedding-only `bge-m3:latest`; `/api/chat`
refuses it with `"bge-m3:latest" does not support chat`. The fallback now
reads each tag's `capabilities` from `/api/tags` (present since Ollama 0.9;
absent means "assume usable") and skips anything not advertising
`completion`, keeping the configured name when nothing installed can chat.
Four unit tests pin it against a one-shot fake `/api/tags` server. Re-running
`live_smoke` on the GPU profile then produced a Thai meeting summary and a
Markdown export (`summary + export: OK`) from the transcribed English sample,
i.e. the full capture → whisper → Ollama → export chain passes on real
hardware.

**Web UI made usable on its own (2026-09-16).** Two findings first: the
production Vercel deploy (`fung-seven.vercel.app`) was still the build from
before PR #37 — its "04 / Demo-ready" section renders as mojibake, which that
PR fixed on 2 Sep — so nothing merged in the two weeks since (the
desktop-recordings tile, the auth fixes) had ever reached the public site;
and `/app` bounces every unauthenticated visitor back to the landing page,
where the only thing a signed-in browser could then do was play desktop
recordings from the same machine — the "เริ่มบันทึก" tile was a literal
"เร็วๆ นี้". The dashboard now records from the microphone in the browser
(`MediaRecorder`, WebM/Opus with an MP4 fallback for Safari), keeps the file
in that browser's IndexedDB with an honest "this browser only, nothing
uploaded" note, and lists/plays/downloads/deletes it — the download is the
hand-off to the desktop for transcription. Alongside: the landing page no
longer white-screens on a build without `VITE_SUPABASE_*` (the client is
built against a reserved `.invalid` host and the sign-in controls hide), the
public "ดู Desktop surface" link into the Tauri shell — which crashes in a
browser on unguarded IPC — is gone, `vercel.json` rewrites every non-asset
path to the SPA, and the dashboard stacks to one column at phone width.
`tests/webRecordings.test.mjs` pins the codec choice, file naming, and these
source-level guarantees. Deploying it is the owner's step: the Vercel CLI on
this machine is logged out and the Git integration evidently did not pick up
the last two weeks of merges.

**Browser recording → desktop transcript (2026-09-16).** The hand-off that
closes the web loop: the loopback API gained `POST /recordings/import`
(reads a `Content-Length` body up to 512 MB, writes it under
`<data root>/imports/web/<recording id>.<ext>`, creates a project named after
the upload and a running `transcript.transcribe` job, and runs the desktop's
own `run_import_pipeline` with a caller-chosen recording id), `GET /jobs/{id}`
(the job row, for polling) and `GET /recordings/{id}/transcript` (the
project-scoped `transcript_view`, resolved through the recording's own
project). CORS now admits `POST` with `Content-Type` and `X-Fung-Filename`.
On the web, each browser recording gets "ถอดเสียงที่ desktop" once the desktop
is connected; the dashboard remembers the desktop job on the recording in
IndexedDB, polls every 2 s, and renders the segments inline when the job
completes (and refreshes the desktop-recordings tile, where the import now
also appears). Rust tests cover body framing over a real socket, the 413,
auth/path gating, custody + job creation + polling to a settled state, and
the transcript route; Node tests pin the client's wire shape. Deployed to
production the same day as the recorder.
Verified by Rust and Node tests below; the real-browser pass on the production
web (which needs this change deployed) and Chrome's one-time "local network"
permission prompt are still to be observed on the owner's machine.

Two follow-ups from 0.2.18b are now corrected rather than left as open bugs:

- **Mobile "resume selected web recorder" is not a standing bug.** Capture
  backend is chosen in exactly one place — `begin()` via
  `acquireCaptureBackend`, which is native-first and falls to web only when
  `startNativeRecorder` reports unavailable (i.e. off-Tauri preview). There is
  no separate resume path that re-picks web. The UAT observation was the
  `dataDir` recorder bug (fixed in PR #42) reporting `available:false` at that
  moment, so capture fell to web. On a real Android device native is always
  tried first.
- **Issue #41 is fail-closed on incompatible state, not an install-idempotency
  bug.** `genesis_adapter::install` registers v1..v10 stepwise, skips
  `REL_SCHEMA_VERSION_CONFLICT` on already-registered steps, and re-registering
  the current version is idempotent (proven by
  `schema_v4_adds_external_tables_and_upgrade_is_idempotent`, which re-runs
  `install` on a v10 store). Normal reboots therefore do not crash. The crash
  was a genuinely incompatible pre-September genesisdb (written by an older
  GenesisBlockDB) — the right behavior is to refuse rather than proceed on
  mismatched state. A friendlier recovery/error is a product decision, not a
  latent correctness bug; left for the owner to direct.

## Current truth sync (2026-09-04)

The development machine moved: `D:\FUNG` no longer exists, and the working
tree is `C:\Users\pc\workspace\fung`. Every staged-runtime path recorded below
under `D:\FUNG\...` is historical; the whisper/CUDA runtime has not been
restaged on the new tree, so the packaged-worker evidence from 0.2.12b-0.2.13b
describes an environment that is gone. This machine now has the full local
toolchain the repo needs: rustup/cargo 1.98 (stable-msvc), VS Build Tools 2022,
and a project-local Android stack under `.toolchains/` (JDK 17, SDK platform 34,
build-tools 36.0.0, NDK 29.0.14206865, platform-tools). Windows Smart App
Control, which blocked freshly built binaries with os error 4551, is
permanently off. `cargo fmt --check`, `clippy --all-targets -D warnings`, and
`cargo test` (419/419) all pass locally.

PR #39 (merge `f6d352f`) landed the 2026-09-01 audit: the desktop shell no
longer renders fabricated data (fake five-recording library, twelve hardcoded
meters, invented activity/event rows, index-derived recording states, fake
default meeting title, `signalCards`/`runtimeStats` fictions) — every surface
now shows real segment/speaker/job/health data or an honest Thai empty state;
the three Supabase edge functions share an env-driven CORS allowlist instead of
a hardcoded wildcard; the BYOM cloud path reads an optional per-provider model
override (UI input included) with the old IDs as named defaults; the landing
page's mojibake Demo section, 404 APK links, and personal-account download URL
are fixed; ~4,600 lines of confirmed dead code were removed (13 unreferenced
assets, 2 unwired scripts, dead TS exports including the never-built "pitching"
tab); and `tests/transcribeConcatOnly.test.py` is wired into CI with the
coverage guard extended beyond `.test.mjs`. The repository is now public with
secret scanning, push protection, and Dependabot enabled (history scanned
clean; the one `glib` alert is dismissed `not_used` — it lives only in tauri's
Linux gtk chain and FUNG ships Windows+Android).

PR #40 (merge `7b37a6e`) made the Android build real again and yielded the
first physical-device render evidence: `pick_folder` is desktop-only in
tauri-plugin-dialog and is now cfg-gated in `backup.rs`/`filesystem_backup.rs`;
`minSdkVersion` is 26 so the NDK links `libaaudio` for cpal; and the two Kotlin
plugin classes the Rust core hard-requires at startup — `RecorderPlugin`
(gapless 5s AAC segments via `setNextOutputFile`, sha256 per segment, plugin
permission flow for the microphone) and `AiProfilePlugin` (sdk/arm64/RAM/
storage probe) — were reimplemented from the Rust contracts as tracked sources
under `src-tauri/mobile/android/`, synced into the generated project by
`mobile_android.ps1` on every init/build. Their originals lived only in the
gitignored `gen/android` tree and were lost with it; the crash was
`ClassNotFoundException` at launch. The rebuilt arm64 debug APK installs,
launches, and renders the mobile surface on a Galaxy A07 (Android 16) — the
0.2.11b "no physical Android device has rendered the mobile surface" gap is
closed at the render level. Later the same day, capture UAT passed on that
device end-to-end: a voice note records/lists/plays with Thailand-local
timestamps; a stranded native meeting capture (recorded before the recorder
path fix below) reconciled into canonical storage and resumed; and the
waveform moves with the actual input level. Pairing UAT stays open behind the
Supabase gate recorded below.
The desktop shell silhouette is now a plain rounded rectangle (the notched
`PANEL_PATH` let the rail and command-deck bar read as overlapping UI), long
titles truncate under the command deck, and over-tall workbench tiles scroll
instead of painting over the detail dock.

Working-tree truth beyond the merges (not yet committed as of this sync):
mobile Google login was dead end-to-end — `authFlow.ts` still invoked
`auth_begin_google_login`, a native command deliberately removed by the
native-session-broker redesign (its absence is pinned by
`tests/nativeSessionCustody.test.mjs`), so every login tap failed with
"command not found". It is rewritten to the phase-1 contract the tests expect:
supabase-js PKCE via `signInWithOAuth(skipBrowserRedirect)` opened in the
system browser through the opener plugin, deep-link return on
`fung://auth/callback`, and `exchangeCodeForSession`. The webview capability
gained `opener:default` (the login tap was otherwise ACL-blocked) and
`core:window:allow-start-dragging`. `fung://auth/callback` is now whitelisted
in the Supabase project's redirect URLs.

The mobile shell also had its own fabricated-data debt, found by on-device
use: `mobileStore` seeded three invented notes plus graph edges ("ประชุมทีม
เสียง 09:42–10:18" et al.) that rendered as real work, no surface listed the
project's recordings at all (the only route to one was the capture screen
immediately after recording it), the RecorderPlugin wrote segments under
`filesDir` while `mobile.rs` reconciles from the app-dir root — so a real
native capture was invisible to the ledger — and the waveform was eighteen
hardcoded bars on a CSS loop. All four are fixed on device: the seeds are
gone (with a purge migration for previously polluted localStorage), a new
`mobile_recordings_query` command lists recordings newest-first on the home
screen with an honest empty state, the recorder writes to `dataDir`
(device-verified reconcile into canonical storage), and the waveform is
driven by measured input level — web via an `AnalyserNode`, native via
`MediaRecorder.getMaxAmplitude` surfaced as `levelPercent` — and sits flat
when no real reading exists.

Two operational discoveries: (1) the desktop panicked on second boot against
the pre-September `%APPDATA%\dev.fung.local` state with
`REL_SCHEMA_VERSION_CONFLICT` (issue #41); the data root held no projects and
was reset, after which repeated boots are clean. See the 2026-09-13 sync below
for the corrected characterization — `genesis_adapter::install` is idempotent
across same-version reboots (test-proven), so this was fail-closed on an
incompatible pre-September genesisdb, not an install-idempotency bug. (2) The
Supabase project (`nqnrvqnijzovkrhxslfp`) had been
auto-paused by the free tier, which removes its DNS entirely — every auth
surface fails with NXDOMAIN until it is restored; at the time of this sync the
restore itself is blocked by a Supabase-wide "Project Lifecycle Actions"
outage, so desktop↔mobile login/pairing UAT remains open behind that external
gate. GitHub Actions minutes are no longer a constraint (public repo), closing
the billing-block failure mode that silently skipped CI from 2026-08-30 to
2026-09-01 and let unformatted code merge.

## Current truth sync (2026-08-23)

PR #16 is merged into `origin/main` at merge commit `26da78466364e479085d0aa5d7f06e24a08bd12c`. Its routing, backup-payload, and connector-timeout changes are therefore part of the current mainline. The physical Android, clean-install restore, real connector, and release gates remain open as recorded below.

The local staged runtime check is split into separate facts: `D:\FUNG\.venv-whisper\Scripts\python.exe` imports `faster-whisper` 1.2.1 and exposes the worker CLI; `D:\FUNG\.venv-whisper\models\small` contains the pinned `Systran/faster-whisper-small` revision `536b0662742c02347bc0e980a01041f333bce120`; `D:\FUNG\runtime\manifest.json` records 11 staged CUDA 12/cuDNN 9 DLLs. `scripts/smoke_gpu_standalone.ps1` passed with `C:\Windows\Media\Alarm01.wav` and `--profile gpu`; this proves the packaged worker path, not Live Meeting real-capture or device UAT.

The approved Phase 4 Google Drive slice is now implemented locally: native
Authorization Code + PKCE on loopback, exact `drive.appdata` scope, OS-keyring
refresh-token custody, authenticated redacted metadata/audit function, and a
separate Desktop connect/upload/restore UI. This is implementation-beta truth,
not provider or release proof: Google Cloud client configuration, Supabase
function deployment, real consent/upload/download/revoke, clean-install
restore, and physical Android/FUNGWIRE delegation remain open.

The 2026-08-26 truth sync is based on `main` commit
`888adeded643f448c283c7990aabc421f71a20be`. Focused verification passes
Backup 17/17, Google Drive contract 6/6, Auth 8/8, Rust Drive 16/16, TypeScript,
and the Vite production build. `cargo check` exits 0 with the same 18 retained
baseline warnings recorded by the final D-GDA6 Terra review. The W1 source
contract passes 7 checks in the current host, while its executable PostgreSQL
17 check is skipped because Docker Desktop is unavailable; this run therefore
does not replace the earlier independently recorded PostgreSQL evidence and
does not close staging, provider, device, or production gates.

## Routing and backup-payload overlay (historical snapshot, 2026-08-19; superseded)

This section preserves the pre-merge state. PR #16 was subsequently merged as recorded in the current truth sync above.

`fix/routing-predicate-and-backup-audio` is pushed and open as PR #16 with
both CI jobs green (`frontend` 27s, `rust` 6m8s). It is **not merged**, so
`main` does not yet carry any of the following.

Two reachability defects and one timeout defect are closed at source level:

- **Mobile surface on Android.** `resolveRootRoute` returned `"desktop"` for
  every Tauri runtime with no platform branch, so the APK loaded the fixed
  1280x780 desktop shell and the whole `src/mobile/` tree was unreachable on
  the device it was written for. Routing now takes an `isMobilePlatform` input
  derived from the webview user agent, and neither Tauri shell is gated on
  Supabase any more. **This is code and unit-test evidence only — no physical
  Android device has rendered the mobile surface since the change.**
- **Backup UI reachability.** All seven backup commands were called only from
  `src/web/AccountSettings.tsx`, a surface the Tauri runtime can never route
  to and where `invoke` does not exist. The controls moved to a shared
  `src/components/BackupPanel.tsx` mounted by both shells. This supersedes the
  0.2.10b claim that "Desktop AccountSettings has a labelled development/test
  UI" — no desktop AccountSettings existed when that was written.
- **Backup contents.** `run_backup_job` encrypted the Genesis bundle and
  nothing else, while every audio chunk lives as a loose file referenced by an
  absolute path; there is no Genesis blob API call anywhere in the tree. A
  "verified" archive was therefore a database-only backup. `backup_payload.rs`
  now carries the bundle plus every audio file the ledger references under the
  same authenticated envelope, records files that are unreadable or no longer
  match their capture-time digest as explicitly omitted rather than dropping
  them, and reports both counts through `BackupRunReport` / `RestoreResult`.
  Pre-container archives still restore.
- **Connector tool timeout.** `execute_stdio_tool` started its deadline before
  `spawn_stdio`, so `limits.timeout` covered process launch and handshake.
  Measured on this host, spawn was 1.34s against 48ms for the whole
  initialize/list/call exchange. Startup now has its own budget; the caller's
  timeout bounds the requested work; total stays capped by
  `MAX_MCP_EXECUTION`. This also removed a long-standing suite flake, which
  reproduced on `main` in 2 of 3 full runs.

Two bounds are now named in errors rather than hit silently: the archive
payload is capped at 2 GiB because the envelope encrypts from one in-memory
buffer, and audio chunks are enumerated per recording because GenesisBlockDB
rejects any query limit outside `1..1000` and offers no offset — a saturated
read fails closed instead of truncating.

Automated evidence on this host: Rust **238/238** across six consecutive full
runs, all eight Node suites **46/46**, `npm run build` passing. Still open and
not claimed here: physical Android render, real clean-install restore into the
approved Phase 4 roots, U9 closure, and every release gate.

## Public Desktop distribution overlay (2026-08-14)

`Freshair129/FUNG-Releases` is now the public binary-only Desktop channel. Its
v0.1.0 latest URL downloaded anonymously as 515,089,576 bytes with SHA-256
`f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`,
matching the installer that passed local and real-hardware gates. Release
verification workflow run `31770231402` passed. The Landing cutover has a
source regression that rejects the private `Freshair129/FUNG/releases` path;
production deployment remains the final gate for this overlay.

## Desktop v0.1.0 release-candidate overlay (2026-08-14)

The isolated `codex/desktop-live-release` worktree now has a self-contained
Windows x64 CPU release candidate: portable CPython 3.11.9, pinned
`faster-whisper` 1.2.1 dependencies, pinned local `small` model, both worker
scripts, licenses, and a SHA-256 manifest. A 30.082-second real-hardware run on
the Fantech Leviosa microphone and Scarlett Solo system loopback produced 8
durable chunks and 10 transcript segments. The final NSIS candidate installed,
launched a visible `FUNG` window, and transcribed the speech fixture from its
installed runtime with 0.9988 confidence.

The final local asset is 515,089,576 bytes with SHA-256
`f67a78e0b216628d19335646342eea20575d5c7b5a16cccf7daf32c6b780d414`.
The production-style browser gate renders the version, approximate 491 MB
size, unsigned SmartScreen notice, and stable latest-release URL with zero
console errors. GitHub upload/hash equality and Vercel production promotion
remain publication gates and are not claimed by this pre-tag record.

## Phase 3 post-merge overlay (2026-08-12)

The Phase 3 follow-up is merged into `main` at `cea2d93` (PR #10), keeping the
routed Live Meeting and external-retrieval surfaces intact while adding the
scoped Python worker hardening. Automated evidence is Rust `195/195`, Python
concat `5/5`, mobile `4/4`, auth `5/5`, design-system `2/2`, and TypeScript/Vite
build passing. Post-merge GitHub CI run `31609642060` passed for frontend and
Rust. Real provider/device UAT and release/product gates remain open; this
overlay does not promote Phase 3 to fully release-ready.

## Implemented

| Area | Current Truth |
| --- | --- |
| Tauri v2 scaffold | Present with Rust backend and React/Vite frontend. |
| Frontend build | `npm run build` passed. |
| Rust check | `cargo check` passed. |
| SQLite WAL | Backend initializes SQLite with WAL, foreign keys, and busy timeout. |
| Local API | Basic loopback `/health` API path exists. |
| CLI | `fung-cli` smoke path exists and confirms WAL setting. |
| Contracts | API, MCP, CLI, GenesisBlockDB, job model, and SQLite schema files exist. |
| Layout RCA fix | Signals are inside the panel; `.fab-signals` removed. |
| Design direction | Skeuomorphic material implemented in UI and docs. |
| Live Meeting UI | `LiveMeetingPanel` renders source-aware transcript, current topic, local Knowledge Base question, and post-meeting summary surfaces. |
| Live capture/transcript route | Rust capture and long-lived whisper worker route microphone plus optional system audio into durable chunks and live transcript events. |
| Local meeting intelligence | Topic tracking and manual questions search stored transcript/knowledge graph while keeping source citations. |
| Post-meeting package | Overview, timeline/key points, decisions/actions, provenance rows, and Markdown export are implemented in `meeting_intel.rs`. |
| Live Meeting entry | Both the P1 card and fixed microphone rail open the real P1 `live-capture` panel; the rail path has a source regression test. |
| External retrieval contract | Rust DTOs define non-secret connectors, three read-only capabilities, suggestion/preview/run/sanitized-result states, and stable errors; the pure trust module stays separate from sibling transport/command modules. |
| External retrieval persistence | Genesis schema v9 extends connector metadata and registers grants, previews, runs, and sanitized results through the normal adapter path. |
| External retrieval trust foundation | Default-deny grant policy, canonical preview hash, exact field minimization, zeroized OS-keyring lifecycle, connector disconnect/revoke, typed Genesis audit payloads, and hostile-result sanitization are implemented and tested. |
| External retrieval backend | Allowlisted stdio MCP `2025-11-25` initialize/list/call, bounded process I/O, timeout/cancel/cleanup, durable one-time execution, all eight planned Tauri commands, and document/CRM fixture execution are implemented behind default-off `FUNG_EXTERNAL_MEETING_TOOLS=1`. |
| External retrieval operator UI | `ExternalMeetingToolsPanel` is embedded in Live Meeting behind default-off `VITE_FUNG_EXTERNAL_MEETING_TOOLS=1` with connector list/register/disconnect, exact field and transcript-evidence selection, preview/deny/approve, running/cancel, meeting-scope revoke, sanitized result, inert source references, policy/evidence/time provenance, and run history. |
| Phase 4 filesystem/Google Drive backup | The local filesystem path remains a development/test transport with Genesis full export → XChaCha20-Poly1305/Argon2id encryption → atomic bounded-root write, clean-target restore, and deep fixture verification. The separate Google Drive path now has native PKCE/keyring custody, exact `drive.appdata`, redacted metadata/audit, resumable appDataFolder upload, digest-bound download, and clean-target restore controls. No real provider or clean-install restore has been run. |
| Mobile device reconciliation | The Android `devices` row is always resolved by (current user, fingerprint); the cached `fung.device.id` is only a mirror, replaced when stale and cleared on sign-out/revocation. Supabase RLS ownership policies were rechecked and required no migration. |
| GPU runtime staging | `stage_gpu_runtime.ps1` stages FUNG-owned CUDA 12/cuDNN DLLs and writes a SHA-256 manifest. |
| GPU worker launch | The transcription subprocess resolves FUNG resources at runtime, selects an explicit CPU/GPU profile, and prepends FUNG's CUDA directory to its own `PATH`. |

## Partially Implemented

| Area | Current Truth | Gap |
| --- | --- | --- |
| Project CRUD | Backend commands exist for project creation/listing. | Needs full UI workflows and persistence QA. |
| Job model | Basic create/list commands exist. | Needs execution engine, retries, pause/resume, failure recovery. |
| Model providers | Seed local providers exist. | Needs provider diagnostics and real adapter execution. |
| Transcript read completeness | Closed at the source: GenesisBlockDB commit `1ff6862` adds `RelationalQuery::offset` (offset pages are ordered by the base table's primary key, so consecutive pages partition the result set), and FUNG pins that rev. `genesis_adapter::query_all` reads length-driven tables whole in `ROW_CAP`-sized pages, and every reader that used to refuse or truncate at the ceiling now reads whole: transcript view, `meeting_intel::load_segments`/`meeting_ask`/`meeting_summaries`, subtitle export, `fungwire_client::gather_segments`, audio integrity, backup inventory, recovery, diarization, graph build, and gap fill. Rust regression 419/419 includes tests proving a ROW_CAP+N recording is read whole, ordered, and unduplicated. | `capped`/`searchedRowsCapped`/`unread_recordings` fields stay in the serialized contracts for frontend stability but are truthfully never set any more; removing them (and their dormant UI notices) is cleanup, not correctness. Reads that genuinely want at most one page (single row by id, top-N) still use the single-read path. |
| Export UI | Subtitle export is real: `export.render` is a job the engine runs, `transcript_export` writes `.srt` and `.vtt` beside the recording, both are recorded in `export_artifacts`, and `list_export_artifacts` lets the shell tell the user where they landed. Formatting is unit-tested against the ways transcript text corrupts each format (blank lines, `<`, `-->`, zero-length cues). Segment reads page past the engine ceiling, so a long recording exports whole, cues sorted by start time. | Capped at one recording per run. Audio export (WAV/MP3) and the separate export queue are still unimplemented. The write path is tested against a real GenesisBlockDB store (files on disk, both artifact rows, retry idempotence, whole-file export past the old ceiling); what is untested is the packaged app's own click-to-file round trip. |
| Summary/intent UI | Summary/action output pipeline and display surface exist. | Intent-specific UI and complete evidence-span review remain incomplete. |
| Live speaker attribution | Source channels map to editable `เรา`/`อีกฝ่าย` labels. | This is capture provenance, not arbitrary live multi-speaker diarization. |
| Live intelligence runtime | Topic and summary routes exist; capture can degrade without the worker. | Current machine has no bundled Whisper interpreter/model and `faster_whisper` is unavailable, so live transcription requires runtime installation plus UAT. |
| Live Meeting entry | The fixed microphone rail now opens the real panel and its regression passes. | Current packaged-app interaction/UAT remains to be rerun after the prior bootstrap incident. |
| Audio import | `import_and_transcribe` is implemented, registered, and reachable from the desktop UI; it transcribes a picked file and persists segments. Recorded as "not implemented" through 0.2.10b, which was wrong. | The source file is referenced in place — no copy into project storage, no chunking, no checksum — so moving or deleting it invalidates a recording the ledger still reports `completed`. No `model_runs` row is written, so imported transcripts carry no model provenance. |
| External meeting retrieval | Backend plus operator workflow, stdio fixture transport, zero-process-before-approval, document/CRM reads, connector lifecycle, sanitized result rendering, recording-row isolation, and bounded relaunch persistence smoke are tested at unit/source/integration level. | Automated keyboard/1200×780 visual UAT, detailed connector health, artifact-wide secret scan, real-device capture-isolation UAT, summary/export review after restart, and real-connector UAT remain. |
| URL ingest | `fetch_and_transcribe`, `media_fetch_status` and `media_fetch_consent_set` are implemented, registered, and reachable from the desktop UI. A pasted http(s) link is fetched audio-only by a pinned yt-dlp worker, taken into custody, and transcribed through the same pipeline as a file import; the path is declared in the egress register §1.6 and guarded by `tests/egressRegister.test.mjs`. | Proven against a local HTTP server (`Generic` extractor) only. No real site — including YouTube — has been fetched on a device, and without the opt-in `deno` runtime YouTube is expected to fail outright. The staged runtime has never been installed on a machine here, so the readiness probe is unit-tested rather than run. yt-dlp is pinned to 2026.7.4 and the app has no update path, so extractor rot is a standing maintenance cost. |
| GPU standalone release | DLL staging and child-process isolation are implemented. | Must build and run a copied packaged bundle with a speech fixture; NVIDIA redistribution approval remains a release gate. |

## Not Implemented Yet

- Speaker diarization *reach*. `diarize.py` now ships in `bundle.resources`,
  `scripts/stage_diarization_runtime.ps1` installs the `torch`/`pyannote.audio`
  tree into the staged runtime, and `diarization_status` reports which
  prerequisite is missing before a worker is spawned — so an installed build
  can run diarization once the operator opts in. The dependencies stay out of
  the default bundle deliberately: the tree is hundreds of megabytes and the
  `pyannote/speaker-diarization-3.1` weights are gated per user on Hugging
  Face, so they cannot be redistributed in an installer.
  What remains is reach: the only route into diarization is still Zoom
  mixed-audio import. A locally captured meeting cannot be diarized, because
  its audio is per-channel chunks rather than one file, and deciding what to
  feed the model (system channel alone, or a mixdown) changes what the speaker
  timings mean. Neither the dependency install nor a diarization run has been
  executed on a device.
- URL ingest *at scale*. The path works, but two things are unproven and one
  is structural. Unproven: a real extractor run on a device, and the `deno`
  JS-runtime install that YouTube's signature challenges need. Structural:
  yt-dlp tracks sites that change deliberately, so a pinned version decays —
  a build six months old will fail on links a current yt-dlp handles, and
  nothing in the app tells the user that is why. Re-pinning is a lockfile
  regeneration (`stage_media_fetch_runtime.ps1 -GenerateLock`) and a release.
- Noise reduction.
- Source separation/layer generation.
- Real transcript editor.
- Full MCP server runtime.
- Full local API beyond initial health/project/job surfaces.
- Vendor-specific production connectors and all external write capabilities.

## Latest Verification Evidence

| Check | Result |
| --- | --- |
| `npm run build` | Passed on 2026-08-14 after `npm ci` restored missing local CLI binaries |
| `cargo check` | Passed |
| `npm audit --audit-level=moderate` | Passed on 2026-08-23; current lockfile reports **0 vulnerabilities** across 84 audited dependencies. |
| SVG XML validation | Passed |
| Browser layout check | Signals parent is `.panel-glass`; floating signal count is `0` |
| Compact viewport check | `1200 x 780` rendered without body scroll |
| Live Meeting code-route inspection | `LiveMeetingPanel.tsx`, `live_meeting.rs`, and `meeting_intel.rs` route capture, transcript, topic, local search, summary, actions, and Markdown export. This is implementation evidence, not a current real-device UAT. |
| FUNG-owned CUDA runtime manifest | Passed: 11 staged CUDA 12/cuDNN DLLs present and hash-recorded. |
| Isolated CUDA provider probe | Passed: with `G-Music`, Torch, and CUDA Toolkit paths excluded, FUNG Python/CTranslate2 reported `cuda_device_count=1`. |
| Clean-path GPU transcription | Passed: FUNG transcribed `C:\Windows\Media\Alarm01.wav` with `gpu` profile while its worker `PATH` began with `D:\FUNG\runtime\cuda12\bin` and excluded G-Music/Torch/CUDA Toolkit paths. |
| Release-layout GPU transcription | Passed: the Tauri release resource layout (`target\release\.venv-whisper`, `runtime`, and `scripts`) completed the same clean-path GPU transcription smoke. |
| Rust validation | `cargo check` passed after runtime-launcher changes. |
| Live Meeting rail regression | `npm run test:desktop-bootstrap` passed 5/5; the microphone control selects P1 `live-capture` and opens `LiveMeetingPanel`. |
| External MCP trust-foundation tests | Focused Rust tests passed 14/14: policy deny/allow matrix, canonical preview hash, minimizer, keyring lifecycle, Genesis disconnect/revoke, typed audit, secret-field rejection, hostile-result sanitizer, and resource limits. |
| External MCP Sprint 4 tests | Focused Rust cluster passed 23/23: trust contracts, stdio lifecycle/allowlist, timeout/cancel/cleanup, eight-command surface, connector/keyring/grant/disconnect lifecycle, document and CRM fixture calls, one-time execution, durable sanitized result/audit, failure terminalization, and active recording-row isolation. |
| External tool frontend tests | Passed 5/5: default-off flag, exact argument minimization, UI state transitions, Thai capture-safe errors, eight-command client surface, revoke control, and Live Meeting embedding. |
| Genesis v9 migration | Focused Rust integration test passed grant → preview → run → sanitized result round trip and reinstall idempotency. |
| Full Rust library regression | Passed **217/217** on 2026-08-19 with the exact plan command (adds the Phase 4 backup-job/restore cluster); existing compiler warnings are non-failing and unrelated to this status. |
| Phase 4 backup/restore tests | Focused Rust cluster passed: export→encrypt→write→clean-restore round trip with note/graph/audio-chunk identity, wrong-secret/tamper/missing-archive/existing-target preservation, plaintext-staging boundary rejection, job-serialization guard, and fail-closed status. Node `test:backup-flow` passed 10/10 (opaque picker contract, restore confirmation gate, truthful error text) and `test:device-reconcile` passed 6/6 (ownership lookup, duplicate avoidance, stale-cache replacement, sign-out clearing). |
| Phase 4 clean-install/device gates | The real clean-install restore on the approved `D:\FUNG-Phase4-TestStorage`/`D:\FUNG-Phase4-TestRestore` roots and the physical Android/Dashboard identity check have not been run; U9 and release gates remain open. |
| Auth/mobile/design regressions | Passed auth 5/5, mobile capture 4/4, and design-system publishing 2/2. |
| Diff hygiene | `git diff --check` passed. Repository-wide `cargo fmt --check` still reports broad pre-existing formatting debt outside this scoped change. |
| Rebuilt Desktop runtime | The debug `fung.exe` launched with title `FUNG`; a close/relaunch smoke observed PID 37720 then PID 9088 and non-zero window handles, with Genesis counts unchanged (`projects=1`, `recordings=1`, `transcript_segments=13`, `audit_events=1`). Windows Graphics Capture, browser screenshot, and keyboard automation remain unavailable, so visual/keyboard UAT is still open. |
| Real connector/device diagnostics | Claude Desktop MCP registry is empty, no approved vendor endpoint/credential is configured, and `adb`/`scrcpy` are absent. The real-connector and physical-device gates remain blocked, not waived. |
| Python worker syntax | `py_compile scripts/transcribe.py` passed. |
| Current Whisper runtime availability | `py -3` reports no `faster_whisper`, while FUNG's staged `.venv-whisper` runtime imports `faster-whisper` 1.2.1, has the pinned `small` model, and passes the standalone GPU smoke with the staged CUDA 12/cuDNN 9 bundle. Live Meeting real-capture, device, visual, and connector UAT remain open. |
| Loopback recordings API (0.2.20b) | Rust **434/434** on 2026-09-13 (419 prior + 15 in `local_api::tests`: request/range/origin/token parsing, open `/health` vs 401 elsewhere, 204 preflight, 405, newest-first list with channels from chunk names, in-order stitching independent of row order, whole-file import passthrough with MIME, not-found/traversal rows never followed, missing slices skipped and counted, 200/206/416 `Range`, nested-id rejection, and two real-socket tests for exact `Content-Length` and CORS grant only for allow-listed origins). `cargo clippy --all-targets -D warnings` clean. `npm run build` passed; Node `test:local-api-client` 4/4 (loopback-only parse/build), `test:egress` 8/8 with the single-file `fetch` allowance, `test:ci-coverage` 2/2, `test:diarization` 8/8. Real-browser playback against the production web is not yet observed (needs deploy). |
| Live Supabase bring-up (0.2.20b) | 8 migrations applied to `nqnrvqnijzovkrhxslfp` on 2026-09-13 (`list_migrations` shows all eight); 11 public tables all `relrowsecurity = true`; 14 functions present; `w1_authority_schema.sql` read-only posture block passed; security linter: 0 errors (INFO on server-only tables without policies, WARN on the two intentional `authenticated` SECURITY DEFINER pairing RPCs); 3 Edge functions ACTIVE v1 with `verify_jwt`, each returning 401 without a JWT and no `Access-Control-Allow-Origin` for an unlisted origin. The two write-and-rollback adversarial blocks of the SQL test were not run (read-only session). |

Screenshot artifacts from the latest UI validation:

- `output/playwright/fung-skeuo-1280x720.png`
- `output/playwright/fung-skeuo-1200x780.png`

## Next Milestones

| Milestone | Goal | Exit Criteria |
| --- | --- | --- |
| M1 Recording Core | Real long recording with chunks | Can record, stop, recover, and list chunk metadata locally. |
| M2 Audio Import/Export | File import and WAV export | Can import audio and export source/derived WAV. |
| M3 Transcription MVP | BYOM transcription job | Transcript segments saved with timestamps and provenance. |
| M4 Speaker Review | Diarization and label editing | Speakers are editable and linked to segments. |
| M5 Summary/Intent | Evidence-based AI outputs | Summary and intent cite transcript time ranges. |
| M6 MCP/CLI Completeness | Automation over same state | MCP and CLI can drive project/job/export workflows. |
| M7 Controlled Meeting Retrieval | Read-only document/CRM lookup through MCP | Suggest → preview → per-call approve, minimization, keyring, audit, sanitized result, denial/revoke/failure gates pass. |

## Known Risks

| Risk | Level | Mitigation |
| --- | --- | --- |
| Long recording data loss | High | Chunked writes, WAL state, recovery scan. |
| Model runtime instability | High | Adapter isolation, diagnostics, resumable jobs. |
| Legal/privacy misuse | High | Local-first defaults, opt-in cloud, inference labels. |
| Speaker misidentification | Medium | Editable labels, no definitive identity claims. |
| CPU/GPU overload | Medium | Queue control, pause/resume, runtime profile. |
| CUDA redistribution | High | Treat NVIDIA redistribution terms as a release gate; stage only from an approved, version-pinned source. |

## Version Diff

| Version | Change |
| --- | --- |
| 0.2.20b | Google login works end to end after the shared-project Redirect-URL fix (callback had landed on ZURI's `localhost:3000`). Recorded that the live `public` schema is empty (migrations never applied) and Storage has no buckets. Added the same-machine loopback recordings API (`local_api.rs`: token-gated `/recordings` + stitched/ranged `/recordings/{id}/audio`, allow-listed CORS, custody-resolved paths) and the web "ไฟล์ล่าสุด" tile with a loopback-only client pinned by the egress suite. Rust 434/434, clippy clean, build and Node suites green; real-browser pass on production pending deploy. |
| 0.2.19b | Supabase back online but login gated on the disabled Google provider (dashboard config, not code); recorded PR #44 (web paired-device list, REQ-B-08) and PR #43 (tracked brand kit); corrected two 0.2.18b follow-ups — mobile capture is native-first in the one `begin()` path (no resume-picks-web bug), and issue #41 is fail-closed on an incompatible pre-September genesisdb, not an install-idempotency bug (`install` is reboot-idempotent, test-proven). |
| 0.2.18b | Truth-synced the audit sweep (PR #39: honest desktop UI, CORS allowlist, BYOM model override, landing fixes, ~4,600 lines of dead code out, `.py` suite in CI, repo public + secret scanning/push protection/Dependabot), the Android build restoration (PR #40: cfg-gated `pick_folder`, minSdk 26, reimplemented tracked `RecorderPlugin`/`AiProfilePlugin`, rectangular shell) with first physical Galaxy A07 render, the working-tree mobile login rewrite to supabase-js PKCE + deep link with opener capability, the `D:\FUNG` → `C:\Users\pc\workspace\fung` machine move with full local toolchain, issue #41's second-boot schema conflict, and the Supabase free-tier pause/NXDOMAIN gate blocking login/pairing UAT. |
| 0.2.17b | Bumped GenesisBlockDB to main tip `79b41a3` (0.2.5): offset paging now rides mainline plus the SQL-surface/edge-projection/journal-retention work. `OpenOptions` gained `retention` (FUNG passes `None` = `frontier_only`, the prior behavior), and three frontier assertions became deltas because `open()`'s schema registrations now advance the frontier. Rust 419/419, Vite build and focused Node suites passing. |
| 0.2.16b | Closed the 1000-row read ceiling: GenesisBlockDB `1ff6862` adds primary-key-ordered offset paging, FUNG pins it, `query_all` reads length-driven tables whole, and every refusal/truncation reader now reads complete. Rust 419/419, engine relational suites green, frontend build and focused Node suites passing. |
| 0.2.15b | Truth-synced merged Google Drive/Native Broker local evidence, current focused tests, baseline warnings, and the Docker-bounded W1 verification gap. |
| 0.2.14b | Added the approved local Google Drive implementation slice: native PKCE/keyring adapter, authenticated metadata/audit function, separate Desktop UI, resumable appDataFolder transport, and digest-bound restore. Real provider, deployment, clean-install, and device proof remain open. |
| 0.2.11b | Recorded PR #16: mobile/desktop routing fix, reachable backup UI, audio-bearing backup payload, and the connector startup/timeout split. Corrected two 0.2.10b inaccuracies (desktop AccountSettings, audio import). Physical Android and clean-install restore stay open. |
| 0.2.10b | Recorded Phase 4 filesystem test backup/restore implementation and mobile device-reconciliation hardening with 217/217 Rust plus green focused Node evidence; clean-install restore and physical Android identity gates stay open. |
| 0.2.9b | Truth-synced current frontend/Rust verification, the missing local Whisper runtime, and two high npm audit findings; desktop capture/transcription and release/UAT boundaries remain distinct. (Renumbered from a parallel 0.2.7b during the Phase 4 merge; `main` had assigned 0.2.7b/0.2.8b to the Desktop release records below.) |
| 0.2.8b | Recorded the public binary-only Desktop channel, anonymous full-download equality, release workflow pass, and Landing private-repository regression. |
| 0.2.7b | Recorded the verified self-contained Desktop v0.1.0 release candidate, 30-second real-device transcript UAT, final installer hash, and remaining publication gates. |
| 0.2.5b | Recorded 195/195 full Rust regression after the Windows Python launcher fallback, expanded all-26 source-intent annotation coverage, bounded relaunch persistence evidence, and exact visual/device/real-connector blockers. |
| 0.2.6b | Recorded PR #10 merge at `cea2d93`, post-merge CI run `31609642060` passing, and the remaining real provider/device and release gates. |
| 0.2.4b | Recorded Sprint 4 connector/operator UI, eight-command and grant provenance boundary, focused frontend/backend evidence, successful dev launch, and remaining visual/restart/device/real-connector gates. |
| 0.2.3b | Recorded bounded Sprint 3 stdio execution, fixtures, isolation evidence, default-off flag, and unresolved full-suite environment failures. |
| 0.2.2b | Recorded the tested Sprint 2 policy, preview hash, minimization, keyring, disconnect/revoke, audit, and sanitizer foundation; MCP transport and UI remain explicitly unimplemented. |
| 0.2.1b | Recorded the real microphone-rail entry fix plus Sprint 1 typed external MCP and Genesis v9 foundations; runtime execution remains explicitly unimplemented. |
| 0.2.0b | Truth-synced the routed Live Meeting core and kept external MCP/CRM retrieval explicitly unimplemented. |
| 0.1.3b | Added successful release-layout GPU transcription smoke evidence. |
| 0.1.2b | Added successful clean-path GPU transcription smoke evidence. |
| 0.1.1b | Added the implemented standalone GPU runtime staging/launch path and its current verification boundary. |
| 0.1.0b | Added implementation progress truth table and next milestones. |

## Changelog

| Version | Date | Status | Summary | Commit Hash | Agent |
|---------|------|--------|---------|-------------|-------|
| 0.2.20b | 2026-09-13 | beta | Web Google login works (Redirect-URL fix on the ZURI-shared project); live `public` schema found empty and Storage bucket-less; added the same-machine loopback recordings API and the web recordings tile with a loopback-only, egress-pinned client. Rust 434/434, clippy clean, build + Node suites green. | working-tree | Claude |
| 0.2.19b | 2026-09-13 | beta | Supabase online; login gated on disabled Google provider. Recorded PR #44 (web paired devices) and PR #43 (brand kit); corrected the native-first and issue-#41 characterizations from 0.2.18b. | `f161a1d` | Claude |
| 0.2.18b | 2026-09-04 | beta | Truth-synced PR #39 audit merge, PR #40 Android restoration with first physical A07 render, mobile login rewrite (working tree), machine move + full local toolchain, issue #41, and the Supabase pause gate. | `7b37a6e` | Claude |
| 0.2.17b | 2026-08-31 | beta | Bumped GenesisBlockDB to main `79b41a3` (0.2.5) with `retention: None` on every `OpenOptions` and delta-based frontier assertions; Rust 419/419, frontend build and Node suites green. | working-tree | Claude |
| 0.2.16b | 2026-08-31 | beta | Closed the 1000-row read ceiling via GenesisBlockDB offset paging (`1ff6862`) and whole-read `query_all` across all length-driven readers; summarise/export/delegate/backup now cover recordings past 1000 rows. | `db0b779` | Claude |
| 0.2.15b | 2026-08-26 | beta | Truth-synced merged Google Drive/Native Broker local evidence, current focused tests, baseline warnings, and the Docker-bounded W1 verification gap. | `888aded` | ATHER |
| 0.2.14b | 2026-08-23 | beta | Added the approved local Google Drive native PKCE/keyring, metadata audit, separate UI, resumable appDataFolder transport, and digest-bound restore; external provider/deployment/device gates remain open. | working-tree | ATHER |
| 0.2.13b | 2026-08-23 | beta | Staged the pinned faster-whisper small model and 11-file CUDA 12/cuDNN 9 bundle; standalone GPU smoke passed, while Live Meeting/device/connector UAT remains open. | working-tree | ATHER |
| 0.2.12b | 2026-08-23 | beta | Truth-synced the merged PR #16 state and separated staged package, model, and CUDA-runtime evidence; physical, restore, connector, and release gates remain open. | working-tree | ATHER |
| 0.2.11b | 2026-08-19 | beta | PR #16 pushed and green but unmerged; routing, backup-payload and connector-timeout defects closed at source with 238/238 Rust and 46/46 Node evidence; device and restore gates unchanged. | `68f0201` | ATHER |
| 0.2.10b | 2026-08-19 | beta | Phase 4 Tasks 5–9 landed on `codex/phase-4-filesystem-test-backup`; automated backup/restore and reconciliation evidence recorded, release/U9 gates unchanged. | working-tree | ATHER |
| 0.2.9b | 2026-08-14 | beta | Current desktop/web build and 212-test Rust evidence recorded; missing Whisper runtime and two high npm audit findings prevent a live-transcription/release claim. | working-tree | ATHER |
| 0.2.8b | 2026-08-14 | beta | Published and verified the public Desktop v0.1.0 channel; production Landing cutover remains the final gate. | pending | ATHER |
| 0.2.7b | 2026-08-14 | beta | Verified the Windows CPU release candidate, real-device Live Meeting transcript, installer runtime, and production-style web CTA before publication. | pending | ATHER |
| 0.2.6b | 2026-08-12 | beta | Phase 3 follow-up merged to `main`; post-merge frontend/Rust CI passed while provider/device and release gates remain open. | `cea2d93` | ATHER |
| 0.2.5b | 2026-08-12 | beta | Closed the prior full Rust regression, expanded annotation intent, and recorded bounded restart smoke plus environment-bounded UAT blockers. | pending | ATHER |
| 0.2.4b | 2026-08-11 | beta | Added verified Sprint 4 operator workflow and remaining UAT boundaries. | pending | ATHER |
| 0.2.3b | 2026-08-11 | beta | Added verified Sprint 3 bounded stdio backend evidence and remaining UI/UAT gaps. | pending | ATHER |
| 0.2.2b | 2026-08-11 | beta | Added verified Sprint 2 external-retrieval trust-foundation evidence and remaining runtime/UI gaps. | pending | ATHER |
| 0.2.1b | 2026-08-11 | beta | Added verified Live Meeting rail entry and Sprint 1 external MCP contract/schema evidence. | pending | ATHER |
| 0.2.0b | 2026-08-11 | beta | Recorded routed Live Meeting capability, verification boundaries, entry mismatch, and external retrieval gap. | pending | ATHER |
| 0.1.3b | 2026-07-19 | beta | Recorded release-layout GPU smoke evidence. | N/A | ATHER |
| 0.1.2b | 2026-07-19 | beta | Recorded the clean-path GPU transcription smoke result. | N/A | ATHER |
| 0.1.1b | 2026-07-19 | beta | Recorded standalone GPU runtime implementation and validation evidence. | N/A | ATHER |
| 0.1.0b | 2026-07-05 | beta | Added real progress doc. | N/A | ATHER |
