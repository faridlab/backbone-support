# backbone-support — business flows & golden cases

## Flow: raise → respond → (pause/resume) → resolve, and optionally escalate
```
create_sla (targets per priority)
   │
   ▼  raise_issue → snapshot response_by / resolution_by from the priority target (agreement first_response_due)
   │
   ▼  record_first_response → replied / resolution_due
   │
   ▼  pause_sla … resume_sla → add the paused span back to the deadlines (a hold never counts vs SLA)
   │
   ▼  resolve_issue → fulfilled iff now <= (pause-adjusted) resolution_by → IssueResolved{fulfilled}
   │
   └▶ escalate_to_project (optional) → ProjectPort → REAL backbone-project Project → IssueEscalated
```
Support posts NO GL and has no ledger seam; the only outbound arrow spawns delivery work.

Separate flow: `file_warranty_claim` → `is_under_warranty = claim_date <= warranty_expiry` →
`resolve_warranty_claim` (accepted/rejected).

## Golden cases (`tests/support_golden_cases.rs`)
- **SGC-1 — raise snapshots deadlines.** high priority, 60-min response / 240-min resolution, opened
  09:00 → `response_by = 10:00`, `resolution_by = 13:00`, agreement `first_response_due`.
- **SGC-2 — resolve within → fulfilled.** Resolved 12:00 < 13:00 → `fulfilled`.
- **SGC-3 — resolve past → failed.** Resolved 14:00 > 13:00 → `failed`.
- **SGC-4 — pause extends the clock.** Hold 10:00→12:00 (2h) pushes `resolution_by` to 15:00; resolved
  14:00 (past the original 13:00) is still `fulfilled` — the hold is not counted against the SLA.
- **SGC-5 — validation.** SLA needs a priority target with resolution ≥ response; issue needs a subject;
  a priority with no SLA target is rejected.

## Integrity probes (`tests/integrity_probes.rs`)
- **IP-1 — escalate idempotent.** Retry returns the same project, drives the project seam once.
- **IP-2 — escalate requires customer.** No customer → refused before the seam.
- **IP-3 — resolved terminal.** A resolved ticket can't be re-resolved / paused / escalated; only closed.
- **IP-4 — no-SLA always fulfilled.** An untracked ticket (no deadlines) resolves fulfilled.
- **IP-5 — warranty coverage computed.** In-window → under warranty; past-expiry / unknown → not.
- **IP-6 — resolve verdict atomic with the deadline (maturity).** A pause+resume forced into resolve's
  read→write gap extends the deadline; the fulfilled/failed verdict must reflect the extended deadline
  (computed inside the gated UPDATE), never a stale pre-extension read.
- **IP-7 — first-response breach fails the SLA (completeness).** A late first response sets
  `response_breached` + `failed`; a met resolution can't launder it back to fulfilled; resolving late
  with no reply at all also breaches; on-time-both → fulfilled.

## Seam (`tests/support_escalation_seam.rs`)
- **SESEAM-1 — issue → REAL project.** `escalate_to_project` opens a real `project.projects` row for the
  customer, names the originating issue, links `escalated_project_id`; idempotent re-escalate opens no
  second project.

## §5 round-trip (`scripts/support_escalation_seam_roundtrip.sh`)
Regen (`--force`) leaves the seam files byte-identical; the oracle + seam re-run green.
