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
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/extended-input.json"
        ))
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
    if scenario == "unsupported-keyboard" || scenario == "keyboard-probe-timeout" {
        let error = match Session::open(SessionOptions {
            enhanced_keyboard: true,
            ..SessionOptions::default()
        }) {
            Ok(_) => panic!("unsupported negotiation unexpectedly succeeded"),
            Err(error) => error,
        };
        if scenario == "unsupported-keyboard" {
            assert!(matches!(error, SessionError::UnsupportedCapability(_)));
        } else {
            assert!(matches!(
                error,
                SessionError::Io {
                    operation: "query enhanced keyboard support",
                    ..
                }
            ));
        }
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        let recovered = Session::open(SessionOptions::default()).unwrap();
        recovered.render_json(FRAME).unwrap();
        recovered.close().unwrap();
        return;
    }
    let extended = scenario.starts_with("extended");
    let session = Arc::new(
        Session::open(SessionOptions {
            alternate_screen: scenario != "no-alternate",
            mouse_capture: extended,
            bracketed_paste: extended,
            focus_reporting: extended,
            enhanced_keyboard: extended,
            ..SessionOptions::default()
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

    if extended {
        println!("PTY_READY\r");
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/extended-input.json"))
                .unwrap();
        for expected in fixture["events"].as_array().unwrap() {
            let EventPoll::Event(event) = session.poll_event(Duration::from_secs(5)).unwrap()
            else {
                panic!("missing extended event");
            };
            assert_eq!(&serde_json::to_value(event).unwrap(), expected);
        }
        if scenario == "extended-drop" {
            drop(session);
        } else {
            session.close().unwrap();
        }
        assert!(!terminal::is_raw_mode_enabled().unwrap());
        return;
    }
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
                    ..
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

#[test]
#[ignore = "requires Unix PTYs and python3"]
fn custom_terminal_lifecycle() {
    let output = Command::new("python3")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/integration/custom_streams.py"
        ))
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "custom_pty_child", "--ignored", "--nocapture"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
#[ignore = "invoked by the custom PTY harness"]
fn custom_pty_child() {
    use ratatui_js_crossterm::TerminalStream;
    use std::{
        fs::File,
        io::Write,
        os::fd::{AsRawFd, FromRawFd},
    };
    let Ok(scenario) = std::env::var("RATATUI_JS_CUSTOM_SCENARIO") else {
        return;
    };
    let fd: i32 = std::env::var("RATATUI_JS_CUSTOM_FD")
        .unwrap()
        .parse()
        .unwrap();
    // SAFETY: the supervisor explicitly transfers this inherited descriptor.
    let original = unsafe { File::from_raw_fd(fd) };
    let output_fd: i32 = std::env::var("RATATUI_JS_CUSTOM_OUTPUT_FD")
        .unwrap()
        .parse()
        .unwrap();
    // SAFETY: fcntl validates output_fd and returns a new owned descriptor.
    let marker_fd = unsafe { libc::fcntl(output_fd, libc::F_DUPFD_CLOEXEC, 3) };
    assert!(marker_fd >= 0);
    // SAFETY: marker_fd was just returned by fcntl and has no other owner.
    let mut marker = unsafe { File::from_raw_fd(marker_fd) };
    let selection = if scenario == "tty" {
        TerminalStream::Tty
    } else {
        TerminalStream::FileDescriptor(original.as_raw_fd())
    };
    let mut options = SessionOptions {
        input: selection,
        output: if scenario == "cleanup" {
            TerminalStream::FileDescriptor(output_fd)
        } else {
            selection
        },
        ..SessionOptions::default()
    };
    if scenario == "render-failure" {
        assert!(matches!(
            Session::open(options),
            Err(SessionError::Io {
                operation: "create renderer",
                ..
            })
        ));
        return;
    }
    if scenario == "invalid" {
        let invalid = original.try_clone().unwrap();
        let closed = invalid.as_raw_fd();
        drop(invalid);
        assert!(
            Session::open(SessionOptions {
                input: TerminalStream::FileDescriptor(closed),
                ..options
            })
            .is_err()
        );
        assert!(
            Session::open(SessionOptions {
                input: TerminalStream::FileDescriptor(-5),
                ..options
            })
            .is_err()
        );
        // Input duplication must not hide an invalid output by reusing its fd.
        assert!(
            Session::open(SessionOptions {
                output: TerminalStream::FileDescriptor(closed),
                ..options
            })
            .is_err()
        );
        let null = File::open("/dev/null").unwrap();
        assert!(matches!(
            Session::open(SessionOptions {
                input: TerminalStream::FileDescriptor(null.as_raw_fd()),
                ..options
            }),
            Err(SessionError::NotTerminal)
        ));
        assert!(
            Session::open(SessionOptions {
                output: TerminalStream::FileDescriptor(null.as_raw_fd()),
                ..options
            })
            .is_err()
        );
        let write_only = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/tty")
            .unwrap();
        assert!(
            Session::open(SessionOptions {
                input: TerminalStream::FileDescriptor(write_only.as_raw_fd()),
                ..options
            })
            .is_err()
        );
        let nonblocking = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/tty")
            .unwrap();
        // SAFETY: fcntl changes only this independently opened file description.
        assert_eq!(
            unsafe { libc::fcntl(nonblocking.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        assert!(
            Session::open(SessionOptions {
                input: TerminalStream::FileDescriptor(nonblocking.as_raw_fd()),
                ..options
            })
            .is_err()
        );
        Session::open(options).unwrap().close().unwrap();
        return;
    }
    if scenario == "rollback" {
        options.enhanced_keyboard = true;
        assert!(matches!(
            Session::open(options),
            Err(SessionError::UnsupportedCapability(_))
        ));
        options.enhanced_keyboard = false;
        Session::open(options).unwrap().close().unwrap();
        return;
    }
    if scenario == "extended" {
        options.enhanced_keyboard = true;
        options.mouse_capture = true;
        options.bracketed_paste = true;
        options.focus_reporting = true;
    }
    let session = Arc::new(Session::open(options).unwrap());
    drop(original); // The caller's fd is no longer available to the reader/control.
    assert!(matches!(
        Session::open(options),
        Err(SessionError::TerminalBusy)
    ));
    let rendered = session.render_json(FRAME).unwrap();
    assert_eq!((rendered.width, rendered.height), (80, 24));
    marker.write_all(b"CUSTOM_READY").unwrap();
    if scenario == "cleanup" {
        marker.write_all(b"CUSTOM_CLEANUP").unwrap();
        let _ = session.poll_event(Duration::from_secs(5));
        let error = session.close().unwrap_err();
        assert!(matches!(error, SessionError::Shutdown(_)));
        assert_eq!(
            session.close().unwrap_err().description(),
            error.description()
        );
        assert!(matches!(
            Session::open(options),
            Err(SessionError::TerminalPoisoned)
        ));
        return;
    }
    assert_eq!(
        session.poll_event(Duration::from_secs(5)).unwrap(),
        EventPoll::Event(TerminalEvent::Resize {
            width: 90,
            height: 30
        })
    );
    let rendered = session.render_json(FRAME).unwrap();
    assert_eq!((rendered.width, rendered.height), (90, 30));
    marker.write_all(b"CUSTOM_RESIZED").unwrap();
    if scenario == "extended" {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/extended-input.json"))
                .unwrap();
        for expected in fixture["events"].as_array().unwrap() {
            let EventPoll::Event(event) = session.poll_event(Duration::from_secs(5)).unwrap()
            else {
                panic!("missing custom extended event");
            };
            assert_eq!(&serde_json::to_value(event).unwrap(), expected);
        }
    } else {
        assert!(matches!(
            session.poll_event(Duration::from_secs(5)).unwrap(),
            EventPoll::Event(TerminalEvent::Key {
                key: KeyCode::Character { value: 'q' },
                ..
            })
        ));
    }
    if scenario == "drop" {
        drop(session);
        Session::open(SessionOptions {
            input: TerminalStream::FileDescriptor(marker.as_raw_fd()),
            output: TerminalStream::FileDescriptor(marker.as_raw_fd()),
            ..SessionOptions::default()
        })
        .unwrap()
        .close()
        .unwrap();
        return;
    }
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
    session.close().unwrap();
    assert_eq!(consumer.join().unwrap().unwrap(), EventPoll::Closed);
    session.close().unwrap();
    drop(session);
    // Reopen through a still-valid duplicate after releasing all session handles.
    Session::open(SessionOptions {
        input: TerminalStream::FileDescriptor(marker.as_raw_fd()),
        output: TerminalStream::FileDescriptor(marker.as_raw_fd()),
        ..SessionOptions::default()
    })
    .unwrap()
    .close()
    .unwrap();
}
