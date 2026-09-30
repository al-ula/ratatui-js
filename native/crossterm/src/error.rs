use std::sync::Arc;

use ratatui_js_core::RenderError;
use thiserror::Error;

/// One failed restoration step. Other cleanup steps are still attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
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
