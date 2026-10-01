use std::{
    collections::VecDeque,
    io,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ratatui::crossterm::event;

use crate::{EventPoll, SessionError, TerminalEvent, events::normalize};

const QUEUE_CAPACITY: usize = 256;
const READER_POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lifecycle {
    Open,
    Closing,
    Closed,
}

pub(crate) struct State {
    pub lifecycle: Lifecycle,
    pub rendering_failed: bool,
    queue: VecDeque<TerminalEvent>,
    input_failure: Option<String>,
    waiting: bool,
}

pub(crate) struct InputQueue {
    pub state: Mutex<State>,
    changed: Condvar,
}

impl InputQueue {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                lifecycle: Lifecycle::Open,
                rendering_failed: false,
                queue: VecDeque::new(),
                input_failure: None,
                waiting: false,
            }),
            changed: Condvar::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        lock(&self.state).lifecycle == Lifecycle::Open
    }

    pub fn begin_close(&self) {
        let mut state = lock(&self.state);
        state.lifecycle = Lifecycle::Closing;
        state.queue.clear();
        self.changed.notify_all();
    }

    pub fn finish_close(&self) {
        lock(&self.state).lifecycle = Lifecycle::Closed;
        self.changed.notify_all();
    }

    fn push(&self, event: TerminalEvent) -> bool {
        let mut state = lock(&self.state);
        while state.lifecycle == Lifecycle::Open && state.queue.len() == QUEUE_CAPACITY {
            state = self.changed.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if state.lifecycle != Lifecycle::Open {
            return false;
        }
        state.queue.push_back(event);
        self.changed.notify_all();
        true
    }

    fn fail(&self, message: String) {
        let mut state = lock(&self.state);
        state.input_failure = Some(message);
        self.changed.notify_all();
    }

    pub fn poll(&self, timeout: Duration) -> Result<EventPoll, SessionError> {
        let mut state = lock(&self.state);
        if state.waiting {
            return Err(SessionError::ConcurrentEventWait);
        }
        if state.lifecycle != Lifecycle::Open {
            return Ok(EventPoll::Closed);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(SessionError::InvalidTimeout)?;
        state.waiting = true;
        let result = loop {
            if state.lifecycle != Lifecycle::Open {
                break Ok(EventPoll::Closed);
            }
            if let Some(message) = &state.input_failure {
                break Err(SessionError::Input(message.clone()));
            }
            if let Some(event) = state.queue.pop_front() {
                self.changed.notify_all();
                break Ok(EventPoll::Event(event));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break Ok(EventPoll::Timeout);
            }
            let (next_state, _) = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner());
            state = next_state;
        };
        state.waiting = false;
        result
    }
}

/// Only private bookkeeping locks are recovered. Renderer poison is handled
/// separately: it disables drawing, while still allowing resource cleanup.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

pub(crate) trait InputSource: Send + 'static {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>>;
}

pub(crate) struct CrosstermInput;

impl CrosstermInput {
    pub fn prepare(&mut self) -> io::Result<()> {
        // Register Crossterm's event source and resize notifications before
        // returning open; poll preserves any event it observes for read().
        event::poll(Duration::ZERO).map(|_| ())
    }
}

impl InputSource for CrosstermInput {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>> {
        if event::poll(timeout)? {
            Ok(normalize(event::read()?))
        } else {
            Ok(None)
        }
    }
}

pub(crate) fn start_reader(
    queue: Arc<InputQueue>,
    mut source: impl InputSource,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("ratatui-js-input".into())
        .spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| -> io::Result<()> {
                while queue.is_open() {
                    if let Some(event) = source.next(READER_POLL_INTERVAL)?
                        && !queue.push(event)
                    {
                        break;
                    }
                }
                Ok(())
            }));
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(error)) => queue.fail(error.to_string()),
                Err(_) => queue.fail("input reader panicked".into()),
            }
        })
}

#[cfg(test)]
mod tests;

#[cfg(unix)]
pub(crate) enum SelectedInput {
    Standard(CrosstermInput),
    Unix(Arc<Mutex<crate::streams::UnixInput>>),
}
#[cfg(unix)]
impl InputSource for SelectedInput {
    fn next(&mut self, timeout: Duration) -> io::Result<Option<TerminalEvent>> {
        match self {
            Self::Standard(source) => source.next(timeout),
            Self::Unix(source) => lock(source).next(timeout),
        }
    }
}

#[cfg(unix)]
impl SelectedInput {
    pub fn prepare(&mut self) -> io::Result<()> {
        match self {
            Self::Standard(source) => source.prepare(),
            Self::Unix(source) => lock(source).prepare(),
        }
    }
}
