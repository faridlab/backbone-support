//! Integrity probes — the ticket/escalation/warranty invariants that keep the funnel honest under retry.

mod common;

use backbone_support::application::service::support_write_service::{
    NewIssue, NewSla, NewSlaPriority, NewWarrantyClaim, SupportError, SupportWriteService,
};
use common::*;
use uuid::Uuid;

async fn sla_high(svc: &SupportWriteService, company: Uuid) -> Uuid {
    svc.create_sla(NewSla {
        company_id: company, name: "Standard".into(), is_default: true,
        priorities: vec![NewSlaPriority { priority: "high".into(), response_time_mins: 60, resolution_time_mins: 240 }],
    }).await.unwrap()
}
fn an_issue(company: Uuid, sla: Option<Uuid>, customer: Option<Uuid>) -> NewIssue {
    NewIssue {
        company_id: company, customer_id: customer, subject: "Printer down".into(),
        description: None, priority: "high".into(), sla_id: sla,
    }
}

/// IP-1 — a ticket escalates to a delivery project AT MOST ONCE: a retry returns the same project and
/// drives the project seam only once.
#[tokio::test]
async fn ip1_escalate_idempotent() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let project = FakeProject::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high(&svc, company).await;
    let issue = svc.raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();

    let a = svc.escalate_to_project(issue, &project, &sink).await.unwrap();
    let b = svc.escalate_to_project(issue, &project, &sink).await.unwrap();
    assert_eq!(a, b, "same project on retry");
    assert_eq!(project.open_count(), 1, "project seam driven exactly once");
    let pid: Option<Uuid> = sqlx::query_scalar(
        "SELECT escalated_project_id FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(pid, Some(a));
}

/// IP-2 — a ticket with no customer cannot be escalated (fails closed before driving the seam).
#[tokio::test]
async fn ip2_escalate_requires_customer() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let project = FakeProject::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high(&svc, company).await;
    let issue = svc.raise_issue(an_issue(company, Some(sla), None), dt("2026-07-07T09:00:00Z")).await.unwrap();

    let err = svc.escalate_to_project(issue, &project, &sink).await.unwrap_err();
    assert!(matches!(err, SupportError::Invalid(_)), "no customer → refused");
    assert_eq!(project.open_count(), 0, "seam not driven");
}

/// IP-3 — a resolved ticket is terminal: it cannot be re-resolved, paused, or escalated; only closed.
#[tokio::test]
async fn ip3_resolved_terminal() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let project = FakeProject::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high(&svc, company).await;
    let issue = svc.raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();
    svc.resolve_issue(issue, dt("2026-07-07T10:00:00Z"), &sink).await.unwrap();

    assert!(matches!(svc.resolve_issue(issue, dt("2026-07-07T11:00:00Z"), &sink).await, Err(SupportError::InvalidState(_))));
    assert!(matches!(svc.pause_sla(issue, dt("2026-07-07T11:00:00Z")).await, Err(SupportError::InvalidState(_))));
    assert!(matches!(svc.escalate_to_project(issue, &project, &sink).await, Err(SupportError::InvalidState(_))));
    let _ = &pool;
    svc.close_issue(issue).await.unwrap(); // resolved → closed is allowed
}

/// IP-4 — a ticket with no SLA is always fulfilled on resolve (no promise to breach).
#[tokio::test]
async fn ip4_no_sla_always_fulfilled() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    // No default SLA seeded, none supplied → untracked ticket.
    let issue = svc.raise_issue(an_issue(company, None, Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();
    let (rb, resb): (Option<chrono::DateTime<chrono::Utc>>, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT response_by, resolution_by FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert!(rb.is_none() && resb.is_none(), "no SLA → no deadlines");

    let fulfilled = svc.resolve_issue(issue, dt("2026-07-09T09:00:00Z"), &sink).await.unwrap();
    assert!(fulfilled, "no deadline can't be breached");
}

/// IP-5 — warranty coverage is computed from expiry: in-window is under warranty, past-expiry / unknown
/// is not.
#[tokio::test]
async fn ip5_warranty_coverage_computed() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();

    let under = svc.file_warranty_claim(NewWarrantyClaim {
        company_id: company, customer_id: Some(Uuid::new_v4()), item_id: Uuid::new_v4(),
        serial_no: Some("SN-1".into()), warranty_expiry: Some(dt("2026-12-31T00:00:00Z")),
        issue_id: None, description: None,
    }, dt("2026-07-07T09:00:00Z"), &sink).await.unwrap();
    let expired = svc.file_warranty_claim(NewWarrantyClaim {
        company_id: company, customer_id: Some(Uuid::new_v4()), item_id: Uuid::new_v4(),
        serial_no: Some("SN-2".into()), warranty_expiry: Some(dt("2026-01-01T00:00:00Z")),
        issue_id: None, description: None,
    }, dt("2026-07-07T09:00:00Z"), &sink).await.unwrap();
    let unknown = svc.file_warranty_claim(NewWarrantyClaim {
        company_id: company, customer_id: Some(Uuid::new_v4()), item_id: Uuid::new_v4(),
        serial_no: None, warranty_expiry: None, issue_id: None, description: None,
    }, dt("2026-07-07T09:00:00Z"), &sink).await.unwrap();

    let cov = |id: Uuid, pool: sqlx::PgPool| async move {
        sqlx::query_scalar::<_, bool>("SELECT is_under_warranty FROM support.warranty_claims WHERE id=$1")
            .bind(id).fetch_one(&pool).await.unwrap()
    };
    assert!(cov(under, pool.clone()).await, "expiry in the future → under warranty");
    assert!(!cov(expired, pool.clone()).await, "past expiry → not under warranty");
    assert!(!cov(unknown, pool.clone()).await, "unknown expiry → not under warranty");
}

/// IP-7 (completeness council 2026-07-07) — a missed FIRST RESPONSE breaches the SLA, even when the
/// resolution later lands on time. `response_by` was snapshotted + pause-maintained but never judged;
/// a late reply used to leave `agreement_status='resolution_due'` and resolve stamped `fulfilled` — a
/// false "SLA met". Now a late first response sets `response_breached` + `failed`, and resolve requires
/// BOTH legs met. Also covers the NULL-blind case: resolving late with no reply at all breaches.
#[tokio::test]
async fn ip7_first_response_breach_fails_sla() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let sla = sla_high(&svc, company).await; // response_by = 10:00, resolution_by = 13:00

    // (a) Late first response (11:30 > 10:00) breaches the response leg immediately.
    let late = svc.raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();
    svc.record_first_response(late, dt("2026-07-07T11:30:00Z")).await.unwrap();
    let (agr, breached): (String, bool) = sqlx::query_as(
        "SELECT agreement_status::text, response_breached FROM support.issues WHERE id=$1")
        .bind(late).fetch_one(&pool).await.unwrap();
    assert!(breached, "a late first response is a breach");
    assert_eq!(agr, "failed", "a missed first response fails the SLA");
    // …and resolving within the resolution deadline does NOT launder it back to fulfilled.
    let fulfilled = svc.resolve_issue(late, dt("2026-07-07T12:00:00Z"), &sink).await.unwrap();
    assert!(!fulfilled, "a met resolution can't mask a blown first response");

    // (b) NULL-blind case: never responded, resolved past the response deadline → also failed.
    let silent = svc.raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();
    let f2 = svc.resolve_issue(silent, dt("2026-07-07T12:30:00Z"), &sink).await.unwrap();
    assert!(!f2, "resolving past response_by with no reply is a response breach");

    // (c) Control: on-time response + on-time resolution is fulfilled.
    let ok = svc.raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z")).await.unwrap();
    svc.record_first_response(ok, dt("2026-07-07T09:30:00Z")).await.unwrap();
    assert!(svc.resolve_issue(ok, dt("2026-07-07T12:00:00Z"), &sink).await.unwrap(), "both legs met → fulfilled");
}

/// IP-6 (maturity council 2026-07-07) — the resolve VERDICT races a concurrent pause+resume.
/// `resolve_issue` used to compute `fulfilled` from a snapshot read, then write it under a status-only
/// gate: a pause+resume slipping into that read→write gap extends `resolution_by` (and restores an open
/// status so the gate still matches), stamping a MET SLA as `failed`. The verdict is now computed inside
/// the gated UPDATE from the row's live `resolution_by`.
///
/// The interleave is forced deterministically: a transaction (standing in for a concurrent pause+resume)
/// holds the issue row lock across resolve's read→write gap — resolve reads the OLD deadline, blocks on
/// its UPDATE, the transaction extends the deadline and commits, then resolve's UPDATE proceeds. The
/// verdict must reflect the extended deadline (fulfilled), not the stale read (failed).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ip6_resolve_verdict_atomic_with_deadline() {
    use chrono::{DateTime, Utc};
    let pool = pool().await;
    let company = Uuid::new_v4();
    let setup = SupportWriteService::new(pool.clone());
    let sla = sla_high(&setup, company).await; // resolution_by = opened + 240m = 13:00
    let issue = setup
        .raise_issue(an_issue(company, Some(sla), Some(Uuid::new_v4())), dt("2026-07-07T09:00:00Z"))
        .await
        .unwrap();
    // On-time first response (09:30 < 10:00) so the response leg is met — isolates the resolution verdict.
    setup.record_first_response(issue, dt("2026-07-07T09:30:00Z")).await.unwrap();

    // A concurrent pause+resume, held in a transaction so it commits INSIDE resolve's read→write gap.
    let mut txb = pool.begin().await.unwrap();
    // pause: lock the row (uncommitted — resolve's plain read still sees the committed open/13:00 state).
    sqlx::query("UPDATE support.issues SET status='on_hold'::issue_status, paused_at=$2 WHERE id=$1")
        .bind(issue).bind(dt("2026-07-07T09:00:00Z")).execute(&mut *txb).await.unwrap();

    // resolve at 14:00 (past the ORIGINAL 13:00) — its gated UPDATE blocks on the row lock TX_B holds.
    let s1 = SupportWriteService::new(pool.clone());
    let resolve = tokio::spawn(async move {
        let sink = LoggingSink;
        s1.resolve_issue(issue, dt("2026-07-07T14:00:00Z"), &sink).await
    });
    // Give resolve time to take its snapshot read and reach the (now-blocked) UPDATE.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    // resume: 2h credit → resolution_by 15:00, restore open, then COMMIT — unblocking resolve's UPDATE.
    sqlx::query(
        r#"UPDATE support.issues SET status='open'::issue_status, paused_at=NULL, total_paused_mins=120,
           resolution_by = resolution_by + interval '120 minutes' WHERE id=$1"#)
        .bind(issue).execute(&mut *txb).await.unwrap();
    txb.commit().await.unwrap();

    let _ = resolve.await.unwrap();

    let (status, agr, resb, resolved): (String, String, DateTime<Utc>, DateTime<Utc>) = sqlx::query_as(
        "SELECT status::text, agreement_status::text, resolution_by, resolved_at FROM support.issues WHERE id=$1")
        .bind(issue).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "resolved");
    assert_eq!(resb, dt("2026-07-07T15:00:00Z"), "deadline was extended by the 2h hold");
    assert!(resolved <= resb, "resolved 14:00 is within the extended 15:00 deadline");
    assert_eq!(agr, "fulfilled",
        "the verdict must reflect the pause-extended deadline, not a stale pre-extension read");
}
