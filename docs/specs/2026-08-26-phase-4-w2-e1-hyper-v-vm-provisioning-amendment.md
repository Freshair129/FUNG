---
version: "0.2.0b"
created_at: "2026-08-26T00:00:00+07:00,Luna 5.6,cycle-2 base 1ce72c51cfc5849381d3506ddbc4f94f096f62c8"
last_update: "2026-08-26T00:00:00+07:00,Luna 5.6,cycle-2 correction"
status: "candidate"
superseded_by: null
attributes:
  domain: "cloud-backup-and-account"
  doc_type: "technical-design"
  scope: "Phase 4 W2 E1 disposable Hyper-V clean Windows provisioning only"
  language: "Thai"
  risk: "HIGH"
  complexity: "C-3"
  authorization: "Draft only; Boss exact-hash approval, Terra review, and separate lifecycle approval required"
  base_commit: "1ce72c51cfc5849381d3506ddbc4f94f096f62c8"
  candidate_commit: "externally bound after focused commit"
  candidate_sha256: "externally bound after final bytes"
---

# Phase 4 W2 E1 Hyper-V VM Provisioning Amendment — D-GDA8

## สถานะและขอบเขต

เอกสารนี้เป็น candidate สำหรับกำหนดขอบเขตการเตรียม disposable clean Windows VM
เพื่อเก็บหลักฐาน E1 เรื่อง real OS-keyring lifecycle เท่านั้น ระดับงาน HIGH/C-3
การอนุมัติเอกสารนี้ไม่ใช่คำสั่งให้สร้าง VM, ดาวน์โหลด ISO, เปลี่ยนสมาชิกกลุ่ม,
ยกระดับสิทธิ์, เปลี่ยน host/network, ติดตั้ง guest, เข้าถึง keyring, รัน lifecycle,
แก้ code/config, ลบข้อมูล, push/PR/merge/release/deploy หรือทำ production action.

Provisioning PASS หมายถึงเฉพาะ VM boundary และ baseline ที่ตรวจสอบได้เท่านั้น
ไม่ใช่ E1 lifecycle PASS และไม่ใช่หลักฐาน production readiness.

## [ASSUMPTIONS]

1. Base source คือ `f9a139b7907dfdd4cab9bbb36ab1e0bee21c92c5` และ D-GDA7 candidate
   ยังคงผูกกับ commit `4915432e8629f94f59a48507a67688773b700133` และ SHA-256
   `3E6B88D1266CC2B23E88B90CCEE968EB64F61F0C485C22C030B6EDFE4ED98E8F`.
2. Dirty/untracked worktree เดิมอยู่นอก candidate และห้ามถูกแตะต้องหรือรวมใน
   provisioning evidence.
3. `Boss` เป็นผู้เลือก elevated provisioning session, media, test account,
   artifact/harness และอนุมัติ target identity; agent ไม่เดา credential, license,
   account, provider, VM หรือ path เพิ่มเติม.
4. คำว่า clean VM หมายถึง disposable guest ใหม่ที่มี baseline ที่บันทึกก่อนมี
   keyring/test material ไม่ใช่ current host และไม่ใช่การลบ user data เพื่อทำให้
   host ดูสะอาด.
5. สถานะและค่าทรัพยากรจาก controller preflight เป็น observed facts ณ drafting
   boundary อาจเปลี่ยนได้ ต้องตรวจซ้ำใน provisioning envelope.
6. E1 lifecycle ต้องมี evidence harness/test bundle ที่ hash-pinned และได้รับ
   approval แยก หรือใช้ real provider path ที่ได้รับอนุมัติ; เอกสารนี้ไม่สร้าง
   synthetic-material authority และไม่อนุมัติการรัน lifecycle.

## 1. หลักฐานตั้งต้นและ root-cause boundary

### 1.1 Observed controller facts

| หมวด | ข้อเท็จจริงที่สังเกตได้ | ความหมายของเอกสารนี้ |
|---|---|---|
| Host | Windows 10 Pro build `19045`; `HypervisorPresent=true` | มี hypervisor อยู่ แต่ยังไม่ใช่หลักฐานว่า VM target พร้อม |
| Hyper-V surface | `Get-VM`/`New-VM` มีอยู่; services `vmms`, `vmcompute`, `hns` กำลังทำงาน | เป็น command/service presence เท่านั้น |
| Authority | process `Elevated=false`; identity ไม่อยู่ใน Hyper-V Administrators; `Get-VM` ล้มเหลวด้วย `VirtualizationException` | ปัจจุบันยังไม่มี operator access ที่เลือกและอนุมัติแล้ว |
| Host resources | RAM รวม 32 GB, free ประมาณ 7.1 GB, 12 logical processors, D: free ประมาณ 677.7 GB | ต่ำกว่า conservative free-RAM threshold 12 GB; สถานะปัจจุบัน `NOT READY` |
| CPU feature report | virtualization booleans รายงาน false ขณะที่ hypervisor ทำงานอยู่ | non-authoritative ภายใต้ active hypervisor; ไม่ใช่หลักฐานว่า hardware ไม่มี virtualization |
| Approved targets | one VM identity `FUNG-W2-E1-KEYRING-C1`, two filesystem roots ตาม §2, and one exact VHDX file ตาม §2 ยังไม่มี; source worktree/artifact references เป็น provenance inputs ไม่ใช่ target roots | ยังไม่มี exact target ที่ approved/provisioned |
| Media/harness | ยังไม่มี approved VM/ISO/media/harness path | provisioning และ lifecycle ยังเริ่มไม่ได้ |

### 1.2 Evidence-backed boundary

E1 ถูก block จาก operator access ที่ยังไม่ถูกเลือก/อนุมัติ, OS media และ license
provenance ที่ยังไม่มี, exact target ที่ยังไม่ได้ยืนยัน, resource readiness ที่
ไม่ผ่าน free-RAM threshold และ evidence harness ที่ยังไม่ได้อนุมัติ ไม่ใช่จาก
หลักฐานว่า Hyper-V ไม่มีอยู่ และไม่ใช่จากหลักฐานว่า source lifecycle code ล้มเหลว.

ข้อความนี้เป็น boundary จาก observed facts และไม่ใช่ RCA claim ใหม่. Source contract
ที่ตรวจแล้วแสดง `NativeRegisteredBroker` ใช้ `NativeKeyring`, `NativeClock`,
`NativeListener`, `NativeProvider`; `startup_recover()` ถูกเรียกจาก application
setup และ registered broker façade เป็น non-test route. สิ่งนี้ยืนยัน call boundary
สำหรับหลักฐานภายหลัง แต่ยังไม่พิสูจน์ real OS keyring lifecycle บน clean VM.

## 2. Immutable identity และ target boundary

หาก Boss อนุมัติการ provisioning ในภายหลัง ต้องใช้ **one VM identity, two filesystem
roots, and one exact VHDX file** ต่อไปนี้แบบ exact match เท่านั้น และต้อง fail-closed
เมื่อมี collision, path mismatch, alias, wildcard, junction, mount หรือ target ที่
resolve ออกนอก root. Source worktree และ artifact references เป็น provenance inputs
เท่านั้น ไม่ใช่ target roots:

| รายการ | ค่าเสนอแบบ immutable | สถานะตอนร่าง |
|---|---|---|
| VM identity | `FUNG-W2-E1-KEYRING-C1` | ยังไม่สร้าง |
| VM/config filesystem root | `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1` | ยังไม่สร้าง |
| Exact VHDX file | `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1\FUNG-W2-E1-KEYRING-C1.vhdx` | ยังไม่สร้าง |
| Evidence filesystem root | `D:\FUNG-W2-Evidence\D-GDA7\E1\cycle-1` | ยังไม่สร้าง |
| Source/artifact provenance inputs | clean worktree และ artifact references ที่ผูกกับ exact approved source commit | ต้องเลือกและ hash ก่อน staging; ไม่ใช่ target roots |
| Lifecycle boundary | หยุดก่อน E1 lifecycle | บังคับโดย amendment นี้ |

ห้ามใช้ broad root เช่น `D:\`, workspace root, glob, recursive discovery หรือชื่อ VM
ที่คล้ายกันแทน exact target. การมี parent directory อยู่ไม่เท่ากับการอนุมัติให้
reuse หรือสร้าง child target. Collision preflight ต้องตรวจครบทั้ง VM identity/name,
VM/config filesystem root, evidence filesystem root, exact VHDX file, parent
existence/ownership, canonical/resolved path, reparse point/junction, mount/path
escape และ existing Hyper-V registration ก่อนทุก write; ความไม่แน่นอนใด ๆ ให้
fail-closed. ห้าม wildcard หรือ broad target ใด ๆ.

## 3. Decisions D-GDA8-01 ถึง D-GDA8-08

Decision ทั้งแปดต้องผูกกับ candidate commit และ candidate file SHA-256 เดียวกัน
การเปลี่ยน byte, whitespace, line ending, ID, authority, path, resource envelope,
media rule หรือ lifecycle boundary ทำให้ review/hash/approval เดิมเป็นโมฆะ.

| ID | Decision และข้อบังคับ | สถานะ |
|---|---|---|
| **D-GDA8-01** | **Authority/access:** Boss เป็น operator และ approval authority. Preferred non-persistent path คือ Boss-provided elevated provisioning session ที่ Boss ควบคุมเอง. ห้าม agent เพิ่มสมาชิก Hyper-V Administrators, เปลี่ยน IAM/group, ปรับ policy หรือยกระดับสิทธิ์อัตโนมัติ. ทุก group membership/IAM mutation ต้องมี explicit approval แยก. | candidate |
| **D-GDA8-02** | **Exact identity/path collision:** ใช้ one VM identity, two filesystem roots, and one exact VHDX file ตาม §2 แบบ exact เท่านั้น: VM `FUNG-W2-E1-KEYRING-C1`, VM/config root `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1`, exact VHDX `D:\FUNG-W2-VM\D-GDA7\E1\cycle-1\FUNG-W2-E1-KEYRING-C1.vhdx` และ evidence root `D:\FUNG-W2-Evidence\D-GDA7\E1\cycle-1`. ตรวจ VM name, both roots, exact VHDX, parent existence/ownership, canonical/resolved path, reparse/junction, mount/path escape และ existing Hyper-V registration; parent existence ไม่ authorize reuse. ความไม่แน่นอน, wildcard, glob, broad target, alias หรือ provenance ไม่ชัด = fail-closed. | candidate |
| **D-GDA8-03** | **Resource envelope:** Gen2 VM, 4 vCPU, static RAM 6 GB, dynamic VHDX maximum 80 GB. ก่อน start ต้องมี free RAM อย่างน้อย 12 GB และ free disk อย่างน้อย 120 GB บน volume ที่ใช้. Current free RAM ประมาณ 7.1 GB จึงเป็น `NOT READY` และห้าม provision/start จาก fact นี้. | candidate |
| **D-GDA8-04** | **OS media:** Boss ต้อง supply currently supported Windows x64 ISO/path, SHA-256 และ license provenance out-of-band. Agent ห้าม download ISO, เก็บ activation key หรือใส่ ISO content ใน repo/chat. ใช้ Gen2 Secure Boot; เปิด vTPM เฉพาะเมื่อ selected OS ต้องใช้และต้องบันทึกเหตุผล/setting แบบ redacted. media/hash/license ขาด = `BLOCKED`. | candidate |
| **D-GDA8-05** | **Isolation:** default คือไม่มี vNIC และไม่มี external network. Artifact transfer ทำได้เฉพาะ read-only hash-pinned media หรือกลไก no-network อื่นที่อนุมัติแยก. ห้าม host share, clipboard, provider credential, personal account และ production material. Guest account/material ต้องเป็น synthetic และ disposable เท่านั้น. | candidate |
| **D-GDA8-06** | **Checkpoint/secret contamination:** ปิด automatic checkpoints และห้ามสร้าง Hyper-V checkpoint/snapshot ทุกชนิดตลอด provisioning รวม baseline/manual/production checkpoint. ห้ามใช้ export, clone, memory dump หรือ save-state เป็นสิ่งทดแทน checkpoint. Baseline ต้องเป็น redacted immutable settings/install/provenance manifest พร้อม hashes ที่ capture ขณะ VM clean และ powered off ก่อน guest account, password, authentication state, keyring/test material หรือ secret-bearing state; baseline ไม่ใช่ VM checkpoint. Synthetic guest credentials อาจมีอยู่นอก evidence ได้ แต่ห้ามเข้า evidence และห้ามถูก snapshot/checkpointed, cloned, exported หรือ dump; test values ห้ามเข้า evidence. หลัง review ให้ power off VM; การลบ VM/VHD/evidence เป็น separate exact approval. Future checkpoint ต้องมี amendment ใหม่ที่ผูก exact hash และ address guest credential persistence. | candidate |
| **D-GDA8-07** | **Artifact/harness gate:** staging ต้องมาจาก clean worktree ที่ exact approved source commit และบันทึก hash ของ artifact/dependencies. ทุก artifact ต้องระบุชัดว่า non-production. Provisioning ต้องหยุดก่อน lifecycle execution. ก่อน E1 lifecycle ต้องมี approval แยกสำหรับ real `NativeKeyring`/registered-broker evidence harness ที่ hash-pinned หรือ later provider lane; fake keyring และ test-only façade ห้ามถูกอ้างเป็น production proof. | candidate |
| **D-GDA8-08** | **E1 evidence matrix and stop/rollback/retention:** lifecycle ภายหลังต้องครอบคลุม baseline absence, synthetic write/rotate, app restart, guest reboot, `startup_recover`, logout/revoke/shutdown cleanup, stale-generation denial/readback/absence พร้อม Terra review. Provisioning PASS ไม่ใช่ lifecycle PASS. เมื่อเกิด ambiguity หรือพบ checkpoint/snapshot/export/clone/memory dump/save-state ให้ stop, power off, retain immutable evidence และเปิด cleanup ด้วย approval แยก. | candidate |

## 4. PIC และ authority matrix

| บทบาท | PIC/authority | สิทธิ์และขอบเขต |
|---|---|---|
| Approval authority | Boss | อนุมัติ D-GDA8 exact hash, elevated session, VM/media/target, lifecycle amendment, cleanup และ external gates แยกกัน |
| Provisioning operator | Boss หรือ session ที่ Boss จัดให้โดยตรง | สร้าง/ตั้งค่า disposable VM ตาม approved packet เท่านั้น; ห้ามขยาย scope เอง |
| Guest/account owner | Boss | เลือก synthetic local account และวิธีส่ง material แบบ no-network; ห้าม personal/production account |
| Artifact/harness owner | Boss + Luna packet | จัดหา candidate, source commit, dependency hashes และ lifecycle bundle provenance; ไม่ใส่ secret ลง repo/chat |
| Documentation worker | Luna 5.6 | ร่าง amendment และ evidence schema ตาม write scope; ไม่สร้าง VM/ISO/credential/keyring |
| Review gate | Terra 5.6 | ตรวจ exact bytes, authority, isolation, resource/media rules, AC/SC และ evidence; read-only |
| Final gate | Codex/ATHER | ตรวจ commit/hash/path/evidence truth และอนุมัติการประกอบงานตาม workflow; ไม่ patch source/config เอง |
| E1 lifecycle verifier | Boss operator + Terra review gate | ดำเนินและตรวจ lifecycle หลังมี approval/harness แยก; provisioning report อย่างเดียวปิด gate ไม่ได้ |

## 5. Prerequisite/input matrix — สิ่งที่ Boss ต้อง supply/select

| Input | ต้องระบุ/ส่งมอบ | ถ้าไม่มีหรือไม่ตรง |
|---|---|---|
| Elevated access path | ชื่อ session/host boundary และวิธีที่ Boss ควบคุม elevated provisioning; ไม่ใช่ automatic group mutation | `BLOCKED`; ห้าม retry ด้วย current unelevated process |
| ISO/media | supported Windows x64 ISO path, SHA-256, license provenance และ Secure Boot/vTPM decision | `BLOCKED`; ห้าม agent download/activate |
| Resource availability | ยืนยัน free RAM ≥ 12 GB, free disk ≥ 120 GB, CPU/volume ที่จะใช้ และ timestamp สด | `NOT READY/BLOCKED`; current ~7.1 GB RAM ไม่ผ่าน |
| Exact target | ยืนยัน one VM identity, two filesystem roots, one exact VHDX file และ collision-check authority ตาม §2; source/artifact refs เป็น provenance inputs เท่านั้น | `BLOCKED`; ห้ามสร้าง guessed path หรือ reuse parent/child จากการมีอยู่ของ parent |
| Guest accounts | synthetic local account, password handoff out-of-band, retention/expiry และ no-network boundary | `BLOCKED`; ห้าม personal account/credential ใน chat |
| Artifact/harness candidate | exact source commit, artifact/dependency hashes และ candidate ที่ non-production; lifecycle harness ต้องแยก approval | provisioning อาจหยุดหลัง baseline แต่ E1 lifecycle = `BLOCKED` |
| Retention/cleanup | Boss ระบุผู้ถือ VM powered-off และอนุมัติภายหลังหากจะ cleanup | retain; ห้าม delete/overwrite จาก amendment นี้ |

## 6. Provisioning runbook placeholders

ส่วนนี้เป็น category placeholders สำหรับ task packet ภายหลัง ไม่ใช่ executable
PowerShell script และไม่มี secret หรือ activation key. ทุก `<...>` ต้องถูกแทนด้วย
ค่าที่ Boss อนุมัติใน execution envelope เท่านั้น.

```text
P0 Authority preflight:
  category = read-only identity/elevation/Hyper-V authorization/service check
  inputs = <Boss session reference>, <operator reference>, <timestamp>
  gate = current process/identity must match approved boundary

P1 Exact collision preflight:
  category = exact VM identity/name, both filesystem roots, exact VHDX, parent
             existence/ownership, canonical path, reparse-point/junction, mount,
             path-escape and existing Hyper-V registration inspection
  inputs = <exact VM identity>, <exact VM/config root>, <exact VHDX file>,
           <exact evidence root>
  gate = all exact targets and any parent/reuse decision are explicitly resolved;
         parent existence never authorizes reuse; any ambiguity fails closed

P2 Resource preflight:
  category = host free RAM, free disk, logical processor, volume and Hyper-V
             capability observation
  inputs = <12 GB RAM threshold>, <120 GB disk threshold>, <selected volume>
  gate = current values meet thresholds at time of start

P3 Media/provenance preflight:
  category = ISO existence/read-only access, SHA-256 and license provenance
             verification; Secure Boot/vTPM setting record
  inputs = <Boss-supplied ISO path>, <approved ISO hash>, <license provenance>
  gate = all values match; no media download or activation action by agent

P4 Isolation declaration:
  category = VM network adapter absence, host-share/clipboard policy, removable
             read-only artifact-transfer boundary
  inputs = <no-network transfer reference>, <synthetic guest account reference>
  gate = no vNIC/external network and no provider/personal material

P5 VM definition:
  category = approved Hyper-V generation, vCPU, static-memory, dynamic-VHDX and
             Secure Boot configuration record
  inputs = Gen2 / 4 vCPU / 6 GB / 80 GB / <vTPM decision>
  gate = one VM identity, two filesystem roots, one exact VHDX file and settings
         match D-GDA8; no broad path or alternate VHDX is accepted

P6 Baseline:
  category = supported guest installation/bootstrap followed by power-off and a
             redacted immutable settings/install/provenance manifest with hashes
  inputs = <Boss-supplied media>, <baseline manifest hashes>
  gate = capture while VM is clean and powered off, before any guest account,
         password, authentication state, keyring/test material or other secret;
         no checkpoint/snapshot/export/clone/memory dump/save-state at any time

P7 Artifact staging:
  category = hash verification of clean-source build/artifact/dependencies via
             approved read-only no-network transfer
  inputs = <approved source commit>, <artifact manifest>, <dependency hashes>
  gate = non-production bundle only; lifecycle harness gate remains separate

P8 Provisioning stop and evidence:
  category = redacted envelope, final settings readback, powered-off state,
              one VM identity, two filesystem roots, one exact VHDX file,
              provenance and Terra review package
  inputs = <artifact hashes>, <redacted exact target refs>, <cleanup disposition>
  gate = stop before lifecycle; no checkpoint/snapshot/export/clone/memory
         dump/save-state occurred; provisioning result is not E1 lifecycle result
```

ไม่มีขั้นตอนใดใน runbook นี้อนุญาตให้เข้าถึง real keyring, ทำ OAuth/provider call,
ใช้ Supabase, ส่ง credential, รัน `startup_recover`, ทดสอบ logout/revoke/shutdown,
หรือ claim ว่า E1 ผ่าน. คำสั่งสร้าง checkpoint/snapshot, export, clone, memory dump,
save-state หรือลบ/ทำลาย VM, VHD, evidence หรือ source
ไม่อยู่ใน write/rollback boundary ของ amendment นี้.

## 7. E1 lifecycle matrix ที่ต้องอนุมัติแยกภายหลัง

ตารางนี้เป็น acceptance contract สำหรับ lane E1 เท่านั้น ไม่ใช่ provisioning task
และไม่อนุญาตให้รันตอนนี้. ต้องใช้ real OS keyring บน named clean VM และเส้นทาง
registered broker เดียวกับ source contract; test-only fake keyring ใช้ได้เฉพาะ
เมื่อมีการระบุว่าเป็น deterministic local evidence และห้ามเลื่อนชั้นเป็น E1 proof.

| ลำดับ | Lifecycle evidence ที่ต้องมี | ผลที่ต้องพิสูจน์ |
|---|---|---|
| E1-01 | Baseline absence | ก่อน material ใด ๆ ไม่มี account/Drive keyring entry หรือ public connected state |
| E1-02 | Synthetic write/rotate | เขียนและ rotate disposable test material ผ่าน approved path พร้อม redacted readback/hash |
| E1-03 | App restart | ปิด/เปิด application แล้ว registered startup path reconstructs state โดยไม่ publish stale material |
| E1-04 | Guest reboot | reboot guest แล้ว `startup_recover`/startup route ให้ผลตาม marker/index/slot contract |
| E1-05 | Startup recovery | ตรวจ Account และทุก registered Drive domain ด้วย non-test façade และ real OS keyring |
| E1-06 | Logout/revoke/shutdown cleanup | transition, drain, cleanup และ absence/readback สำเร็จ; uncertainty = `cleanup_failed` |
| E1-07 | Stale-generation denial | stale pre-send/post-send result ถูก deny ไม่ resurrect credential/status/archive/publication |
| E1-08 | Retention/closure | หลัง review power off; evidence redact/hash ครบ และ Terra ตรวจ exact package |

E1 lifecycle acceptance ต้องมี envelope แยก, artifact/harness hash แยก, operator/time
แยก, Terra review และ Boss approval ที่ระบุ lifecycle scope. ไม่สามารถปิด E1 ด้วย
provisioning report, D-GDA6 local tests, current host observation หรือ provider
simulation เพียงอย่างเดียว.

## 8. Acceptance Criteria — provisioning เท่านั้น

| ID | Criterion | Required evidence |
|---|---|---|
| AC-PROV-01 | one VM identity, two filesystem roots, and one exact VHDX file ตรง exact identity และไม่ชน target อื่น; ตรวจ parent existence/ownership, reparse/junction, mount/path escape และ existing Hyper-V registration แล้ว | canonical identity/collision record |
| AC-PROV-02 | provisioning ทำใน Boss-approved elevated boundary โดยไม่มี automatic group/IAM mutation | redacted authority envelope |
| AC-PROV-03 | VM settings เป็น Gen2, 4 vCPU, static 6 GB, dynamic VHDX 80 GB และ start preflight ผ่าน RAM ≥ 12 GB/disk ≥ 120 GB | settings readback + timestamped resource record |
| AC-PROV-04 | ISO เป็น Boss-supplied supported Windows x64 media ที่ hash/license provenance ตรง และ Secure Boot/vTPM decision ถูกบันทึก | redacted media provenance |
| AC-PROV-05 | guest default ไม่มี vNIC/external network, ไม่มี host share/clipboard/provider credential และ transfer เป็น approved hash-pinned no-network path | isolation record |
| AC-PROV-06 | automatic checkpoint ปิด และไม่มี Hyper-V checkpoint/snapshot ทุกชนิด; baseline เป็น redacted immutable settings/install/provenance manifest พร้อม hashes ขณะ VM clean และ powered off; ไม่มี guest credential/test material ใน baseline และไม่มี checkpoint/snapshot/export/clone/memory dump/save-state; VM ปิดหลัง review | checkpoint/contamination record |
| AC-PROV-07 | source/artifact/dependency hash ตรง exact approved commit และ provisioning หยุดก่อน lifecycle | manifest + stop-state record |
| AC-PROV-08 | envelope redacted ครบ, evidence root แยก, provenance/review ครบ และ Terra PASS/WARN ที่ยอมรับได้ | immutable provisioning envelope + Terra review |

## 9. Success Criteria — provisioning เท่านั้น

- SC-PROV-01: มี immutable envelope ที่ผูก `W2-E1-DGDA8`, source commit,
  candidate commit/hash, operator, exact target, timestamp, settings, media hash,
  artifact hashes, result และ retention disposition.
- SC-PROV-02: ไม่มี token, OAuth code, password, activation key, recovery phrase,
  keyring value, memory dump, personal identity หรือ provider response ใน repo,
  evidence root, log, screenshot หรือ chat. Synthetic guest credentials ที่อยู่นอก
  evidence ต้องไม่ถูก snapshot/checkpointed, cloned, exported หรือ dump.
- SC-PROV-03: VM boundary อ่านกลับได้และอยู่ใน `powered-off` state หลัง provisioning
  review โดยไม่ลบ target และไม่ overwrite evidence; baseline เป็น manifest ที่
  capture ตอน VM clean/powered-off และไม่มี checkpoint/snapshot/export/clone/
  memory dump/save-state เกิดขึ้น.
- SC-PROV-04: artifact/harness status แยกชัดเจนว่า `provisioning-ready`,
  `lifecycle-blocked` หรือ `lifecycle-approved`; ไม่มี inference ข้าม evidence class.
- SC-PROV-05: Terra ตรวจ exact package และ Codex final gate ยืนยัน one-file candidate
  scope; provisioning PASS ไม่ถูกเขียนเป็น E1 lifecycle PASS.

## 10. Provisioning exit criteria และ E1 lifecycle exit แยกกัน

### 10.1 Provisioning exit

Provisioning จะถือว่า `PASS — provisioning only` ได้เมื่อ AC-PROV-01 ถึง
AC-PROV-08 ผ่าน, one VM identity, two filesystem roots, one exact VHDX file ถูก
อ่านกลับได้, VM ถูก power off, baseline manifest อยู่ในสถานะ clean/powered-off,
ไม่มี checkpoint/snapshot/export/clone/memory dump/save-state, no-network/isolation
และ contamination controls อ่านกลับได้, artifact/evidence hashes ตรง, Terra review
ผ่าน และไม่มี lifecycle execution. ถ้า resource/media/authority/target/harness ใดขาด ให้ `BLOCKED` พร้อม
เหตุผลที่ตรวจสอบได้; ห้ามแปลงเป็น PASS ด้วย inference.

### 10.2 E1 lifecycle exit (ยัง pending)

E1 จะถือว่า PASS ได้ต่อเมื่อ E1-01 ถึง E1-08 มีหลักฐาน real OS-keyring บน named
clean VM, ใช้ approved registered-broker/harness path, มี redacted readback/absence,
restart/reboot/revoke/cleanup/stale-denial evidence, Terra review และ Boss exact
lifecycle approval. D-GDA8 provisioning PASS, D-GDA6 local PASS, fake keyring,
current-host run หรือ static source inspection ไม่ปิดเงื่อนไขนี้.

## 11. Stop conditions, rollback, retention และ destructive boundary

### 11.1 Stop conditions

STOP และคงสถานะ `BLOCKED` เมื่อเกิดข้อใดข้อหนึ่ง:

- elevated authority, operator, VM identity/name หรือ resolved target ไม่ตรง approved packet;
- one VM identity, two filesystem roots, one exact VHDX file หรือ collision check
  ไม่ตรง approved packet; parent existence/ownership ambiguity, junction, mount,
  path escape, existing Hyper-V registration หรือ existing exact VHDX = stop;
- free RAM ต่ำกว่า 12 GB หรือ free disk ต่ำกว่า 120 GB ณ เวลาเริ่ม;
- ISO hash/license provenance ขาดหรือไม่ตรง, OS support ไม่ชัด, Secure Boot/vTPM
  setting ไม่ตรง;
- network, host share, clipboard, personal account, provider credential หรือ
  production identifier ปรากฏใน guest/material/transfer boundary;
- checkpoint/snapshot/export/clone/memory dump/save-state เกิดขึ้นเมื่อใดก็ตาม หรือ
  synthetic guest credential/authentication state ถูก snapshot/checkpointed, cloned,
  exported หรือ dump;
- source/artifact/dependency hash ไม่ตรง หรือ lifecycle harness ไม่ได้ approval;
- มีความจำเป็นต้องแก้ source/config/migration, ปรับ IAM/group, ดาวน์โหลด media,
  ใช้ provider หรือเข้าถึง keyringนอก scope นี้.

### 11.2 Rollback และ retention

- หาก preflight ยังไม่ผ่าน: ไม่สร้าง target และเก็บเฉพาะ redacted blocked envelope.
- หาก provisioning เริ่มแล้วพบปัญหา: หยุดการสร้าง/ติดตั้งที่ boundary ปัจจุบัน,
  power off เมื่อทำได้อย่างปลอดภัย, retain one VM identity, two filesystem roots,
  one exact VHDX file และ evidence เพื่อ Terra ตรวจ; ห้ามลบเพื่อกลบหลักฐาน.
- ไม่มี delete, `Remove-VM`, `Remove-Item`, recursive cleanup, VHD overwrite,
  checkpoint/snapshot deletion หรือ evidence deletion ที่ได้รับอนุญาตจาก amendment นี้.
- การ cleanup ภายหลังต้องเป็น approval ใหม่ที่ระบุ one VM identity, two filesystem
  roots, one exact VHDX file, ผู้ปฏิบัติ, เหตุผล, retention check และผลลัพธ์.
- Retention default คือเก็บ VM powered-off และ immutable provisioning evidence
  ที่ exact evidence root จน Boss/Terra อนุมัติ disposition ใหม่; ห้าม checkpoint,
  snapshot, export หรือ clone ไป target อื่น.

## 12. Evidence/provenance schema และ redaction

Provisioning envelope ต้องมี schema version และ fields ต่อไปนี้ โดยใช้ reference,
hash หรือ boolean ที่ไม่เปิดเผย secret:

| Field | ข้อกำหนด |
|---|---|
| `task_id` / `decision_ids` | `W2-E1-DGDA8-DRAFT` หรือ execution ID ที่ Boss อนุมัติ และ D-GDA8-01…08 |
| `source_commit` | exact clean-worktree source commit ที่ build/stage |
| `candidate_commit` / `candidate_sha256` | amendment commit และ file SHA-256 แบบ immutable |
| `lane` / `environment_class` | `E1` / `clean-vm-provisioning`, ห้ามใช้ `production` |
| `operator_ref` / `authority_ref` | redacted Boss/session reference ไม่ใช่ token หรือ password |
| `vm_ref` / `target_refs` | one VM identity, two filesystem roots, one exact VHDX file ตาม §2; parent existence/ownership and collision result; source/artifact refs เป็น provenance inputs ไม่ใช่ target roots; redact personal parent context |
| `host_preflight` | OS/build/hypervisor/resources/tools/threshold result โดยไม่เก็บ credential |
| `media_ref` / `media_sha256` / `license_ref` | out-of-band references และ hash; ห้าม ISO content/key |
| `settings` | Gen2, vCPU, RAM, exact VHDX file, Secure Boot, vTPM, network state, automatic-checkpoint-disabled state, and zero checkpoint/snapshot/export/clone/memory-dump/save-state occurrence |
| `artifact_manifest` | artifact/dependency names, versions, hashes และ non-production label |
| `command_refs` | category/hashed invocation reference; ไม่บันทึก secret-bearing command line |
| `timestamps` | raw UTC และ ICT offsets เดิม, ไม่ overwrite timezone evidence |
| `result` / `stop_reason` | `PASS provisioning-only`, `BLOCKED`, `FAIL` หรือ `WARN` พร้อม reason |
| `retention` / `cleanup_disposition` | powered-off state และ separate-approval reference; ไม่ claim cleanup ถ้ายังไม่ทำ |
| `review` | Terra verdict, reviewer, reviewed bytes และ Codex final-gate result |

ต้อง scan candidate/report/envelope/log/screenshot ตาม scope ที่อนุมัติด้วย secret
pattern และตรวจ zero findings. Redact token, OAuth code, password, activation key,
recovery phrase, keyring value, private key, email, device serial, personal path,
QR/code, URL query, provider response และ memory contents. Hash ของ secret-bearing
artifact อย่างเดียวไม่ทำให้การเก็บ artifact นั้นปลอดภัย.

## 13. Evidence classes และการไม่เลื่อนชั้นหลักฐาน

| Evidence class | พิสูจน์ได้ | พิสูจน์ไม่ได้ |
|---|---|---|
| local/static | source graph, contract, deterministic tests, build, diff | real OS keyring หรือ VM lifecycle |
| host-preflight | command/service presence, current resource/authority observation | VM creation success หรือ hardware virtualization absence |
| provisioning | one VM identity, two filesystem roots, one exact VHDX file, settings, isolation, redacted immutable clean/powered-off baseline manifest, zero checkpoint/snapshot/export/clone/memory-dump/save-state occurrence, powered-off retention | keyring write/rotate/restart/revoke lifecycle |
| real-keyring lifecycle | E1 matrix บน named clean VM และ registered route | provider/device/release/production readiness โดยลำพัง |
| provider/staging | real OAuth/Drive/Supabase/Edge evidence ตาม D-GDA7 | clean VM หรือ physical device |
| device/UAT | physical Android/Dashboard/FUNGWIRE identity/delegation/revoke | VM/keyring/provider completeness |
| release/production | signing, package, deploy, monitoring, approval evidence | ไม่ถูกเปิดโดย D-GDA8 |

## 14. Cross-check กับ source/doc contracts

- D-GDA7 กำหนด E1 เป็น clean Windows VM/equivalent + real OS-keyring lifecycle
  และแยก E1 จาก E0, E2-E6; amendment นี้เลือก Hyper-V เป็น proposed disposable
  boundary โดยไม่เปลี่ยน dependency หรือ claim ว่า E1 ผ่าน.
- D-GDA6 Terra final review รับรอง local/static registered façade และ startup
  recovery matrix แต่ระบุชัดว่า real OS-keyring/clean VM ยัง open; D-GDA8 ใช้
  `NativeRegisteredBroker`/`startup_recover` เป็น contract reference เท่านั้น.
- `auth_session.rs` แสดง native composition `NativeKeyring`, `NativeClock`,
  `NativeListener`, `NativeProvider` และ `RegisteredBrokerEntrypoints`; `lib.rs`
  เรียก `auth_session::startup_recover()` ใน setup และลงทะเบียน broker commands.
  Amendment นี้ไม่แก้ source และไม่สร้าง second authority.
- Workflow Luna–Terra กำหนด Luna เขียน bounded task, Terra review read-only,
  Codex final gate และ Boss เป็น approval authority; candidate นี้คง model ดังกล่าว.

## 15. Verification plan สำหรับ candidate

การร่างนี้ทำแบบ read-only ก่อนเขียน target และห้ามสร้าง VM/external action. ก่อน
ส่ง review ต้องตรวจ:

1. อ่านเอกสาร predecessor และ source contract ตาม task packet แล้ว cross-check ID,
   paths, authority, resource, lifecycle และ external-gate boundaries.
2. secret scan เฉพาะ candidate file scope; ต้องรายงาน `0 findings` โดยไม่อ่านค่า
   secret จาก environment/keyring.
3. `git diff --check` สำหรับ candidate.
4. `git diff --name-only`/commit scope ต้องมีเฉพาะไฟล์นี้ และ dirty/untracked เดิม
   ต้องไม่ถูก stage.
5. คำนวณ file SHA-256 หลัง final bytes และส่ง Terra ตรวจ candidate bytes เดียวกัน.

## 16. Exact approval semantics

Candidate นี้ยังไม่ใช่ approval จนกว่าจะ commit เฉพาะไฟล์, คำนวณ SHA-256, ให้ Terra
review exact bytes และ Boss อนุมัติ exact phrase. Proposed phrase:

```text
approve D-GDA8-01 through D-GDA8-08 — commit <candidate-commit> — SHA-256 <64 uppercase hex characters>
```

Approval นี้ (เมื่อเกิดขึ้น) จะ authorize เฉพาะ provisioning workflow ตาม D-GDA8
สำหรับ one VM identity, two filesystem roots, one exact VHDX file ตาม §2 และไม่รวม
lifecycle execution, group/IAM mutation, credential/provider action, cleanup,
push/PR/merge/release/deploy หรือ production. E1 lifecycle ต้องมี separate
amendment/approval และ fresh Terra review.

## Version Diff

- `0.1.0b` -> `0.2.0b`: cycle-2 แก้ Terra P1-01 โดย normalize เป็น one VM
  identity, two filesystem roots, and one exact VHDX file และแยก source/artifact
  references เป็น provenance inputs ไม่ใช่ target roots.
- `0.1.0b` -> `0.2.0b`: cycle-2 แก้ Terra P1-02 โดยห้าม Hyper-V checkpoint/snapshot
  ทุกชนิด และแทน baseline checkpoint ด้วย redacted immutable settings/install/
  provenance manifest + hashes ขณะ VM clean และ powered off; ห้าม export/clone/
  memory dump/save-state เป็น substitute และห้าม guest credential state เข้า evidence.
- คง controls เดิมทั้งหมด: current host RAM ประมาณ 7.1 GB เป็น `NOT READY`,
  automatic elevation/group mutation, ISO download, vNIC/external network,
  lifecycle/harness, cleanup, provider, release และ production ยังคงแยก approval.

## Cycle-2 Fix Matrix — Terra P1-01/P1-02

| Terra finding | Cycle-2 correction | Changed clauses |
|---|---|---|
| P1-01 — exact target boundary contradictory | กำหนด one VM identity `FUNG-W2-E1-KEYRING-C1`, two filesystem roots, exact VHDX file และระบุ source/artifact references เป็น provenance inputs; เพิ่ม collision checks ครบ VM name, roots, VHDX, parent ownership, reparse/junction, mount/path escape และ existing Hyper-V registration; parent existence ไม่ authorize reuse | §1.1, §2, D-GDA8-02, prerequisites, P1, P5, P8, AC-PROV-01, §11.1–11.2, §12, §16 |
| P1-02 — baseline checkpoint may retain guest credential state | ห้าม checkpoint/snapshot ทุกชนิดตลอด provisioning; baseline เป็น redacted immutable settings/install/provenance manifest + hashes ตอน VM clean/powered off ก่อน guest account/credential/secret state; ห้าม export/clone/memory dump/save-state และ guest credentials ห้ามเข้า evidence หรือถูก snapshot/clone/export/dump | D-GDA8-05–06, P6/P8, AC-PROV-06, SC-PROV-02–03, §10.1, §11.1–11.2, §12–13 |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| `0.2.0b` | 2026-08-26 | candidate | Cycle-2 correction for Terra P1-01/P1-02: exact target identity normalized; all Hyper-V checkpoints/snapshots and substitutes prohibited; clean powered-off manifest baseline defined. No provisioning executed. | externally bound after focused commit | Luna 5.6 |
| `0.1.0b` | 2026-08-26 | candidate | Drafted D-GDA8 Hyper-V provisioning boundary; lifecycle and destructive cleanup remain separately gated. | externally bound after focused commit | Luna 5.6 |

— End of D-GDA8 candidate —
