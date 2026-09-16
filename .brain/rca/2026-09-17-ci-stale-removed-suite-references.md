---
version: "0.1.1b"
created_at: "2026-09-17T00:00:00+07:00,Agent: Codex,Commit: working-tree"
last_update: "2026-09-17T00:00:00+07:00,Agent: Codex,Commit: working-tree"
status: "candidate"
superseded_by: null
attributes:
  domain: "agent-governance"
  doc_type: "root-cause-analysis"
  document_kind: "root-cause-analysis"
  scope: "FUNG CI workflow after Google Drive cancellation"
  language: "Thai"
  risk: "LOW"
---

# RCA — CI เรียก test scripts ที่ถูกลบแล้ว

## สถานะความจริง (Truth status)

นี่คือ RCA candidate สำหรับ CI wiring defect และ orphaned test-only helper
support ที่แก้หลัง main ยกเลิก Google Drive และลบ native-session custody suite

## Symptom / อาการ

PR #58 มี `frontend` check ล้ม แม้ build และ test suites ที่มีอยู่ผ่าน เพราะ
workflow เรียก `npm run test:google-drive` ซึ่งไม่มี script ใน `package.json`.
Workflow ยังเรียก `npm run test:native-session-custody` ใน Rust job ทั้งที่ test
file และ script ถูกลบจาก main แล้ว. หลังตัด workflow reference แล้ว Rust CI
ยังพบ `clippy -D warnings` จาก restore-intent helper ที่เหลือแต่ไม่มี
production caller; helper ถูกเก็บไว้เฉพาะ `cfg(test)` เพื่อรักษา test contract
และไม่ให้ dead code อยู่ใน production target

## Evidence / หลักฐาน

1. GitHub Actions frontend log จบที่ `npm error Missing script:
   "test:google-drive"`.
2. `package.json` ไม่มี `test:google-drive` หรือ
   `test:native-session-custody`.
3. `.github/workflows/ci.yml` มีทั้งสอง invocation อยู่ก่อนการแก้.
4. Main truth documents record Google Drive cancellation and removal of the
   active implementation; local `npm run build` and Rust `439 passed; 0
   failed; 1 ignored` passed after conflict resolution.
5. Hosted Rust clippy then reported the unused `domain` parameter and the
   restore-intent constant/fields/methods; local `cargo clippy --lib
   --all-targets -- -D warnings`, `cargo fmt --all --check`, and Rust tests
   passed after the narrow cleanup.

## Root Cause / สาเหตุราก

CI workflow and native helper inventory were not updated atomically with the
removal of the Google Drive and native-session custody suites. The workflow
kept stale command references outside the current package script registry, and
the Rust module kept test-only restore-intent support in the production target.

## Why this escaped detection / เหตุใดจึงหลุดการตรวจ

The main cancellation merge updated source, tests, and documentation but did
not remove every corresponding workflow invocation or gate test-only helper
support behind `cfg(test)`. Local verification of the PR branch occurred
before the main cancellation merge, so it could not expose the stale references
introduced by the combined history.

## Proposed prevention / การป้องกัน

1. Treat removal of a test suite and its CI invocation as one change set.
2. Keep `test:ci-coverage` as the registry check and run it before the ordered
   CI suite list.
3. After deleting a script, search `.github/workflows` and repository docs for
   the script name before merging.
4. Run clippy with `--all-targets -- -D warnings` after removing a native
   caller, and keep test-only contract helpers under `cfg(test)` when retained.

## Version Diff

- `0.1.0b -> 0.1.1b`: added the hosted clippy evidence and the narrow
  test-only-helper cleanup to the same cancellation RCA.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | candidate | Documented and repaired stale CI references to removed test suites. | working-tree | Codex |
| 0.1.1b | 2026-09-17 | candidate | Added the orphaned Rust helper/clippy RCA and recorded the narrow cfg(test) cleanup. | working-tree | Codex |
