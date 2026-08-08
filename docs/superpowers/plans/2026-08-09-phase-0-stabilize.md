# Phase 0: Stabilize & Automate — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Green local build, CI on every PR that works on a fresh runner without `G:/`, portable Rust dependency, clean repo — the safety net for all later agent-executed phases.

**Architecture:** No product code changes. Quarantine the one orphaned broken file, switch `genesis-block-native` from an absolute-path dependency to a pinned git dependency (repo is public), add a two-job GitHub Actions workflow (frontend on ubuntu, Rust on windows), and clean up untracked/stale root files.

**Tech Stack:** GitHub Actions, Cargo git dependencies, existing npm/cargo test suites.

**Master plan:** `docs/superpowers/plans/2026-08-09-fung-master-implementation-plan.md` Phase 0 (REQ-0-01…05)

## Global Constraints

- No product behavior changes in this phase — stabilization only
- `genesis-block-native` git rev pin: `0502c6b971525b8d256e0a8236950a3997831f99` (repo `https://github.com/Freshair129/GenesisBlock.git`, public, local checkout is in sync with origin)
- Rust CI job runs on `windows-latest` (matches dev platform; `keyring` uses `windows-native` feature; no gtk deps needed)
- Frontend CI job runs on `ubuntu-latest` with Node 22
- Do NOT touch checkboxes in `2026-08-09-fung-master-implementation-plan.md` or this file — backfill applies ONLY to the 3 named completed plan files
- Files larger than 5 MB must be gitignored, not committed
- `src/mobile/PitchingAssist.tsx` is currently **untracked** — quarantine is a filesystem move, no `git rm` needed

---

### Task 1: Quarantine PitchingAssist and restore green build

**Files:**
- Move: `src/mobile/PitchingAssist.tsx` → `attic/PitchingAssist.tsx`
- Modify: `.gitignore` (append `attic/`)

**Interfaces:**
- Consumes: nothing (file is orphaned — imported by no other file, confirmed 2026-08-09)
- Produces: green `npm run build` for Tasks 2–4 to build on

- [ ] **Step 1: Verify the file is still orphaned and untracked**

Run:
```bash
grep -rn "PitchingAssist" src/ --include="*.ts" --include="*.tsx" | grep -v "src/mobile/PitchingAssist.tsx"
git status --short src/mobile/PitchingAssist.tsx
```
Expected: first command outputs nothing (no importers); second shows `?? src/mobile/PitchingAssist.tsx` (untracked). If either differs, STOP and report BLOCKED.

- [ ] **Step 2: Move the file to attic/**

```bash
mkdir -p attic
git mv 2>/dev/null || mv src/mobile/PitchingAssist.tsx attic/PitchingAssist.tsx
```
(Use plain `mv` — the file is untracked so `git mv` is not applicable.)

- [ ] **Step 3: Gitignore attic/**

Append to `.gitignore`:
```
# Quarantined code pending disposition (see master plan REQ-R-04)
attic/
```

- [ ] **Step 4: Verify build is green**

Run: `npx tsc --noEmit`
Expected: exit 0, no output.
Run: `npm run build`
Expected: `✓ built in …` and exit 0.

- [ ] **Step 5: Commit**

```bash
git add .gitignore
git commit -m "chore: quarantine orphaned PitchingAssist.tsx to attic/, restoring green build"
```
Note: only `.gitignore` is staged — the moved file was never tracked.

---

### Task 2: Portable genesis-block-native dependency (git dep, pinned)

**Files:**
- Modify: `src-tauri/Cargo.toml:21`
- Regenerates: `src-tauri/Cargo.lock`

**Interfaces:**
- Consumes: public repo `https://github.com/Freshair129/GenesisBlock.git`, rev `0502c6b971525b8d256e0a8236950a3997831f99` (root package name `genesis-block-native`)
- Produces: `cargo test` that works on any machine with network access — required by Task 3's CI job

- [ ] **Step 1: Replace the path dependency**

In `src-tauri/Cargo.toml`, replace line 21:
```toml
genesis-block-native = { path = "G:/GenesisBlock_Dev/GenesisBlock", default-features = false, features = ["mobile"] }
```
with:
```toml
genesis-block-native = { git = "https://github.com/Freshair129/GenesisBlock.git", rev = "0502c6b971525b8d256e0a8236950a3997831f99", default-features = false, features = ["mobile"] }
```

- [ ] **Step 2: Add the documented local-dev override (commented out)**

Append at the end of `src-tauri/Cargo.toml`:
```toml
# --- Local development against a WIP GenesisBlock checkout ---
# Uncomment to build against the local working copy instead of the pinned git rev.
# Never commit this uncommented.
# [patch."https://github.com/Freshair129/GenesisBlock.git"]
# genesis-block-native = { path = "G:/GenesisBlock_Dev/GenesisBlock" }
```

- [ ] **Step 3: Rebuild lock file and run the full Rust suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: dependency fetched from git (first run downloads), all 68 tests pass, 0 failures. This is the proof the git rev is equivalent to the local path (local checkout is at the same rev with only an `index.d.ts` mod, which is not part of the Rust build).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: switch genesis-block-native to pinned git dependency for CI portability"
```

---

### Task 3: GitHub Actions CI workflow

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: green build from Task 1, portable cargo dep from Task 2
- Produces: required status checks `frontend` and `rust` used by the branch-protection step (Task 5, human)

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/ci.yml`:
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
      - run: npm ci
      - run: npm run build
      - run: npm run test:mobile
      - run: npm run test:design-system

  rust:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - run: cargo test --manifest-path src-tauri/Cargo.toml
```

- [ ] **Step 2: Validate YAML locally**

Run: `node -e "const y=require('fs').readFileSync('.github/workflows/ci.yml','utf8'); console.log('bytes:', y.length)"` and visually confirm indentation matches the block above exactly. (True validation happens when the branch is pushed — the PR for this phase is itself the CI test.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow (frontend ubuntu + rust windows)"
```

---

### Task 4: Repo hygiene + checkbox backfill

**Files:**
- Delete: `JULES_REPORT.md` (untracked stub from an unrelated project — filesystem delete only)
- Modify: `AGENTS.md` (points to non-existent `CORE/`/`WORKFLOW/` dirs)
- Modify: `.gitignore` (large binaries under `public/`)
- Add: untracked assets that belong in the repo
- Modify: `docs/superpowers/plans/2026-08-05-zoom-meeting-ingestion.md`, `docs/superpowers/plans/2026-08-08-auth-login-ui.md`, `docs/superpowers/plans/2026-08-08-byom-tts-provider.md` (checkbox backfill)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: clean `git status` on main after merge

- [ ] **Step 1: Delete the stale stub**

```bash
rm JULES_REPORT.md
```
(Untracked — no git operation needed.)

- [ ] **Step 2: Replace AGENTS.md content**

Replace the entire content of `AGENTS.md` with:
```markdown
# FUNG — Agent Entry Point

Read in this order:

1. `docs/superpowers/plans/2026-08-09-fung-master-implementation-plan.md` — roadmap, phase gates, automation protocol
2. `docs/Desktop/ARCHITECTURE.md` — system architecture
3. `docs/Mobile/IMPLEMENTATION_STATUS.md` — mobile ground truth (evidence-based)
4. `docs/Desktop/08-real-progress.md` — desktop ground truth

Rules of engagement live in §9 of the master plan.
```

- [ ] **Step 3: Triage untracked files by size**

Run:
```bash
git status --short | grep '^??'
find public assets src-tauri/icons -type f -size +5M 2>/dev/null
```
For every untracked file: if > 5 MB (e.g. an `.apk` under `public/`), append a matching pattern to `.gitignore`; otherwise stage it. Expected staging set includes: `assets/` (design SVG/PNGs), `src-tauri/icons/*` (all icon files — referenced by `tauri.conf.json` bundle config), `public/` small files, `scripts/generate_app_icon.ps1`, `src/components/FungLogo.tsx` (already tracked via earlier merge — skip if so), `docs/FUNG LOGO - Quiet Archive.png`. Report the exact list staged vs ignored in your report file.

- [ ] **Step 4: Checkbox backfill on the 3 completed plans**

In exactly these 3 files (and NO others):
- `docs/superpowers/plans/2026-08-05-zoom-meeting-ingestion.md`
- `docs/superpowers/plans/2026-08-08-auth-login-ui.md`
- `docs/superpowers/plans/2026-08-08-byom-tts-provider.md`

Replace every occurrence of `- [ ]` with `- [x]`:
```bash
node -e "
const fs = require('fs');
for (const f of [
  'docs/superpowers/plans/2026-08-05-zoom-meeting-ingestion.md',
  'docs/superpowers/plans/2026-08-08-auth-login-ui.md',
  'docs/superpowers/plans/2026-08-08-byom-tts-provider.md',
]) {
  const s = fs.readFileSync(f, 'utf8');
  fs.writeFileSync(f, s.replaceAll('- [ ]', '- [x]'));
  console.log(f, 'done');
}
"
```
Rationale: all 3 plans are verified merged (`5d6bd06`, PR #2, PR #3); unchecked boxes are drift, and drift makes plan files unusable as status signals.

- [ ] **Step 5: Verify nothing broke**

Run: `npx tsc --noEmit`
Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: repo hygiene — remove stale stub, fix AGENTS.md, commit assets, backfill completed-plan checkboxes"
```

---

### Task 5: Branch protection (HUMAN — Boss)

**Not agent-executable.** After this phase's PR merges and CI has run at least once on `main`:

- [ ] **Step 1 (Boss):** GitHub → repo `Freshair129/FUNG` → Settings → Branches → Add rule for `main`:
  - Require a pull request before merging
  - Require status checks to pass: `frontend`, `rust`
- [ ] **Step 2 (agent, after Boss confirms):** verify with `gh api repos/Freshair129/FUNG/branches/main/protection -q '.required_status_checks.contexts'` → expect `["frontend","rust"]`

---

## Self-Review

**Spec coverage:** REQ-0-01 → Task 1; REQ-0-02 → Task 3; REQ-0-03 → Task 2; REQ-0-04 → Task 4; REQ-0-05 → Task 5. All five covered.

**Placeholder scan:** none — every step has exact commands/content.

**Type consistency:** n/a (no new types).

**Order rationale:** Task 1 first (green build gates everything), Task 2 before Task 3 (CI would fail on `G:/` path), Task 4 independent, Task 5 human-gated last.
