use std::{
    io::{self, IsTerminal, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, OnceLock},
};

use ratatui::crossterm::{
    cursor::{Hide, Show},
    event::{
        DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
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
    fn keyboard_supported(&mut self) -> io::Result<bool>;
    fn enable_mouse(&mut self) -> io::Result<()>;
    fn disable_mouse(&mut self) -> io::Result<()>;
    fn enable_paste(&mut self) -> io::Result<()>;
    fn disable_paste(&mut self) -> io::Result<()>;
    fn enable_focus(&mut self) -> io::Result<()>;
    fn disable_focus(&mut self) -> io::Result<()>;
    fn push_keyboard(&mut self) -> io::Result<()>;
    fn pop_keyboard(&mut self) -> io::Result<()>;
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

pub(crate) struct CrosstermControl {
    output: crate::streams::Output,
    #[cfg(unix)]
    custom: Option<(
        crate::streams::UnixModes,
        Arc<Mutex<crate::streams::UnixInput>>,
    )>,
}
impl CrosstermControl {
    pub fn new(output: crate::streams::Output) -> Self {
        Self {
            output,
            #[cfg(unix)]
            custom: None,
        }
    }
    #[cfg(unix)]
    pub fn with_custom(
        mut self,
        modes: crate::streams::UnixModes,
        input: Arc<Mutex<crate::streams::UnixInput>>,
    ) -> Self {
        self.custom = Some((modes, input));
        self
    }
}

impl TerminalControl for CrosstermControl {
    fn check_terminal(&mut self) -> Result<(), SessionError> {
        #[cfg(unix)]
        if let Some((modes, _)) = &self.custom {
            return if modes.is_terminal() {
                Ok(())
            } else {
                Err(SessionError::NotTerminal)
            };
        }
        if io::stdin().is_terminal() && io::stdout().is_terminal() {
            Ok(())
        } else {
            Err(SessionError::NotTerminal)
        }
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        #[cfg(unix)]
        if let Some((modes, _)) = &self.custom {
            return modes.raw_active();
        }
        terminal::is_raw_mode_enabled()
    }

    fn keyboard_supported(&mut self) -> io::Result<bool> {
        #[cfg(unix)]
        if let Some((_, input)) = &self.custom {
            return lock(input).keyboard_supported(&mut self.output);
        }
        terminal::supports_keyboard_enhancement()
    }

    fn enable_mouse(&mut self) -> io::Result<()> {
        execute!(self.output, EnableMouseCapture)
    }

    fn disable_mouse(&mut self) -> io::Result<()> {
        execute!(self.output, DisableMouseCapture)
    }

    fn enable_paste(&mut self) -> io::Result<()> {
        execute!(self.output, EnableBracketedPaste)
    }

    fn disable_paste(&mut self) -> io::Result<()> {
        execute!(self.output, DisableBracketedPaste)
    }

    fn enable_focus(&mut self) -> io::Result<()> {
        execute!(self.output, EnableFocusChange)
    }

    fn disable_focus(&mut self) -> io::Result<()> {
        execute!(self.output, DisableFocusChange)
    }

    fn push_keyboard(&mut self) -> io::Result<()> {
        execute!(
            self.output,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )
    }

    fn pop_keyboard(&mut self) -> io::Result<()> {
        execute!(self.output, PopKeyboardEnhancementFlags)
    }

    fn enable_raw(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        if let Some((modes, _)) = &mut self.custom {
            return modes.enable_raw();
        }
        terminal::enable_raw_mode()
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        execute!(self.output, EnterAlternateScreen)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.output, Hide)
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        execute!(self.output, Clear(ClearType::All))
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.output, Show)
    }

    fn reset_attributes(&mut self) -> io::Result<()> {
        execute!(self.output, SetAttribute(Attribute::Reset))
    }

    fn reset_colors(&mut self) -> io::Result<()> {
        execute!(self.output, ResetColor)
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        execute!(self.output, LeaveAlternateScreen)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        if let Some((modes, _)) = &mut self.custom {
            return modes.restore();
        }
        terminal::disable_raw_mode()
    }
}

pub(crate) struct Modes {
    control: Box<dyn TerminalControl>,
    owner: Option<OwnerGuard>,
    raw_attempted: bool,
    alternate_attempted: bool,
    cursor_attempted: bool,
    mouse_attempted: bool,
    paste_attempted: bool,
    focus_attempted: bool,
    keyboard_attempted: bool,
}

impl Modes {
    pub fn new(control: Box<dyn TerminalControl>, owner: OwnerGuard) -> Self {
        Self {
            control,
            owner: Some(owner),
            raw_attempted: false,
            alternate_attempted: false,
            cursor_attempted: false,
            mouse_attempted: false,
            paste_attempted: false,
            focus_attempted: false,
            keyboard_attempted: false,
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
        // Probe in raw mode before starting the sole reader. Crossterm's probe
        // reads terminal responses and would otherwise race with event polling.
        if options.enhanced_keyboard
            && !self
                .control
                .keyboard_supported()
                .map_err(|e| SessionError::io("query enhanced keyboard support", e))?
        {
            return Err(SessionError::UnsupportedCapability("enhanced keyboard"));
        }
        if options.alternate_screen {
            self.alternate_attempted = true;
            self.control
                .enter_alternate()
                .map_err(|e| SessionError::io("enter alternate screen", e))?;
        }
        if options.mouse_capture {
            self.mouse_attempted = true;
            self.control
                .enable_mouse()
                .map_err(|e| SessionError::io("enable mouse capture", e))?;
        }
        if options.bracketed_paste {
            self.paste_attempted = true;
            self.control
                .enable_paste()
                .map_err(|e| SessionError::io("enable bracketed paste", e))?;
        }
        if options.focus_reporting {
            self.focus_attempted = true;
            self.control
                .enable_focus()
                .map_err(|e| SessionError::io("enable focus reporting", e))?;
        }
        if options.enhanced_keyboard {
            self.keyboard_attempted = true;
            self.control
                .push_keyboard()
                .map_err(|e| SessionError::io("push keyboard enhancement", e))?;
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
        if self.keyboard_attempted {
            cleanup_step(&mut failures, "pop keyboard enhancement", || {
                self.control.pop_keyboard()
            });
        }
        if self.focus_attempted {
            cleanup_step(&mut failures, "disable focus reporting", || {
                self.control.disable_focus()
            });
        }
        if self.paste_attempted {
            cleanup_step(&mut failures, "disable bracketed paste", || {
                self.control.disable_paste()
            });
        }
        if self.mouse_attempted {
            cleanup_step(&mut failures, "disable mouse capture", || {
                self.control.disable_mouse()
            });
        }
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
        if self.raw_attempted {
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
