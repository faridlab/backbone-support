# backbone-support — FSD

## Entities
ServiceLevelAgreement (`is_default`, `is_active`) · ServiceLevelPriority (`priority`,
`response_time_mins`, `resolution_time_mins`) · Issue (`customer_id`/`sla_id`/`escalated_project_id`
logical FKs, `status`, `agreement_status`, `opened_at`, `response_by`, `resolution_by`,
`first_responded_at`, `response_breached`, `resolved_at`, `paused_at`, `total_paused_mins`) ·
WarrantyClaim (`item_id`,
`serial_no`, `warranty_expiry`, `is_under_warranty`, `status`). Enums: IssuePriority {low, medium, high,
urgent}, IssueStatus {open, replied, on_hold, resolved, closed}, AgreementStatus {first_response_due,
resolution_due, fulfilled, failed}, WarrantyStatus {open, accepted, rejected, closed}.

## Write path (`SupportWriteService`, hand-authored, user-owned)
- `create_sla` / `raise_issue(now)` / `record_first_response(now)`
- `pause_sla(now)` / `resume_sla(now)` — the clock engine (`FOR UPDATE`-serialized resume)
- `resolve_issue(now, sink)` → fulfilled/failed vs the pause-adjusted `resolution_by`
- `close_issue`
- `escalate_to_project(&dyn ProjectPort, sink)` → the seam; idempotent + transition-gated
- `file_warranty_claim(now, sink)` / `resolve_warranty_claim`

Clock verbs take an explicit `now: DateTime<Utc>` (deterministic deadline math). Errors: `SupportError
{Db, NotFound, InvalidState, Invalid, ProjectRejected}`.

## Seam (port — zero normal Cargo edge)
- **Escalate → project (proven, SESEAM-1):** `escalate_to_project` drives the REAL backbone-project write
  path to open a delivery Project for the ticket's customer; `escalated_project_id` links a real
  `project.projects` row. Idempotent per issue. ADR-001. The two Tier-4 modules stay independent — the
  edge is a dev-dependency only.
- **Inbound:** none — Issue/WarrantyClaim *read* Customer (party) + Item/serial/warranty (stock/catalog)
  as logical FKs; support posts no GL.

## Test oracle
`support_golden_cases` (5: raise snapshots deadlines, resolve-within → fulfilled, resolve-past → failed,
pause-extends-the-clock, validation), `integrity_probes` (7: escalate idempotent, escalate-requires-
customer, resolved-terminal, no-SLA-always-fulfilled, warranty-coverage-computed, IP-6 resolve verdict
atomic with a racing pause+resume, IP-7 first-response breach fails the SLA), `support_escalation_seam`
(1: issue → REAL project delivery Project, idempotent) + §5 round-trip. **13 tests.**

> The generated `integration_tests.rs` hits an external HTTP server (`API_BASE_URL`, default
> `127.0.0.1:3000`) and is environmental scaffolding, not part of this module's correctness gate; the
> hand-authored oracle above + §5 is the gate.
