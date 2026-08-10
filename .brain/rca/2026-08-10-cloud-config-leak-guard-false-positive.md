---
status: resolved
date: 2026-08-10
scope: Phase 3 cloud-config no-leak static test
severity: medium
---

# RCA — File-level cloud-config leak guard rejected a keyring-only command

## Symptom

After the cloud-config Tauri commands were added to `lib.rs`, the full Rust suite failed `cloud_config::tests::no_source_file_serializes_cloud_config_into_genesis_or_supabase_paths` even though the command saves configuration through the OS keyring.

## Evidence

- `lib.rs` contains `CloudProviderConfig` only in the cloud-config command region and calls `cloud_config::save_cloud_config` there.
- The same file has unrelated `genesis_adapter::commit_rows` calls for application startup, projects, jobs and recordings.
- The old test rejected any source file containing both strings, without verifying that the cloud-config command passed data into a persistence call.

## Root Cause

The static guard used file-level keyword co-occurrence as a proxy for data flow. That was valid while cloud configuration lived in its own module but became a false positive once command wiring correctly shared `lib.rs` with unrelated persistence code.

## Why It Escaped Detection

The Task 2 leak guard was written before Task 8 command registration. The full Rust suite was not green after both changes coexisted.

## Fix and Prevention

- Keep the strict whole-file guard for modules other than `lib.rs`.
- For `lib.rs`, scan the bounded cloud-config command region and reject Genesis, Supabase or localStorage references there.
- Run the full Rust suite after every command-wiring change; a keyring-only command must remain proven not to serialize keys.
