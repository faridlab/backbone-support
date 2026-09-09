//! Durability probe (outbox rollout plan, P2): the cross-module `IssueEscalated` event — which
//! backbone-project SUBSCRIBES to, to open a delivery project for the escalated ticket — is staged in the
//! transactional outbox in the SAME tx as the escalation CAS, so a crash between the CAS and the in-proc
//! publish cannot drop it (no project would ever open). The `LoggingSink` drops the in-proc publish; the
//! event must still be staged in `support.outbox_events`.
//!
//! Tenancy (ADR-0029): the outbox mirror stays company-keyed (it mirrors the framework outbox table),
//! and the escalation seam sources the sibling's legacy owner-company key from the ambient org scope —
//! the probe binds a company-anchored org request scope around the escalation.

mod common;
use common::*;

use backbone_support::application::service::support_ports::{
    ProjectAck, ProjectFromIssue, ProjectPort, SupportRejected,
};
use backbone_support::application::service::support_write_service::{
    NewIssue, NewSla, NewSlaPriority, SupportWriteService,
};
use uuid::Uuid;

/// A fake project port — returns a fresh project id without a real backbone-project (durability, not the seam).
struct FakeProject;
#[async_trait::async_trait]
impl ProjectPort for FakeProject {
    async fn open_delivery_project(
        &self,
        _req: &ProjectFromIssue,
    ) -> Result<ProjectAck, SupportRejected> {
        Ok(ProjectAck {
            project_id: Uuid::new_v4(),
        })
    }
}

// SOD-1 — escalating a ticket durably stages IssueEscalated despite the dropped in-proc publish.
#[tokio::test]
async fn sod1_escalation_is_durably_staged() {
    let pool = pool().await;
    let svc = SupportWriteService::new(pool.clone());
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
                customer_id: Some(Uuid::new_v4()),
                subject: "Onsite repair".into(),
                description: None,
                priority: "high".into(),
                sla_id: Some(sla),
            },
            dt("2026-07-07T09:00:00Z"),
        )
        .await
        .unwrap();

    // LoggingSink drops the in-proc publish — durability must come from the outbox.
    // The seam's legacy owner-company key comes from the ambient org scope — bind one explicitly.
    with_company_scope(&pool, Uuid::new_v4(), async {
        svc.escalate_to_project(issue, &FakeProject, &LoggingSink)
            .await
            .unwrap();
    })
    .await;

    let staged: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM support.outbox_events WHERE aggregate_id=$1 AND event_type='IssueEscalated'")
        .bind(issue.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(
        staged, 1,
        "IssueEscalated durably staged despite the dropped in-proc publish"
    );
}
