// This directory's extension (hand-written; ADR-0031). The generated mod.rs (lib.rs for
// src/) pulls it in with include!, so its lines resolve against this directory.

pub mod support_events;
pub mod support_ports;
pub mod support_write_service;
pub use support_events::{
    IssueEscalated, IssueResolved, LoggingSink, SupportEvent, SupportEventSink, WarrantyClaimFiled,
};
pub use support_ports::{ProjectAck, ProjectFromIssue, ProjectPort, SupportRejected};
pub use support_write_service::{
    NewIssue, NewSla, NewSlaPriority, NewWarrantyClaim, SupportError, SupportWriteService,
};
