# RCA: Qwen LOTUSDIS test path type not in scope

Date: 2026-09-30  
Risk: LOW — Rust test harness only  
Status: Fixed; verified by the staged 55-clip FUNG worker pilot

## Symptom

The Rust library test target did not compile after adding the staged Qwen
LOTUSDIS pilot test.

## Evidence

- `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib` reported
  `cannot find type Path in this scope` at the new test's `Path::new` call.
- The Rust test module imports `PathBuf`, but not `std::path::Path`.

## Root Cause

The test used an unqualified `Path` identifier that was not imported in its
module. This prevented the test target from compiling before any tests ran.

## Why the issue escaped detection

The import was added after the earlier helper tests. The final Cargo compile
was the first check of this integrated Rust test module.

## Proposed prevention

Use the fully qualified `std::path::Path::new` at this single call site, then
rerun the consolidated Rust and worker tests.
