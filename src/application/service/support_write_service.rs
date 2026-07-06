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

use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Row};
use uuid::Uuid;

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
}

impl SupportWriteService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            r#"INSERT INTO support.service_level_agreements (id, company_id, name, is_default, is_active)
               VALUES ($1,$2,$3,$4,true)"#,
        )
        .bind(id).bind(s.company_id).bind(&s.name).bind(s.is_default)
        .execute(&mut *tx)
        .await?;
        for p in &s.priorities {
            sqlx::query(
                r#"INSERT INTO support.service_level_priorities
                     (id, sla_id, priority, response_time_mins, resolution_time_mins)
                   VALUES ($1,$2,$3::issue_priority,$4,$5)"#,
            )
            .bind(Uuid::new_v4()).bind(id).bind(&p.priority).bind(p.response_time_mins).bind(p.resolution_time_mins)
            .execute(&mut *tx)
            .await?;
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
        // Resolve the SLA: explicit, else the company default.
        let sla_id: Option<Uuid> = match i.sla_id {
            Some(id) => Some(id),
            None => sqlx::query_scalar(
                r#"SELECT id FROM support.service_level_agreements
                   WHERE company_id=$1 AND is_default=true AND is_active=true
                     AND (metadata->>'deleted_at') IS NULL LIMIT 1"#,
            )
            .bind(i.company_id)
            .fetch_optional(&self.pool)
            .await?,
        };
        // Snapshot deadlines from the matching priority target (if an SLA is bound).
        let (mut response_by, mut resolution_by) = (None, None);
        if let Some(sid) = sla_id {
            let target = sqlx::query(
                r#"SELECT response_time_mins, resolution_time_mins FROM support.service_level_priorities
                   WHERE sla_id=$1 AND priority=$2::issue_priority"#,
            )
            .bind(sid)
            .bind(&i.priority)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(SupportError::Invalid("SLA has no target for this priority".into()))?;
            let resp: i32 = target.get("response_time_mins");
            let reso: i32 = target.get("resolution_time_mins");
            response_by = Some(now + Duration::minutes(resp as i64));
            resolution_by = Some(now + Duration::minutes(reso as i64));
        }
        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO support.issues
                 (id, company_id, customer_id, subject, description, priority, sla_id, status,
                  agreement_status, opened_at, response_by, resolution_by, total_paused_mins)
               VALUES ($1,$2,$3,$4,$5,$6::issue_priority,$7,'open'::issue_status,
                       'first_response_due'::agreement_status,$8,$9,$10,0)"#,
        )
        .bind(id).bind(i.company_id).bind(i.customer_id).bind(&i.subject).bind(&i.description)
        .bind(&i.priority).bind(sla_id).bind(now).bind(response_by).bind(resolution_by)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// Record the first response — moves the clock from first-response to resolution tracking, and
    /// **judges the response leg**: a response after `response_by` breaches the SLA. The breach is
    /// persisted (`response_breached`) and immediately flips agreement_status to `failed`, because a
    /// missed first response fails the SLA even if the resolution later lands on time (a met resolution
    /// no longer masks a blown response — completeness council 2026-07-07). Judged inside the gated
    /// UPDATE against the row's live `response_by` (pause-adjusted), never a stale read.
    pub async fn record_first_response(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        let moved = sqlx::query(
            r#"UPDATE support.issues
               SET first_responded_at=$2, status='replied'::issue_status,
                   response_breached = (response_by IS NOT NULL AND $2 > response_by),
                   agreement_status = CASE WHEN response_by IS NOT NULL AND $2 > response_by
                                           THEN 'failed'::agreement_status
                                           ELSE 'resolution_due'::agreement_status END
               WHERE id=$1 AND status='open'::issue_status AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(issue_id).bind(now)
        .execute(&self.pool)
        .await?;
        if moved.rows_affected() != 1 {
            return Err(SupportError::InvalidState("issue is not awaiting a first response"));
        }
        Ok(())
    }

    /// Pause the SLA clock (ticket on hold — waiting on customer / third party).
    pub async fn pause_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        let moved = sqlx::query(
            r#"UPDATE support.issues
               SET status='on_hold'::issue_status, paused_at=$2
               WHERE id=$1 AND status IN ('open','replied') AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(issue_id).bind(now)
        .execute(&self.pool)
        .await?;
        if moved.rows_affected() != 1 {
            return Err(SupportError::InvalidState("only an open/replied issue can be paused"));
        }
        Ok(())
    }

    /// Resume the SLA clock — add the paused span back to the outstanding deadlines so a hold never
    /// counts against the SLA. Restores the running status (open if not yet responded, else replied).
    pub async fn resume_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            r#"SELECT paused_at, first_responded_at FROM support.issues
               WHERE id=$1 AND status='on_hold'::issue_status FOR UPDATE"#,
        )
        .bind(issue_id)
        .fetch_optional(&mut *tx)
        .await?;
        let row = match row {
            Some(r) => r,
            None => {
                tx.rollback().await?;
                return Err(SupportError::InvalidState("issue is not on hold"));
            }
        };
        let paused_at: DateTime<Utc> = row.get("paused_at");
        let responded: Option<DateTime<Utc>> = row.get("first_responded_at");
        let paused_mins = (now - paused_at).num_minutes().max(0);
        let running = if responded.is_some() { "replied" } else { "open" };
        // Extend the resolution deadline by the paused span; extend the response deadline too while the
        // first response is still outstanding.
        sqlx::query(
            r#"UPDATE support.issues
               SET status=$3::issue_status, paused_at=NULL,
                   total_paused_mins = total_paused_mins + $2,
                   resolution_by = resolution_by + ($2 * interval '1 minute'),
                   response_by = CASE WHEN first_responded_at IS NULL
                                      THEN response_by + ($2 * interval '1 minute') ELSE response_by END
               WHERE id=$1"#,
        )
        .bind(issue_id).bind(paused_mins as i32).bind(running)
        .execute(&mut *tx)
        .await?;
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
        let issue = sqlx::query(
            r#"SELECT company_id, status::text AS status, resolution_by FROM support.issues
               WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SupportError::NotFound("issue"))?;
        let status: String = issue.get("status");
        if status == "on_hold" {
            return Err(SupportError::InvalidState("resume the issue before resolving"));
        }
        if status != "open" && status != "replied" {
            return Err(SupportError::InvalidState("only an open/replied issue can be resolved"));
        }
        let company_id: Uuid = issue.get("company_id");
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
        let row = sqlx::query(
            r#"UPDATE support.issues
               SET status='resolved'::issue_status, resolved_at=$2,
                   agreement_status = CASE
                       WHEN (resolution_by IS NULL OR $2 <= resolution_by)
                            AND NOT response_breached
                            AND (first_responded_at IS NOT NULL OR response_by IS NULL OR $2 <= response_by)
                       THEN 'fulfilled'::agreement_status
                       ELSE 'failed'::agreement_status END
               WHERE id=$1 AND status IN ('open','replied')
               RETURNING (agreement_status = 'fulfilled'::agreement_status) AS fulfilled"#,
        )
        .bind(issue_id).bind(now)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SupportError::InvalidState("issue is no longer resolvable"))?;
        let fulfilled: bool = row.get("fulfilled");
        sink.publish(&SupportEvent::IssueResolved(IssueResolved { issue_id, company_id, fulfilled }));
        Ok(fulfilled)
    }

    /// Close a resolved ticket (terminal).
    pub async fn close_issue(&self, issue_id: Uuid) -> Result<(), SupportError> {
        let moved = sqlx::query(
            r#"UPDATE support.issues SET status='closed'::issue_status
               WHERE id=$1 AND status='resolved'::issue_status AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(issue_id)
        .execute(&self.pool)
        .await?;
        if moved.rows_affected() != 1 {
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
        let issue = sqlx::query(
            r#"SELECT company_id, customer_id, subject, status::text AS status, escalated_project_id
               FROM support.issues WHERE id=$1 AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SupportError::NotFound("issue"))?;
        if let Some(pid) = issue.get::<Option<Uuid>, _>("escalated_project_id") {
            return Ok(pid); // already escalated
        }
        let status: String = issue.get("status");
        if status == "resolved" || status == "closed" {
            return Err(SupportError::InvalidState("a resolved/closed issue cannot be escalated"));
        }
        let company_id: Uuid = issue.get("company_id");
        let customer_id: Uuid = issue
            .get::<Option<Uuid>, _>("customer_id")
            .ok_or(SupportError::Invalid("issue has no customer to open a project for".into()))?;

        // Open the delivery project (idempotent per issue on the project side).
        let ack = project
            .open_delivery_project(&ProjectFromIssue {
                company_id, issue_id, customer_id, subject: issue.get("subject"),
            })
            .await
            .map_err(|r| SupportError::ProjectRejected(r.code))?;

        // Gate: claim the escalation exactly once.
        let moved = sqlx::query(
            r#"UPDATE support.issues SET escalated_project_id=$2
               WHERE id=$1 AND escalated_project_id IS NULL"#,
        )
        .bind(issue_id).bind(ack.project_id)
        .execute(&self.pool)
        .await?;
        if moved.rows_affected() != 1 {
            let pid: Uuid = sqlx::query_scalar(
                "SELECT escalated_project_id FROM support.issues WHERE id=$1")
                .bind(issue_id).fetch_one(&self.pool).await?;
            return Ok(pid);
        }
        sink.publish(&SupportEvent::IssueEscalated(IssueEscalated { issue_id, company_id, project_id: ack.project_id }));
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
        sqlx::query(
            r#"INSERT INTO support.warranty_claims
                 (id, company_id, customer_id, item_id, serial_no, claim_date, warranty_expiry,
                  is_under_warranty, status, issue_id, description)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,'open'::warranty_status,$9,$10)"#,
        )
        .bind(id).bind(c.company_id).bind(c.customer_id).bind(c.item_id).bind(&c.serial_no)
        .bind(now).bind(c.warranty_expiry).bind(under).bind(c.issue_id).bind(&c.description)
        .execute(&self.pool)
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
        let moved = sqlx::query(
            r#"UPDATE support.warranty_claims SET status=$2::warranty_status, resolution=$3
               WHERE id=$1 AND status='open'::warranty_status AND (metadata->>'deleted_at') IS NULL"#,
        )
        .bind(claim_id).bind(status).bind(&resolution)
        .execute(&self.pool)
        .await?;
        if moved.rows_affected() != 1 {
            return Err(SupportError::InvalidState("only an open claim can be adjudicated"));
        }
        Ok(())
    }
}
