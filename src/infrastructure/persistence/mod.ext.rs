// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

// The hand-written support SQL's parameter/projection types (see the repositories declared
// `user_owned` in metaphor.codegen.yaml).
pub use issue_repository::{EscalationCandidateRow, IssueStateRow, NewIssueRow, PausedIssueRow};
pub use service_level_agreement_repository::NewSlaRow;
pub use service_level_priority_repository::{NewSlaPriorityRow, SlaTargetRow};
pub use warranty_claim_repository::NewWarrantyClaimRow;
