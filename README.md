# FUNG

FUNG is a local-first audio workspace for recording, transcription, review and
evidence-based meeting knowledge. Desktop uses Tauri/React and an embedded
GenesisBlockDB adapter; optional account/device services use the separate
Supabase control plane.

- [Documentation hub](docs/README.md)
- [Desktop architecture and three-layer data ownership](docs/Desktop/ARCHITECTURE.md)
- [Current UI wireframes, wireflows and user flows](docs/design/2026-10-16-desktop-navigation-and-flows.md)
- [Desktop implementation evidence](docs/Desktop/08-real-progress.md)
- [Mobile implementation evidence](docs/Mobile/IMPLEMENTATION_STATUS.md)
- [Agent instructions](AGENTS.md)

Product scope, current implementation and runtime acceptance are tracked
separately in those documents. Browser Desktop preview cannot validate native
capture, local data access or physical-device/provider behavior.

Development scripts and dependency versions are in [package.json](package.json).
Use `npm run dev` for the Vite frontend, `npm run build` for TypeScript and the
production frontend, and `npm run desktop` for the configured Tauri development
runtime. Native builds require the project's external runtime resources; read
the platform evidence and packaging requirements before claiming a standalone build.
