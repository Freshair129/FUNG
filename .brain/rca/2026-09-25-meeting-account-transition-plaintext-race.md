# RCA: account transition during protected meeting plaintext publication

**Date:** 2026-09-25<br>
**Risk:** HIGH<br>
**Status:** Remediated and independently reviewed; cross-logon-session runtime remains NOT_RUN

## Symptom

A logout or account switch could begin after a local-owner lifecycle witness was
checked but before protected meeting data reached the renderer. Affected outputs
include a People snapshot, encrypted metric values and their aggregate, and a
private meeting draft. Stale account data could also remain visible in the panel
after an account transition.

## Evidence

- The final frozen-source review found `list_meeting_people` extracted only the
  identity context from `capture_native_identity_context`, dropping its
  `AccountOperationGuard` before decrypting profile and identity-link rows.
- The same review found `compute_meeting_knowledge_metric` dropped the guard
  after identity capture, then decrypted stored metric values and returned the
  computed aggregate without holding the broker lifecycle fence.
- `meeting_agent_ask` emitted a private draft after the owner-vault fence but
  without holding the registered account broker fence through runtime
  insertion and event publication.
- Refresh failure can move the broker to `signed_out` or `refresh_failed`, but
  the refresh path did not emit `auth-session-changed`, leaving displayed local
  data visible until another event or panel remount.
- The meeting panel subscribed to draft and vault state but had no account
  lifecycle event to clear bound-account plaintext after login, logout, or
  account switch.

## Root Cause

The owner-session fence checked a lifecycle witness at one point in time. It
did not serialize protected plaintext reads or publication with the broker's
account lifecycle mutex. `list_meeting_people` and
`compute_meeting_knowledge_metric` discarded the captured account operation
guard before decrypting account-bound values and constructing their outputs.
The draft path also lacked a broker fence through publication. The UI had no
account-transition signal for automatic refresh failure, so it could not
invalidate late responses and clear already displayed account-bound data on
that lifecycle transition.

## Why the issue escaped detection

Existing account-fence regressions covered durable commits, while metric,
People, and draft tests covered encryption, data custody and vault-lock behavior
separately. They did not interleave account switch/logout with decrypted metric
computation, a People read or draft publication; auth-event tests covered
explicit login/logout but not refresh failure; and the mock panel had no account
lifecycle event fixture.

## Remediation

The account guard now provides a result-returning lifecycle fence. The People
snapshot retains its captured guard through protected reads and result
construction. The metric compute path retains its guard through decryption,
aggregation and result construction. `meeting_agent_ask` holds a guard through
final runtime insertion, event emission, and return-value construction. Native
login, cancel, logout, and lifecycle-changing refresh outcomes emit
`auth-session-changed`; the panel responds by locking its local vault, clearing
private knowledge/People/draft state, and rejecting late results from the
previous account lifecycle.

## Verification

The account-lifecycle UI fixture passed: an account-transition event cleared
the visible private draft and locked the selected vault in the mock panel. The
final Rust library run passed 576/576 with one ignored, meeting-knowledge
integration passed 17/17, and all 32 registered Node/Python suites passed; the
W1 PostgreSQL case was skipped because Docker is unavailable. Formatting,
diff-check, all-target check/clippy/build and the Desktop production build
passed. An independent post-fix source review verified the People, metric,
draft and refresh-event paths and found no blocker. The AppContainer probe
requires access to the per-user Windows profile store; it passed with that
access and fails closed in the restricted sandbox. Native Tauri interaction and
simultaneous cross-logon-session mutex behavior remain NOT_RUN.
