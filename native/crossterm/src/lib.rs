//! Crossterm terminal ownership, rendering, input, and explicit shutdown.
//!
//! One session is allowed per loaded library instance. The host must not read
//! terminal input or change terminal modes concurrently. No application loop,
//! interrupt handler, or global panic hook is installed by this crate.

mod error;
mod events;
mod input;
mod session;
mod streams;
mod terminal;
pub use streams::TerminalStream;

pub use error::{CleanupDescription, CleanupFailure, ErrorDescription, SessionError};
pub use events::{
    KeyCode, KeyKind, KeyModifier, KeyState, MediaKeyCode, ModifierKeyCode, MouseButton, MouseKind,
    TerminalEvent,
};
pub use session::{EventPoll, Session, SessionOptions};
