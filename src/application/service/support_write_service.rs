//! The hand-authored support write path (user-owned; survives regen).
//!
//! An SLA-clock ticketing engine + warranty claims. Posts NO GL and owns no money. The load-bearing
//! logic is the SLA clock: an Issue binds an SLA at raise time, snapshots concrete `response_by` /
//! `resolution_by` deadlines from the matching priority, can be PAUSED (the paused span is added back to
//! the deadlines on resume), and on resolve is judged `fulfilled` iff it beat the (pause-adjusted)
//! resolution deadline. The one outbound seam — escalate a ticket into a real backbone-project delivery
//! Project — runs through `ProjectPort` (zero normal Cargo edge), idempotent per issue.
//!
//! Tenancy: none, by design (ADR-0029). The module is tenant-agnostic — no tenant key on any
//! write, no scope binding of its own. The COMPOSING service owns the posture: when it
//! mounts these routes under an auth middleware that binds a row scope (e.g.
//! `with_org_request_scope`), the database fence owns tenant isolation; a deployment that mounts
//! them unfenced gets an unfenced module. Multi-statement transactions here relay an ambient
//! scope onto the transaction when one is bound, and stay plain otherwise. The ONE exception is
//! the escalation seam: the sibling port's delivery-Project row still carries the sibling's
//! legacy owner-company key, sourced fail-closed from the ambient org scope.
//!
//! Clock verbs take an explicit `now: DateTime<Utc>` so the deadline math is deterministic under test.

use backbone_orm::org_scope;
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
    /// The escalation seam needs the sibling's legacy owner-company key (backbone-project is
    /// not tenant-agnostic) and the outbox mirror is company-keyed, but no org scope carrying
    /// a company node was bound on this request. Fails closed rather than guessing a key.
    #[error("no company in the ambient org scope — escalation requires one (composition-installed tenancy, ADR-0029)")]
    NoCompanyScope,
}

pub struct NewSlaPriority {
    pub priority: String, // issue_priority variant
    pub response_time_mins: i32,
    pub resolution_time_mins: i32,
}
pub struct NewSla {
    pub name: String,
    pub is_default: bool,
    pub priorities: Vec<NewSlaPriority>,
}

pub struct NewIssue {
    pub customer_id: Option<Uuid>,
    pub subject: String,
    pub description: Option<String>,
    pub priority: String, // issue_priority variant
    pub sla_id: Option<Uuid>,
}

pub struct NewWarrantyClaim {
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
        Self {
            pool,
            slas,
            sla_priorities,
            issues,
            warranty_claims,
        }
    }

    /// Define an SLA with its per-priority first-response + resolution targets. Each target's resolution
    /// time must be >= its response time; at least one priority row is required.
    pub async fn create_sla(&self, s: NewSla) -> Result<Uuid, SupportError> {
        if s.name.trim().is_empty() {
            return Err(SupportError::Invalid("SLA needs a name".into()));
        }
        if s.priorities.is_empty() {
            return Err(SupportError::Invalid(
                "an SLA needs at least one priority target".into(),
            ));
        }
        for p in &s.priorities {
            if p.response_time_mins < 0 || p.resolution_time_mins < 0 {
                return Err(SupportError::Invalid(
                    "SLA times must be non-negative".into(),
                ));
            }
            if p.resolution_time_mins < p.response_time_mins {
                return Err(SupportError::Invalid(
                    "resolution target must be >= response target".into(),
                ));
            }
        }
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        // Propagate the ambient request scope, when one is bound, onto this transaction: the
        // repositories' scoped helpers ride the request-dedicated connection, but this pool
        // transaction does not, and rows a deployment's fence decorates are invisible to an
        // unscoped connection. Binding the ambient scope relay-only keeps the module
        // posture-agnostic — unfenced deployments have no ambient scope and skip this entirely.
        if let Some(scope) = org_scope::current_org_scope() {
            org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        self.slas
            .insert_sla(
                &mut tx,
                &NewSlaRow {
                    id,
                    name: &s.name,
                    is_default: s.is_default,
                },
            )
            .await?;
        for p in &s.priorities {
            self.sla_priorities
                .insert_priority(
                    &mut tx,
                    &NewSlaPriorityRow {
                        id: Uuid::new_v4(),
                        sla_id: id,
                        priority: &p.priority,
                        response_time_mins: p.response_time_mins,
                        resolution_time_mins: p.resolution_time_mins,
                    },
                )
                .await?;
        }
        tx.commit().await?;
        Ok(id)
    }

    /// Raise a ticket. It binds an SLA (the given one, else the default) and snapshots concrete
    /// `response_by` / `resolution_by` deadlines from the matching priority target. With no SLA the clock
    /// is untracked (deadlines NULL).
    pub async fn raise_issue(&self, i: NewIssue, now: DateTime<Utc>) -> Result<Uuid, SupportError> {
        if i.subject.trim().is_empty() {
            return Err(SupportError::Invalid("issue needs a subject".into()));
        }
        // Resolve the SLA: explicit, else the default. Under a composing service's row fence
        // (RLS), the scope bound on the request connection limits what the default lookup sees;
        // with no fence mounted, this is a plain lookup.
        let sla_id: Option<Uuid> = match i.sla_id {
            Some(id) => Some(id),
            None => self.slas.find_default_id(&self.pool).await?,
        };
        // Snapshot deadlines from the matching priority target (if an SLA is bound).
        let (mut response_by, mut resolution_by) = (None, None);
        if let Some(sid) = sla_id {
            let target = self
                .sla_priorities
                .find_target(&self.pool, sid, &i.priority)
                .await?
                .ok_or(SupportError::Invalid(
                    "SLA has no target for this priority".into(),
                ))?;
            response_by = Some(now + Duration::minutes(target.response_time_mins as i64));
            resolution_by = Some(now + Duration::minutes(target.resolution_time_mins as i64));
        }
        let id = Uuid::new_v4();
        self.issues
            .insert_issue(
                &self.pool,
                &NewIssueRow {
                    id,
                    customer_id: i.customer_id,
                    subject: &i.subject,
                    description: i.description.as_deref(),
                    priority: &i.priority,
                    sla_id,
                    opened_at: now,
                    response_by,
                    resolution_by,
                },
            )
            .await?;
        Ok(id)
    }

    /// Record the first response — moves the clock from first-response to resolution tracking, and
    /// **judges the response leg**: a response after `response_by` breaches the SLA. The breach is
    /// persisted (`response_breached`) and immediately flips agreement_status to `failed`, because a
    /// missed first response fails the SLA even if the resolution later lands on time (a met resolution
    /// no longer masks a blown response — completeness council 2026-07-07). Judged inside the gated
    /// UPDATE against the row's live `response_by` (pause-adjusted), never a stale read.
    pub async fn record_first_response(
        &self,
        issue_id: Uuid,
        now: DateTime<Utc>,
    ) -> Result<(), SupportError> {
        let moved = self
            .issues
            .record_first_response(&self.pool, issue_id, now)
            .await?;
        if moved != 1 {
            return Err(SupportError::InvalidState(
                "issue is not awaiting a first response",
            ));
        }
        Ok(())
    }

    /// Pause the SLA clock (ticket on hold — waiting on customer / third party).
    pub async fn pause_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        let moved = self.issues.pause(&self.pool, issue_id, now).await?;
        if moved != 1 {
            return Err(SupportError::InvalidState(
                "only an open/replied issue can be paused",
            ));
        }
        Ok(())
    }

    /// Resume the SLA clock — add the paused span back to the outstanding deadlines so a hold never
    /// counts against the SLA. Restores the running status (open if not yet responded, else replied).
    pub async fn resume_sla(&self, issue_id: Uuid, now: DateTime<Utc>) -> Result<(), SupportError> {
        let mut tx = self.pool.begin().await?;
        // The locking read + the deadline extension are one transaction on the pool, not a
        // request-connection statement — relay the ambient request scope onto it, when one is
        // bound, so a decorated deployment's fence sees the row.
        if let Some(scope) = org_scope::current_org_scope() {
            org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        let row = self.issues.lock_on_hold(&mut tx, issue_id).await?;
        let row = match row {
            Some(r) => r,
            None => {
                tx.rollback().await?;
                return Err(SupportError::InvalidState("issue is not on hold"));
            }
        };
        let paused_mins = (now - row.paused_at).num_minutes().max(0);
        let running = if row.first_responded_at.is_some() {
            "replied"
        } else {
            "open"
        };
        // Extend the resolution deadline by the paused span; extend the response deadline too while the
        // first response is still outstanding.
        self.issues
            .apply_resume(&mut tx, issue_id, paused_mins as i32, running)
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
        let issue = self
            .issues
            .find_state(&self.pool, issue_id)
            .await?
            .ok_or(SupportError::NotFound("issue"))?;
        if issue.status == "on_hold" {
            return Err(SupportError::InvalidState(
                "resume the issue before resolving",
            ));
        }
        if issue.status != "open" && issue.status != "replied" {
            return Err(SupportError::InvalidState(
                "only an open/replied issue can be resolved",
            ));
        }
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
        let fulfilled = self
            .issues
            .resolve(&self.pool, issue_id, now)
            .await?
            .ok_or(SupportError::InvalidState("issue is no longer resolvable"))?;
        sink.publish(&SupportEvent::IssueResolved(IssueResolved {
            issue_id,
            fulfilled,
        }));
        Ok(fulfilled)
    }

    /// Close a resolved ticket (terminal).
    pub async fn close_issue(&self, issue_id: Uuid) -> Result<(), SupportError> {
        let moved = self.issues.close(&self.pool, issue_id).await?;
        if moved != 1 {
            return Err(SupportError::InvalidState(
                "only a resolved issue can be closed",
            ));
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
        let issue = self
            .issues
            .find_escalation_candidate(&self.pool, issue_id)
            .await?
            .ok_or(SupportError::NotFound("issue"))?;
        if let Some(pid) = issue.escalated_project_id {
            return Ok(pid); // already escalated
        }
        if issue.status == "resolved" || issue.status == "closed" {
            return Err(SupportError::InvalidState(
                "a resolved/closed issue cannot be escalated",
            ));
        }
        let customer_id: Uuid = issue.customer_id.ok_or(SupportError::Invalid(
            "issue has no customer to open a project for".into(),
        ))?;
        // The delivery-project seam is a cross-module port whose sibling (backbone-project) is
        // not tenant-agnostic: its Project rows still carry the legacy owner-company key. That
        // key is the SIBLING's domain parameter (composition-installed tenancy, ADR-0029), not
        // a fence of this module's — source it from the ambient org scope and fail closed when
        // the request carries no company-anchored scope. The same key feeds the company-keyed
        // outbox mirror row staged below.
        let company_id = org_scope::current_org_scope()
            .and_then(|s| s.legacy_company_id())
            .ok_or(SupportError::NoCompanyScope)?;

        // Open the delivery project (idempotent per issue on the project side).
        let ack = project
            .open_delivery_project(&ProjectFromIssue {
                company_id,
                issue_id,
                customer_id,
                subject: issue.subject,
            })
            .await
            .map_err(|r| SupportError::ProjectRejected(r.code))?;

        // Gate: claim the escalation exactly once, and stage the event in the SAME tx (outbox rollout plan,
        // P2): backbone-project subscribes to IssueEscalated to open the delivery project, so a crash between
        // the CAS and the in-proc publish must not drop it.
        let mut tx = self.pool.begin().await?;
        // Relay the ambient request scope onto this transaction so the CAS and the outbox stage
        // run fenced under a decorated deployment, regardless of who drives the escalation.
        if let Some(scope) = org_scope::current_org_scope() {
            org_scope::bind_org_scope_on(&mut *tx, &scope).await?;
        }
        let moved = self
            .issues
            .claim_escalation(&mut tx, issue_id, ack.project_id)
            .await?;
        if moved != 1 {
            tx.rollback().await?;
            let pid = self
                .issues
                .fetch_escalated_project_id(&self.pool, issue_id)
                .await?;
            return Ok(pid);
        }
        let event = SupportEvent::IssueEscalated(IssueEscalated {
            issue_id,
            project_id: ack.project_id,
        });
        let record = backbone_outbox::OutboxRecord::new(
            "IssueEscalated",
            "Issue",
            issue_id.to_string(),
            company_id,
            serde_json::to_value(&event).map_err(|e| SupportError::Invalid(e.to_string()))?,
            chrono::Utc::now(),
        );
        backbone_outbox::outbox::stage(&mut *tx, "support", &record)
            .await
            .map_err(|e| SupportError::Invalid(format!("outbox stage: {e}")))?;
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
        let claim = NewWarrantyClaimRow {
            id,
            customer_id: c.customer_id,
            item_id: c.item_id,
            serial_no: c.serial_no.as_deref(),
            claim_date: now,
            warranty_expiry: c.warranty_expiry,
            is_under_warranty: under,
            issue_id: c.issue_id,
            description: c.description.as_deref(),
        };
        self.warranty_claims
            .insert_claim(&self.pool, &claim)
            .await?;
        sink.publish(&SupportEvent::WarrantyClaimFiled(WarrantyClaimFiled {
            claim_id: id,
            is_under_warranty: under,
        }));
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
        let moved = self
            .warranty_claims
            .adjudicate(&self.pool, claim_id, status, resolution.as_deref())
            .await?;
        if moved != 1 {
            return Err(SupportError::InvalidState(
                "only an open claim can be adjudicated",
            ));
        }
        Ok(())
    }
}
