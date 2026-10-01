//! Opt-in Unix PTY tests. Python supplies the PTY/termios operations without
//! adding an unsafe test-only FFI boundary or another native dependency.
#![cfg(unix)]

use std::{
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use ratatui::crossterm::terminal;
use ratatui_js_crossterm::{
    EventPoll, KeyCode, KeyKind, Session, SessionError, SessionOptions, TerminalEvent,
};

const FRAME: &[u8] =
    br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[[{"text":"PTY frame"}]]}}"#;

#[test]
#[ignore = "requires Unix PTY support and python3; run explicitly"]
fn real_terminal_lifecycle() {
    let output = Command::new("python3")
        .arg("-c")
        .arg(include_str!("terminal_pty.py"))
        .arg(std::env::current_exe().unwrap())
        .output()
        .expect("python3 is required for PTY tests");
    assert!(
        output.status.success(),
        "PTY tests failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
#[ignore = "child fixture; invoked by the PTY harness"]
fn pty_child() {
    let Ok(scenario) = std::env::var("RATATUI_JS_PTY_SCENARIO") else {
        return;
    };
    if scenario == "raw-active" {
        terminal::enable_raw_mode().unwrap();
        assert!(matches!(
            Session::open(SessionOptions::default()),
            Err(SessionError::RawModeActive)
        ));
        assert!(terminal::is_raw_mode_enabled().unwrap());
        terminal::disable_raw_mode().unwrap();
        return;
    }
    let session = Arc::new(
        Session::open(SessionOptions {
            alternate_screen: scenario != "no-alternate",
        })
        .unwrap(),
    );
    assert!(terminal::is_raw_mode_enabled().unwrap());
    assert!(matches!(
        Session::open(SessionOptions::default()),
        Err(SessionError::TerminalBusy)
    ));
    assert!(session.render_json(b"{").is_err());
    let initial = session.render_json(FRAME).unwrap();
    assert_eq!((initial.width, initial.height), (80, 24));

    if scenario == "close-wait" {
        let waiting = Arc::clone(&session);
        let consumer = thread::spawn(move || waiting.poll_event(Duration::from_secs(60)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !matches!(
            session.poll_event(Duration::ZERO),
            Err(SessionError::ConcurrentEventWait)
        ) {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        session.render_json(FRAME).unwrap();
        session.close().unwrap();
        assert_eq!(consumer.join().unwrap().unwrap(), EventPoll::Closed);
        session.close().unwrap();
    } else if scenario == "drop" {
        drop(session);
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        Session::open(SessionOptions::default())
            .unwrap()
            .close()
            .unwrap();
    } else {
        // Test-only readiness markers are outside the rendering API. The resize
        // below forces a complete redraw before inspecting the new dimensions.
        println!("PTY_READY\r");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            assert!(Instant::now() < deadline, "parent did not send input");
            match session.poll_event(Duration::from_millis(100)).unwrap() {
                EventPoll::Event(TerminalEvent::Resize { width, height }) => {
                    assert_eq!((width, height), (90, 30));
                    let result = session.render_json(FRAME).unwrap();
                    assert_eq!((result.width, result.height), (90, 30));
                    println!("PTY_RESIZED\r");
                }
                EventPoll::Event(TerminalEvent::Key {
                    key: KeyCode::Character { value: 'q' },
                    kind: KeyKind::Press,
                    modifiers,
                }) => {
                    assert!(modifiers.is_empty());
                    break;
                }
                EventPoll::Timeout => {}
                other => panic!("unexpected input: {other:?}"),
            }
        }
        session.close().unwrap();
        assert!(matches!(
            session.render_json(FRAME),
            Err(SessionError::Closed)
        ));
    }
    assert!(!terminal::is_raw_mode_enabled().unwrap());
}

#[test]
fn redirected_stdio_is_rejected() {
    // Use a child so the test is reliable even if the parent was run in a TTY.
    if std::env::var_os("RATATUI_JS_REDIRECTED_CHILD").is_some() {
        assert!(matches!(
            Session::open(SessionOptions::default()),
            Err(SessionError::NotTerminal)
        ));
        return;
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "redirected_stdio_is_rejected", "--nocapture"])
        .env("RATATUI_JS_REDIRECTED_CHILD", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
