# Bounded implementation progress

| Date | Work | State | Evidence |
|---|---|---|---|
| 2026-10-15 | Desktop shell viewport and scroll correction (#81) | Named shell regions; fixed skip-link/overlay stacking; one main page scroller; collapsible project context and narrow navigation. Shell 14/14 and build passed; five browser viewport sizes, capture/modal fixtures and keyboard disclosure checked. Installed native app remains unverified for this UI patch. | [Report](2026-10-15-desktop-shell-layout.md) |
| 2026-10-15 | Standalone import/transcription and shutdown durability fix (#79) | Real loopback import → CPU Whisper → completed job/transcript → edit/export/reopen passed on a 48 kHz synthetic fixture (12/12 English keywords); Rust correction/playback suites passed; native UI edit/export survived a normal `CloseMainWindow` close and second boot with explicit Genesis checkpoints. Playback output configuration is unsupported here; enabled API/in-flight and forced-crash shutdown remain unverified. | [Report](2026-10-15-standalone-readiness.md) |
| 2026-09-30 | Meeting & Task Manager local HTTP adapter | Source/fixture tests and independent review passed; real model/audio/browser/installed gates unrun | [Report](2026-09-30-meeting-task-manager-local-adapter.md) |

These entries track bounded local changes. They do not promote programme Phase
0–5, meeting provider, package or release acceptance.
