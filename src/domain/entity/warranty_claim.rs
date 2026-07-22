use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::WarrantyStatus;
use super::AuditMetadata;

/// Strongly-typed ID for WarrantyClaim
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WarrantyClaimId(pub Uuid);

impl WarrantyClaimId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for WarrantyClaimId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for WarrantyClaimId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for WarrantyClaimId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<WarrantyClaimId> for Uuid {
    fn from(id: WarrantyClaimId) -> Self { id.0 }
}

impl AsRef<Uuid> for WarrantyClaimId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for WarrantyClaimId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WarrantyClaim {
    pub id: Uuid,
    pub company_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub item_id: Uuid,
    pub serial_no: Option<String>,
    pub claim_date: DateTime<Utc>,
    pub warranty_expiry: Option<DateTime<Utc>>,
    pub is_under_warranty: bool,
    pub status: WarrantyStatus,
    pub issue_id: Option<Uuid>,
    pub description: Option<String>,
    pub resolution: Option<String>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl WarrantyClaim {
    /// Create a builder for WarrantyClaim
    pub fn builder() -> WarrantyClaimBuilder {
        WarrantyClaimBuilder::default()
    }

    /// Create a new WarrantyClaim with required fields
    pub fn new(company_id: Uuid, item_id: Uuid, claim_date: DateTime<Utc>, is_under_warranty: bool, status: WarrantyStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            customer_id: None,
            item_id,
            serial_no: None,
            claim_date,
            warranty_expiry: None,
            is_under_warranty,
            status,
            issue_id: None,
            description: None,
            resolution: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> WarrantyClaimId {
        WarrantyClaimId(self.id)
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
    pub fn status(&self) -> &WarrantyStatus {
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

    /// Set the serial_no field (chainable)
    pub fn with_serial_no(mut self, value: String) -> Self {
        self.serial_no = Some(value);
        self
    }

    /// Set the warranty_expiry field (chainable)
    pub fn with_warranty_expiry(mut self, value: DateTime<Utc>) -> Self {
        self.warranty_expiry = Some(value);
        self
    }

    /// Set the issue_id field (chainable)
    pub fn with_issue_id(mut self, value: Uuid) -> Self {
        self.issue_id = Some(value);
        self
    }

    /// Set the description field (chainable)
    pub fn with_description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the resolution field (chainable)
    pub fn with_resolution(mut self, value: String) -> Self {
        self.resolution = Some(value);
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
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "serial_no" => {
                    if let Ok(v) = serde_json::from_value(value) { self.serial_no = v; }
                }
                "claim_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.claim_date = v; }
                }
                "warranty_expiry" => {
                    if let Ok(v) = serde_json::from_value(value) { self.warranty_expiry = v; }
                }
                "is_under_warranty" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_under_warranty = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                "issue_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.issue_id = v; }
                }
                "description" => {
                    if let Ok(v) = serde_json::from_value(value) { self.description = v; }
                }
                "resolution" => {
                    if let Ok(v) = serde_json::from_value(value) { self.resolution = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for WarrantyClaim {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "WarrantyClaim"
    }
}

impl backbone_core::PersistentEntity for WarrantyClaim {
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

impl backbone_orm::EntityRepoMeta for WarrantyClaim {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("customer_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("issue_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "warranty_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
    fn company_field() -> Option<&'static str> {
        Some("company_id")
    }
}

/// Builder for WarrantyClaim entity
///
/// Provides a fluent API for constructing WarrantyClaim instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct WarrantyClaimBuilder {
    company_id: Option<Uuid>,
    customer_id: Option<Uuid>,
    item_id: Option<Uuid>,
    serial_no: Option<String>,
    claim_date: Option<DateTime<Utc>>,
    warranty_expiry: Option<DateTime<Utc>>,
    is_under_warranty: Option<bool>,
    status: Option<WarrantyStatus>,
    issue_id: Option<Uuid>,
    description: Option<String>,
    resolution: Option<String>,
}

impl WarrantyClaimBuilder {
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

    /// Set the item_id field (required)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the serial_no field (optional)
    pub fn serial_no(mut self, value: String) -> Self {
        self.serial_no = Some(value);
        self
    }

    /// Set the claim_date field (required)
    pub fn claim_date(mut self, value: DateTime<Utc>) -> Self {
        self.claim_date = Some(value);
        self
    }

    /// Set the warranty_expiry field (optional)
    pub fn warranty_expiry(mut self, value: DateTime<Utc>) -> Self {
        self.warranty_expiry = Some(value);
        self
    }

    /// Set the is_under_warranty field (default: `false`)
    pub fn is_under_warranty(mut self, value: bool) -> Self {
        self.is_under_warranty = Some(value);
        self
    }

    /// Set the status field (default: `WarrantyStatus::default()`)
    pub fn status(mut self, value: WarrantyStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Set the issue_id field (optional)
    pub fn issue_id(mut self, value: Uuid) -> Self {
        self.issue_id = Some(value);
        self
    }

    /// Set the description field (optional)
    pub fn description(mut self, value: String) -> Self {
        self.description = Some(value);
        self
    }

    /// Set the resolution field (optional)
    pub fn resolution(mut self, value: String) -> Self {
        self.resolution = Some(value);
        self
    }

    /// Build the WarrantyClaim entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<WarrantyClaim, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let item_id = self.item_id.ok_or_else(|| "item_id is required".to_string())?;
        let claim_date = self.claim_date.ok_or_else(|| "claim_date is required".to_string())?;

        Ok(WarrantyClaim {
            id: Uuid::new_v4(),
            company_id,
            customer_id: self.customer_id,
            item_id,
            serial_no: self.serial_no,
            claim_date,
            warranty_expiry: self.warranty_expiry,
            is_under_warranty: self.is_under_warranty.unwrap_or(false),
            status: self.status.unwrap_or(WarrantyStatus::default()),
            issue_id: self.issue_id,
            description: self.description,
            resolution: self.resolution,
            metadata: AuditMetadata::default(),
        })
    }
}
