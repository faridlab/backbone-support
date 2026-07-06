//! Golden cases — the SLA-clock oracle. Exact deadline snapshots, fulfilled/failed verdicts, and the
//! pause-extends-the-clock rule. Clock verbs take a fixed `now` so the arithmetic is deterministic.

mod common;

use backbone_support::application::service::support_write_service::{
    NewIssue, NewSla, NewSlaPriority, SupportError, SupportWriteService,
};
use chrono::{DateTime, Utc};
use common::*;
use uuid::Uuid;

/// An SLA whose `high` priority promises a 60-min first response and a 240-min (4h) resolution.
async fn sla_high_4h(svc: &SupportWriteService, company: Uuid) -> Uuid {
    svc.create_sla(NewSla {
        company_id: company, name: "Standard".into(), is_default: true,
        priorities: vec![NewSlaPriority { priority: "high".into(), response_time_mins: 60, resolution_time_mins: 240 }],
    }).await.unwrap()
}
fn high_issue(company: Uuid, sla: Uuid) -> NewIssue {
    NewIssue {
        company_id: company, customer_id: Some(Uuid::new_v4()), subject: "Printer down".into(),
        description: None, priority: "high".into(), sla_id: Some(sla),
    }
}

/// SGC-1 — raising a ticket snapshots concrete deadlines from the matching priority target.
#[tokio::test]
async fn sgc1_raise_snapshots_deadlines() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let sla = sla_high_4h(&svc, company).await;
    let issue = svc.raise_issue(high_issue(company, sla), dt("2026-07-07T09:00:00Z")).await.unwrap();

    let (status, agr, rb, resb): (String, String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
        "SELECT status::text, agreement_status::text, response_by, resolution_by FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "open");
    assert_eq!(agr, "first_response_due");
    assert_eq!(rb, dt("2026-07-07T10:00:00Z"), "response_by = opened + 60 min");
    assert_eq!(resb, dt("2026-07-07T13:00:00Z"), "resolution_by = opened + 240 min");
}

/// SGC-2 — resolving within the resolution deadline is fulfilled.
#[tokio::test]
async fn sgc2_resolve_within_deadline_fulfilled() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high_4h(&svc, company).await;
    let issue = svc.raise_issue(high_issue(company, sla), dt("2026-07-07T09:00:00Z")).await.unwrap();
    svc.record_first_response(issue, dt("2026-07-07T09:30:00Z")).await.unwrap();

    let fulfilled = svc.resolve_issue(issue, dt("2026-07-07T12:00:00Z"), &sink).await.unwrap();
    assert!(fulfilled, "resolved 12:00 < 13:00 deadline");
    let agr: String = sqlx::query_scalar("SELECT agreement_status::text FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(agr, "fulfilled");
}

/// SGC-3 — resolving after the resolution deadline is failed.
#[tokio::test]
async fn sgc3_resolve_past_deadline_failed() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high_4h(&svc, company).await;
    let issue = svc.raise_issue(high_issue(company, sla), dt("2026-07-07T09:00:00Z")).await.unwrap();

    let fulfilled = svc.resolve_issue(issue, dt("2026-07-07T14:00:00Z"), &sink).await.unwrap();
    assert!(!fulfilled, "resolved 14:00 > 13:00 deadline");
    let agr: String = sqlx::query_scalar("SELECT agreement_status::text FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(agr, "failed");
}

/// SGC-4 — a hold does not count against the SLA: pausing 2h pushes the resolution deadline out 2h, so
/// a ticket resolved past its ORIGINAL deadline but within the extended one is still fulfilled.
#[tokio::test]
async fn sgc4_pause_extends_the_clock() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high_4h(&svc, company).await;
    let issue = svc.raise_issue(high_issue(company, sla), dt("2026-07-07T09:00:00Z")).await.unwrap();
    // First response on time (09:30 < 10:00) so the response leg is met — isolates the resolution clock.
    svc.record_first_response(issue, dt("2026-07-07T09:30:00Z")).await.unwrap();
    // Original resolution_by = 13:00. Hold 10:00 → 12:00 (2h).
    svc.pause_sla(issue, dt("2026-07-07T10:00:00Z")).await.unwrap();
    svc.resume_sla(issue, dt("2026-07-07T12:00:00Z")).await.unwrap();

    let (resb, paused): (DateTime<Utc>, i32) = sqlx::query_as(
        "SELECT resolution_by, total_paused_mins FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(resb, dt("2026-07-07T15:00:00Z"), "resolution_by pushed out by the 2h hold");
    assert_eq!(paused, 120);

    // Resolve at 14:00 — past the ORIGINAL 13:00 but within the extended 15:00 → fulfilled.
    let fulfilled = svc.resolve_issue(issue, dt("2026-07-07T14:00:00Z"), &sink).await.unwrap();
    assert!(fulfilled, "the hold time is not counted against the SLA");
}

/// SGC-5 — the input guards: an SLA needs a priority with resolution >= response; an issue needs a
/// subject; an issue whose priority has no SLA target is rejected.
#[tokio::test]
async fn sgc5_validation() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let company = Uuid::new_v4();

    let no_priority = svc.create_sla(NewSla {
        company_id: company, name: "X".into(), is_default: false, priorities: vec![],
    }).await;
    assert!(matches!(no_priority, Err(SupportError::Invalid(_))), "SLA needs a priority target");

    let bad_times = svc.create_sla(NewSla {
        company_id: company, name: "X".into(), is_default: false,
        priorities: vec![NewSlaPriority { priority: "low".into(), response_time_mins: 240, resolution_time_mins: 60 }],
    }).await;
    assert!(matches!(bad_times, Err(SupportError::Invalid(_))), "resolution must be >= response");

    let sla = sla_high_4h(&svc, company).await;
    let no_subject = svc.raise_issue(NewIssue {
        company_id: company, customer_id: None, subject: "  ".into(), description: None,
        priority: "high".into(), sla_id: Some(sla),
    }, dt("2026-07-07T09:00:00Z")).await;
    assert!(matches!(no_subject, Err(SupportError::Invalid(_))), "issue needs a subject");

    // The SLA only targets `high`; a `low` ticket bound to it has no target.
    let no_target = svc.raise_issue(NewIssue {
        company_id: company, customer_id: None, subject: "Slow".into(), description: None,
        priority: "low".into(), sla_id: Some(sla),
    }, dt("2026-07-07T09:00:00Z")).await;
    assert!(matches!(no_target, Err(SupportError::Invalid(_))), "no SLA target for this priority");
}
