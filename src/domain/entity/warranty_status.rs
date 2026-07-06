use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "warranty_status", rename_all = "snake_case")]
pub enum WarrantyStatus {
    Open,
    Accepted,
    Rejected,
    Closed,
}

impl std::fmt::Display for WarrantyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

impl FromStr for WarrantyStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "closed" => Ok(Self::Closed),
            _ => Err(format!("Unknown WarrantyStatus variant: {}", s)),
        }
    }
}

impl Default for WarrantyStatus {
    fn default() -> Self {
        Self::Open
    }
}
