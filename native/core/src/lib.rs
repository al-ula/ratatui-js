//! Runtime-independent frame decoding and rendering. Terminal control and FFI
//! belong in separate layers; no application state or loop lives here.

mod error;
mod protocol;
mod renderer;

pub use error::{ErrorCode, ErrorDescription, RenderError};
pub use protocol::{
    Border, Color, Constraint, FrameDescription, LayoutChild, MAX_COLLECTION_ITEMS, MAX_DEPTH,
    MAX_FRAME_BYTES, MAX_NODES, MAX_SPANS, Node, PROTOCOL_VERSION, Padding, RenderResult, Style,
    TextLine, TextSpan, WidgetStateUpdate,
};
pub use renderer::Renderer;
