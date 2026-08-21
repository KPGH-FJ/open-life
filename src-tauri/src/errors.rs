use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
    /// Data storage error (SQLite, file I/O)
    Database {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    /// Configuration error (missing/invalid config)
    Config {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    /// Resource not found (run, proposal, session, etc.)
    NotFound { message: String },
    /// Permission denied (safe mode, tool policy, etc.)
    PermissionDenied { message: String },
    /// Operation timed out
    Timeout { message: String },
    /// External service error (LLM API, MCP, network)
    ExternalService {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        hint: Option<String>,
    },
    /// Data format/serialization error
    Serialization { message: String },
    /// Catch-all for other unexpected errors
    Internal {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
}

impl AppError {
    pub fn message(&self) -> &str {
        match self {
            AppError::Database { message, .. } => message,
            AppError::Config { message, .. } => message,
            AppError::NotFound { message } => message,
            AppError::PermissionDenied { message } => message,
            AppError::Timeout { message } => message,
            AppError::ExternalService { message, .. } => message,
            AppError::Serialization { message } => message,
            AppError::Internal { message, .. } => message,
        }
    }

    pub fn is_recoverable(&self) -> bool {
        match self {
            AppError::Timeout { .. } => true,
            AppError::ExternalService { .. } => true,
            AppError::Database { .. } => false,
            AppError::Config { .. } => true,
            AppError::NotFound { .. } => false,
            AppError::PermissionDenied { .. } => true,
            AppError::Serialization { .. } => false,
            AppError::Internal { .. } => false,
        }
    }

    pub fn retry_hint(&self) -> Option<&str> {
        match self {
            AppError::Timeout { .. } => Some("retry_after_delay"),
            AppError::ExternalService { .. } => Some("retry_after_delay"),
            AppError::Config { .. } => Some("check_settings"),
            AppError::PermissionDenied { .. } => Some("review_permissions"),
            _ => None,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for AppError {}

// Convenience constructors — reduce typing in command modules.
impl AppError {
    pub fn db(msg: impl Into<String>) -> Self {
        AppError::Database {
            message: msg.into(),
            hint: None,
        }
    }

    pub fn db_with_hint(msg: impl Into<String>, hint: impl Into<String>) -> Self {
        AppError::Database {
            message: msg.into(),
            hint: Some(hint.into()),
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        AppError::NotFound {
            message: msg.into(),
        }
    }

    pub fn permission(msg: impl Into<String>) -> Self {
        AppError::PermissionDenied {
            message: msg.into(),
        }
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        AppError::Timeout {
            message: msg.into(),
        }
    }

    pub fn external(msg: impl Into<String>) -> Self {
        AppError::ExternalService {
            message: msg.into(),
            hint: None,
        }
    }

    pub fn serialization(msg: impl Into<String>) -> Self {
        AppError::Serialization {
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        AppError::Internal {
            message: msg.into(),
            code: None,
        }
    }

    pub fn internal_with_code(msg: impl Into<String>, code: impl Into<String>) -> Self {
        AppError::Internal {
            message: msg.into(),
            code: Some(code.into()),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        let msg = e.to_string();
        let lower = msg.to_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            AppError::Timeout { message: msg }
        } else if lower.contains("permission") || lower.contains("denied") {
            AppError::PermissionDenied { message: msg }
        } else if lower.contains("not found") || lower.contains("no such") {
            AppError::NotFound { message: msg }
        } else if lower.contains("connection") || lower.contains("http") || lower.contains("api") {
            AppError::ExternalService {
                message: msg,
                hint: None,
            }
        } else {
            AppError::Internal {
                message: msg,
                code: None,
            }
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serialization {
            message: e.to_string(),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        let msg = e.to_string();
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound { message: msg }
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            AppError::PermissionDenied { message: msg }
        } else {
            AppError::Database {
                message: msg,
                hint: None,
            }
        }
    }
}

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::Internal {
            message: msg,
            code: None,
        }
    }
}

impl From<openlife_core::life_model::patch::PatchError> for AppError {
    fn from(e: openlife_core::life_model::patch::PatchError) -> Self {
        AppError::Serialization {
            message: format!("LifeModel patch error: {:?}", e),
        }
    }
}
