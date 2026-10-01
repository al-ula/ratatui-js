use super::*;

#[derive(Default)]
pub(crate) struct MockState {
    pub calls: Vec<&'static str>,
    pub failures: Vec<&'static str>,
    pub panics: Vec<&'static str>,
    pub not_terminal: bool,
    pub raw_enabled: bool,
    pub keyboard_supported: bool,
}

pub(crate) struct MockControl(pub Arc<Mutex<MockState>>);

impl MockControl {
    fn call(&self, operation: &'static str) -> io::Result<()> {
        let mut state = lock(&self.0);
        state.calls.push(operation);
        let fail = state.failures.contains(&operation);
        let panic = state.panics.contains(&operation);
        drop(state);
        assert!(!panic, "injected {operation} panic");
        if fail {
            Err(io::Error::other(format!("injected {operation} failure")))
        } else {
            Ok(())
        }
    }
}

impl TerminalControl for MockControl {
    fn check_terminal(&mut self) -> Result<(), SessionError> {
        self.call("check terminal")
            .map_err(|e| SessionError::io("check terminal", e))?;
        if lock(&self.0).not_terminal {
            Err(SessionError::NotTerminal)
        } else {
            Ok(())
        }
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        self.call("check raw mode")?;
        Ok(lock(&self.0).raw_enabled)
    }

    fn keyboard_supported(&mut self) -> io::Result<bool> {
        self.call("query enhanced keyboard support")?;
        Ok(lock(&self.0).keyboard_supported)
    }

    fn enable_mouse(&mut self) -> io::Result<()> {
        self.call("enable mouse capture")
    }

    fn disable_mouse(&mut self) -> io::Result<()> {
        self.call("disable mouse capture")
    }

    fn enable_paste(&mut self) -> io::Result<()> {
        self.call("enable bracketed paste")
    }

    fn disable_paste(&mut self) -> io::Result<()> {
        self.call("disable bracketed paste")
    }

    fn enable_focus(&mut self) -> io::Result<()> {
        self.call("enable focus reporting")
    }

    fn disable_focus(&mut self) -> io::Result<()> {
        self.call("disable focus reporting")
    }

    fn push_keyboard(&mut self) -> io::Result<()> {
        self.call("push keyboard enhancement")
    }

    fn pop_keyboard(&mut self) -> io::Result<()> {
        self.call("pop keyboard enhancement")
    }

    fn enable_raw(&mut self) -> io::Result<()> {
        self.call("enable raw mode")
    }

    fn enter_alternate(&mut self) -> io::Result<()> {
        self.call("enter alternate screen")
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.call("hide cursor")
    }

    fn clear_screen(&mut self) -> io::Result<()> {
        self.call("clear initial screen")
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.call("show cursor")
    }

    fn reset_attributes(&mut self) -> io::Result<()> {
        self.call("reset attributes")
    }

    fn reset_colors(&mut self) -> io::Result<()> {
        self.call("reset colors")
    }

    fn leave_alternate(&mut self) -> io::Result<()> {
        self.call("leave alternate screen")
    }

    fn flush(&mut self) -> io::Result<()> {
        self.call("flush terminal")
    }

    fn disable_raw(&mut self) -> io::Result<()> {
        self.call("disable raw mode")
    }
}

fn fixture() -> (Modes, Arc<Mutex<MockState>>, Arc<Mutex<Ownership>>) {
    let state = Arc::new(Mutex::new(MockState::default()));
    let slot = Arc::new(Mutex::new(Ownership::Free));
    let modes = Modes::new(
        Box::new(MockControl(Arc::clone(&state))),
        OwnerGuard::acquire(Arc::clone(&slot)).unwrap(),
    );
    (modes, state, slot)
}

#[test]
fn ownership_rejects_a_second_owner_and_poisoned_reuse() {
    let slot = Arc::new(Mutex::new(Ownership::Free));
    let mut owner = OwnerGuard::acquire(Arc::clone(&slot)).unwrap();
    assert!(matches!(
        OwnerGuard::acquire(Arc::clone(&slot)),
        Err(SessionError::TerminalBusy)
    ));
    owner.restored = false;
    drop(owner);
    assert!(matches!(
        OwnerGuard::acquire(slot),
        Err(SessionError::TerminalPoisoned)
    ));
}

#[test]
fn preflight_failures_do_not_modify_or_restore_modes() {
    for failure in ["not terminal", "raw active", "check raw mode"] {
        let (mut modes, state, slot) = fixture();
        {
            let mut state = lock(&state);
            state.not_terminal = failure == "not terminal";
            state.raw_enabled = failure == "raw active";
            if failure == "check raw mode" {
                state.failures.push(failure);
            }
        }
        assert!(modes.initialize(SessionOptions::default()).is_err());
        assert!(modes.restore().is_empty());
        assert!(
            lock(&state)
                .calls
                .iter()
                .all(|call| call.starts_with("check"))
        );
        assert_eq!(*lock(&slot), Ownership::Free);
    }
}

#[test]
fn every_attempted_initialization_step_is_rolled_back() {
    for failure in [
        "enable raw mode",
        "enter alternate screen",
        "hide cursor",
        "clear initial screen",
    ] {
        let (mut modes, state, slot) = fixture();
        lock(&state).failures.push(failure);
        assert!(modes.initialize(SessionOptions::default()).is_err());
        assert!(modes.restore().is_empty());
        let calls = &lock(&state).calls;
        assert!(calls.contains(&"disable raw mode"));
        if failure != "enable raw mode" {
            assert!(calls.contains(&"leave alternate screen"));
            assert!(calls.contains(&"reset attributes"));
            assert!(calls.contains(&"reset colors"));
            assert!(calls.contains(&"flush terminal"));
        }
        if failure == "hide cursor" || failure == "clear initial screen" {
            assert!(calls.contains(&"show cursor"));
        }
        assert_eq!(*lock(&slot), Ownership::Free);
    }
}

#[test]
fn every_cleanup_failure_is_reported_without_skipping_later_steps() {
    let steps = [
        "show cursor",
        "reset attributes",
        "reset colors",
        "leave alternate screen",
        "flush terminal",
        "disable raw mode",
    ];
    for failure in steps {
        let (mut modes, state, slot) = fixture();
        modes.initialize(SessionOptions::default()).unwrap();
        lock(&state).failures.push(failure);
        let errors = modes.restore();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].operation, failure);
        for step in steps {
            assert!(lock(&state).calls.contains(&step));
        }
        assert_eq!(*lock(&slot), Ownership::Poisoned);
    }
}

#[test]
fn cleanup_aggregates_errors_and_does_not_retry_on_drop() {
    let (mut modes, state, slot) = fixture();
    modes.initialize(SessionOptions::default()).unwrap();
    lock(&state).failures = vec!["show cursor", "disable raw mode"];
    assert_eq!(modes.restore().len(), 2);
    let before = lock(&state).calls.clone();
    drop(modes);
    assert_eq!(lock(&state).calls, before);
    assert_eq!(*lock(&slot), Ownership::Poisoned);
}

#[test]
fn no_alternate_screen_skips_both_screen_operations() {
    let (mut modes, state, slot) = fixture();
    modes
        .initialize(SessionOptions {
            alternate_screen: false,
            ..SessionOptions::default()
        })
        .unwrap();
    drop(modes);
    let calls = &lock(&state).calls;
    assert!(!calls.contains(&"enter alternate screen"));
    assert!(!calls.contains(&"leave alternate screen"));
    assert!(calls.contains(&"show cursor"));
    assert!(calls.contains(&"disable raw mode"));
    assert_eq!(*lock(&slot), Ownership::Free);
}

#[test]
fn cleanup_panics_are_reported_and_later_cleanup_still_runs() {
    let (mut modes, state, slot) = fixture();
    modes.initialize(SessionOptions::default()).unwrap();
    lock(&state).panics = vec!["show cursor", "reset attributes"];
    let errors = modes.restore();
    assert_eq!(errors.len(), 2);
    assert!(
        errors
            .iter()
            .all(|failure| failure.message.contains("panicked"))
    );
    assert!(lock(&state).calls.contains(&"disable raw mode"));
    assert_eq!(*lock(&slot), Ownership::Poisoned);
}

#[test]
fn initial_style_failures_still_attempt_all_restoration_steps() {
    for failure in ["reset attributes", "reset colors"] {
        let (mut modes, state, slot) = fixture();
        lock(&state).failures.push(failure);
        assert!(modes.initialize(SessionOptions::default()).is_err());
        assert_eq!(modes.restore().len(), 1);
        assert!(lock(&state).calls.contains(&"show cursor"));
        assert!(lock(&state).calls.contains(&"leave alternate screen"));
        assert!(lock(&state).calls.contains(&"disable raw mode"));
        assert_eq!(*lock(&slot), Ownership::Poisoned);
    }
}

fn extended_options() -> SessionOptions {
    SessionOptions {
        mouse_capture: true,
        bracketed_paste: true,
        focus_reporting: true,
        enhanced_keyboard: true,
        ..SessionOptions::default()
    }
}

#[test]
fn default_sessions_do_not_probe_or_enable_extended_modes() {
    let (mut modes, state, _) = fixture();
    modes.initialize(SessionOptions::default()).unwrap();
    modes.restore();
    assert!(!lock(&state).calls.iter().any(|call| call.contains("mouse")
        || call.contains("paste")
        || call.contains("focus")
        || call.contains("keyboard")));
}

#[test]
fn unsupported_keyboard_and_probe_failures_restore_raw_mode() {
    for failure in [None, Some("query enhanced keyboard support")] {
        let (mut modes, state, slot) = fixture();
        if let Some(failure) = failure {
            lock(&state).failures.push(failure);
        }
        let error = modes.initialize(extended_options()).unwrap_err();
        if failure.is_none() {
            assert!(matches!(
                error,
                SessionError::UnsupportedCapability("enhanced keyboard")
            ));
            assert_eq!(error.description().code, "unsupportedCapability");
        }
        assert!(modes.restore().is_empty());
        let calls = &lock(&state).calls;
        assert!(calls.contains(&"disable raw mode"));
        assert!(!calls.contains(&"push keyboard enhancement"));
        assert!(!calls.contains(&"pop keyboard enhancement"));
        assert!(!calls.contains(&"enter alternate screen"));
        assert_eq!(*lock(&slot), Ownership::Free);
    }
}

#[test]
fn extended_initialization_errors_and_panics_restore_every_attempted_mode() {
    let steps = [
        ("enable mouse capture", "disable mouse capture"),
        ("enable bracketed paste", "disable bracketed paste"),
        ("enable focus reporting", "disable focus reporting"),
        ("push keyboard enhancement", "pop keyboard enhancement"),
    ];
    for (index, (enable, _)) in steps.iter().enumerate() {
        for panic in [false, true] {
            let (mut modes, state, slot) = fixture();
            {
                let mut state = lock(&state);
                state.keyboard_supported = true;
                if panic {
                    state.panics.push(enable);
                } else {
                    state.failures.push(enable);
                }
            }
            let outcome = catch_unwind(AssertUnwindSafe(|| modes.initialize(extended_options())));
            assert!(if panic {
                outcome.is_err()
            } else {
                outcome.unwrap().is_err()
            });
            assert!(modes.restore().is_empty());
            let calls = &lock(&state).calls;
            for (_, disable) in &steps[..=index] {
                assert!(calls.contains(disable));
            }
            for (_, disable) in &steps[index + 1..] {
                assert!(!calls.contains(disable));
            }
            assert!(calls.contains(&"leave alternate screen"));
            assert!(calls.contains(&"disable raw mode"));
            assert_eq!(*lock(&slot), Ownership::Free);
        }
    }
}

#[test]
fn extended_cleanup_continues_after_failures_and_panics_and_poisons_ownership() {
    let steps = [
        "pop keyboard enhancement",
        "disable focus reporting",
        "disable bracketed paste",
        "disable mouse capture",
    ];
    for failure in steps {
        for panic in [false, true] {
            let (mut modes, state, slot) = fixture();
            lock(&state).keyboard_supported = true;
            modes.initialize(extended_options()).unwrap();
            if panic {
                lock(&state).panics.push(failure);
            } else {
                lock(&state).failures.push(failure);
            }
            let errors = modes.restore();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].operation, failure);
            for step in steps {
                assert!(lock(&state).calls.contains(&step));
            }
            assert!(lock(&state).calls.contains(&"leave alternate screen"));
            assert!(lock(&state).calls.contains(&"disable raw mode"));
            assert_eq!(*lock(&slot), Ownership::Poisoned);
            let before = lock(&state).calls.clone();
            drop(modes);
            assert_eq!(lock(&state).calls, before);
        }
    }
}
