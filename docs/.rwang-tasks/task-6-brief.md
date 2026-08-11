# Task 6 Brief — Scoped Traceability Annotation Contract (RED)

## Role

You are a fresh TDD worker for FUNG's H3 traceability gate.

## Task

Create the smallest executable contract test that requires structured RWANG annotations on the touched External Retrieval source/test files, while validating every referenced requirement ID against the canonical requirements document. The test must be RED before any annotations are added.

## Scope

Implementation targets:

- `src-tauri/src/external_mcp.rs`
- `src-tauri/src/external_mcp_transport.rs`
- `src-tauri/src/external_mcp_commands.rs`
- `src/lib/externalMeetingTools.ts`
- `src/components/ExternalMeetingToolsPanel.tsx`
- `src/tauri.ts`

Test targets:

- `tests/externalMeetingTools.test.mjs`
- `tests/desktopBootstrap.test.mjs`

The contract must recognize `@req`, `@spec`, `@designs`, and `@tested` comment annotations, but it must not claim that an annotation proves runtime/UAT. Keep the required IDs limited to IDs visibly supported by the current source/test evidence (FR-101 and FR-106–FR-116 plus NFR-102, NFR-103, NFR-105, NFR-106, NFR-107, NFR-108 where the target file actually supports them).

## Constraints

- Do not add annotations or change application behavior in this RED task.
- Do not update the graph yet.
- Use Node's existing test runner and the repository's current ESM conventions.

## Output contract

1. Add `tests/traceabilityAnnotations.test.mjs`.
2. Add `test:traceability` to `package.json` only if needed for the focused command.
3. Run the focused test and record the expected failure (missing annotations) in `docs/.rwang-tasks/task-6-report.md`.
4. Return RED or BLOCKED and exact paths changed.
