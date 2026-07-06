# backbone-support — BRD

## Documents
ServiceLevelAgreement (+ ServiceLevelPriority) · Issue (SLA clocks) · WarrantyClaim. Own Postgres
schema `support`. Posts NO GL, owns no money.

## Business rules

**BR-1 (define an SLA).** `create_sla` requires a name + ≥ 1 priority target; each target's
`resolution_time_mins` must be ≥ its `response_time_mins` and both non-negative. `is_default` marks the
SLA applied to a ticket that names none.

**BR-2 (raise a ticket).** `raise_issue` requires a subject, binds an SLA (the given one, else the
company default), and **snapshots concrete deadlines** from the matching priority target: `response_by =
opened_at + response_time`, `resolution_by = opened_at + resolution_time`. A ticket bound to an SLA that
has no target for its priority is rejected. With no SLA, the clock is untracked (deadlines NULL). Status
`open`, agreement_status `first_response_due`.

**BR-3 (first response).** `record_first_response` moves an **open** ticket → `replied`, stamping
`first_responded_at`, and **judges the response leg** against the live `response_by`: a reply after the
deadline sets `response_breached` and flips agreement_status → `failed` (a missed first response fails
the SLA even if the resolution later lands on time — completeness council 2026-07-07). An on-time reply
→ `resolution_due`.

**BR-4 (pause / resume — the clock rule).** `pause_sla` puts an open/replied ticket **on hold** (clock
stopped), stamping `paused_at`. `resume_sla` adds the paused span back to the outstanding deadlines
(`resolution_by` always; `response_by` too while the first response is still outstanding) and restores
the running status — **so a hold never counts against the SLA**. Serialized with `FOR UPDATE` on the
ticket row.

**BR-5 (resolve).** `resolve_issue` requires an open/replied ticket (an on-hold ticket must be resumed
first) and judges it **`fulfilled` iff BOTH legs are met**: resolution within `resolution_by` AND the
first response not breached (an on-time recorded reply, or — no reply yet — resolving within
`response_by`); resolving late with no reply breaches. A ticket with no SLA is always fulfilled (no
promise to breach). The verdict is computed **inside** the gated
transition UPDATE from the row's live `resolution_by`, so a pause+resume racing the resolve can never
stamp a stale verdict — a met SLA is never reported breached (maturity council 2026-07-07). Sets
agreement_status fulfilled/failed, status `resolved`. `close_issue` moves resolved → closed (terminal).
Emits `IssueResolved{fulfilled}`.

**BR-6 (escalate — the one outbound seam).** `escalate_to_project` requires a **customer** and a
non-terminal ticket, drives `ProjectPort::open_delivery_project` (idempotent per issue), then
transition-gates on `escalated_project_id IS NULL` so a ticket escalates **at most once** (a retry
returns the existing project). Emits `IssueEscalated`.

**BR-7 (warranty).** `file_warranty_claim` computes `is_under_warranty = claim_date <= warranty_expiry`
(unknown/absent expiry → not covered) at file time. `resolve_warranty_claim` adjudicates an **open**
claim accepted/rejected with a note. Emits `WarrantyClaimFiled{is_under_warranty}`.

## Events
`IssueResolved`, `IssueEscalated`, `WarrantyClaimFiled`.

## Deferred (with reason)
Email-to-ticket ingestion (manual entry first), AMC contracts (→ billing subscriptions), support
portal / search source, CSAT, canned responses.
