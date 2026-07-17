//! The hand-authored support write path (user-owned; survives regen).
//!
//! An SLA-clock ticketing engine + warranty claims. Posts NO GL and owns no money. The load-bearing
//! logic is the SLA clock: an Issue binds an SLA at raise time, snapshots concrete `response_by` /
//! `resolution_by` deadlines from the matching priority, can be PAUSED (the paused span is added back to
//! the deadlines on resume), and on resolve is judged `fulfilled` iff it beat the (pause-adjusted)
//! resolution deadline. The one outbound seam — escalate a ticket into a real backbone-project delivery
//! Project — runs through `ProjectPort` (zero normal Cargo edge), idempotent per issue.
//!
//! Clock verbs take an explicit `now: DateTime<Utc>` so the deadline math is deterministic under test.

use backbone_orm::company_scope;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    IssueRepository, NewIssueRow, NewSlaPriorityRow, NewSlaRow, NewWarrantyClaimRow,
    ServiceLevelAgreementRepository, ServiceLevelPriorityRepository, WarrantyClaimRepository,
};

use super::support_events::*;
use super::support_ports::*;

#[derive(Debug, thiserror::Error)]
pub enum SupportError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("project rejected: {0}")]
    ProjectRejected(String),
}

pub struct NewSlaPriority {
    pub priority: String, // issue_priority variant
    pub response_time_mins: i32,
    pub resolution_time_mins: i32,
}
pub struct NewSla {
    pub company_id: Uuid,
    pub name: String,
    pub is_default: bool,
    pub priorities: Vec<NewSlaPriority>,
}

pub struct NewIssue {
    pub company_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub subject: String,
    pub description: Option<String>,
    pub priority: String, // issue_priority variant
    pub sla_id: Option<Uuid>,
}

pub struct NewWarrantyClaim {
    pub company_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub item_id: Uuid,
    pub serial_no: Option<String>,
    pub warranty_expiry: Option<DateTime<Utc>>,
    pub issue_id: Option<Uuid>,
    pub description: Option<String>,
}

pub struct SupportWriteService {
    pool: PgPool,
    slas: ServiceLevelAgreementRepository,
    sla_priorities: ServiceLevelPriorityRepository,
    issues: IssueRepository,
    warranty_claims: WarrantyClaimRepository,
}

impl SupportWriteService {
    pub fn new(pool: PgPool) -> Self {
        let slas = ServiceLevelAgreementRepository::new(pool.clone());
        let sla_priorities = ServiceLevelPriorityRepository::new(pool.clone());
        let issues = IssueRepository::new(pool.clone());
        let warranty_claims = WarrantyClaimRepository::new(pool.clone());
        Self { pool, slas, sla_priorities, issues, warranty_claims }
    }

    /// Define an SLA with its per-priority first-response + resolution targets. Each target's resolution
    /// time must be >= its response time; at least one priority row is required.
    pub async fn create_sla(&self, s: NewSla) -> Result<Uuid, SupportError> {
        if s.name.trim().is_empty() {
            return Err(SupportError::Invalid("SLA needs a name".into()));
        }
        if s.priorities.is_empty() {
            return Err(SupportError::Invalid("an SLA needs at least one priority target".into()));
        }
        for p in &s.priorities {
            if p.response_time_mins < 0 || p.resolution_time_mins < 0 {
                return Err(SupportError::Invalid("SLA times must be non-negative".into()));
            }
            if p.resolution_time_mins < p.response_time_mins {
                return Err(SupportError::Invalid("resolution target must be >= response target".into()));
            }
        }
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008): the company is on the DTO, so bind it explicitly onto our own
        // transaction — every statement below then runs with `app.company_id` set, for request and
        // non-request (job) callers alike.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, s.company_id).await?;
        self.slas.insert_sla(&mut tx, &NewSlaRow {
            id,
            company_id: s.company_id,
            name: &s.name,
            is_default: s.is_default,
        }).await?;
        for p in &s.priorities {
            self.sla_priorities.insert_priority(&mut tx, &NewSlaPriorityRow {
                id: Uuid::new_v4(),
                sla_id: id,
                priority: &p.priority,
                response_time_mins: p.response_time_mins,
                resolution_time_mins: p.resolution_time_mins,
            }).await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// Raise a ticket. It binds an SLA (the given one, else the company default) and snapshots concrete
    /// `response_by` / `resolution_by` deadlines from the matching priority target. With no SLA the clock
    /// is untracked (deadlines NULL).
    pub async fn raise_issue(&self, i: NewIssue, now: DateTime<Utc>) -> Result<Uuid, SupportError> {
        if i.subject.trim().is_empty() {
            return Err(SupportError::Invalid("issue needs a subject".into()));
        }
        // RLS scope (ADR-0008): the company is on the DTO — bind it for the whole body so the SLA
        // lookups and the issue insert all run with `app.company_id` set. The explicit `company_id`
        // filter below stays as defense-in-depth.
        let company = i.company_id;
        company_scope::with_company_scope(Some(company), async move {
            // Resolve the SLA: explicit, else the company default.
            let sla_id: Option<Uuid> = match i.sla_id {
                Some(id) => Some(id),
                None => self.slas.find_default_id(&self.pool, i.company_id).await?,
            };
            // Snapshot deadlines from the matching priority target (if an SLA is bound).
            let (mut response_by, mut resolution_by) = (None, None);
            if let Some(sid) = sla_id {
                let target = self
                    .sla_priorities
                    .find_target(&self.pool, sid, &i.priority)
                    .await?
                    .ok_or(SupportError::Invalid("SLA has no target for this priority".into()))?;
                response_by = Some(now + Duration::minutes(target.response_time_mins as i64));
                resolution_by = Some(now + Duration::minutes(target.resolution_time_mins as i64));
            }
            let id = Uuid::new_v4();
            self.issues.insert_issue(&self.pool, &NewIssueRow {
                id,
                company_id: i.company_id,
                customer_id: i.customer_id,
                subject: &i.subject,
                description: i.description.as_deref(),
                priority: &i.priority,
                sla_id,
                opened_at: now,
                response_by,
                resolution_by,
            }).await?;
            Ok(id)
        })
        .await
    }

    /// Record the first response — moves the clock from first-response to resolution tracking, and
    /// **judges the response leg**: a response after `response_by` breaches the SLA. The breach is
    /// persisted (`response_breached`) and immediately flips agreement_status to `failed`, because a
    /// missed first response fails the SLA even if the resolution later lands on time (a met resolution
    /// no longer masks a blown response — completeness council 2026-07-07). Judged inside the gated
    /// UPDATE against the row's live `response_by` (pause-adjusted), never a stale read.
    pub async fn record_first_response(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by the issue id alone — there is no company
        // argument to scope from. The write rides the request-dedicated connection (which carries the
        // caller's `app.company_id`), so RLS fences it: another company's issue simply is not updated.
        let moved = self.issues.record_first_response(&self.pool, issue_id, now).await?;
        if moved != 1 {
            return Err(SupportError::InvalidState("issue is not awaiting a first response"));
        }
        Ok(())
    }

    /// Pause the SLA clock (ticket on hold — waiting on customer / third party).
    pub async fn pause_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        // RLS scope (ADR-0008), ID-only pattern — see `record_first_response`.
        let moved = self.issues.pause(&self.pool, issue_id, now).await?;
        if moved != 1 {
            return Err(SupportError::InvalidState("only an open/replied issue can be paused"));
        }
        Ok(())
    }

    /// Resume the SLA clock — add the paused span back to the outstanding deadlines so a hold never
    /// counts against the SLA. Restores the running status (open if not yet responded, else replied).
    pub async fn resume_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        // RLS scope (ADR-0008), ID-only pattern: this method carries NO company — not on a parameter,
        // and the locking read cannot be split out of the transaction. So the tx is bound to the
        // AMBIENT task-local scope. Under HTTP that is the request's company. An EVENT-driven or job
        // CALLER MUST wrap this call in `with_company_scope(Some(event.company_id))`, otherwise the
        // `FOR UPDATE` read below is fenced to nothing and the resume fails closed.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_current_company(&mut tx).await?;
        let row = self.issues.lock_on_hold(&mut tx, issue_id).await?;
        let row = match row {
            Some(r) => r,
            None => {
                tx.rollback().await?;
                return Err(SupportError::InvalidState("issue is not on hold"));
            }
        };
        let paused_mins = (now - row.paused_at).num_minutes().max(0);
        let running = if row.first_responded_at.is_some() { "replied" } else { "open" };
        // Extend the resolution deadline by the paused span; extend the response deadline too while the
        // first response is still outstanding.
        self.issues.apply_resume(&mut tx, issue_id, paused_mins as i32, running).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Resolve a ticket. Judged `fulfilled` iff resolved at/before the (pause-adjusted) resolution
    /// deadline — a ticket with no SLA is always fulfilled (no promise to breach). Emits `IssueResolved`.
    pub async fn resolve_issue(
        &self,
        issue_id: Uuid,
        now: DateTime<Utc>,
        sink: &dyn SupportEventSink,
    ) -> Result<bool, SupportError> {
        // RLS scope (ADR-0008), ID-only pattern — see `record_first_response`: the read is fenced by the
        // request-dedicated connection, so another company's issue is simply not found.
        let issue = self
            .issues
            .find_state(&self.pool, issue_id)
            .await?
            .ok_or(SupportError::NotFound("issue"))?;
        if issue.status == "on_hold" {
            return Err(SupportError::InvalidState("resume the issue before resolving"));
        }
        if issue.status != "open" && issue.status != "replied" {
            return Err(SupportError::InvalidState("only an open/replied issue can be resolved"));
        }
        let company_id = issue.company_id;
        // Compute the verdict INSIDE the gated UPDATE from the row's LIVE resolution_by — not from the
        // snapshot read above (which is only for the friendly on_hold/not-found errors). A concurrent
        // pause+resume that commits between that read and here extends resolution_by and restores an
        // open/replied status, so a read-then-write split would stamp a stale verdict (a met SLA marked
        // `failed`). Judging in the same statement as the transition closes that race (maturity council
        // 2026-07-07). A ticket with no SLA (NULL resolution_by) is always fulfilled.
        // fulfilled requires BOTH legs met: the resolution is within resolution_by, AND the first
        // response leg is not breached — either an already-recorded on-time response, or (no response
        // recorded yet) resolving within response_by counts as responding in time. A ticket that blew
        // its response deadline — recorded late, or still unanswered past response_by — fails even if
        // the resolution lands on time (completeness council 2026-07-07).
        // Having read the issue's company off the row above, bind it EXPLICITLY for the gated UPDATE —
        // so the write is fenced for non-request callers (jobs, event subscribers) too, not only under
        // an ambient request scope.
        let fulfilled = company_scope::with_company_scope(
            Some(company_id),
            self.issues.resolve(&self.pool, issue_id, now),
        )
        .await?
        .ok_or(SupportError::InvalidState("issue is no longer resolvable"))?;
        sink.publish(&SupportEvent::IssueResolved(IssueResolved { issue_id, company_id, fulfilled }));
        Ok(fulfilled)
    }

    /// Close a resolved ticket (terminal).
    pub async fn close_issue(&self, issue_id: Uuid) -> Result<(), SupportError> {
        // RLS scope (ADR-0008), ID-only pattern — see `record_first_response`.
        let moved = self.issues.close(&self.pool, issue_id).await?;
        if moved != 1 {
            return Err(SupportError::InvalidState("only a resolved issue can be closed"));
        }
        Ok(())
    }

    /// Escalate a ticket into a real backbone-project delivery Project (drives `ProjectPort`, idempotent
    /// per issue). Transition-gates on `escalated_project_id IS NULL` so a ticket escalates **once**.
    /// Emits `IssueEscalated`.
    pub async fn escalate_to_project(
        &self,
        issue_id: Uuid,
        project: &dyn ProjectPort,
        sink: &dyn SupportEventSink,
    ) -> Result<Uuid, SupportError> {
        // RLS scope (ADR-0008), ID-only pattern — see `record_first_response`. The company read off this
        // row is bound explicitly onto the escalation transaction below.
        let issue = self
            .issues
            .find_escalation_candidate(&self.pool, issue_id)
            .await?
            .ok_or(SupportError::NotFound("issue"))?;
        if let Some(pid) = issue.escalated_project_id {
            return Ok(pid); // already escalated
        }
        if issue.status == "resolved" || issue.status == "closed" {
            return Err(SupportError::InvalidState("a resolved/closed issue cannot be escalated"));
        }
        let company_id = issue.company_id;
        let customer_id: Uuid = issue
            .customer_id
            .ok_or(SupportError::Invalid("issue has no customer to open a project for".into()))?;

        // Open the delivery project (idempotent per issue on the project side).
        let ack = project
            .open_delivery_project(&ProjectFromIssue {
                company_id, issue_id, customer_id, subject: issue.subject,
            })
            .await
            .map_err(|r| SupportError::ProjectRejected(r.code))?;

        // Gate: claim the escalation exactly once, and stage the event in the SAME tx (outbox rollout plan,
        // P2): backbone-project subscribes to IssueEscalated to open the delivery project, so a crash between
        // the CAS and the in-proc publish must not drop it.
        // RLS scope (ADR-0008): bind the company read off the issue row EXPLICITLY onto this tx, so the
        // CAS and the outbox stage are fenced regardless of who drives the escalation.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, company_id).await?;
        let moved = self.issues.claim_escalation(&mut tx, issue_id, ack.project_id).await?;
        if moved != 1 {
            tx.rollback().await?;
            let pid: Uuid = company_scope::with_company_scope(
                Some(company_id),
                self.issues.fetch_escalated_project_id(&self.pool, issue_id),
            )
            .await?;
            return Ok(pid);
        }
        let event = SupportEvent::IssueEscalated(IssueEscalated { issue_id, company_id, project_id: ack.project_id });
        let record = backbone_outbox::OutboxRecord::new(
            "IssueEscalated", "Issue", issue_id.to_string(),
            serde_json::to_value(&event).map_err(|e| SupportError::Invalid(e.to_string()))?,
            chrono::Utc::now(),
        );
        backbone_outbox::outbox::stage(&mut *tx, "support", &record)
            .await.map_err(|e| SupportError::Invalid(format!("outbox stage: {e}")))?;
        tx.commit().await?;
        sink.publish(&event);
        Ok(ack.project_id)
    }

    /// File a warranty claim. Coverage is computed at file time: `is_under_warranty = claim_date <=
    /// warranty_expiry` (an unknown/absent expiry is out of warranty). Emits `WarrantyClaimFiled`.
    pub async fn file_warranty_claim(
        &self,
        c: NewWarrantyClaim,
        now: DateTime<Utc>,
        sink: &dyn SupportEventSink,
    ) -> Result<Uuid, SupportError> {
        let under = c.warranty_expiry.map(|e| now <= e).unwrap_or(false);
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008): the company is on the DTO — bind it for the insert.
        let claim = NewWarrantyClaimRow {
            id,
            company_id: c.company_id,
            customer_id: c.customer_id,
            item_id: c.item_id,
            serial_no: c.serial_no.as_deref(),
            claim_date: now,
            warranty_expiry: c.warranty_expiry,
            is_under_warranty: under,
            issue_id: c.issue_id,
            description: c.description.as_deref(),
        };
        company_scope::with_company_scope(
            Some(c.company_id),
            self.warranty_claims.insert_claim(&self.pool, &claim),
        )
        .await?;
        sink.publish(&SupportEvent::WarrantyClaimFiled(WarrantyClaimFiled { claim_id: id, company_id: c.company_id, is_under_warranty: under }));
        Ok(id)
    }

    /// Adjudicate an open warranty claim (accept or reject) with a resolution note.
    pub async fn resolve_warranty_claim(
        &self,
        claim_id: Uuid,
        accepted: bool,
        resolution: Option<String>,
    ) -> Result<(), SupportError> {
        let status = if accepted { "accepted" } else { "rejected" };
        // RLS scope (ADR-0008), ID-only pattern — see `record_first_response`.
        let moved = self
            .warranty_claims
            .adjudicate(&self.pool, claim_id, status, resolution.as_deref())
            .await?;
        if moved != 1 {
            return Err(SupportError::InvalidState("only an open claim can be adjudicated"));
        }
        Ok(())
    }
}
