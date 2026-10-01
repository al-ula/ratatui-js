use super::*;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};
use serde_json::json;

#[test]
fn all_native_events_serialize_to_shared_typescript_fixtures() {
    let mut events = vec![
        Event::Resize(65535, 0),
        Event::FocusGained,
        Event::FocusLost,
        Event::Paste("界\n\t\u{1b}😀".into()),
        Event::Paste(String::new()),
    ];
    let keys = [
        NativeKey::Enter,
        NativeKey::Esc,
        NativeKey::Backspace,
        NativeKey::Tab,
        NativeKey::BackTab,
        NativeKey::Left,
        NativeKey::Right,
        NativeKey::Up,
        NativeKey::Down,
        NativeKey::Home,
        NativeKey::End,
        NativeKey::PageUp,
        NativeKey::PageDown,
        NativeKey::Insert,
        NativeKey::Delete,
        NativeKey::Null,
        NativeKey::CapsLock,
        NativeKey::ScrollLock,
        NativeKey::NumLock,
        NativeKey::PrintScreen,
        NativeKey::Pause,
        NativeKey::Menu,
        NativeKey::KeypadBegin,
        NativeKey::Char('😀'),
        NativeKey::F(255),
        NativeKey::Media(NativeMedia::Play),
        NativeKey::Media(NativeMedia::Pause),
        NativeKey::Media(NativeMedia::PlayPause),
        NativeKey::Media(NativeMedia::Reverse),
        NativeKey::Media(NativeMedia::Stop),
        NativeKey::Media(NativeMedia::FastForward),
        NativeKey::Media(NativeMedia::Rewind),
        NativeKey::Media(NativeMedia::TrackNext),
        NativeKey::Media(NativeMedia::TrackPrevious),
        NativeKey::Media(NativeMedia::Record),
        NativeKey::Media(NativeMedia::LowerVolume),
        NativeKey::Media(NativeMedia::RaiseVolume),
        NativeKey::Media(NativeMedia::MuteVolume),
        NativeKey::Modifier(NativeModifier::LeftShift),
        NativeKey::Modifier(NativeModifier::LeftControl),
        NativeKey::Modifier(NativeModifier::LeftAlt),
        NativeKey::Modifier(NativeModifier::LeftSuper),
        NativeKey::Modifier(NativeModifier::LeftHyper),
        NativeKey::Modifier(NativeModifier::LeftMeta),
        NativeKey::Modifier(NativeModifier::RightShift),
        NativeKey::Modifier(NativeModifier::RightControl),
        NativeKey::Modifier(NativeModifier::RightAlt),
        NativeKey::Modifier(NativeModifier::RightSuper),
        NativeKey::Modifier(NativeModifier::RightHyper),
        NativeKey::Modifier(NativeModifier::RightMeta),
        NativeKey::Modifier(NativeModifier::IsoLevel3Shift),
        NativeKey::Modifier(NativeModifier::IsoLevel5Shift),
    ];
    for code in keys {
        events.push(Event::Key(KeyEvent::new_with_kind_and_state(
            code,
            KeyModifiers::all(),
            KeyEventKind::Press,
            KeyEventState::all(),
        )));
    }
    for kind in [KeyEventKind::Repeat, KeyEventKind::Release] {
        events.push(Event::Key(KeyEvent::new_with_kind_and_state(
            NativeKey::Char('😀'),
            KeyModifiers::all(),
            kind,
            KeyEventState::all(),
        )));
    }
    for kind in [
        MouseEventKind::Down,
        MouseEventKind::Up,
        MouseEventKind::Drag,
    ] {
        for button in [
            NativeButton::Left,
            NativeButton::Right,
            NativeButton::Middle,
        ] {
            events.push(Event::Mouse(MouseEvent {
                kind: kind(button),
                column: 0,
                row: 65535,
                modifiers: KeyModifiers::all(),
            }));
        }
    }
    for kind in [
        MouseEventKind::Moved,
        MouseEventKind::ScrollUp,
        MouseEventKind::ScrollDown,
        MouseEventKind::ScrollLeft,
        MouseEventKind::ScrollRight,
    ] {
        events.push(Event::Mouse(MouseEvent {
            kind,
            column: 0,
            row: 65535,
            modifiers: KeyModifiers::NONE,
        }));
    }
    let actual: Vec<_> = events
        .into_iter()
        .map(|event| serde_json::to_value(normalize(event).unwrap()).unwrap())
        .collect();
    let fixtures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../../../../tests/fixtures/events.json")).unwrap();
    assert_eq!(actual, fixtures);
}

#[test]
fn basic_keys_keep_their_existing_wire_shape() {
    assert_eq!(
        serde_json::to_value(
            normalize(Event::Key(KeyEvent::new(
                NativeKey::Char('q'),
                KeyModifiers::NONE
            )))
            .unwrap()
        )
        .unwrap(),
        json!({"type":"key", "key":{"type":"character", "value":"q"}, "kind":"press", "modifiers":[]})
    );
}

#[test]
fn unknown_flags_are_dropped_without_changing_their_meaning() {
    assert_eq!(
        normalize(Event::Key(KeyEvent::new(
            NativeKey::Char('q'),
            KeyModifiers::from_bits_retain(0x80)
        ))),
        None
    );
    assert_eq!(
        normalize(Event::Key(KeyEvent::new_with_kind_and_state(
            NativeKey::Char('q'),
            KeyModifiers::NONE,
            KeyEventKind::Press,
            KeyEventState::from_bits_retain(0x80)
        ))),
        None
    );
    assert_eq!(
        normalize(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::from_bits_retain(0x80)
        })),
        None
    );
}
