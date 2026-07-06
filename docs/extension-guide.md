# backbone-support — Extension Guide

## Public surface (stable)
- **Events** (`application::service::support_events`): `IssueResolved` (carries `fulfilled`),
  `IssueEscalated`, `WarrantyClaimFiled` (carries `is_under_warranty`), the `SupportEvent` union, and
  `SupportEventSink` (a consuming service supplies its own sink — bus, outbox, …).
- **Port** (`application::service::support_ports`): `ProjectPort` + its DTOs (`ProjectFromIssue`,
  `ProjectAck`, `SupportRejected`) — the escalation seam a composing service implements over
  backbone-project. **Zero normal Cargo edge**; the two Tier-4 modules stay independent.
- **Write path** (`application::service::support_write_service::SupportWriteService`): the guarded verbs
  (`create_sla`, `raise_issue`, `record_first_response`, `pause_sla`, `resume_sla`, `resolve_issue`,
  `close_issue`, `escalate_to_project`, `file_warranty_claim`, `resolve_warranty_claim`). Clock verbs
  take an explicit `now`, so a consumer controls the SLA arithmetic (and tests are deterministic).

## How a consuming service wires the escalation seam
Implement `ProjectPort` over the real `backbone_project::...::ProjectWriteService` (open a delivery
Project for the ticket's customer, deduped on a stable per-issue key for idempotency), and pass it to
`escalate_to_project`. See `tests/common/mod.rs` (`RealProject`) for the reference adapter.

## Not a contract
- The 12 generated CRUD endpoints per entity (`BackboneCrudHandler`) are convenience scaffolding. Do
  **not** mutate ticket/SLA-clock state through the generic PATCH surface — it bypasses the deadline
  snapshots, the pause-adjustment, and the escalate-once gate. Use `SupportWriteService`.
- `// <<< CUSTOM` blocks inside generated files preserve local edits only; not a cross-module extension
  point.

## Invariants a consumer must not break
- A ticket escalates at most once; the adapter's per-issue key SHOULD be UNIQUE-backed for the
  crash-window idempotency to hold (parked).
- Deadlines are snapshotted at raise time; do not recompute them from the live SLA.
- A pause must be matched by a resume before resolving — `resolve_issue` refuses an on-hold ticket.
