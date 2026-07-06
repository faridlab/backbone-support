//! Support's one outbound port (hand-authored, user-owned) — the optional seam where a ticket becomes
//! delivery work. Support holds only this trait + its own DTOs; a composing service wires the real
//! backbone-project behind it. **Zero normal Cargo edge** to project — the two Tier-4 modules stay
//! independent (brief §7), and the DTOs are the wire contract, duplicated per consumer by design.
//! Support posts no GL and has no ledger seam; escalation just spawns a delivery Project.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Escalate a support ticket into a delivery project (open a backbone-project Project for the customer).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFromIssue {
    pub company_id: Uuid,
    pub issue_id: Uuid,
    pub customer_id: Uuid,
    pub subject: String,
}

/// The spawned delivery project's id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectAck {
    pub project_id: Uuid,
}

/// The project seam — a composing service implements it over backbone-project.
#[async_trait::async_trait]
pub trait ProjectPort: Send + Sync {
    async fn open_delivery_project(
        &self,
        req: &ProjectFromIssue,
    ) -> Result<ProjectAck, SupportRejected>;
}

/// A downstream rejection surfaced to support.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SupportRejected {
    pub code: String,
    pub message: String,
}
