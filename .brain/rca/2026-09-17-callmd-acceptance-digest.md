---
version: "0.1.0b"
created_at: "2026-09-17T06:53:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T06:53:00+07:00,Codex"
status: "beta"
superseded_by: null
attributes:
  doc_type: "root-cause-analysis"
  scope: "Controller acceptance digest transcription erratum; no product input change"
  risk: "LOW metadata correction"
---

# Acceptance digest erratum

## Symptom / evidence

Terra Hume01a0ac9b-9802-7961-a4b0-2b318ba57403 found the controller packet
contained a65-character acceptance digest. Independent Get-FileHash at06:49
confirms the actual root docs/specs/2026-09-17-callmd-desktop-acceptance.md:
`8686bf25f8aa9accf170e4d41409dab777246178e942178dfee7ee656a0e613c` (64).
The original accepted worker report callmd-parallel-amendment-fix1.md:41 records
exactly these same64hexbytes in uppercase at03:35. Thus the accepted document
has not changed; the copied controller digest gained an extra f in dffee7.

## Root cause / why it escaped

Manual transcription into controller summaries/review records/packets propagated
the malformed string. Some later reviews stated hash agreement too broadly.
Length validation was absent from dispatch bookkeeping; the actual documents
were read and versioned but that did not validate the copied digest literal.

Affected historical snapshots are callmd-backend-interface-review.md:25 and
callmd-parallel-amendment-review.md:39; preserve their bytes as historical review
records and use this explicit erratum, not their malformed acceptance literal,
as current provenance. Manifest current references are corrected. Backend FIX1
report's assertion that an f was omitted is also incorrect; active FIX2 owner
was told to correct it before its next freeze. No implementation source changed.

## Correction / prevention

Use actual Get-FileHash output, validate64hexcharacters before dispatch, and
compare current authority to original worker receipts rather than retyping.
Controller independently verified this digest and matched the preserved original
worker receipt; Hume independently verified the current file too. All other
manifest SHA256/digest literals passed length validation. Corrected identity is
the same approved v0.1.3b artifact; no semantic contract/test/authority change,
no source-review invalidation, no new approval and no test/CI waiver. Historical
malformed strings are explicitly not valid immutable content pins.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-09-17 | beta | Correct controller digest against current hash and original receipt | UNCOMMITTED;base376ef30 | Codex orchestrator |
