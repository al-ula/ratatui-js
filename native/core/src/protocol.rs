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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartAxis {
    pub bounds: [f64; 2],
    #[serde(
        default,
        deserialize_with = "supplied",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(default)]
    pub labels: Vec<TextLine>,
    #[serde(default)]
    pub style: Style,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GraphType {
    #[default]
    Line,
    Scatter,
    Bar,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChartDataset {
    #[serde(
        default,
        deserialize_with = "supplied",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    pub data: Vec<[f64; 2]>,
    #[serde(default)]
    pub graph_type: GraphType,
    #[serde(default)]
    pub style: Style,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScrollbarOrientation {
    #[default]
    VerticalRight,
    VerticalLeft,
    HorizontalBottom,
    HorizontalTop,
}

fn default_column_spacing() -> u16 {
    1
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
    Table {
        id: String,
        rows: Vec<Vec<TextLine>>,
        widths: Vec<Constraint>,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        header: Option<Vec<TextLine>>,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        selected: Option<u32>,
        #[serde(default)]
        offset: u32,
        #[serde(default = "default_column_spacing")]
        column_spacing: u16,
        #[serde(default)]
        style: Style,
        #[serde(default = "default_highlight_style")]
        highlight_style: Style,
    },
    Tabs {
        id: String,
        titles: Vec<TextLine>,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        selected: Option<u32>,
        #[serde(default)]
        style: Style,
        #[serde(default = "default_highlight_style")]
        highlight_style: Style,
    },
    Gauge {
        ratio: f64,
        #[serde(
            default,
            deserialize_with = "supplied",
            skip_serializing_if = "Option::is_none"
        )]
        label: Option<TextSpan>,
        #[serde(default)]
        style: Style,
        #[serde(default)]
        gauge_style: Style,
    },
    Chart {
        datasets: Vec<ChartDataset>,
        x_axis: ChartAxis,
        y_axis: ChartAxis,
        #[serde(default)]
        style: Style,
    },
    Scrollbar {
        id: String,
        content_length: u32,
        #[serde(default)]
        position: u32,
        #[serde(default)]
        viewport_content_length: u32,
        #[serde(default)]
        orientation: ScrollbarOrientation,
        #[serde(default)]
        style: Style,
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

    fn line(&mut self, spans: &[TextSpan], path: &str) -> Result<(), RenderError> {
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

    fn widget_id(&mut self, id: &str, path: &str) -> Result<(), RenderError> {
        self.text(id, path)?;
        if id.is_empty() || !self.ids.insert(id.to_owned()) {
            return Err(RenderError::invalid(
                path,
                "expected a nonempty, unique widget ID",
            ));
        }
        Ok(())
    }

    fn selection(&self, selected: Option<u32>, len: usize, path: &str) -> Result<(), RenderError> {
        if selected.is_some_and(|value| value as usize >= len) {
            return Err(RenderError::invalid(
                path,
                "selection is outside the content",
            ));
        }
        Ok(())
    }

    fn offset(&self, offset: u32, len: usize, path: &str) -> Result<(), RenderError> {
        if offset as usize >= len.max(1) {
            return Err(RenderError::invalid(path, "offset is outside the content"));
        }
        Ok(())
    }

    fn constraint(&self, constraint: &Constraint, path: &str) -> Result<(), RenderError> {
        match constraint {
            Constraint::Percentage { value } if *value > 100 => {
                Err(RenderError::invalid(path, "percentage exceeds 100"))
            }
            Constraint::Fill { value: 0 } => {
                Err(RenderError::invalid(path, "fill must be positive"))
            }
            _ => Ok(()),
        }
    }

    fn table_row(
        &mut self,
        row: &[TextLine],
        columns: usize,
        path: &str,
    ) -> Result<(), RenderError> {
        self.collection(row.len(), path)?;
        if row.len() != columns {
            return Err(RenderError::invalid(path, "cell count must match widths"));
        }
        for (index, cell) in row.iter().enumerate() {
            self.line(cell, &format!("{path}[{index}]"))?;
        }
        Ok(())
    }

    fn axis(&mut self, axis: &ChartAxis, path: &str) -> Result<(), RenderError> {
        let [min, max] = axis.bounds;
        if !min.is_finite() || !max.is_finite() || min >= max || !(max - min).is_finite() {
            return Err(RenderError::invalid(
                path,
                "expected increasing bounds with a finite range",
            ));
        }
        if let Some(title) = &axis.title {
            self.text(title, &format!("{path}.title"))?;
        }
        self.collection(axis.labels.len(), &format!("{path}.labels"))?;
        for (index, label) in axis.labels.iter().enumerate() {
            self.line(label, &format!("{path}.labels[{index}]"))?;
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
                    self.constraint(&child.constraint, &format!("{child_path}.constraint.value"))?;
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
            Node::Table {
                id,
                rows,
                widths,
                header,
                selected,
                offset,
                ..
            } => {
                self.widget_id(id, &format!("{path}.id"))?;
                self.collection(widths.len(), &format!("{path}.widths"))?;
                for (index, width) in widths.iter().enumerate() {
                    self.constraint(width, &format!("{path}.widths[{index}]"))?;
                }
                self.collection(rows.len(), &format!("{path}.rows"))?;
                for (index, row) in rows.iter().enumerate() {
                    self.table_row(row, widths.len(), &format!("{path}.rows[{index}]"))?;
                }
                if let Some(header) = header {
                    self.table_row(header, widths.len(), &format!("{path}.header"))?;
                }
                self.selection(*selected, rows.len(), &format!("{path}.selected"))?;
                self.offset(*offset, rows.len(), &format!("{path}.offset"))?;
            }
            Node::Tabs {
                id,
                titles,
                selected,
                ..
            } => {
                self.widget_id(id, &format!("{path}.id"))?;
                self.collection(titles.len(), &format!("{path}.titles"))?;
                for (index, title) in titles.iter().enumerate() {
                    self.line(title, &format!("{path}.titles[{index}]"))?;
                }
                self.selection(*selected, titles.len(), &format!("{path}.selected"))?;
            }
            Node::Gauge { ratio, label, .. } => {
                if !ratio.is_finite() || !(0.0..=1.0).contains(ratio) {
                    return Err(RenderError::invalid(
                        &format!("{path}.ratio"),
                        "ratio must be between 0 and 1",
                    ));
                }
                if let Some(label) = label {
                    self.line(std::slice::from_ref(label), &format!("{path}.label"))?;
                }
            }
            Node::Chart {
                datasets,
                x_axis,
                y_axis,
                ..
            } => {
                self.axis(x_axis, &format!("{path}.xAxis"))?;
                self.axis(y_axis, &format!("{path}.yAxis"))?;
                self.collection(datasets.len(), &format!("{path}.datasets"))?;
                for (index, dataset) in datasets.iter().enumerate() {
                    let dataset_path = format!("{path}.datasets[{index}]");
                    if let Some(name) = &dataset.name {
                        self.text(name, &format!("{dataset_path}.name"))?;
                    }
                    self.collection(dataset.data.len(), &format!("{dataset_path}.data"))?;
                    if dataset
                        .data
                        .iter()
                        .flatten()
                        .any(|value| !value.is_finite())
                    {
                        return Err(RenderError::invalid(
                            &format!("{dataset_path}.data"),
                            "expected finite coordinates",
                        ));
                    }
                }
            }
            Node::Scrollbar {
                id,
                content_length,
                position,
                ..
            } => {
                self.widget_id(id, &format!("{path}.id"))?;
                self.offset(
                    *position,
                    *content_length as usize,
                    &format!("{path}.position"),
                )?;
            }
            Node::List {
                id,
                items,
                selected,
                offset,
                ..
            } => {
                self.widget_id(id, &format!("{path}.id"))?;
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
