use std::{
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use ratatui::backend::{Backend, CrosstermBackend};
use ratatui_js_core::{RenderError, RenderResult, Renderer};

use crate::{
    CleanupFailure, SessionError, TerminalEvent,
    input::{CrosstermInput, InputQueue, Lifecycle, lock, start_reader},
    terminal::{CrosstermControl, Modes, OwnerGuard, TerminalControl},
};

#[derive(Debug, Clone, Copy)]
pub struct SessionOptions {
    pub alternate_screen: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            alternate_screen: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPoll {
    Event(TerminalEvent),
    Timeout,
    Closed,
}

/// Owns a real terminal. Share with `Arc<Session>` for concurrent rendering,
/// event polling, and closing. Always call `close()` to observe cleanup errors.
pub struct Session {
    inner: SessionInner<Renderer<CrosstermBackend<io::Stdout>>>,
}

impl Session {
    pub fn open(options: SessionOptions) -> Result<Self, SessionError> {
        let owner = OwnerGuard::global()?;
        let inner = SessionInner::open(
            options,
            Box::new(CrosstermControl),
            owner,
            || {
                Renderer::new(CrosstermBackend::new(io::stdout()))
                    .map_err(|e| SessionError::io("create renderer", e))
            },
            |queue| start_reader(queue, CrosstermInput),
        )?;
        Ok(Self { inner })
    }

    /// Borrows bytes only until this synchronous call completes. A rejected
    /// frame leaves rendering available; backend I/O failure disables it.
    pub fn render_json(&self, bytes: &[u8]) -> Result<RenderResult, SessionError> {
        self.inner.render_json(bytes)
    }

    /// One outstanding wait is allowed. Zero timeout checks without waiting.
    pub fn poll_event(&self, timeout: Duration) -> Result<EventPoll, SessionError> {
        self.inner.input.poll(timeout)
    }

    /// Idempotent, including its error outcome. Wakes input waits, finishes an
    /// admitted render, and attempts every applicable terminal cleanup step.
    pub fn close(&self) -> Result<(), SessionError> {
        self.inner.close()
    }
}

// The internal rendering seam permits lifecycle tests with Ratatui's real
// TestBackend, and controlled failure/blocking tests without touching a terminal.
trait RenderTarget: Send {
    fn render_json(&mut self, bytes: &[u8]) -> Result<RenderResult, RenderError>;
}

impl<B: Backend + Send> RenderTarget for Renderer<B> {
    fn render_json(&mut self, bytes: &[u8]) -> Result<RenderResult, RenderError> {
        Renderer::render_json(self, bytes)
    }
}

struct CloseState {
    reader: Option<JoinHandle<()>>,
    modes: Modes,
    outcome: Option<Result<(), SessionError>>,
}

struct SessionInner<R: RenderTarget> {
    renderer: Mutex<Option<R>>,
    input: Arc<InputQueue>,
    close_state: Mutex<CloseState>,
}

impl<R: RenderTarget> SessionInner<R> {
    fn open(
        options: SessionOptions,
        control: Box<dyn TerminalControl>,
        owner: OwnerGuard,
        create_renderer: impl FnOnce() -> Result<R, SessionError>,
        spawn_reader: impl FnOnce(Arc<InputQueue>) -> io::Result<JoinHandle<()>>,
    ) -> Result<Self, SessionError> {
        let mut modes = Modes::new(control, owner);
        let creation = catch_unwind(AssertUnwindSafe(|| {
            modes.initialize(options)?;
            let renderer = create_renderer()?;
            let input = Arc::new(InputQueue::new());
            let reader = spawn_reader(Arc::clone(&input))
                .map_err(|e| SessionError::io("start input reader", e))?;
            Ok((renderer, input, reader))
        }))
        .unwrap_or(Err(SessionError::Panic("session initialization")));
        match creation {
            Ok((renderer, input, reader)) => Ok(Self {
                renderer: Mutex::new(Some(renderer)),
                input,
                close_state: Mutex::new(CloseState {
                    reader: Some(reader),
                    modes,
                    outcome: None,
                }),
            }),
            Err(cause) => {
                let cleanup = modes.restore();
                if cleanup.is_empty() {
                    Err(cause)
                } else {
                    Err(SessionError::Initialization {
                        cause: Box::new(cause),
                        cleanup,
                    })
                }
            }
        }
    }

    fn render_json(&self, bytes: &[u8]) -> Result<RenderResult, SessionError> {
        let mut renderer = lock(&self.renderer);
        {
            let state = lock(&self.input.state);
            if state.lifecycle != Lifecycle::Open {
                return Err(SessionError::Closed);
            }
            if state.rendering_failed || self.renderer.is_poisoned() {
                return Err(SessionError::RenderingFailed);
            }
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            renderer.as_mut().expect("open renderer").render_json(bytes)
        }));
        match outcome {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(error)) => {
                if matches!(error, RenderError::Backend(_)) {
                    lock(&self.input.state).rendering_failed = true;
                }
                Err(SessionError::Frame(Arc::new(error)))
            }
            Err(_) => {
                lock(&self.input.state).rendering_failed = true;
                Err(SessionError::Panic("renderer"))
            }
        }
    }

    fn close(&self) -> Result<(), SessionError> {
        // Holding this lock elects the shutdown caller. Neither the reader nor
        // rendering uses it, so joining/waiting here cannot block their progress.
        let mut close = lock(&self.close_state);
        if let Some(outcome) = &close.outcome {
            return outcome.clone();
        }
        self.input.begin_close();
        let mut failures = Vec::new();
        if let Some(reader) = close.reader.take()
            && reader.join().is_err()
        {
            failures.push(CleanupFailure {
                operation: "join input reader",
                message: "input reader panicked outside its error boundary".into(),
            });
        }
        let mut renderer = lock(&self.renderer);
        // Ratatui's Terminal destructor may write to the backend. Drop it before
        // our explicit restoration/flush, while terminal output is serialized.
        if catch_unwind(AssertUnwindSafe(|| drop(renderer.take()))).is_err() {
            failures.push(CleanupFailure {
                operation: "drop renderer",
                message: "renderer destructor panicked".into(),
            });
        }
        failures.extend(close.modes.restore());
        let outcome = if failures.is_empty() {
            Ok(())
        } else {
            Err(SessionError::Shutdown(failures))
        };
        close.outcome = Some(outcome.clone());
        self.input.finish_close();
        outcome
    }
}

impl<R: RenderTarget> Drop for SessionInner<R> {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests;
