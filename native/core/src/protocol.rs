use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::RenderError;

pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1_048_576;
pub const MAX_DEPTH: usize = 32;
pub const MAX_NODES: usize = 4_096;
pub const MAX_COLLECTION_ITEMS: usize = 4_096;
pub const MAX_SPANS: usize = 16_384;

// Omission means "not supplied"; explicit null is not part of protocol v1.
fn supplied<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Style {
    #[serde(
        default,
        deserialize_with = "supplied",
        skip_serializing_if = "Option::is_none"
    )]
    pub fg: Option<Color>,
    #[serde(
        default,
        deserialize_with = "supplied",
        skip_serializing_if = "Option::is_none"
    )]
    pub bg: Option<Color>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub dim: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(default)]
    pub underlined: bool,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub crossed_out: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpan {
    pub text: String,
    #[serde(default)]
    pub style: Style,
}

pub type TextLine = Vec<TextSpan>;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Constraint {
    Length { value: u16 },
    Min { value: u16 },
    Max { value: u16 },
    Percentage { value: u16 },
    Fill { value: u16 },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutChild {
    pub constraint: Constraint,
    pub node: Node,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Padding {
    #[serde(default)]
    pub left: u16,
    #[serde(default)]
    pub right: u16,
    #[serde(default)]
    pub top: u16,
    #[serde(default)]
    pub bottom: u16,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Border {
    #[default]
    Plain,
    Rounded,
    Double,
    None,
}

fn default_highlight_style() -> Style {
    Style {
        reversed: true,
        ..Style::default()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Node {
    Row {
        children: Vec<LayoutChild>,
        #[serde(default)]
        spacing: u16,
    },
    Column {
        children: Vec<LayoutChild>,
        #[serde(default)]
        spacing: u16,
    },
    Block {
        child: Box<Node>,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        title: Option<String>,
        #[serde(default)]
        border: Border,
        #[serde(default)]
        padding: Padding,
        #[serde(default)]
        style: Style,
    },
    Paragraph {
        lines: Vec<TextLine>,
        #[serde(default)]
        wrap: bool,
        #[serde(default)]
        style: Style,
    },
    List {
        id: String,
        items: Vec<TextLine>,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        selected: Option<u32>,
        #[serde(default)]
        offset: u32,
        #[serde(default)]
        style: Style,
        #[serde(default = "default_highlight_style")]
        highlight_style: Style,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrameDescription {
    pub protocol_version: u32,
    pub root: Node,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetStateUpdate {
    pub id: String,
    pub offset: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<u32>,
}

#[derive(Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub width: u16,
    pub height: u16,
    pub widget_states: Vec<WidgetStateUpdate>,
}

impl FrameDescription {
    pub fn decode(bytes: &[u8]) -> Result<Self, RenderError> {
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(RenderError::invalid("$", "encoded frame is too large"));
        }
        let value: serde_json::Value = serde_json::from_slice(bytes)?;
        let version = value
            .get("protocolVersion")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| {
                RenderError::invalid("$.protocolVersion", "expected an unsigned 32-bit integer")
            })?;
        if version != PROTOCOL_VERSION {
            return Err(RenderError::UnsupportedProtocol { received: version });
        }
        // Deserialize the original bytes so duplicate struct fields are rejected.
        let frame: Self = serde_json::from_slice(bytes)
            .map_err(|error| RenderError::invalid("$", error.to_string()))?;
        frame.validate()?;
        Ok(frame)
    }

    pub fn validate(&self) -> Result<(), RenderError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(RenderError::UnsupportedProtocol {
                received: self.protocol_version,
            });
        }
        ValidationState::default().node(&self.root, "$.root", 1)?;
        Ok(())
    }
}

#[derive(Default)]
struct ValidationState {
    nodes: usize,
    spans: usize,
    text_bytes: usize,
    ids: HashSet<String>,
}

impl ValidationState {
    fn collection(&self, len: usize, path: &str) -> Result<(), RenderError> {
        if len > MAX_COLLECTION_ITEMS {
            return Err(RenderError::invalid(path, "too many items"));
        }
        Ok(())
    }

    fn text(&mut self, text: &str, path: &str) -> Result<(), RenderError> {
        if text.chars().any(char::is_control) {
            return Err(RenderError::invalid(
                path,
                "control characters are not allowed; use explicit lines",
            ));
        }
        self.text_bytes += text.len();
        if self.text_bytes > MAX_FRAME_BYTES {
            return Err(RenderError::invalid(path, "too much text"));
        }
        Ok(())
    }

    fn line(&mut self, spans: &TextLine, path: &str) -> Result<(), RenderError> {
        self.collection(spans.len(), path)?;
        for (index, span) in spans.iter().enumerate() {
            self.spans += 1;
            if self.spans > MAX_SPANS {
                return Err(RenderError::invalid(path, "too many spans"));
            }
            self.text(&span.text, &format!("{path}[{index}].text"))?;
        }
        Ok(())
    }

    fn node(&mut self, node: &Node, path: &str, depth: usize) -> Result<(), RenderError> {
        if depth > MAX_DEPTH {
            return Err(RenderError::invalid(path, "maximum nesting depth exceeded"));
        }
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err(RenderError::invalid(path, "too many nodes"));
        }
        match node {
            Node::Row { children, .. } | Node::Column { children, .. } => {
                self.collection(children.len(), &format!("{path}.children"))?;
                for (index, child) in children.iter().enumerate() {
                    let child_path = format!("{path}.children[{index}]");
                    match child.constraint {
                        Constraint::Percentage { value } if value > 100 => {
                            return Err(RenderError::invalid(
                                &format!("{child_path}.constraint.value"),
                                "percentage exceeds 100",
                            ));
                        }
                        Constraint::Fill { value: 0 } => {
                            return Err(RenderError::invalid(
                                &format!("{child_path}.constraint.value"),
                                "fill must be positive",
                            ));
                        }
                        _ => {}
                    }
                    self.node(&child.node, &format!("{child_path}.node"), depth + 1)?;
                }
            }
            Node::Block { child, title, .. } => {
                if let Some(title) = title {
                    self.text(title, &format!("{path}.title"))?;
                }
                self.node(child, &format!("{path}.child"), depth + 1)?;
            }
            Node::Paragraph { lines, .. } => {
                self.collection(lines.len(), &format!("{path}.lines"))?;
                for (index, line) in lines.iter().enumerate() {
                    self.line(line, &format!("{path}.lines[{index}]"))?;
                }
            }
            Node::List {
                id,
                items,
                selected,
                offset,
                ..
            } => {
                self.text(id, &format!("{path}.id"))?;
                if id.is_empty() || !self.ids.insert(id.clone()) {
                    return Err(RenderError::invalid(
                        &format!("{path}.id"),
                        "expected a nonempty, unique widget ID",
                    ));
                }
                self.collection(items.len(), &format!("{path}.items"))?;
                if selected.is_some_and(|selected| selected as usize >= items.len()) {
                    return Err(RenderError::invalid(
                        &format!("{path}.selected"),
                        "selection is outside the list",
                    ));
                }
                if *offset as usize >= items.len().max(1) {
                    return Err(RenderError::invalid(
                        &format!("{path}.offset"),
                        "offset is outside the list",
                    ));
                }
                for (index, line) in items.iter().enumerate() {
                    self.line(line, &format!("{path}.items[{index}]"))?;
                }
            }
        }
        Ok(())
    }
}
