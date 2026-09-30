# Capability error redaction — review finding

- Symptom: the new, unreleased capability response could serialize an internal
  ledger read error as `reasonCode`.
- Evidence: `capabilities` forwarded `meeting_agent_model::readiness.reason_code`;
  its `local_config` query propagates `Result<_, String>` errors from Genesis.
  The independent source reviewer identified this path before delivery.
- Root cause: the adapter assumed every readiness error was already a stable
  public code, although the internal helper has a mixed error domain.
- Why it escaped initial detection: fixtures exercised an absent provider, which
  returns a fixed code, rather than an unexpected storage-read failure.
- Prevention: a shared exact allowlist for capability and draft error responses;
  unexpected and path-bearing strings become `LOCAL_MODEL_UNAVAILABLE`. Add a
  direct test for raw paths and prefix-spoofed errors. No live data was exposed
  or runtime server started in this implementation turn.
