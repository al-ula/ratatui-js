use ratatui::crossterm::event::{Event, KeyCode as NativeKey, KeyEventKind, KeyModifiers};
use serde::Serialize;

/// JSON representation matches `packages/protocol/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum KeyCode {
    Character { value: char },
    Function { value: u8 },
    Enter,
    Escape,
    Backspace,
    Tab,
    BackTab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyKind {
    Press,
    Repeat,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyModifier {
    Shift,
    Control,
    Alt,
    Super,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
    Key {
        key: KeyCode,
        kind: KeyKind,
        modifiers: Vec<KeyModifier>,
    },
    Resize {
        width: u16,
        height: u16,
    },
}

pub(crate) fn normalize(event: Event) -> Option<TerminalEvent> {
    match event {
        Event::Resize(width, height) => Some(TerminalEvent::Resize { width, height }),
        Event::Key(event) => {
            // Dropping an unrepresentable modifier could turn a modified key
            // into an unrelated application shortcut (for example, plain q).
            let supported = KeyModifiers::SHIFT
                | KeyModifiers::CONTROL
                | KeyModifiers::ALT
                | KeyModifiers::SUPER;
            if !event.modifiers.difference(supported).is_empty() {
                return None;
            }
            let key = match event.code {
                NativeKey::Char(value) => KeyCode::Character { value },
                NativeKey::F(value) => KeyCode::Function { value },
                NativeKey::Enter => KeyCode::Enter,
                NativeKey::Esc => KeyCode::Escape,
                NativeKey::Backspace => KeyCode::Backspace,
                NativeKey::Tab => KeyCode::Tab,
                NativeKey::BackTab => KeyCode::BackTab,
                NativeKey::Left => KeyCode::Left,
                NativeKey::Right => KeyCode::Right,
                NativeKey::Up => KeyCode::Up,
                NativeKey::Down => KeyCode::Down,
                NativeKey::Home => KeyCode::Home,
                NativeKey::End => KeyCode::End,
                NativeKey::PageUp => KeyCode::PageUp,
                NativeKey::PageDown => KeyCode::PageDown,
                NativeKey::Insert => KeyCode::Insert,
                NativeKey::Delete => KeyCode::Delete,
                _ => return None,
            };
            let kind = match event.kind {
                KeyEventKind::Press => KeyKind::Press,
                KeyEventKind::Repeat => KeyKind::Repeat,
                KeyEventKind::Release => KeyKind::Release,
            };
            let modifiers = [
                (KeyModifiers::SHIFT, KeyModifier::Shift),
                (KeyModifiers::CONTROL, KeyModifier::Control),
                (KeyModifiers::ALT, KeyModifier::Alt),
                (KeyModifiers::SUPER, KeyModifier::Super),
            ]
            .into_iter()
            .filter_map(|(flag, modifier)| event.modifiers.contains(flag).then_some(modifier))
            .collect();
            Some(TerminalEvent::Key {
                key,
                kind,
                modifiers,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;
    use serde_json::json;

    use super::*;

    #[test]
    fn key_and_resize_json_match_the_typescript_contract() {
        let event = normalize(Event::Key(KeyEvent::new_with_kind(
            NativeKey::Char('界'),
            KeyModifiers::SHIFT | KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
            KeyEventKind::Repeat,
        )))
        .unwrap();
        assert_eq!(
            serde_json::to_value(event).unwrap(),
            json!({
                "type": "key", "key": {"type": "character", "value": "界"},
                "kind": "repeat", "modifiers": ["shift", "control", "alt", "super"]
            })
        );
        assert_eq!(
            serde_json::to_value(normalize(Event::Resize(80, 24)).unwrap()).unwrap(),
            json!({"type": "resize", "width": 80, "height": 24})
        );
    }

    #[test]
    fn all_supported_keys_and_kinds_are_preserved() {
        for (native, expected) in [
            (NativeKey::Enter, KeyCode::Enter),
            (NativeKey::Esc, KeyCode::Escape),
            (NativeKey::Backspace, KeyCode::Backspace),
            (NativeKey::Tab, KeyCode::Tab),
            (NativeKey::BackTab, KeyCode::BackTab),
            (NativeKey::Left, KeyCode::Left),
            (NativeKey::Right, KeyCode::Right),
            (NativeKey::Up, KeyCode::Up),
            (NativeKey::Down, KeyCode::Down),
            (NativeKey::Home, KeyCode::Home),
            (NativeKey::End, KeyCode::End),
            (NativeKey::PageUp, KeyCode::PageUp),
            (NativeKey::PageDown, KeyCode::PageDown),
            (NativeKey::Insert, KeyCode::Insert),
            (NativeKey::Delete, KeyCode::Delete),
            (NativeKey::Char('é'), KeyCode::Character { value: 'é' }),
            (NativeKey::F(12), KeyCode::Function { value: 12 }),
        ] {
            for (native_kind, kind) in [
                (KeyEventKind::Press, KeyKind::Press),
                (KeyEventKind::Repeat, KeyKind::Repeat),
                (KeyEventKind::Release, KeyKind::Release),
            ] {
                assert_eq!(
                    normalize(Event::Key(KeyEvent::new_with_kind(
                        native,
                        KeyModifiers::NONE,
                        native_kind,
                    ))),
                    Some(TerminalEvent::Key {
                        key: expected.clone(),
                        kind,
                        modifiers: vec![],
                    })
                );
            }
        }
    }

    #[test]
    fn unsupported_events_and_modifiers_are_not_misrepresented() {
        for event in [
            Event::FocusGained,
            Event::FocusLost,
            Event::Paste("text".into()),
            Event::Key(KeyEvent::new(NativeKey::Null, KeyModifiers::NONE)),
            Event::Key(KeyEvent::new(NativeKey::Char('q'), KeyModifiers::META)),
            Event::Key(KeyEvent::new(NativeKey::Char('q'), KeyModifiers::HYPER)),
            Event::Key(KeyEvent::new(
                NativeKey::Char('q'),
                KeyModifiers::from_bits_retain(0x80),
            )),
        ] {
            assert_eq!(normalize(event), None);
        }
    }
}
