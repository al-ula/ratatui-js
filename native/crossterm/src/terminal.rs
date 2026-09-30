use std::{
    io::{self, IsTerminal, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, OnceLock},
};

use ratatui::crossterm::{
    cursor::{Hide, Show},
    execute,
    style::{Attribute, ResetColor, SetAttribute},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use crate::{CleanupFailure, SessionError, SessionOptions, input::lock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ownership {
    Free,
    Busy,
    Poisoned,
}

pub(crate) struct OwnerGuard {
    slot: Arc<Mutex<Ownership>>,
    restored: bool,
}

impl OwnerGuard {
    pub fn acquire(slot: Arc<Mutex<Ownership>>) -> Result<Self, SessionError> {
        {
            let mut state = lock(&slot);
            match *state {
                Ownership::Free => *state = Ownership::Busy,
                Ownership::Busy => return Err(SessionError::TerminalBusy),
                Ownership::Poisoned => return Err(SessionError::TerminalPoisoned),
            }
        }
        Ok(Self {
            slot,
            restored: true,
        })
    }

    pub fn global() -> Result<Self, SessionError> {
        static SLOT: OnceLock<Arc<Mutex<Ownership>>> = OnceLock::new();
        Self::acquire(Arc::clone(
            SLOT.get_or_init(|| Arc::new(Mutex::new(Ownership::Free))),
        ))
    }
}

impl Drop for OwnerGuard {
    fn drop(&mut self) {
        *lock(&self.slot) = if self.restored {
            Ownership::Free
        } else {
            Ownership::Poisoned
        };
    }
}

/// A narrow terminal-operation seam for lifecycle failure tests. It is not a
/// backend-selection interface and never participates in widget rendering.
pub(crate) trait TerminalControl: Send {
    fn check_terminal(&mut self) -> Result<(), SessionError>;
    fn raw_mode_enabled(&mut self) -> io::Result<bool>;
    fn enable_raw(&mut self) -> io::Result<()>;
    fn enter_alternate(&mut self) -> io::Result<()>;
    fn hide_cursor(&mut self) -> io::Result<()>;
    fn clear_screen(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
    fn reset_attributes(&mut self) -> io::Result<()>;
    fn reset_colors(&mut self) -> io::Result<()>;
    fn leave_alternate(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
    fn disable_raw(&mut self) -> io::Result<()>;
}

pub(crate) struct CrosstermControl;

impl TerminalControl for CrosstermControl {
    fn check_terminal(&mut self) -> Result<(), SessionError> {
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            Ok(())
        } else {
            Err(SessionError::NotTerminal)
        }
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        terminal::is_raw_mode_enabled()
    }

    fn enable_raw(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        execute!(io::stdout(), EnterAlternateScreen)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(io::stdout(), Hide)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        execute!(io::stdout(), Clear(ClearType::All))
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(io::stdout(), Show)
    }

    fn reset_attributes(&mut self) -> io::Result<()> {
        execute!(io::stdout(), SetAttribute(Attribute::Reset))
    }

    fn reset_colors(&mut self) -> io::Result<()> {
        execute!(io::stdout(), ResetColor)
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        execute!(io::stdout(), LeaveAlternateScreen)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }
}

pub(crate) struct Modes {
    control: Box<dyn TerminalControl>,
    owner: Option<OwnerGuard>,
    raw_attempted: bool,
    alternate_attempted: bool,
    cursor_attempted: bool,
}

impl Modes {
    pub fn new(control: Box<dyn TerminalControl>, owner: OwnerGuard) -> Self {
        Self {
            control,
            owner: Some(owner),
            raw_attempted: false,
            alternate_attempted: false,
            cursor_attempted: false,
        }
    }

    pub fn initialize(&mut self, options: SessionOptions) -> Result<(), SessionError> {
        self.control.check_terminal()?;
        if self
            .control
            .raw_mode_enabled()
            .map_err(|e| SessionError::io("check raw mode", e))?
        {
            return Err(SessionError::RawModeActive);
        }
        // From the first attempted mutation onwards, unexpected unwinding must
        // not release ownership as if the terminal were known to be restored.
        self.owner
            .as_mut()
            .expect("initializing owned modes")
            .restored = false;
        self.raw_attempted = true;
        self.control
            .enable_raw()
            .map_err(|e| SessionError::io("enable raw mode", e))?;
        if options.alternate_screen {
            self.alternate_attempted = true;
            self.control
                .enter_alternate()
                .map_err(|e| SessionError::io("enter alternate screen", e))?;
        }
        self.cursor_attempted = true;
        self.control
            .hide_cursor()
            .map_err(|e| SessionError::io("hide cursor", e))?;
        // Ratatui initializes blank diff buffers, not the physical screen or
        // inherited styles. Establish that baseline before its first draw.
        self.control
            .reset_attributes()
            .map_err(|e| SessionError::io("reset initial attributes", e))?;
        self.control
            .reset_colors()
            .map_err(|e| SessionError::io("reset initial colors", e))?;
        self.control
            .clear_screen()
            .map_err(|e| SessionError::io("clear initial screen", e))?;
        Ok(())
    }

    pub fn restore(&mut self) -> Vec<CleanupFailure> {
        if self.owner.is_none() {
            return Vec::new();
        }
        let mut failures = Vec::new();
        if self.cursor_attempted {
            cleanup_step(&mut failures, "show cursor", || self.control.show_cursor());
        }
        if self.cursor_attempted || self.alternate_attempted {
            cleanup_step(&mut failures, "reset attributes", || {
                self.control.reset_attributes()
            });
            cleanup_step(&mut failures, "reset colors", || {
                self.control.reset_colors()
            });
        }
        if self.alternate_attempted {
            cleanup_step(&mut failures, "leave alternate screen", || {
                self.control.leave_alternate()
            });
        }
        if self.cursor_attempted || self.alternate_attempted {
            cleanup_step(&mut failures, "flush terminal", || self.control.flush());
        }
        if self.raw_attempted {
            cleanup_step(&mut failures, "disable raw mode", || {
                self.control.disable_raw()
            });
        }
        if let Some(mut owner) = self.owner.take() {
            owner.restored = failures.is_empty();
        }
        failures
    }
}

fn cleanup_step(
    failures: &mut Vec<CleanupFailure>,
    operation: &'static str,
    action: impl FnOnce() -> io::Result<()>,
) {
    let message = match catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(())) => return,
        Ok(Err(error)) => error.to_string(),
        Err(_) => "terminal cleanup operation panicked".into(),
    };
    failures.push(CleanupFailure { operation, message });
}

impl Drop for Modes {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

#[cfg(test)]
pub(crate) mod tests;
