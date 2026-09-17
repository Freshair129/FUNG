---
version: "0.1.0b"
created_at: "2026-09-17T16:28:07+07:00,Codex"
last_update: "2026-09-17T16:28:07+07:00,Codex"
status: "under review"
superseded_by: null
attributes:
  domain: "desktop-playback-custody"
  doc_type: "verification-report"
  scope: "Approved Windows test-only playback diagnostics"
  complexity: "C-2"
  risk: "HIGH: custody investigation; no runtime behavior change"
---

# Playback diagnostic verification

## Scope and frozen source boundary

The approved diagnostic-only scope was implemented without a fixture
canonicalization fix, comparator change, assertion change, timeout change,
production helper change, workflow change, dependency change, or hosted run.
All source additions and calls are inside the existing
`#[cfg(test)] mod tests` beginning at line 2057 in
`src-tauri/src/desktop_playback.rs`.

Source SHA-256:

- preimage: `b1de7c78b94c5711af57c7f9f9e7c0ad54c088dce1f50fb1abdcb3067a583fae`
- post-diagnostic source: `22caf03015b390bca32dc25076256f3c17edeacc45a5a8b9a8091d3007e47990`

The UTF-8 prefix before the exact `#[cfg(test)]\nmod tests {` boundary at byte
offset `65854` remains unchanged:
`e2c09351b7a9d0c1718eaa0cb5ab6c1bdf99f6afb027b35941db6b267b55621d`.

## Bounded verification

Commands and results:

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
exit 0
```

Formatting passed. Cargo emitted the existing tooling warning
`could not canonicalize path C:\Users\pc`.

```text
cargo test --offline --manifest-path src-tauri/Cargo.toml desktop_playback::tests --lib -- --nocapture
exit 0
```

Final focused result: `20 passed; 0 failed; 0 ignored; 452 filtered out`.
This compile/test exercised the Windows-gated diagnostic code path at compile
time, but no diagnostic line was emitted because all affected expectations
passed.

```text
git diff --check
exit 0
```

The first focused compile reported one warning introduced by this patch:
`desktop_playback.rs:2140`, `reject_status` had an initial assignment that was
always overwritten. That assignment was removed and the focused command was
rerun successfully.

The final focused compile retained only baseline warnings outside this lease:

- `src/auth_session.rs:3384`: unused `domain` parameter.
- `src/backup.rs:110`: dead-code `acquire_job` method.

No new warning remains in `desktop_playback.rs`.

## Evidence boundary and remaining gates

- Local diagnostic path: `NOT_EXERCISED` — the four affected fixtures passed,
  so failure-only output did not run.
- Hosted diagnostic collection: `NOT_RUN` — no commit, push, or hosted CI was
  performed by this lease.
- Root cause: `UNKNOWN`; no behavior fix or acceptance waiver is claimed.
- Source write lease: frozen and released after the post-diagnostic source
  hash above. Further work is report/review/publication only.

## Version diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | under review | Recorded bounded test-only diagnostic implementation and local evidence; hosted collection remains not run. | UNCOMMITTED; base `76b14c5e4a3bd8ba2e8602de908abf2351f35f97` | Codex |
