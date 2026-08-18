---
version: "0.1.1b"
created_at: "2026-08-14T02:40:00+07:00,ATHER"
last_update: "2026-08-14T02:47:00+07:00,ATHER"
status: "beta"
superseded_by: null
attributes:
  domain: "backup-cryptography"
  doc_type: "technical-design"
  scope: "FUNG Phase 4 Task 3"
---

# Phase 4 Archive Envelope Cryptography Decision (Candidate)

## Purpose and Boundary

This candidate specifies the encrypted envelope for the already-approved
development/test filesystem transport. It does not authorize a Google Drive
adapter, a filesystem write, a restore command, or a production/release claim.
GenesisBlockDB remains the sole export/restore authority; FUNG encrypts only
the opaque export bytes returned by its approved U9 contract.

## Proposed Decision

| Area | Candidate choice | Why |
| --- | --- | --- |
| Payload cipher | `chacha20poly1305 = "=0.10.1"`, `XChaCha20Poly1305`, feature `stream` | Authenticated streaming encryption with an extended nonce construction; the RustCrypto AEAD API provides a dedicated stream module. |
| Stream mode | `aead::stream::StreamLE31`, 64 KiB plaintext chunks | The 20-byte random stream nonce prefix plus the counter/final-bit space produces unique per-chunk nonces for one archive; final-chunk marking detects truncation. |
| Key wrapping cipher | Separate `XChaCha20Poly1305` invocation with a fresh random 24-byte nonce | Keeps a random data-encryption key distinct from the recovery secret and authenticates the envelope. |
| Recovery secret | App-generated 256-bit entropy encoded as an English BIP-39 24-word phrase with `bip39 = "=2.2.2"` | User-held, portable recovery material; BIP-39 supports 24-word mnemonic codes and the selected crate maintains the English list. |
| KDF | `argon2 = "=0.5.3"`, Argon2id v1.3, 64 MiB memory, 3 iterations, 1 lane, 32-byte output, fresh 16-byte salt | Produces the wrapping key from the recovery phrase. Parameters and salt are versioned non-secret manifest fields. |
| Data key | Fresh random 32-byte key per archive | Compromise of one archive does not reuse the payload key of another archive. |
| Hash | Existing `sha2` SHA-256 over final archive bytes | Detects accidental corruption before recovery and is stored as non-secret metadata. |

`Cargo.lock` must resolve exactly the approved versions before any archive code
is considered verified. No custom cipher, KDF, word list, compression layer, or
raw SQLite export is permitted.

## Wire Format v1

`<archive-id>.fungbk` contains only a fixed binary header followed by encrypted
stream chunks. It is not a tar file and FUNG does not enumerate Genesis
internals. The opaque byte stream comes from `Storage::export_backup` and is
passed back to `Storage::restore_backup` only after decryption and validation.

1. Header: magic `FUNGBK01`, format version, 20-byte stream nonce prefix,
   ciphertext length framing, and a canonical protected-header digest.
2. Protected header: canonical JSON bytes containing archive ID, format
   version, Genesis source-manifest digest, created-at timestamp, chunk size,
   and algorithm/KDF identifiers. Its SHA-256 digest is associated data for
   every payload chunk and for the wrapped data-key envelope.
3. Data-key envelope: KDF salt and parameters, fresh 24-byte envelope nonce,
   and the authenticated ciphertext of the 32-byte data key. It never contains
   the phrase or plaintext data key.
4. Payload: ordered 64 KiB encrypted chunks. Each chunk uses the protected
   header digest as associated data; only the final chunk has `last_block` set.
5. Non-secret sidecar manifest: archive ID, relative archive name, final byte
   count, SHA-256, source-manifest digest, timestamp, format/algorithm IDs,
   KDF parameters, salt, envelope nonce/ciphertext, and terminal state.

The public manifest is written only after the final archive SHA-256 is known.
The protected-header digest prevents that final-file digest from creating a
circular associated-data dependency.

## Recovery-Secret UX

- Generate the 24-word phrase in native code when the user explicitly starts
  their first backup; do not accept a web-supplied key or path.
- Show it once in a dedicated acknowledgement step, require re-entry of two
  prompted word positions before proceeding, and do not write it to logs,
  telemetry, browser state, `localStorage`, Supabase, GenesisBlockDB, or the
  non-secret manifest.
- Never place the phrase in automatic clipboard history. Restore accepts it in
  a masked input, derives the wrapping key in native code, and zeroizes
  temporary byte buffers on all exits where the selected crate API permits it.
- Lost phrase means the archive cannot be restored. The UI must say this before
  a backup is marked verified.

## Required Tests Before a Filesystem Write

1. RustCrypto XChaCha20-Poly1305 and Argon2id known vectors, plus BIP-39
   entropy-to-24-word round trip.
2. Fresh stream nonce prefix per archive; no duplicate prefix across a focused
   test batch; wrong phrase, altered salt/envelope/header, altered payload, and
   truncation reject before Genesis restore.
3. Byte-for-byte archive digest validation and interrupted-write preservation.
4. Serialized command/status/manifests contain no phrase, plaintext data key,
   or provider credential. The existing Task 2 boundary guard remains in
   place.

## Sources Checked (2026-08-14)

- [RustCrypto AEAD 0.5.2 stream API](https://docs.rs/aead/0.5.2/aead/stream/)
- [RustCrypto password-hashes project](https://github.com/RustCrypto/password-hashes)
- [bip39 2.2.2 documentation](https://docs.rs/bip39/2.2.2/bip39/)

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.1b | Approved Task 3 and recorded the implemented pinned envelope plus passing vector/failure evidence. |
| 0.1.0b | Proposed the dependency pins, envelope format, nonce/KDF parameters, recovery UX, and test gates. |

## Approved Decision

Boss approved this candidate for Task 3 implementation only: the pinned
dependencies, `src-tauri/src/backup_archive.rs`, crypto vectors, and failure
tests are now in the branch. Task 4 filesystem writes, Task 5 backup
orchestration, Task 6 restore orchestration, UI, mobile work, Google Drive, U9
closure, and release remain out of scope.

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.1b | 2026-08-14 | beta | Boss approved the bounded Task 3 slice; pinned envelope implementation and vector/failure tests passed. Filesystem/provider/restore work remains out of scope. | working-tree | ATHER |
| 0.1.0b | 2026-08-14 | candidate | Proposed Task 3 dependency pins, envelope format, nonce/KDF parameters, recovery UX, and test gates; no crypto code or dependency changed. | working-tree | ATHER |
