use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::IssuePriority;
use super::AuditMetadata;

/// Strongly-typed ID for ServiceLevelPriority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServiceLevelPriorityId(pub Uuid);

impl ServiceLevelPriorityId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for ServiceLevelPriorityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ServiceLevelPriorityId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for ServiceLevelPriorityId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<ServiceLevelPriorityId> for Uuid {
    fn from(id: ServiceLevelPriorityId) -> Self { id.0 }
}

impl AsRef<Uuid> for ServiceLevelPriorityId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for ServiceLevelPriorityId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ServiceLevelPriority {
    pub id: Uuid,
    pub sla_id: Uuid,
    pub priority: IssuePriority,
    pub response_time_mins: i32,
    pub resolution_time_mins: i32,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl ServiceLevelPriority {
    /// Create a builder for ServiceLevelPriority
    pub fn builder() -> ServiceLevelPriorityBuilder {
        ServiceLevelPriorityBuilder::default()
    }

    /// Create a new ServiceLevelPriority with required fields
    pub fn new(sla_id: Uuid, priority: IssuePriority, response_time_mins: i32, resolution_time_mins: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            sla_id,
            priority,
            response_time_mins,
            resolution_time_mins,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> ServiceLevelPriorityId {
        ServiceLevelPriorityId(self.id)
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


    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "sla_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.sla_id = v; }
                }
                "priority" => {
                    if let Ok(v) = serde_json::from_value(value) { self.priority = v; }
                }
                "response_time_mins" => {
                    if let Ok(v) = serde_json::from_value(value) { self.response_time_mins = v; }
                }
                "resolution_time_mins" => {
                    if let Ok(v) = serde_json::from_value(value) { self.resolution_time_mins = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for ServiceLevelPriority {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "ServiceLevelPriority"
    }
}

impl backbone_core::PersistentEntity for ServiceLevelPriority {
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

impl backbone_orm::EntityRepoMeta for ServiceLevelPriority {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("sla_id".to_string(), "uuid".to_string());
        m.insert("priority".to_string(), "issue_priority".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for ServiceLevelPriority entity
///
/// Provides a fluent API for constructing ServiceLevelPriority instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct ServiceLevelPriorityBuilder {
    sla_id: Option<Uuid>,
    priority: Option<IssuePriority>,
    response_time_mins: Option<i32>,
    resolution_time_mins: Option<i32>,
}

impl ServiceLevelPriorityBuilder {
    /// Set the sla_id field (required)
    pub fn sla_id(mut self, value: Uuid) -> Self {
        self.sla_id = Some(value);
        self
    }

    /// Set the priority field (required)
    pub fn priority(mut self, value: IssuePriority) -> Self {
        self.priority = Some(value);
        self
    }

    /// Set the response_time_mins field (required)
    pub fn response_time_mins(mut self, value: i32) -> Self {
        self.response_time_mins = Some(value);
        self
    }

    /// Set the resolution_time_mins field (required)
    pub fn resolution_time_mins(mut self, value: i32) -> Self {
        self.resolution_time_mins = Some(value);
        self
    }

    /// Build the ServiceLevelPriority entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<ServiceLevelPriority, String> {
        let sla_id = self.sla_id.ok_or_else(|| "sla_id is required".to_string())?;
        let priority = self.priority.ok_or_else(|| "priority is required".to_string())?;
        let response_time_mins = self.response_time_mins.ok_or_else(|| "response_time_mins is required".to_string())?;
        let resolution_time_mins = self.resolution_time_mins.ok_or_else(|| "resolution_time_mins is required".to_string())?;

        Ok(ServiceLevelPriority {
            id: Uuid::new_v4(),
            sla_id,
            priority,
            response_time_mins,
            resolution_time_mins,
            metadata: AuditMetadata::default(),
        })
    }
}
