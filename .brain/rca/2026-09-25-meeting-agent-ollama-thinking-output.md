# RCA: thinking output left the Ollama answer content empty

**Date:** 2026-09-25
**Risk:** LOW
**Status:** Remediated and locally verified

## Symptom

The local Meeting Agent proposal request using `qwen3.5:9b` completed, but the
Ollama response placed its reasoning in `message.thinking` and left
`message.content` empty. FUNG reads only `message.content`, so the proposal
cannot satisfy the required `{answer, refs}` contract and fails closed.

## Evidence

- `meeting_agent_model::generate` sent `model`, `messages`, `stream`, `format`,
  and `num_predict`, without a `think` setting.
- Local `/api/show` identified `qwen3.5:9b` as a thinking-capable model. With
  the adapter-shaped prompt and a synthetic UI fixture, `/api/chat` returned
  empty `message.content` and a nonempty `message.thinking` in 12 seconds.
- The same model and fixture with `think: false` returned
  `{"answer":"42 ล้านบาท","refs":["e0"]}` in 0.5 seconds.
- The Ollama chat API documents `think: false` as requesting no thinking
  output: https://docs.ollama.com/api/chat
- The fixture evidence was synthetic; no user meeting data was sent.

## Root Cause

The adapter relied on the selected model's default thinking mode while parsing
only `message.content`. For this model, the default placed the response in the
separate thinking field and produced no answer content.

## Why the issue escaped detection

The transport tests used stubbed responses and covered malformed answer JSON
and evidence references. They did not call a thinking-capable Ollama model, so
they did not exercise the distinction between `message.thinking` and
`message.content`.

## Remediation and prevention

Set `think: false` for this answer-only Ollama request and add a focused test
that locks this request setting. Continue to accept only valid answer content
and evidence IDs; do not parse or persist reasoning text.

## Verification

Direct Ollama smoke with `think: false` passed against the synthetic fixture:
`qwen3.5:9b` returned the expected Thai answer and `e0` reference in 0.5
seconds. The adapter request regression passed with the Rust module tests (5/5),
`cargo fmt --check`, and Clippy `--all-targets -D warnings`.

Cargo tests and Clippy used a process-scoped `TAURI_CONFIG` override to omit
bundle resource staging because the sandbox could not read a directory under
the existing `.venv-whisper` runtime. No project resource configuration was
changed. Broad Thai answer quality, native UI, real meeting and release
acceptance remain unverified.

