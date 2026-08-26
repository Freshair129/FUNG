---
version: "0.1.0b"
created_at: "2026-08-23T15:34:22+07:00,ATHER"
last_update: "2026-08-23T15:34:22+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "implementation-plan"
  scope: "FUNG Luna-Terra multi-agent execution workflow"
---

# FUNG Luna–Terra Multi-Agent Workflow

## 1. Approval and risk

- Approved by Boss on 2026-08-23.
- Workflow risk: **MEDIUM**. Individual OAuth, IAM, credential, migration,
  device, and release tasks may be classified **HIGH** in their task briefs.
- This document authorizes workflow setup and bounded dispatch. It does not
  waive feature-specific documentation, external approval, or release gates.

## 2. Operating model

| Role | Model / actor | Responsibility | Prohibited action |
|---|---|---|---|
| Worker / fixer | `gpt-5.6-luna` | Implement one bounded task with tests, documentation, evidence, and a review package | Merge its own work, review itself, edit outside its write partition |
| Task and integration reviewer | `gpt-5.6-terra` | Read-only review of scope, correctness, AC/SC, architecture, security, tests, and evidence | Author or repair implementation code |
| Controller / final integrator | Codex / ATHER | Decompose work, issue task packets, enforce partitions, assemble only Terra-approved commits, run the final gate, and report truthfully | Author feature or fix code |
| Approval authority | Boss | Approve scope changes, external configuration, credentials, physical UAT, merge, and release | N/A |

If the controller finds a code defect, it dispatches a fresh Luna fixer. If
integration creates a semantic conflict, an Integration Luna resolves it and
Terra re-reviews the result. The controller may perform Git assembly of clean,
approved commits but does not repair implementation code directly.

## 3. Execution pipeline

```text
Boss approval
  -> Controller task packet
  -> Luna implementation
  -> Terra task review
       FAIL -> fresh Luna fix -> Terra re-review (maximum 3 cycles)
       PASS -> Controller integration
  -> Terra integration review
       FAIL -> Integration Luna -> Terra re-review
       PASS -> Controller final gate
  -> PR + CI
  -> Boss merge/release approval
```

## 4. Task packet contract

Every task packet must include:

1. Task ID, Goal, Acceptance Criteria (AC), and Success Criteria (SC).
2. Base commit SHA and dependency state.
3. Explicit writable paths and forbidden paths.
4. Required input documents and interface contracts.
5. Test and verification commands.
6. Risk classification and external approval boundaries.
7. Required report path and output schema.

Luna must return `DONE`, `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT`, or `BLOCKED`,
plus the commit SHA, changed paths, actual test output, AC/SC evidence, known
gaps, and rollback notes. Delegation is not reported as started until a real
agent ID and status exist.

## 5. Terra review contract

Terra is read-only and returns one verdict:

- `PASS`: hard gates pass and the task may be integrated.
- `WARN`: hard gates pass; non-blocking concerns are carried to final audit.
- `FAIL`: at least one hard gate fails; dispatch a fresh Luna fixer.
- `BLOCKED`: evidence or an external prerequisite is unavailable.

Hard gates are scope compliance, completeness, AC/SC correctness, internal and
cross-task consistency, test reproducibility, security/secret boundaries, and
truthful evidence. Style and non-blocking maintainability observations may be
warnings. Three unsuccessful fix cycles escalate to Boss.

## 6. Parallelism and ownership

- Run at most three Luna workers concurrently.
- Parallel workers must have disjoint write partitions.
- Contract, schema, shared DTO, and security-boundary work is serialized and
  reviewed before downstream frontend/backend lanes start.
- Shared files such as `src-tauri/src/lib.rs`, `Cargo.toml`, `Cargo.lock`, and
  `package.json` have one owner per wave.
- Workers receive bounded file/context packets; raw scratchpads are not passed
  between agents.
- Existing user changes remain untouched unless a task packet explicitly owns
  them.

## 7. Final gate

The controller may assemble a candidate only when:

1. Every included task has a Terra `PASS` or an explicitly accepted `WARN`.
2. Base revisions and dependency order are valid.
3. The integrated diff contains only approved scopes.
4. Required build, test, lint, format, secret-scan, and contract checks pass.
5. Code, tests, requirements, design, status, and provenance agree.
6. External/provider/device evidence is separated from local implementation
   evidence.
7. No unrelated dirty-worktree change is included.

Any final-gate implementation failure returns to Luna and then Terra. The
controller does not patch the failure directly.

## 8. FUNG execution waves

| Wave | Scope | Exit gate |
|---|---|---|
| W0 | Inventory and partition the current dirty worktree | Terra approves ownership, dependencies, and preservation boundaries |
| W1 | Google Drive implementation package | Local code/tests/docs pass Terra and controller gates |
| W2 | OAuth deployment, clean-install restore, Android/FUNGWIRE UAT | Real provider/device evidence or explicit external blocker |
| W3 | Desktop Live Meeting, transcription provenance/export, diarization proof | Runtime and data-integrity gates pass |
| W4 | Security, packaging, signing, and release | Release evidence passes; Boss approves merge/release |

Recording2/Smart Gift is a separate product lane. Its transcript and planning
artifacts do not share an implementation commit with FUNG core unless a later
approved task explicitly creates that dependency.

## 9. Durable handoff

- Run ledger: `docs/verification/implementation-reports/2026-08-23-fung-luna-terra-progress.md`
- Task briefs and reviews use dated files under
  `docs/verification/implementation-reports/`.
- A task marked complete in the ledger is not re-dispatched unless its base or
  acceptance contract changes.

## Version Diff

- `new -> 0.1.0b`: approved Luna worker, Terra review-gate, and Codex
  final-integrator workflow with bounded parallelism and FUNG execution waves.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | beta | Approved Luna–Terra multi-agent workflow; no implementation code changed | working-tree | ATHER |
