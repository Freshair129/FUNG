---
version: "0.1.0c"
created_at: "2026-10-19"
last_update: "2026-10-20"
status: "beta"
superseded_by: null
attributes:
  domain: "operations"
  doc_type: "documentation-index"
  scope: "FUNG operational procedures"
  language: "Thai-first"
---

# FUNG operations

Runbooks describe repeatable operator steps. They do not by themselves prove a deployment, demo, provider integration or release succeeded.

| Runbook | Purpose |
| --- | --- |
| [Web production deployment](web-production-deployment.md) | Vercel static frontend deployment boundary and deployment procedure |
| [Presentation/demo runbook](demos/2026-07-24-presentation-runbook.md) | Product presentation flow and demo setup; historical readiness statements must be rechecked |
| [FUNG workflow MCP](fung-workflow-mcp.md) | Run local functional audio acceptance and inspect per-stage timings without controlling FUNG UI |

For release-specific acceptance, use [releases](../releases/README.md). For actual result evidence, use [verification](../verification/README.md).
