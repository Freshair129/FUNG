# Zoom Integration Setup

This guide covers one-time setup for the Zoom cloud-recording ingestion feature
in the FUNG desktop app: OAuth app registration, account settings, the local
diarization model, and the knowledge-graph extraction model. It also lists
known limitations and the manual UAT checklist for a real Zoom account.

## 1. Create the Zoom OAuth app (once per distribution)

1. https://marketplace.zoom.us → Develop → Build App → **General App**.
2. App type: **User-managed**. Enable **PKCE**; FUNG never uses a client secret
   (`src-tauri/src/zoom_sync.rs` sends no secret in the token exchange).
3. Redirect URL: FUNG binds a loopback listener on an OS-assigned port for
   every connect attempt and sends
   `http://127.0.0.1:<port>/zoom/callback` as `redirect_uri` — the port
   changes on every run, and the path is always `/zoom/callback`. Register a
   loopback redirect URL in the Marketplace app that matches this pattern
   (host `127.0.0.1`, path `/zoom/callback`, any port). This relies on Zoom's
   marketplace accepting loopback redirects with a variable port for
   PKCE/native apps (the pattern recommended by RFC 8252 §7.3 and used by
   Google/Microsoft's own native-app OAuth guidance). **If the Marketplace UI
   rejects a bare `http://127.0.0.1` loopback entry or insists on a fixed
   port**, this is a Zoom account/app-tier restriction outside this doc's
   control — you cannot register a redirect URL that anticipates the runtime
   port, and the OAuth connect flow will not work until Zoom's app accepts a
   loopback pattern. There is no code workaround shipped in this feature.
4. Scopes (least privilege) — these cover the endpoints FUNG actually calls
   (`GET /users/me`, `GET /users/me/recordings`, `GET /meetings/{uuid}/recordings`,
   and the recording file download URLs):
   - `cloud_recording:read:list_user_recordings`
   - `cloud_recording:read:recording`
   - `user:read:user`
5. Copy the **Client ID** and set it for the desktop runtime. FUNG reads it
   from the `FUNG_ZOOM_CLIENT_ID` environment variable
   (`src-tauri/src/zoom_sync.rs`, `client_id_from_env`) — there is no in-app
   settings field for it.

    ```powershell
    [Environment]::SetEnvironmentVariable("FUNG_ZOOM_CLIENT_ID", "<client id>", "User")
    ```

   Restart the terminal (or sign out/in) so the new user environment variable
   is picked up before running `npm run desktop`.

## 2. Recommended Zoom account settings

- Settings → Recording → enable **Record a separate audio file of each
  participant** — this is what gives FUNG exact speaker attribution from the
  per-participant files (Path A). Without it, FUNG falls back to on-device
  diarization (Path B), which produces anonymous "Speaker 1", "Speaker 2", …
  labels instead of real names.

## 3. Local diarization model (Path B only)

Path B is exercised only when a meeting was **not** recorded with separate
per-participant audio files.

1. `D:\FUNG\.venv-whisper\Scripts\pip.exe install pyannote.audio`
2. Accept the model license on Hugging Face while signed in — the diarization
   worker (`scripts/diarize.py`) loads `pyannote/speaker-diarization-3.1` by
   default, which is a gated model:
   - https://huggingface.co/pyannote/speaker-diarization-3.1
   - (this pipeline also pulls `pyannote/segmentation-3.0` as a dependency;
     accept that license too if Hugging Face prompts for it)
3. Create a read token (https://huggingface.co/settings/tokens) and set
   `FUNG_HF_TOKEN`:

    ```powershell
    [Environment]::SetEnvironmentVariable("FUNG_HF_TOKEN", "<token>", "User")
    ```

   `scripts/diarize.py` also accepts a plain `HF_TOKEN` variable as a
   fallback. The token is needed only for the first model download from
   Hugging Face; once cached locally, diarization runs fully offline. If
   `pyannote.audio` is not installed or the model can't be loaded, the import
   job still completes with a transcript — see Known limitations below.

## 4. Knowledge-graph extraction model

`graph.build` (`src-tauri/src/graph_build.rs`) calls the local LLM configured
on the `ollama-summary-intent` provider row. The row is seeded automatically
on install (`src-tauri/src/lib.rs`) with `config_json = {"endpoint":
"http://127.0.0.1:11434"}`; if `config_json` has no `"model"` key the code
falls back to `llama3.1:8b`. Install and pull it via Ollama:

```powershell
ollama pull llama3.1:8b
```

Thai-heavy meetings work better with a Thai-capable model. To override,
update the `model_providers` row with id `ollama-summary-intent` and set
`config_json` to a JSON object that includes both keys you want to control,
e.g. `{"endpoint":"http://127.0.0.1:11434","model":"<model-name>"}` (omitting
either key falls back to the defaults above).

## 5. Privacy invariants

- OAuth tokens live only in the Windows Credential Manager, never in
  GenesisBlockDB, logs, or files (`src-tauri/src/zoom_sync.rs`). Service name
  `FUNG`, entry (username) `zoom-oauth`. To inspect or remove the entry
  manually: Control Panel → Credential Manager → Windows Credentials → look
  for a generic credential under `FUNG`.
- Audio, transcripts, and graph data never leave this machine — recordings
  are downloaded to local disk, transcription/diarization run locally
  (Whisper venv / pyannote), and graph extraction calls a local Ollama
  endpoint by default.

## 6. Known limitations

These are accepted, documented constraints — not bugs to chase. They come
from the underlying storage engine and the feature's design, and a user or
operator running this feature should know about them:

- **1000-row query ceiling.** The storage engine caps every relational query
  at 1000 rows. A transcript longer than 1000 segments has its knowledge
  graph extracted only from the first 1000 segments; the import job records
  this via a `job_events` entry with message *"transcript exceeds the
  1000-row query ceiling; graph extraction covers only the first 1000
  segments"*. Similarly, if a project's cumulative graph nodes/edges exceed
  1000 rows, some superseded extraction rows may remain, reported as
  *"project graph exceeds the 1000-row query ceiling; some superseded
  extraction rows may remain"*. Both are visible only in job events, not in
  the panel UI.
- **A queued import's real outcome lives in the Jobs list, not the panel.**
  Clicking Import in the Zoom panel only queues the job — the panel shows a
  "queued" label ("ส่งเข้าคิวแล้ว" in the Thai UI), not success or failure.
  Whether the import actually completed, is still running, or failed is
  visible only in the Jobs list. Re-importing an already-imported recording
  is rejected with the error "recording is already imported" at request
  time, not as a job outcome.
- **Diarization requires a one-time gated-model setup, then runs offline.**
  The first Path-B diarization run needs internet access and a Hugging Face
  token (`FUNG_HF_TOKEN`/`HF_TOKEN`) to download the gated
  `pyannote/speaker-diarization-3.1` model after you've accepted its license
  (and `pyannote/segmentation-3.0`'s license). After that first download, the
  model is cached and diarization runs fully offline. If `pyannote.audio`
  isn't installed, or the model can't be loaded (no license acceptance, no
  token, offline on first run), the transcript still gets persisted — the
  job records a `job_events` entry noting diarization was unavailable
  (message prefix `"diarization unavailable: …"`) rather than failing the
  whole import.
- **Cleanup deletes commit ahead of the insert batch during re-import.** The
  storage engine can't delete an unbounded row set in one transaction, so a
  crash mid-re-import can leave a recording with no transcript segments while
  its `recordings` row may still read "completed" from an earlier run. A
  later successful re-run self-heals.
- **A narrow retained-turn edge case in cleanup.** If a single recording ever
  accumulates 1000+ *retained* (non-proposed) speaker turns and an unordered
  page happens to return only retained rows, the cleanup sweep can end early
  and leave stale "proposed" turns behind. This needs realistic meeting
  lengths far beyond normal use to reach — the visible consequence, if it
  ever happens, is stale rows rather than data corruption.

## 7. Full validation

Run from `D:\FUNG`:

```powershell
cargo test --manifest-path D:\FUNG\src-tauri\Cargo.toml
npm run build
npm run test:mobile
```

Last run (2026-08-06) on this branch: all three passed —
`cargo test`: 42 passed, 0 failed; `npm run build`: succeeded (tsc + vite
build); `npm run test:mobile`: 4 passed, 0 failed.

## 8. Manual UAT checklist (requires a real Zoom account; not executable in
   this environment — no Zoom account or GPU runtime here)

1. Set `FUNG_ZOOM_CLIENT_ID`, launch `npm run desktop`, open the Zoom panel →
   Connect → browser consent → panel shows connected + account email.
2. Meeting A, recorded with separate audio files ON: import → job runs →
   transcript shows real participant names; speakers are renameable; the
   graph has meeting/speaker/topic nodes.
3. Meeting B, recorded with separate audio files OFF: import → Path B →
   Speaker 1/2/3 labels; if pyannote isn't installed, the transcript still
   appears and the job event notes diarization was unavailable.
4. Re-import Meeting A → rejected with "recording is already imported".
5. Disconnect → token gone from Credential Manager (verify in the Windows
   Credential Manager UI, service `FUNG`); reconnect works.
