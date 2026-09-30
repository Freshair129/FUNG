# RCA: Thai CTC test module loader failure

Date: 2026-09-30  
Risk: LOW — test harness only  
Status: Fixed; verified by the consolidated Python test suite

## Symptom

The new Thai CTC unit test failed before executing tests under Python 3.12.

## Evidence

- `py -3.12 tests/thaiCtcAlignment.test.py` raised `AttributeError` in
  `dataclasses._is_type` while decorating `AlignmentWord`.
- The test used `importlib.util.module_from_spec` and called `exec_module`
  without first registering the module in `sys.modules`.

## Root Cause

The test loader executed a dataclass-bearing module without the normal import
registration step, so Python's dataclass type lookup could not find the module
namespace.

## Why the issue escaped detection

The first test run was the initial execution of this new test file; no earlier
test had exercised its custom loader.

## Proposed prevention

Import the helper module through Python's regular import mechanism and keep the
scripts directory on the test process's module search path.
