# backbone-support — PRD

Tier 4b · Service Delivery pillar · posts **no GL** · no ledger seam.

## Why
An Indonesia SMB that sells or services products needs a ticket queue with **SLA clocks** — the one
thing that turns "we'll get to it" into an accountable promise — plus warranty claims against
serialized items. This is the lean helpdesk core: capture an Issue, bind an SLA, track response +
resolution deadlines (pausing the clock while waiting on the customer), and judge each ticket
fulfilled/failed. Nobody wins a deal on a prettier ticket queue, so the goal is **clean, small,
correct** — the SLA clock is the value, everything else is trimmed.

## Scope (KEEP — pillar brief §4/§6/§7)
- **ServiceLevelAgreement (+ ServiceLevelPriority)** — the SLA master: per priority, a first-response
  and a resolution target (minutes). This is the load-bearing logic (brief §6.3).
- **Issue** — a ticket that READS its customer (party, logical FK), binds an SLA at raise time and
  **snapshots concrete deadlines** (`response_by` / `resolution_by`), tracks `agreement_status`
  (first_response_due → resolution_due → fulfilled/failed), can be put **on hold** (pausing the clock;
  the paused span is added back to the deadlines on resume), and can optionally be **escalated** into a
  real backbone-project delivery Project.
- **WarrantyClaim** — a claim against an `item_id` + `serial_no`; coverage (`is_under_warranty`) is
  computed from the item's warranty expiry (read from stock/catalog, logical FK), then adjudicated
  accepted/rejected.
- **The one outbound seam** — `escalate_to_project` opens a real **backbone-project** delivery Project
  for the ticket's customer, through a port (zero normal Cargo edge; the two Tier-4 modules stay
  independent). Optional — a ticket resolves fine without ever escalating.

## Non-goals (CUT / DEFER — brief §4)
- **Email-to-ticket ingestion** (multichannel) — manual entry first.
- **AMC (annual maintenance contract)** tracking on warranty — likely belongs to billing subscriptions.
- Support portal / search source, canned responses, CSAT surveys.
- Any GL posting — support owns no money.

## Success criteria
- Deadlines are snapshotted exactly from the priority target; a hold never counts against the SLA
  (golden cases).
- Resolve judges fulfilled iff within the pause-adjusted resolution deadline.
- A ticket escalates to exactly one real delivery project, idempotently (proven against REAL
  backbone-project).
- Zero normal Cargo edge to project; survives a full codegen regen (§5).
