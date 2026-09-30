use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    InvalidJson,
    InvalidFrame,
    UnsupportedProtocol,
    Io,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorDescription {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("{path}: {message}")]
    InvalidFrame { path: String, message: String },
    #[error("expected protocol {PROTOCOL_VERSION}, received {received}", PROTOCOL_VERSION = crate::PROTOCOL_VERSION)]
    UnsupportedProtocol { received: u32 },
    #[error("backend failure: {0}")]
    Backend(String),
}

impl RenderError {
    pub(crate) fn invalid(path: &str, message: impl Into<String>) -> Self {
        Self::InvalidFrame {
            path: path.to_owned(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidJson(_) => ErrorCode::InvalidJson,
            Self::InvalidFrame { .. } => ErrorCode::InvalidFrame,
            Self::UnsupportedProtocol { .. } => ErrorCode::UnsupportedProtocol,
            Self::Backend(_) => ErrorCode::Io,
        }
    }

    pub fn description(&self) -> ErrorDescription {
        ErrorDescription {
            code: self.code(),
            message: self.to_string(),
            path: match self {
                Self::InvalidFrame { path, .. } => Some(path.clone()),
                _ => None,
            },
        }
    }
}
