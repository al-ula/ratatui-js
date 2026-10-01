use ratatui::crossterm::event::{
    Event, KeyCode as NativeKey, KeyEventKind, KeyEventState, KeyModifiers,
    MediaKeyCode as NativeMedia, ModifierKeyCode as NativeModifier, MouseButton as NativeButton,
    MouseEventKind,
};
use serde::Serialize;

/// JSON representation matches `packages/protocol/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum KeyCode {
    Character { value: char },
    Function { value: u8 },
    Media { value: MediaKeyCode },
    Modifier { value: ModifierKeyCode },
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
    Null,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    Menu,
    KeypadBegin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaKeyCode {
    Play,
    Pause,
    PlayPause,
    Reverse,
    Stop,
    FastForward,
    Rewind,
    TrackNext,
    TrackPrevious,
    Record,
    LowerVolume,
    RaiseVolume,
    MuteVolume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModifierKeyCode {
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftSuper,
    LeftHyper,
    LeftMeta,
    RightShift,
    RightControl,
    RightAlt,
    RightSuper,
    RightHyper,
    RightMeta,
    IsoLevel3Shift,
    IsoLevel5Shift,
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
    Hyper,
    Meta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyState {
    Keypad,
    CapsLock,
    NumLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MouseKind {
    Down { button: MouseButton },
    Up { button: MouseButton },
    Drag { button: MouseButton },
    Moved,
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
    Key {
        key: KeyCode,
        kind: KeyKind,
        modifiers: Vec<KeyModifier>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        state: Vec<KeyState>,
    },
    Mouse {
        kind: MouseKind,
        column: u16,
        row: u16,
        modifiers: Vec<KeyModifier>,
    },
    Paste {
        text: String,
    },
    Focus {
        focused: bool,
    },
    Resize {
        width: u16,
        height: u16,
    },
}

// Reject unknown flags as a whole: stripping one can turn a modified key into
// an unrelated application shortcut. The same invariant applies to the mouse.
fn normalize_modifiers(flags: KeyModifiers) -> Option<Vec<KeyModifier>> {
    if !flags.difference(KeyModifiers::all()).is_empty() {
        return None;
    }
    Some(
        [
            (KeyModifiers::SHIFT, KeyModifier::Shift),
            (KeyModifiers::CONTROL, KeyModifier::Control),
            (KeyModifiers::ALT, KeyModifier::Alt),
            (KeyModifiers::SUPER, KeyModifier::Super),
            (KeyModifiers::HYPER, KeyModifier::Hyper),
            (KeyModifiers::META, KeyModifier::Meta),
        ]
        .into_iter()
        .filter_map(|(flag, modifier)| flags.contains(flag).then_some(modifier))
        .collect(),
    )
}

fn normalize_button(button: NativeButton) -> MouseButton {
    match button {
        NativeButton::Left => MouseButton::Left,
        NativeButton::Right => MouseButton::Right,
        NativeButton::Middle => MouseButton::Middle,
    }
}

pub(crate) fn normalize(event: Event) -> Option<TerminalEvent> {
    Some(match event {
        Event::Resize(width, height) => TerminalEvent::Resize { width, height },
        Event::Paste(text) => TerminalEvent::Paste { text },
        Event::FocusGained => TerminalEvent::Focus { focused: true },
        Event::FocusLost => TerminalEvent::Focus { focused: false },
        Event::Mouse(event) => TerminalEvent::Mouse {
            kind: match event.kind {
                MouseEventKind::Down(button) => MouseKind::Down {
                    button: normalize_button(button),
                },
                MouseEventKind::Up(button) => MouseKind::Up {
                    button: normalize_button(button),
                },
                MouseEventKind::Drag(button) => MouseKind::Drag {
                    button: normalize_button(button),
                },
                MouseEventKind::Moved => MouseKind::Moved,
                MouseEventKind::ScrollUp => MouseKind::ScrollUp,
                MouseEventKind::ScrollDown => MouseKind::ScrollDown,
                MouseEventKind::ScrollLeft => MouseKind::ScrollLeft,
                MouseEventKind::ScrollRight => MouseKind::ScrollRight,
            },
            column: event.column,
            row: event.row,
            modifiers: normalize_modifiers(event.modifiers)?,
        },
        Event::Key(event) => {
            if !event.state.difference(KeyEventState::all()).is_empty() {
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
                NativeKey::Null => KeyCode::Null,
                NativeKey::CapsLock => KeyCode::CapsLock,
                NativeKey::ScrollLock => KeyCode::ScrollLock,
                NativeKey::NumLock => KeyCode::NumLock,
                NativeKey::PrintScreen => KeyCode::PrintScreen,
                NativeKey::Pause => KeyCode::Pause,
                NativeKey::Menu => KeyCode::Menu,
                NativeKey::KeypadBegin => KeyCode::KeypadBegin,
                NativeKey::Media(value) => KeyCode::Media {
                    value: match value {
                        NativeMedia::Play => MediaKeyCode::Play,
                        NativeMedia::Pause => MediaKeyCode::Pause,
                        NativeMedia::PlayPause => MediaKeyCode::PlayPause,
                        NativeMedia::Reverse => MediaKeyCode::Reverse,
                        NativeMedia::Stop => MediaKeyCode::Stop,
                        NativeMedia::FastForward => MediaKeyCode::FastForward,
                        NativeMedia::Rewind => MediaKeyCode::Rewind,
                        NativeMedia::TrackNext => MediaKeyCode::TrackNext,
                        NativeMedia::TrackPrevious => MediaKeyCode::TrackPrevious,
                        NativeMedia::Record => MediaKeyCode::Record,
                        NativeMedia::LowerVolume => MediaKeyCode::LowerVolume,
                        NativeMedia::RaiseVolume => MediaKeyCode::RaiseVolume,
                        NativeMedia::MuteVolume => MediaKeyCode::MuteVolume,
                    },
                },
                NativeKey::Modifier(value) => KeyCode::Modifier {
                    value: match value {
                        NativeModifier::LeftShift => ModifierKeyCode::LeftShift,
                        NativeModifier::LeftControl => ModifierKeyCode::LeftControl,
                        NativeModifier::LeftAlt => ModifierKeyCode::LeftAlt,
                        NativeModifier::LeftSuper => ModifierKeyCode::LeftSuper,
                        NativeModifier::LeftHyper => ModifierKeyCode::LeftHyper,
                        NativeModifier::LeftMeta => ModifierKeyCode::LeftMeta,
                        NativeModifier::RightShift => ModifierKeyCode::RightShift,
                        NativeModifier::RightControl => ModifierKeyCode::RightControl,
                        NativeModifier::RightAlt => ModifierKeyCode::RightAlt,
                        NativeModifier::RightSuper => ModifierKeyCode::RightSuper,
                        NativeModifier::RightHyper => ModifierKeyCode::RightHyper,
                        NativeModifier::RightMeta => ModifierKeyCode::RightMeta,
                        NativeModifier::IsoLevel3Shift => ModifierKeyCode::IsoLevel3Shift,
                        NativeModifier::IsoLevel5Shift => ModifierKeyCode::IsoLevel5Shift,
                    },
                },
            };
            let kind = match event.kind {
                KeyEventKind::Press => KeyKind::Press,
                KeyEventKind::Repeat => KeyKind::Repeat,
                KeyEventKind::Release => KeyKind::Release,
            };
            let state = [
                (KeyEventState::KEYPAD, KeyState::Keypad),
                (KeyEventState::CAPS_LOCK, KeyState::CapsLock),
                (KeyEventState::NUM_LOCK, KeyState::NumLock),
            ]
            .into_iter()
            .filter_map(|(flag, state)| event.state.contains(flag).then_some(state))
            .collect();
            TerminalEvent::Key {
                key,
                kind,
                modifiers: normalize_modifiers(event.modifiers)?,
                state,
            }
        }
    })
}

#[cfg(test)]
mod tests;
