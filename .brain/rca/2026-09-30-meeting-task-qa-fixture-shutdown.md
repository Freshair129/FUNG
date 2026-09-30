# QA fixture shutdown — review finding

- Symptom: the ignored QA fixture's 15-minute deadline could be extended by a
  slow input stream to its synthetic model server.
- Evidence: the main test set an atomic stop flag then joined the model thread;
  that thread could already be inside the production HTTP request reader. Its
  five-second timeout applies per read, not to the entire request lifetime.
- Root cause: stop flag observation was only at the accept-loop boundary.
- Why it escaped initial detection: browser and Node QA requests sent complete
  bodies promptly, so no in-flight reader existed when stop was requested.
- Prevention: retain a clone of the active fixture-model socket, check stop
  under its mutex when accepting, and close it on stop before joining. Test a
  deliberately incomplete request during shutdown. This affects the explicit
  test-only harness, not production FUNG or the user's running runtime.
