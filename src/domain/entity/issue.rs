use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::IssuePriority;
use super::IssueStatus;
use super::AgreementStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Issue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IssueId(pub Uuid);

impl IssueId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for IssueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for IssueId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for IssueId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<IssueId> for Uuid {
    fn from(id: IssueId) -> Self { id.0 }
}

impl AsRef<Uuid> for IssueId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for IssueId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Issue {
    pub id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub subject: String,
    pub description: Option<String>,
    pub priority: IssuePriority,
    pub sla_id: Option<Uuid>,
    pub status: IssueStatus,
    pub agreement_status: AgreementStatus,
    pub opened_at: DateTime<Utc>,
    pub response_by: Option<DateTime<Utc>>,
    pub resolution_by: Option<DateTime<Utc>>,
    pub first_responded_at: Option<DateTime<Utc>>,
    pub response_breached: bool,
    pub resolved_at: Option<DateTime<Utc>>,
    pub paused_at: Option<DateTime<Utc>>,
    pub total_paused_mins: i32,
    pub escalated_project_id: Option<Uuid>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Issue {
    /// Create a builder for Issue
    pub fn builder() -> IssueBuilder {
        IssueBuilder::default()
    }

    /// Create a new Issue with required fields
    pub fn new(company_id: Uuid, subject: String, priority: IssuePriority, status: IssueStatus, agreement_status: AgreementStatus, opened_at: DateTime<Utc>, response_breached: bool, total_paused_mins: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            customer_id: None,
            subject,
            description: None,
            priority,
            sla_id: None,
            status,
            agreement_status,
            opened_at,
            response_by: None,
            resolution_by: None,
            first_responded_at: None,
            response_breached,
            resolved_at: None,
            paused_at: None,
            total_paused_mins,
            escalated_project_id: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> IssueId {
        IssueId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }

    /// Get the current status
    pub fn status(&self) -> &IssueStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the customer_id field (chainable)
    pub fn with_customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the sla_id field (chainable)
    pub fn with_sla_id(mut self, value: Uuid) -> Self {
        self.sla_id = Some(value);
        self
    }

    /// Set the response_by field (chainable)
    pub fn with_response_by(mut self, value: DateTime<Utc>) -> Self {
        self.response_by = Some(value);
        self
    }

    /// Set the resolution_by field (chainable)
    pub fn with_resolution_by(mut self, value: DateTime<Utc>) -> Self {
        self.resolution_by = Some(value);
        self
    }

    /// Set the first_responded_at field (chainable)
    pub fn with_first_responded_at(mut self, value: DateTime<Utc>) -> Self {
        self.first_responded_at = Some(value);
        self
    }

    /// Set the resolved_at field (chainable)
    pub fn with_resolved_at(mut self, value: DateTime<Utc>) -> Self {
        self.resolved_at = Some(value);
        self
    }

    /// Set the paused_at field (chainable)
    pub fn with_paused_at(mut self, value: DateTime<Utc>) -> Self {
        self.paused_at = Some(value);
        self
    }

    /// Set the escalated_project_id field (chainable)
    pub fn with_escalated_project_id(mut self, value: Uuid) -> Self {
        self.escalated_project_id = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "customer_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.customer_id = v; }
                }
                "subject" => {
                    if let Ok(v) = serde_json::from_value(value) { self.subject = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "priority" => {
                    if let Ok(v) = serde_json::from_value(value) { self.priority = v; }
                }
                "sla_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sla_id = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "agreement_status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.agreement_status = v; }
                }
                "opened_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opened_at = v; }
                }
                "response_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.response_by = v; }
                }
                "resolution_by" => {
                    if let Ok(v) = serde_json::from_value(value) { self.resolution_by = v; }
                }
                "first_responded_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.first_responded_at = v; }
                }
                "response_breached" => {
                    if let Ok(v) = serde_json::from_value(value) { self.response_breached = v; }
                }
                "resolved_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.resolved_at = v; }
                }
                "paused_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.paused_at = v; }
                }
                "total_paused_mins" => {
                    if let Ok(v) = serde_json::from_value(value) { self.total_paused_mins = v; }
                }
                "escalated_project_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.escalated_project_id = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Issue {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Issue"
    }
}

impl backbone_core::PersistentEntity for Issue {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for Issue {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("sla_id".to_string(), "uuid".to_string());
        m.insert("escalated_project_id".to_string(), "uuid".to_string());
        m.insert("priority".to_string(), "issue_priority".to_string());
        m.insert("status".to_string(), "issue_status".to_string());
        m.insert("agreement_status".to_string(), "agreement_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["subject"]
    }
}

/// Builder for Issue entity
///
/// Provides a fluent API for constructing Issue instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct IssueBuilder {
    company_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    subject: Option<String>,
    description: Option<String>,
    priority: Option<IssuePriority>,
    sla_id: Option<Uuid>,
    status: Option<IssueStatus>,
    agreement_status: Option<AgreementStatus>,
    opened_at: Option<DateTime<Utc>>,
    response_by: Option<DateTime<Utc>>,
    resolution_by: Option<DateTime<Utc>>,
    first_responded_at: Option<DateTime<Utc>>,
    response_breached: Option<bool>,
    resolved_at: Option<DateTime<Utc>>,
    paused_at: Option<DateTime<Utc>>,
    total_paused_mins: Option<i32>,
    escalated_project_id: Option<Uuid>,
}

impl IssueBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the customer_id field (optional)
    pub fn customer_id(mut self, value: Uuid) -> Self {
        self.customer_id = Some(value);
        self
    }

    /// Set the subject field (required)
    pub fn subject(mut self, value: String) -> Self {
        self.subject = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the priority field (default: `IssuePriority::default()`)
    pub fn priority(mut self, value: IssuePriority) -> Self {
        self.priority = Some(value);
        self
    }

    /// Set the sla_id field (optional)
    pub fn sla_id(mut self, value: Uuid) -> Self {
        self.sla_id = Some(value);
        self
    }

    /// Set the status field (default: `IssueStatus::default()`)
    pub fn status(mut self, value: IssueStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the agreement_status field (default: `AgreementStatus::default()`)
    pub fn agreement_status(mut self, value: AgreementStatus) -> Self {
        self.agreement_status = Some(value);
        self
    }

    /// Set the opened_at field (required)
    pub fn opened_at(mut self, value: DateTime<Utc>) -> Self {
        self.opened_at = Some(value);
        self
    }

    /// Set the response_by field (optional)
    pub fn response_by(mut self, value: DateTime<Utc>) -> Self {
        self.response_by = Some(value);
        self
    }

    /// Set the resolution_by field (optional)
    pub fn resolution_by(mut self, value: DateTime<Utc>) -> Self {
        self.resolution_by = Some(value);
        self
    }

    /// Set the first_responded_at field (optional)
    pub fn first_responded_at(mut self, value: DateTime<Utc>) -> Self {
        self.first_responded_at = Some(value);
        self
    }

    /// Set the response_breached field (default: `false`)
    pub fn response_breached(mut self, value: bool) -> Self {
        self.response_breached = Some(value);
        self
    }

    /// Set the resolved_at field (optional)
    pub fn resolved_at(mut self, value: DateTime<Utc>) -> Self {
        self.resolved_at = Some(value);
        self
    }

    /// Set the paused_at field (optional)
    pub fn paused_at(mut self, value: DateTime<Utc>) -> Self {
        self.paused_at = Some(value);
        self
    }

    /// Set the total_paused_mins field (default: `0`)
    pub fn total_paused_mins(mut self, value: i32) -> Self {
        self.total_paused_mins = Some(value);
        self
    }

    /// Set the escalated_project_id field (optional)
    pub fn escalated_project_id(mut self, value: Uuid) -> Self {
        self.escalated_project_id = Some(value);
        self
    }

    /// Build the Issue entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Issue, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let subject = self.subject.ok_or_else(|| "subject is required".to_string())?;
        let opened_at = self.opened_at.ok_or_else(|| "opened_at is required".to_string())?;

        Ok(Issue {
            id: Uuid::new_v4(),
            company_id,
            customer_id: self.customer_id,
            subject,
            description: self.description,
            priority: self.priority.unwrap_or(IssuePriority::default()),
            sla_id: self.sla_id,
            status: self.status.unwrap_or(IssueStatus::default()),
            agreement_status: self.agreement_status.unwrap_or(AgreementStatus::default()),
            opened_at,
            response_by: self.response_by,
            resolution_by: self.resolution_by,
            first_responded_at: self.first_responded_at,
            response_breached: self.response_breached.unwrap_or(false),
            resolved_at: self.resolved_at,
            paused_at: self.paused_at,
            total_paused_mins: self.total_paused_mins.unwrap_or(0),
            escalated_project_id: self.escalated_project_id,
            metadata: AuditMetadata::default(),
        })
    }
}
