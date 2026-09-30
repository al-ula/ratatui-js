use std::time::Duration;

use ratatui_js_crossterm::{EventPoll, KeyCode, KeyKind, Session, SessionOptions, TerminalEvent};

const FRAME: &[u8] = br#"{
    "protocolVersion": 1,
    "root": {
        "type": "block",
        "title": "ratatui-js",
        "border": "rounded",
        "child": {
            "type": "paragraph",
            "lines": [[{"text": "Hello from Crossterm. Press q to quit."}]],
            "wrap": true
        }
    }
}"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let alternate_screen = !std::env::args().any(|arg| arg == "--no-alternate-screen");
    let session = Session::open(SessionOptions { alternate_screen })?;
    let outcome = (|| {
        session.render_json(FRAME)?;
        loop {
            match session.poll_event(Duration::from_secs(1))? {
                EventPoll::Event(TerminalEvent::Key {
                    key: KeyCode::Character { value: 'q' },
                    kind: KeyKind::Press | KeyKind::Repeat,
                    modifiers,
                }) if modifiers.is_empty() => break,
                EventPoll::Event(TerminalEvent::Resize { .. }) => {
                    session.render_json(FRAME)?;
                }
                EventPoll::Closed => break,
                _ => {}
            }
        }
        Ok::<_, ratatui_js_crossterm::SessionError>(())
    })();
    // Observe cleanup failure even when the application operation failed too.
    let cleanup = session.close();
    match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup: {cleanup}").into()),
    }
}
