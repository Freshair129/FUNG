---
version: "0.1.0b"
created_at: "2026-08-26T00:00:00+07:00,Luna 5.6,base 034038968703423d06e56836f835818a18c3e1a0"
last_update: "2026-08-26T00:00:00+07:00,Luna 5.6"
status: "candidate"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "technical-design"
  scope: "FUNG Phase 4 W2 external evidence execution; E0-E6; no production promotion"
  language: "Thai"
  risk: "HIGH"
  complexity: "C-3"
  authorization: "Candidate only; Boss exact-hash approval and Terra review required"
  base_commit: "034038968703423d06e56836f835818a18c3e1a0"
  candidate_commit: "externally bound after focused commit"
  candidate_sha256: "externally bound after final bytes"
---

# Phase 4 W2 External Evidence Execution Amendment — D-GDA7

## สถานะและขอบเขต

เอกสาร candidate นี้ต่อจาก D-GDA6 ซึ่ง Terra รับรองเฉพาะ local/static. มีไว้กำหนด
การเก็บหลักฐานภายนอก Phase 4 ผ่าน E0-E6 เท่านั้น ไม่ใช่คำสั่งให้ทำ provider,
credential, keyring, VM, device, deploy, release หรือ production action.

ระดับงาน C-3 และความเสี่ยง HIGH เพราะเกี่ยวกับ OAuth, token custody, Supabase/RLS,
clean-install restore, device identity และ delegation. การ approve ไม่ได้ปิด Phase 4,
U9, release หรือ production readiness.

## [ASSUMPTIONS]

1. Base คือ 034038968703423d06e56836f835818a18c3e1a0; dirty worktree อื่นไม่อยู่ใน candidate.
2. Preflight เป็น presence/readiness facts เท่านั้น; ไม่อ่านหรือเดา secret, client ID, project ref, account หรือ device identity.
3. “staging project: all project” หมายถึงทุก resource ใน staging scope ที่ Boss ระบุใน task packet ไม่ใช่ production blanket authorization.
4. Google Drive OAuth เริ่มจาก FUNG UI และ system browser ตาม native PKCE; Google Cloud client configuration และ Edge deployment เป็น Boss/controller actions.
5. D:\FUNG-Phase4-TestStorage และ D:\FUNG-Phase4-TestRestore เป็น test boundaries จนกว่าจะมี execution trace จริง.
6. Evidence ทุกชิ้นต้องมี task ID, source commit, operator, timestamp, target boundary และ redacted result.

## 1. Observed gap และ root-cause boundary

D-GDA6 Terra final report เป็น PASS locally แต่ยังเปิด clean Windows VM/real OS keyring,
Supabase/Edge/RLS, Google OAuth/Drive provider, device/UAT, signing, release, deployment,
monitoring และ production approval.

Controller preflight พบว่า presence ของ VITE_GOOGLE_DRIVE_CLIENT_ID, SUPABASE_ACCESS_TOKEN,
SUPABASE_PROJECT_REF, VITE_SUPABASE_URL และ VITE_SUPABASE_ANON_KEY เป็น false; supabase,
adb และ scrcpy ไม่พร้อม; wsl, docker และ Get-VM cmdlet มีอยู่แต่ไม่ใช่ clean-VM proof.
TestStorage มี FUNG-DEV-TEST/README และ TestRestore มี README เท่านั้น.

Observed root-cause boundary: D-GDA6 พิสูจน์ source/deterministic adapters ใน checkout เดียว
แต่ W2 ต้องพิสูจน์ OS, staging authority, Google provider และ physical device คนละ trust
boundary. ข้อความนี้ไม่ใช่ RCA ใหม่และไม่กล่าวอ้างว่า environment/provider เสีย; หากพบ defect
ให้สร้าง RCA แยกตาม AGENTS.md ก่อนแก้ไข.

## 2. Authority และ PIC

| Role | PIC/authority | Boundary |
|---|---|---|
| Approval authority | Boss | approve IDs, exact hash, external config, physical UAT, exit; ไม่อนุมัติ production โดยปริยาย |
| Bootstrap operator E0-E1 | Boss | disposable operator/VM และ real OS keyring; ห้ามใช้ production keyring/archive |
| Edge deployer E2 | Boss | deploy/verify Edge ใน staging เท่านั้น |
| Staging project | Boss; all project = staging scope | ไม่ใช่ production grant |
| RLS/grant verifier | Boss + Terra review gate | ตรวจ owner/RLS/grant/replay; Terra ไม่ deploy/แก้ policy |
| OAuth/provider and device operator | Boss | Google test account และ physical Android ตาม target sheet |
| Documentation worker | Luna 5.6 | เขียนไฟล์นี้เท่านั้น; ไม่ทำ external action/แก้ code |
| Review gate | Terra 5.6 | read-only PASS/WARN/FAIL/BLOCKED |
| Final gate | Codex/ATHER | ตรวจ bytes, paths, evidence; ไม่ patch code/override authority |

## 3. Decisions D-GDA7-01 ถึง D-GDA7-08

ทุก decision approve แยกได้ แต่ candidate ต้องผูกด้วย commit และ SHA-256 เดียวกัน.

| ID | Decision และข้อบังคับ |
|---|---|
| D-GDA7-01 | E0 แยก evidence root จาก source/worktree; บันทึก tool/commit/operator/time/target แบบ redacted; E0 ต้อง PASS ก่อน lane อื่น |
| D-GDA7-02 | E1 ใช้ clean Windows VM หรือ equivalent ที่มี real OS keyring; พิสูจน์ session/Drive lifecycle, restart recovery, revoke และ cleanup/readback ด้วย test material |
| D-GDA7-03 | E2 ใช้ staging เท่านั้น; ตรวจ Supabase/Edge deployment, auth.uid ownership, RLS/grants, metadata/audit redaction, deny-before-provider และ one-use replay |
| D-GDA7-04 | E3 เริ่ม UI → system browser → exact redirect/PKCE; scope ต้อง drive.appdata เท่านั้น; พิสูจน์ consent/callback/upload/download/digest/revoke และ negative paths |
| D-GDA7-05 | E4 clean install ไม่มี restored token; reconnect ผ่าน UI แล้ว restore encrypted archive ลง clean target; ตรวจ notes/graph/managed artifacts/manifest; fixture-only ไม่ปิด U9 |
| D-GDA7-06 | E5 physical Android/Dashboard/FUNGWIRE ตรวจ identity/ownership/capability/progress/revoke; Drive token ห้ามอยู่ mobile/payload/storage |
| D-GDA7-07 | E6 รวม evidence index/hashes/open gates และ truth-sync จาก observed evidence หลัง Terra review; ห้าม promote |
| D-GDA7-08 | Terra review bytes เดียวกัน + Boss exact-hash approval; maximum 3 fix/evidence cycles; เปลี่ยน byte/ID/scope/target/authority ต้อง review/hash ใหม่ |

## 4. Sequence และ dependencies

~~~text
E0 evidence workspace / secret-safe preflight
  -> E1 clean Windows VM + real OS keyring
  -> E2 staging Supabase/Edge/RLS/grant/replay
  -> E3 Google installed-app OAuth + Drive lifecycle
  -> E4 clean-install reconnect + filesystem/Drive restore/U9
  -> E5 physical Android/Dashboard/FUNGWIRE identity/delegation/revoke
  -> E6 closure/truth-sync only
~~~

E0 เป็น prerequisite ของทุก lane; E1 ก่อน E3/E4; E2 ก่อน E3; E3 ก่อน E4; E3 และ E4
ก่อน E5. E6 รับ PASS หรือ explicit OPEN/BLOCKED ได้ แต่ห้ามเปลี่ยน BLOCKED เป็น PASS ด้วย inference.

## 5. Runbook placeholders

ค่าที่อยู่ใน <...> ต้องมาจาก approved environment และ secret ต้องส่ง out-of-band.
ห้ามบันทึก token, OAuth code, client secret หรือ dump .env/keyring.

~~~text
E0: EVIDENCE_ROOT=<approved-empty-root>; record commit/tools/target; Get-Command supabase, adb, scrcpy, docker, wsl, Get-VM
E1: VM_NAME=<approved-clean-vm>; run native lifecycle + startup_recover; inspect redacted keyring absence/readback; dispose VM
E2: STAGING_PROJECT_REF=<Boss-approved-ref>; link/deploy/inspect staging RLS/grants/replay only
E3: launch FUNG UI; connect Google Drive; complete system-browser consent; upload/download digest; disconnect/revoke; negative cases
E4: reset clean baseline; reconnect via UI; restore encrypted archive to D:\FUNG-Phase4-TestRestore\restore-<archive-id>; verify manifest/notes/graph/blobs
E5: DEVICE_ID=<redacted>; pair physical Android; verify Dashboard/FUNGWIRE delegation; inspect token absence; revoke and retry denial
E6: index artifacts with SHA-256; secret-redaction scan; map AC/SC/exit to PASS/OPEN/BLOCKED; Terra reviews before ledger update
~~~

คำสั่ง E2/E3/E4/E5 เป็น controller/Boss actions. ไม่มี supabase CLI/staging authority/clean VM/physical device
ให้สถานะ BLOCKED ไม่ใช่ PASS. Google Cloud client ID, redirect, consent screen และ Edge deploy ไม่ได้มาจาก UI อัตโนมัติ.

## 6. Artifacts และ contamination controls

ทุก run ต้องมี envelope: task_id, lane, source_commit, environment_class, redacted target_ref, operator, ICT timestamps,
redacted command refs, result PASS/WARN/FAIL/BLOCKED, artifact SHA-256 และ cleanup disposition.

ใช้ disposable VM/profile/account/device และแยก evidence root จาก D:\FUNG. ห้ามนำ .env, authorization/access/refresh token,
keyring dump, recovery phrase, plaintext archive, private key, personal device content หรือ production identifier เข้า repo,
evidence หรือ chat. Redact email, subject, personal path, QR/code, URL parameters และ serial จากภาพ/วิดีโอ.

Archive E3/E4 ต้อง encrypted และ digest-bound; provider scope มากกว่า drive.appdata ให้ STOP/FAIL; staging evidence ห้ามเป็น production evidence.
ก่อน lane ใหม่ต้องไม่มี pending browser session, token, keyring material หรือ mounted path ข้าม boundary.

## 7. Acceptance Criteria (AC)

| ID | Criterion | Evidence |
|---|---|---|
| AC-GDA7-01 | E0 root แยก source และ preflight ไม่มี secret | manifest + redaction scan |
| AC-GDA7-02 | E1 clean VM พิสูจน์ OS keyring lifecycle/restart/revoke/cleanup | VM envelope + trace |
| AC-GDA7-03 | E2 ตรวจ owner/foreign-owner/RLS/grant/Edge/audit/replay | staging report + Terra verification |
| AC-GDA7-04 | E3 UI/system-browser OAuth ขอ exact drive.appdata และ lifecycle ผ่าน | redacted UI/audit/provider metadata |
| AC-GDA7-05 | E3 encrypted upload/download digest และ negatives ผ่าน | provider/archive digest + negative trace |
| AC-GDA7-06 | E4 clean-install reconnect และ restore ลง clean target โดย manifest ตรง | recording + restore manifest |
| AC-GDA7-07 | E5 physical Android/Dashboard/FUNGWIRE delegation และ post-revoke denial ผ่าน | device trace + token-absence inspection |
| AC-GDA7-08 | E6 truth-sync แยก evidence class และคง open gates | evidence index + Terra closure review |

D-GDA6 local tests เป็น implementation evidence เท่านั้น ไม่แทน AC-GDA7-02, 03, 04, 06 หรือ 07.

## 8. Success Criteria (SC) และ exit

- SC-01: ทุก PASS มี envelope, hash, operator, time, target และ cleanup.
- SC-02: ไม่มี secret ใน diff, evidence root, report, screenshot, log หรือ chat.
- SC-03: local/static แยกจาก clean-VM/staging/provider/device/release/production.
- SC-04: OAuth, keyring, RLS/grant, Drive, restore และ revoke ตรง approved boundaries; ไม่มี second authority.
- SC-05: E6 ระบุ PASS/OPEN/BLOCKED/FAIL พร้อม artifact.

Exit ต้องมี Terra PASS หรือ accepted WARN, Boss exact-hash approval, E0 PASS, E1-E5 PASS หรือ explicit Boss-acknowledged
OPEN/BLOCKED, E4/E5 artifacts หากจะพิจารณา U9/device gate, E6 Terra-reviewed index และไม่มี unreviewed code/config/deploy/release change.
External prerequisite ขาดให้ BLOCKED; document exit ไม่ใช่ production readiness.

## 9. Stop และ rollback

STOP/FAIL เมื่อ target/authority ไม่ชัด, พบ secret, scope/redirect/owner/state ไม่ตรง, RLS/grant/replay พิสูจน์ไม่ได้,
keyring cleanup/readback ambiguous, stale provider result ถูก publish, clean target ไม่ว่าง, digest ไม่ตรง, restore in-place,
Genesis authority ไม่ชัด, revoked device ยังทำงานได้, token อยู่ใน mobile/FUNGWIRE หรือจำเป็นต้องแก้ code/config/migration โดยไม่มี amendment.

- E0 ลบเฉพาะ redacted evidence ที่ไม่สมบูรณ์; ห้ามลบ source.
- E1 dispose เฉพาะ disposable VM/test keyring ผ่าน approved procedure.
- E2 rollback เฉพาะ staging เมื่อ Boss อนุมัติ.
- E3 disconnect/revoke ผ่าน UI/provider contract และตรวจ redacted metadata/keyring absence.
- E4 ลบเฉพาะ restore-<archive-id> หลัง hash/cleanup; ห้ามลบ source/user data.
- E5 revoke test device/capability; ห้าม factory-reset/personal-data deletion.
- E6 append correction record พร้อม reason/hash/reviewer; ห้าม rewrite history.

## 10. Evidence matrix และ boundary

| Class | พิสูจน์ | ก่อน GDA7 | Release/production? |
|---|---|---|---|
| local/static | source graph, deterministic tests, build, diff | PASS เฉพาะ local/static | ไม่ได้ |
| clean-VM | Windows install/keyring/restart/cleanup | OPEN | ไม่ได้ลำพัง |
| staging | Supabase/Edge/RLS/grant/replay | OPEN | ไม่ได้ |
| provider | real consent/upload/download/revoke | OPEN | ไม่ได้ลำพัง |
| device | physical Android/Dashboard/FUNGWIRE/revoke | OPEN | ไม่ได้ลำพัง |
| release | signing/artifact/install | OUT OF SCOPE | ไม่ได้จาก GDA7 |
| production | deploy/monitoring/approval | OUT OF SCOPE | ไม่ได้จาก GDA7 |

## 11. Verification และ exact approval

Luna ตรวจ input cross-reference, decision/lane/dependency completeness, secret absence, internal consistency, intended-path diff
และ git diff --check เท่านั้น; ไม่ทำ external action และไม่อัปเดต master/status ledger.

Proposed approval command:

~~~text
approve D-GDA7-01 through D-GDA7-08 — commit <candidate-commit> — SHA-256 <64 uppercase hex characters>
~~~

เปลี่ยน byte, whitespace, line ending, metadata, ID, scope, target หรือ authority แล้ว review/hash เดิมเป็นโมฆะ.
Terra review หรือ hash ไม่ใช่ implementation/production authorization.

## Version Diff

- new -> 0.1.0b: เพิ่ม candidate สำหรับ Phase 4 W2 external evidence หลัง D-GDA6 local/static PASS.
- เพิ่ม D-GDA7-01 ถึง D-GDA7-08, E0-E6, roles, dependencies, placeholders, AC/SC/exit, stop/rollback และ evidence matrix.
- ยืนยัน UI/system-browser OAuth และ controller-owned Google Cloud/Edge actions; ไม่แก้ code/config/ledger และไม่เปิด production.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-26 | candidate | Drafted W2 external evidence amendment; external and production actions remain unauthorized. | externally bound after focused commit | Luna 5.6 |
