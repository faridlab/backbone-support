//! The escalation seam, end-to-end: **backbone-support → the REAL backbone-project**.
//!   SESEAM-1 (Issue → delivery Project): `escalate_to_project` drives the REAL project write path to
//!            open a Project for the ticket's customer; `escalated_project_id` links a real
//!            `project.projects` row. Idempotent per issue.
//! This edge is a dev-dependency ONLY — the shipped support library depends on neither project nor GL,
//! keeping the two Tier-4 modules independent.

mod common;

use backbone_support::application::service::support_write_service::{
    NewIssue, NewSla, NewSlaPriority, SupportWriteService,
};
use common::*;
use uuid::Uuid;

/// SESEAM-1 — escalating a ticket opens a REAL backbone-project delivery Project for its customer.
#[tokio::test]
async fn seseam1_issue_escalates_to_real_project() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let project = RealProject {
        svc: backbone_project::application::service::project_write_service::ProjectWriteService::new(pool.clone()),
        pool: pool.clone(),
    };
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let sla = svc.create_sla(NewSla {
        company_id: company, name: "Standard".into(), is_default: true,
        priorities: vec![NewSlaPriority { priority: "high".into(), response_time_mins: 60, resolution_time_mins: 240 }],
    }).await.unwrap();
    let issue = svc.raise_issue(NewIssue {
        company_id: company, customer_id: Some(customer), subject: "Onsite repair needed".into(),
        description: None, priority: "high".into(), sla_id: Some(sla),
    }, dt("2026-07-07T09:00:00Z")).await.unwrap();

    let project_id = svc.escalate_to_project(issue, &project, &sink).await.unwrap();

    // A REAL project row exists, for the ticket's customer.
    let (cust, ptype, name): (Option<Uuid>, String, String) = sqlx::query_as(
        "SELECT customer_id, project_type::text, project_name FROM project.projects WHERE id=$1")
        .bind(project_id).fetch_one(&pool).await.unwrap();
    assert_eq!(cust, Some(customer), "delivery project is for the ticket's customer");
    assert_eq!(ptype, "external");
    assert!(name.contains(&issue.to_string()), "project names the originating issue");
    // The issue links the project.
    let linked: Option<Uuid> = sqlx::query_scalar(
        "SELECT escalated_project_id FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(linked, Some(project_id));

    // Idempotent: a second escalate opens no second project.
    let again = svc.escalate_to_project(issue, &project, &sink).await.unwrap();
    assert_eq!(again, project_id);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project.projects WHERE company_id=$1 AND customer_id=$2")
        .bind(company).bind(customer).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1, "exactly one delivery project for the ticket");
}
