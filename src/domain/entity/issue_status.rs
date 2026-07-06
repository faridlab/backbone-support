use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "issue_status", rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Replied,
    OnHold,
    Resolved,
    Closed,
}

impl std::fmt::Display for IssueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open => write!(f, "open"),
            Self::Replied => write!(f, "replied"),
            Self::OnHold => write!(f, "on_hold"),
            Self::Resolved => write!(f, "resolved"),
            Self::Closed => write!(f, "closed"),
        }
    }
}

impl FromStr for IssueStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "open" => Ok(Self::Open),
            "replied" => Ok(Self::Replied),
            "on_hold" => Ok(Self::OnHold),
            "resolved" => Ok(Self::Resolved),
            "closed" => Ok(Self::Closed),
            _ => Err(format!("Unknown IssueStatus variant: {}", s)),
        }
    }
}

impl Default for IssueStatus {
    fn default() -> Self {
        Self::Open
    }
}
