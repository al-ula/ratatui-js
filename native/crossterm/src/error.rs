use std::sync::Arc;

use ratatui_js_core::RenderError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One failed restoration step. Other cleanup steps are still attempted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CleanupFailure {
    pub operation: &'static str,
    pub message: String,
}

/// Lifecycle errors are deliberately separate from rendering-protocol errors.
#[derive(Debug, Clone, Error)]
pub enum SessionError {
    #[error("a terminal session is already open")]
    TerminalBusy,
    #[error("terminal ownership is poisoned after failed restoration")]
    TerminalPoisoned,
    #[error("stdin and stdout must both be connected to a terminal")]
    NotTerminal,
    #[error("Crossterm raw mode is already enabled by another owner")]
    RawModeActive,
    #[error("{operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
    },
    #[error(transparent)]
    Frame(Arc<RenderError>),
    #[error("session is closed")]
    Closed,
    #[error("rendering is disabled after a backend failure or panic")]
    RenderingFailed,
    #[error("an event wait is already outstanding")]
    ConcurrentEventWait,
    #[error("event timeout exceeds the monotonic clock's range")]
    InvalidTimeout,
    #[error("input reader failed: {0}")]
    Input(String),
    #[error("{0} panicked")]
    Panic(&'static str),
    #[error("initialization failed: {cause}; rollback failures: {cleanup:?}")]
    Initialization {
        cause: Box<SessionError>,
        cleanup: Vec<CleanupFailure>,
    },
    #[error("shutdown failures: {0:?}")]
    Shutdown(Vec<CleanupFailure>),
}

impl SessionError {
    pub(crate) fn io(operation: &'static str, error: impl std::fmt::Display) -> Self {
        Self::Io {
            operation,
            message: error.to_string(),
        }
    }
}

/// Stable wire error, shared by the C ABI and JavaScript adapters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ErrorDescription {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<ErrorDescription>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<Vec<CleanupDescription>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CleanupDescription {
    pub operation: String,
    pub message: String,
}

impl ErrorDescription {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            path: None,
            operation: None,
            cause: None,
            cleanup: None,
        }
    }
}

impl SessionError {
    pub fn description(&self) -> ErrorDescription {
        let code = match self {
            Self::TerminalBusy => "terminalBusy",
            Self::TerminalPoisoned => "terminalPoisoned",
            Self::NotTerminal => "notTerminal",
            Self::RawModeActive => "rawModeActive",
            Self::Io { .. } => "io",
            Self::Frame(error) => {
                let frame = error.description();
                let code = match frame.code {
                    ratatui_js_core::ErrorCode::InvalidJson => "invalidJson",
                    ratatui_js_core::ErrorCode::InvalidFrame => "invalidFrame",
                    ratatui_js_core::ErrorCode::UnsupportedProtocol => "unsupportedProtocol",
                    ratatui_js_core::ErrorCode::Io => "io",
                };
                let mut result = ErrorDescription::new(code, frame.message);
                result.path = frame.path;
                return result;
            }
            Self::Closed => "closed",
            Self::RenderingFailed => "renderingFailed",
            Self::ConcurrentEventWait => "concurrentEventWait",
            Self::InvalidTimeout => "invalidTimeout",
            Self::Input(_) => "input",
            Self::Panic(_) => "panic",
            Self::Initialization { .. } => "initialization",
            Self::Shutdown(_) => "shutdown",
        };
        let mut result = ErrorDescription::new(code, self.to_string());
        match self {
            Self::Io { operation, .. } | Self::Panic(operation) => {
                result.operation = Some((*operation).into());
            }
            Self::Initialization { cause, cleanup } => {
                result.cause = Some(Box::new(cause.description()));
                result.cleanup = Some(cleanup_descriptions(cleanup));
            }
            Self::Shutdown(cleanup) => result.cleanup = Some(cleanup_descriptions(cleanup)),
            _ => {}
        }
        result
    }
}

fn cleanup_descriptions(failures: &[CleanupFailure]) -> Vec<CleanupDescription> {
    failures
        .iter()
        .map(|failure| CleanupDescription {
            operation: failure.operation.into(),
            message: failure.message.clone(),
        })
        .collect()
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn shared_error_fixtures_round_trip() {
        let fixtures: Vec<ErrorDescription> =
            serde_json::from_str(include_str!("../../../tests/fixtures/errors.json")).unwrap();
        for fixture in fixtures {
            let json = serde_json::to_value(&fixture).unwrap();
            assert_eq!(
                serde_json::from_value::<ErrorDescription>(json).unwrap(),
                fixture
            );
        }
    }

    #[test]
    fn preserves_frame_path_and_all_rollback_failures() {
        let error = SessionError::Initialization {
            cause: Box::new(SessionError::Frame(Arc::new(RenderError::InvalidFrame {
                path: "root.lines".into(),
                message: "invalid".into(),
            }))),
            cleanup: vec![
                CleanupFailure {
                    operation: "raw mode",
                    message: "failed".into(),
                },
                CleanupFailure {
                    operation: "cursor",
                    message: "failed too".into(),
                },
            ],
        }
        .description();
        assert_eq!(error.cause.unwrap().path.as_deref(), Some("root.lines"));
        assert_eq!(error.cleanup.unwrap().len(), 2);
    }
}
