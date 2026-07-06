//! Support domain events (hand-authored, user-owned) — the public extension surface.
//!
//! backbone-support posts NO GL and owns no money. Its events are read-side signals: an SLA outcome
//! (`IssueResolved` carrying whether the resolution deadline was met), a ticket escalated into delivery
//! work (`IssueEscalated`), and a warranty claim filed (`WarrantyClaimFiled` carrying the under-warranty
//! verdict). A consuming service supplies the sink (bus, outbox, …).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A ticket was resolved — `fulfilled` is true iff it was resolved within the (pause-adjusted) SLA
/// resolution deadline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueResolved {
    pub issue_id: Uuid,
    pub company_id: Uuid,
    pub fulfilled: bool,
}

/// A ticket was escalated into a real backbone-project delivery Project.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueEscalated {
    pub issue_id: Uuid,
    pub company_id: Uuid,
    pub project_id: Uuid,
}

/// A warranty claim was filed — `is_under_warranty` is the coverage verdict at file time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WarrantyClaimFiled {
    pub claim_id: Uuid,
    pub company_id: Uuid,
    pub is_under_warranty: bool,
}

/// The support domain-event union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SupportEvent {
    IssueResolved(IssueResolved),
    IssueEscalated(IssueEscalated),
    WarrantyClaimFiled(WarrantyClaimFiled),
}

/// Sink the write path publishes to. A consuming service supplies its own (bus, outbox, …).
pub trait SupportEventSink: Send + Sync {
    fn publish(&self, event: &SupportEvent);
}

/// A no-op/logging sink for tests and single-process composition.
#[derive(Debug, Default, Clone)]
pub struct LoggingSink;

impl SupportEventSink for LoggingSink {
    fn publish(&self, event: &SupportEvent) {
        tracing::info!(?event, "support event");
    }
}
