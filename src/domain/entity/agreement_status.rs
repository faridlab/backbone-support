use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "agreement_status", rename_all = "snake_case")]
pub enum AgreementStatus {
    FirstResponseDue,
    ResolutionDue,
    Fulfilled,
    Failed,
}

impl std::fmt::Display for AgreementStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FirstResponseDue => write!(f, "first_response_due"),
            Self::ResolutionDue => write!(f, "resolution_due"),
            Self::Fulfilled => write!(f, "fulfilled"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl FromStr for AgreementStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "first_response_due" => Ok(Self::FirstResponseDue),
            "resolution_due" => Ok(Self::ResolutionDue),
            "fulfilled" => Ok(Self::Fulfilled),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("Unknown AgreementStatus variant: {}", s)),
        }
    }
}

impl Default for AgreementStatus {
    fn default() -> Self {
        Self::FirstResponseDue
    }
}
