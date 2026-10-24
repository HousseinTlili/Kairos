//! Error types for Hermes calls.
//!
//! Messages never contain the API key or request headers; they carry Hermes's own error text,
//! which is safe to show.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HermesError {
    #[error("Hermes rejected the API key")]
    Unauthorized,

    #[error("Hermes isn't reachable: {0}")]
    Unreachable(String),

    #[error("Hermes is missing required capabilities: {}", .0.join(", "))]
    CapabilityMissing(Vec<String>),

    /// Idempotency-key conflicts, foreign sessions (`permission_denied`), unknown executions.
    #[error("Hermes refused the request ({code}): {message}")]
    Conflict { code: String, message: String },

    #[error("Hermes doesn't know it ({code}): {message}")]
    NotFound { code: String, message: String },

    /// An approval answered with an out-of-date `execution_generation`.
    #[error("that approval is no longer current: {message}")]
    Stale { message: String },

    #[error("Hermes answered HTTP {status}: {message}")]
    Http {
        status: u16,
        code: Option<String>,
        message: String,
    },

    #[error("couldn't read Hermes's response: {0}")]
    Parse(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl HermesError {
    /// The error kind sent across the Tauri boundary (M1 §8).
    pub fn kind(&self) -> &'static str {
        match self {
            HermesError::Unauthorized => "auth",
            HermesError::Unreachable(_) => "unreachable",
            HermesError::CapabilityMissing(_) => "capability_missing",
            HermesError::Conflict { .. } => "conflict",
            HermesError::NotFound { .. } => "not_found",
            HermesError::Stale { .. } => "stale",
            HermesError::Http { .. } | HermesError::InvalidArgument(_) => "http",
            HermesError::Parse(_) => "parse",
        }
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    #[serde(default)]
    message: String,
    #[serde(default)]
    code: Option<String>,
}

/// Maps a non-success Hermes response to an error. Hermes's `code` decides first, because it is
/// more specific than the status (Hermes uses 409 for several different refusals).
pub fn error_from_response(status: u16, body: &str) -> HermesError {
    let (code, message) = match serde_json::from_str::<ErrorBody>(body) {
        Ok(parsed) => (parsed.error.code, parsed.error.message),
        Err(_) => (None, body.trim().chars().take(300).collect()),
    };

    match code.as_deref() {
        Some("gateway_auth_failed") => return HermesError::Unauthorized,
        Some("stale_generation") => return HermesError::Stale { message },
        Some(c @ ("run_not_found" | "not_found" | "session_not_found")) => {
            return HermesError::NotFound {
                code: c.to_string(),
                message,
            };
        }
        Some(
            c @ ("idempotency_key_conflict"
            | "admission_conflict"
            | "permission_denied"
            | "unknown_execution"),
        ) => {
            return HermesError::Conflict {
                code: c.to_string(),
                message,
            };
        }
        _ => {}
    }

    match status {
        401 => HermesError::Unauthorized,
        404 => HermesError::NotFound {
            code: code.unwrap_or_else(|| "not_found".into()),
            message,
        },
        409 => HermesError::Conflict {
            code: code.unwrap_or_else(|| "conflict".into()),
            message,
        },
        _ => HermesError::Http {
            status,
            code,
            message,
        },
    }
}
