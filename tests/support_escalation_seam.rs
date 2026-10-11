//! The escalation seam, end-to-end: **backbone-support → the REAL backbone-project**.
//!   SESEAM-1 (Issue → delivery Project): `escalate_to_project` drives the REAL project write path to
//!            open a Project for the ticket's customer; `escalated_project_id` links a real
//!            `project.projects` row. Idempotent per issue.
//! This edge is a dev-dependency ONLY — the shipped support library depends on neither project nor GL,
//! keeping the two Tier-4 modules independent.
//!
//!   SESEAM-2 (no company scope): escalation needs no company — backbone-project takes none — so a
//!            request without a company-anchored scope escalates too, its outbox record echoing
//!            the nil company (ADR-0029: nothing keys a statement on the echo).
//!
//! Tenancy (ADR-0029): the port's and the outbox record's `company_id` is a legacy echo of the
//! ambient org scope's company when the composing service bound one, nil otherwise. SESEAM-1 binds
//! one, as a composing service does for every request.

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
        svc:
            backbone_project::application::service::project_write_service::ProjectWriteService::new(
                pool.clone(),
            ),
        pool: pool.clone(),
    };
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let customer = Uuid::new_v4();
    let sla = svc
        .create_sla(NewSla {
            name: "Standard".into(),
            is_default: true,
            priorities: vec![NewSlaPriority {
                priority: "high".into(),
                response_time_mins: 60,
                resolution_time_mins: 240,
            }],
        })
        .await
        .unwrap();
    let issue = svc
        .raise_issue(
            NewIssue {
                customer_id: Some(customer),
                subject: "Onsite repair needed".into(),
                description: None,
                priority: "high".into(),
                sla_id: Some(sla),
            },
            dt("2026-07-07T09:00:00Z"),
        )
        .await
        .unwrap();

    // A composing service binds a company-anchored scope; the port and the outbox echo it.
    let (project_id, again) = with_company_scope(&pool, company, async {
        let a = svc
            .escalate_to_project(issue, &project, &sink)
            .await
            .unwrap();
        let b = svc
            .escalate_to_project(issue, &project, &sink)
            .await
            .unwrap();
        (a, b)
    })
    .await;

    // A REAL project row exists, for the ticket's customer.
    let (cust, ptype, name): (Option<Uuid>, String, String) = sqlx::query_as(
        "SELECT customer_id, project_type::text, project_name FROM project.projects WHERE id=$1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        cust,
        Some(customer),
        "delivery project is for the ticket's customer"
    );
    assert_eq!(ptype, "external");
    assert!(
        name.contains(&issue.to_string()),
        "project names the originating issue"
    );
    // The issue links the project.
    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT escalated_project_id FROM support.issues WHERE id=$1")
            .bind(issue)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked, Some(project_id));

    // Idempotent: a second escalate opened no second project.
    assert_eq!(again, project_id);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project.projects WHERE customer_id=$1",
    )
    .bind(customer)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n, 1, "exactly one delivery project for the ticket");
}

/// SESEAM-2 — with no company in the ambient scope, escalation still opens the project; the
/// outbox record echoes the nil company instead of the escalation failing.
#[tokio::test]
async fn seseam2_escalation_needs_no_company_scope() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let project = RealProject {
        svc:
            backbone_project::application::service::project_write_service::ProjectWriteService::new(
                pool.clone(),
            ),
        pool: pool.clone(),
    };
    let customer = Uuid::new_v4();
    let sla = svc
        .create_sla(NewSla {
            name: "No scope".into(),
            is_default: false,
            priorities: vec![NewSlaPriority {
                priority: "low".into(),
                response_time_mins: 60,
                resolution_time_mins: 240,
            }],
        })
        .await
        .unwrap();
    let issue = svc
        .raise_issue(
            NewIssue {
                customer_id: Some(customer),
                subject: "No scope bound".into(),
                description: None,
                priority: "low".into(),
                sla_id: Some(sla),
            },
            dt("2026-07-07T09:00:00Z"),
        )
        .await
        .unwrap();

    let project_id = svc
        .escalate_to_project(issue, &project, &LoggingSink)
        .await
        .expect("escalation without a company scope opens the project");

    let linked: Option<Uuid> =
        sqlx::query_scalar("SELECT escalated_project_id FROM support.issues WHERE id=$1")
            .bind(issue)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked, Some(project_id));
    let echoed: Uuid = sqlx::query_scalar(
        "SELECT company_id FROM support.outbox_events WHERE event_type='IssueEscalated' AND aggregate_id=$1",
    )
    .bind(issue.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(echoed, Uuid::nil(), "no bound company: the outbox echoes nil");
}
