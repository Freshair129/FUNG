---
version: "0.1.0b"
created_at: "2026-08-23T16:16:38+07:00,ATHER"
last_update: "2026-08-23T19:16:58+07:00,ATHER"
status: "candidate"
superseded_by: null
attributes:
  domain: "cloud-backup-security"
  doc_type: "root-cause-analysis"
  scope: "W1-A native Google Drive authorization, cancellation, and opener boundaries"
---

# RCA — Google Drive Core Failed the Terra Security Gate

## Symptom

W1-A passed all focused and full automated tests but failed Terra's HIGH-risk
review because native authorization trusted caller-supplied identity,
cancellation was not atomic with token persistence, and URL-opening capability
was broader than the approved OAuth endpoint.

## Evidence

1. Drive commands receive `user_id`/`device_id` from the webview and validate
   shape but do not derive the authoritative identity from a verified native
   session and registered device.
2. OAuth completion checks cancellation before provider exchange; the pending
   flow can be cancelled after that check and still reach keyring persistence.
3. The Tauri capability uses `opener:allow-open-url`, whose generated contract
   allows URL opening without a preconfigured scope.
4. Local tests passed 3/3 Drive, 17/17 backup-flow, 8/8 focused Rust, and
   370/370 full Rust, showing that the test suite did not encode these attack
   boundaries.

## Root Cause

The implementation treated identity-derived key names, an early cancellation
check, and a generic platform capability as security controls. These mechanisms
provide addressing, state observation, and functionality, but they do not
provide authorization, atomic terminal-state enforcement, or least privilege.

## Why the issue escaped detection

- Tests validated happy paths and malformed input but not a malicious webview
  supplying a foreign identity tuple.
- Cancellation tests did not force the interleaving between initial state check
  and keyring persistence.
- Capability tests checked that URL opening was available, not that its scope
  was restricted to the approved authorization origin.
- Passing local tests were correctly treated as implementation evidence, but a
  separate security reviewer was required to expose missing threat cases.

## Proposed prevention

1. Make native, verified authorization context mandatory before keyring or
   provider access; frontend identity values are hints at most.
2. Model OAuth session terminal states explicitly and transition them under one
   synchronization boundary.
3. Test deterministic cancellation/exchange/persistence interleavings.
4. Require scoped platform capabilities plus native allowlists for all external
   URL/egress surfaces.
5. Add negative authorization tests to every provider adapter before a local
   implementation can pass its review gate.

## W1-A-F2 addendum — server authority and atomicity

The first implementation fix moved private-key custody to the OS keyring and
added signed server authorization, but Terra still failed the lane because the
server record accepted as native authority remained browser-writable.

Additional root causes:

1. Cryptographic proof was checked against a `devices` row that the same
   authenticated browser could insert and mutate. Key possession therefore did
   not establish an independently approved native device.
2. Connection scope and operation capability were conflated. One active Drive
   connection implicitly allowed both write and restore.
3. Replay prevention used an isolate-local map plus a durable read-then-insert
   audit sequence without a unique transactional reservation.
4. Local build evidence depended on an untracked stylesheet and Deno lockfile,
   so the reviewed commit was not clean-checkout reproducible.

Required prevention is a server-owned pending-enrollment/approval state
machine, independent default-deny operation grants, one atomic durable replay
reservation, an exact native-owned login listener, and clean-checkout supply-
chain verification. Legacy/pairing rows must never be silently promoted to
Drive authority.

## Version Diff

- `new -> 0.1.0b`: documented the three Terra findings and prevention controls
  before a fix cycle.
- `0.1.0b`: added the W1-A-F2 server-authority, grant, replay, and clean-checkout
  root causes after independent Terra review.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-08-23 | candidate | Added W1-A-F2 authority/schema RCA addendum | `db0b949` | ATHER |
| 0.1.0b | 2026-08-23 | candidate | RCA for W1-A Terra security-gate failure | `617eba0` | ATHER |
