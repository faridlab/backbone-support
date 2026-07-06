//! Shared test helpers: a live pool + a fake idempotent-per-issue project port (for golden/integrity),
//! the REAL backbone-project adapter (for the escalation seam), and an event-capturing sink. Clock verbs
//! take an explicit `now`, so `dt()` builds fixed timestamps. Fresh random ids per test.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use backbone_support::application::service::support_events::{SupportEvent, SupportEventSink};
pub use backbone_support::application::service::support_events::LoggingSink;
use backbone_support::application::service::support_ports::{
    ProjectAck, ProjectFromIssue, ProjectPort, SupportRejected,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_support".into())
}
pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}
/// A fixed UTC timestamp from an RFC3339 string (e.g. `dt("2026-07-07T09:00:00Z")`).
pub fn dt(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

/// A sink that records every published support event.
#[derive(Clone, Default)]
pub struct CapturingSink {
    pub events: Arc<Mutex<Vec<SupportEvent>>>,
}
impl CapturingSink {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn events(&self) -> Vec<SupportEvent> {
        self.events.lock().unwrap().clone()
    }
}
impl SupportEventSink for CapturingSink {
    fn publish(&self, event: &SupportEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// A fake project seam that opens a delivery project idempotently per issue.
#[derive(Clone, Default)]
pub struct FakeProject {
    pub opened: Arc<Mutex<HashMap<Uuid, Uuid>>>,
    pub calls: Arc<Mutex<u32>>,
}
impl FakeProject {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn open_count(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
}
#[async_trait::async_trait]
impl ProjectPort for FakeProject {
    async fn open_delivery_project(&self, req: &ProjectFromIssue) -> Result<ProjectAck, SupportRejected> {
        *self.calls.lock().unwrap() += 1;
        let mut m = self.opened.lock().unwrap();
        let pid = *m.entry(req.issue_id).or_insert_with(Uuid::new_v4);
        Ok(ProjectAck { project_id: pid })
    }
}

/// ACL over the REAL backbone-project: open a delivery Project from a ticket. Idempotent per issue via a
/// stable project_name marker (`Support ISSUE-<issue_id>`); on a repeat, return the existing project.
pub struct RealProject {
    pub svc: backbone_project::application::service::project_write_service::ProjectWriteService,
    pub pool: PgPool,
}
#[async_trait::async_trait]
impl ProjectPort for RealProject {
    async fn open_delivery_project(&self, req: &ProjectFromIssue) -> Result<ProjectAck, SupportRejected> {
        use backbone_project::application::service::project_write_service::NewProject;
        let name = format!("Support ISSUE-{} — {}", req.issue_id, req.subject);
        // Idempotent: if a project already exists for this issue, return it.
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM project.projects WHERE company_id=$1 AND project_name=$2 AND (metadata->>'deleted_at') IS NULL LIMIT 1")
            .bind(req.company_id).bind(&name).fetch_optional(&self.pool).await
            .map_err(|e| SupportRejected { code: "project_lookup_failed".into(), message: e.to_string() })?;
        if let Some(id) = existing {
            return Ok(ProjectAck { project_id: id });
        }
        let res = self.svc.create_project(NewProject {
            company_id: req.company_id,
            project_name: name,
            project_type: "external".into(),
            customer_id: Some(req.customer_id),
            source_so_id: None,
            currency: None,
        }).await;
        res.map(|id| ProjectAck { project_id: id })
            .map_err(|e| SupportRejected { code: "project_rejected".into(), message: format!("{e:?}") })
    }
}
