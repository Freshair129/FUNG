# Context Loading Rules

Goal: minimize context while maximizing correctness.

Priority:
1. Product Spec
2. Architecture
3. Feature docs
4. Relevant source files
5. Tests

Load strategy:
- Load only documents related to the current task.
- Prefer summaries before full documents.
- Expand context only when blocked.
- Never load unrelated modules.

Task mapping:
UX -> Product, UX docs, Design System
Backend -> Architecture, API, DB
Frontend -> UX + Components
Bug -> Error logs + affected modules + tests
Review -> Diff + impacted docs
