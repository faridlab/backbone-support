# ADR-001 — Support boundary, the SLA clock, and the escalation seam

Status: accepted · 2026-07-07 · Tier 4b (Service Delivery pillar; posts no GL)

## Context
Support is the leaf-est module in the pillar: it READS other contexts (party for the customer, stock/
catalog for warranty) and posts nothing to the ledger. Its defining value is not the ticket queue —
that is commodity — but the **SLA clock**: a promise (first-response + resolution deadlines) that a
ticket either keeps or breaks, honestly, even when work is paused waiting on the customer.

## Decision
1. **SLA targets are snapshotted onto the ticket at raise time.** `raise_issue` resolves the SLA (given
   or company default), reads the matching priority's targets, and writes concrete `response_by` /
   `resolution_by` deadlines onto the Issue. A later SLA edit never rewrites an in-flight ticket's
   promise. A ticket with no SLA is untracked (NULL deadlines) and always fulfilled.
2. **A hold never counts against the SLA.** `pause_sla` stops the clock; `resume_sla` adds the paused
   span back to the outstanding deadlines (`resolution_by` always, `response_by` only while the first
   response is still due) and accumulates `total_paused_mins`. `resolve_issue` judges `fulfilled` iff
   `now <= resolution_by` — the pause-adjusted deadline. Resume is `FOR UPDATE`-serialized on the ticket.
3. **One outbound seam, via a port + events (zero normal Cargo edge).** `escalate_to_project →
   ProjectPort::open_delivery_project` opens a real backbone-project delivery Project; a composing
   service wires the real project behind the port. Support never imports backbone-project — the two
   Tier-4 modules stay independent (brief §7). Idempotent + transition-gated on `escalated_project_id IS
   NULL` (escalate once).
4. **Warranty coverage is a computed read, not owned data.** Support does not own serial/warranty master
   data; it reads `item_id` + `serial_no` + `warranty_expiry` (logical FKs) and records
   `is_under_warranty = claim_date <= warranty_expiry` at file time.
5. **Posts no GL.** Support owns no money and has no ledger seam.

## Consequences
- The helpdesk is self-contained; turn support off and no ledger changes. Escalation only *feeds* the
  delivery pillar (backbone-project), never the ledger.
- Proven end-to-end (`tests/support_escalation_seam.rs` drives the REAL project write path) and survives
  regen (§5).

## Parking lot (each with a gate)
- **First-response SLA judged nowhere** — FIXED (completeness council 2026-07-07): `response_by` was
  snapshotted + pause-maintained but never judged, so a late (or absent) first response left a ticket
  `fulfilled` — a false "SLA met" on the headline number. `record_first_response` now judges the response
  leg (`response_breached` + `failed`), and `resolve_issue` requires BOTH legs met (IP-7,
  proven-by-revert).
- **`is_default` SLA uniqueness unenforced** — two defaults could exist; `raise_issue`'s `LIMIT 1`
  lookup picks arbitrarily. Gate: a partial-unique index on `(company_id) WHERE is_default`.
- **Resolve verdict judged from a stale read** — FIXED (maturity council 2026-07-07): `resolve_issue`
  computed `fulfilled` from a snapshot read then wrote it under a status-only gate, so a pause+resume
  racing into the read→write gap could stamp a met SLA as `failed`. The verdict is now computed inside
  the gated UPDATE from the row's live `resolution_by` (one atomic statement; IP-6, proven-by-revert).
- **Escalate-before-gate crash window** — `escalate_to_project` calls the project port before the gate;
  the gate makes the *record* escalate-once, but "at most one downstream project" is delegated to the
  adapter deduping on a stable per-issue marker (the `RealProject` adapter looks up by a
  `Support ISSUE-<id>` project name — best-effort, NOT UNIQUE-backed). Gate: a go-live outbox/saga + a
  UNIQUE business key on the project side.
- **Response-SLA breach not surfaced** — the clock tracks the *resolution* outcome as
  `agreement_status`; a missed first response is derivable (`first_responded_at > response_by`) but not
  recorded as a distinct state. Gate: a reporting need for first-response-breach.
- **Email-to-ticket ingestion, AMC contracts, support portal, CSAT** — deferred (PRD non-goals).
