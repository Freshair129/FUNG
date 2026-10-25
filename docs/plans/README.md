---
version: "0.1.2b"
created_at: "2026-10-19"
last_update: "2026-10-25"
status: "beta"
superseded_by: null
attributes:
  domain: "project-planning"
  doc_type: "documentation-index"
  scope: "FUNG implementation plans"
  language: "Thai-first"
---

# FUNG implementation plans

แผนในโฟลเดอร์นี้ระบุขอบเขตและลำดับงาน ไม่ใช่หลักฐานว่างานเสร็จหรือฟีเจอร์ทำงานแล้ว

## จุดอ้างอิงหลัก

- [Application renderer-transition repair, issue 96](2026-10-25-issue-96-application-transition.md) tracks root-cause isolation, source regressions and installed-package release gates.

- [Multichannel source-export repair, issue 97](2026-10-24-multichannel-source-export-fix.md) specifies all-channel custody, atomic media publication, timeline fail-closed behavior and honest job outcomes; acceptance evidence remains separate.

- [Product MCP feature roadmap](2026-10-20-product-mcp-feature-roadmap.md) เป็น roadmap ที่อยู่ระหว่าง execution; [execution DAG](2026-10-20-product-mcp-execution-dag.json) เป็น checkpoint ledger (ไม่ใช่ scheduler หรือหลักฐานว่า feature ทำงานแล้ว). MCP-00 inventory และ MCP-01 contract v0.1.4b ผ่าน independent review; MCP-02 loopback bridge อยู่ระหว่างตรวจรับ implementation.

- [Master implementation plan](2026-08-09-fung-master-implementation-plan.md) เป็นแผนระยะและ protocol ที่ระบุไว้ใน repo `AGENTS.md`; ตรวจสถานะจริงกับ Desktop/Mobile status docs ก่อนใช้.
- [Desktop current progress](../Desktop/08-real-progress.md) และ [Mobile implementation status](../Mobile/IMPLEMENTATION_STATUS.md) เป็นแหล่งดูผลการทำงานและ gate ล่าสุดตาม platform.
- [Desktop meeting workflow MVP implementation](2026-10-22-desktop-meeting-workflow-mvp.md) follows the approved Home → Live → Archive journey and tracks app/system source capture, measured per-channel waveforms, mockup evidence, and MVP verification gates.
- [Dated execution plans](.) เป็นเอกสารงานตามวันที่และ feature; สถานะของแต่ละแผนอยู่ใน front matter/เนื้อหาของไฟล์นั้น.
- [External retrieval plan](2026-08-11-live-meeting-external-retrieval-implementation.md) เป็นแผนที่ย้ายมาจาก path เก่า; compatibility pointer ยังอยู่ที่ `../implementation-plan.md`.

## กติกา

- ตั้งชื่อแผนใหม่เป็น `YYYY-MM-DD-<feature>-plan.md` หรือใช้รูปแบบที่ชัดเจนซึ่งมีอยู่ในชุดเดิม.
- ลิงก์ไปยัง requirement/spec ที่เป็นต้นทาง และระบุ decision/architecture dependency.
- ระบุ prerequisite, owner, task, verification evidence และ exit criteria.
- เมื่อแผนถูกแทนที่ ให้ระบุสถานะและลิงก์ใหม่; อย่าอัปเดตแผนเก่าให้ดูเหมือนผลทดสอบปัจจุบัน.

## Additional dated plans

- [2026-08-05-zoom-meeting-ingestion](2026-08-05-zoom-meeting-ingestion.md)
- [2026-08-08-auth-login-ui](2026-08-08-auth-login-ui.md)
- [2026-08-08-byom-tts-provider](2026-08-08-byom-tts-provider.md)
- [2026-08-09-phase-0-stabilize](2026-08-09-phase-0-stabilize.md)
- [2026-08-09-phase-1-pairing-login](2026-08-09-phase-1-pairing-login.md)
- [2026-08-09-phase-2-fungwire](2026-08-09-phase-2-fungwire.md)
- [2026-08-09-phase-3-byom-cloud-keys](2026-08-09-phase-3-byom-cloud-keys.md)
- [2026-08-13-phase-4-google-drive-backup-mobile-account](2026-08-13-phase-4-google-drive-backup-mobile-account.md) — `superseded`
- [2026-08-23-recording2-smart-gift-catalog-task-breakdown](2026-08-23-recording2-smart-gift-catalog-task-breakdown.md) — `candidate`
- [2026-09-17-callmd-playback-diagnostic-orchestration](2026-09-17-callmd-playback-diagnostic-orchestration.md) — `beta`
- [2026-09-17-callmd-playback-diagnostic-review](2026-09-17-callmd-playback-diagnostic-review.md) — `under review`
- [2026-09-17-callmd-playback-diagnostic-verification](2026-09-17-callmd-playback-diagnostic-verification.md) — `under review`
- [2026-09-17-callmd-playback-fixture-fix-orchestration](2026-09-17-callmd-playback-fixture-fix-orchestration.md) — `beta`
- [2026-09-17-callmd-playback-fixture-fix-review](2026-09-17-callmd-playback-fixture-fix-review.md) — `beta`
- [2026-09-17-callmd-playback-fixture-fix-verification](2026-09-17-callmd-playback-fixture-fix-verification.md) — `beta`
- [2026-09-17-callmd-pr59-ci-repair-orchestration](2026-09-17-callmd-pr59-ci-repair-orchestration.md) — `beta`
- [2026-09-17-callmd-pr59-ci-repair-review](2026-09-17-callmd-pr59-ci-repair-review.md) — `candidate`
- [2026-09-17-callmd-pr59-ci-repair-verification](2026-09-17-callmd-pr59-ci-repair-verification.md) — `beta`
- [2026-09-22-meeting-local-completion-remediation](2026-09-22-meeting-local-completion-remediation.md) — `candidate`
- [2026-09-22-phase-3-byom-acceptance-runbook](2026-09-22-phase-3-byom-acceptance-runbook.md) — `candidate`
- [2026-09-22-phase-3-controller-blocker-remediation](2026-09-22-phase-3-controller-blocker-remediation.md) — `beta`
- [2026-10-16-design-system-source-of-truth](2026-10-16-design-system-source-of-truth.md)
- [2026-10-20-product-mcp-feature-roadmap](2026-10-20-product-mcp-feature-roadmap.md) — active execution
- [2026-10-20-product-mcp-execution-dag](2026-10-20-product-mcp-execution-dag.json) — active checkpoint ledger, paired with the roadmap

- [Dedicated endpoint routing plan](2026-10-25-dedicated-endpoint-routing-plan.md) — `candidate`; native qualification gates product integration and installed acceptance.
