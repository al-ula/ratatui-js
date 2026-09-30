use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint as NativeConstraint, Direction, Layout, Rect},
    style::{Color as NativeColor, Modifier, Style as NativeStyle},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
};

use crate::{
    Border, Color, Constraint, FrameDescription, LayoutChild, Node, RenderError, RenderResult,
    Style, TextLine, WidgetStateUpdate,
};

/// A renderer can target TestBackend now and a platform terminal backend later.
pub struct Renderer<B: Backend> {
    terminal: Terminal<B>,
}

impl<B: Backend> Renderer<B> {
    pub fn new(backend: B) -> Result<Self, RenderError> {
        let terminal =
            Terminal::new(backend).map_err(|error| RenderError::Backend(error.to_string()))?;
        Ok(Self { terminal })
    }

    pub fn terminal(&self) -> &Terminal<B> {
        &self.terminal
    }

    pub fn render_json(&mut self, bytes: &[u8]) -> Result<RenderResult, RenderError> {
        let frame = FrameDescription::decode(bytes)?;
        self.draw_validated(&frame)
    }

    pub fn render(&mut self, description: &FrameDescription) -> Result<RenderResult, RenderError> {
        // Validation completes before drawing can mutate the terminal's buffers.
        description.validate()?;
        self.draw_validated(description)
    }

    fn draw_validated(
        &mut self,
        description: &FrameDescription,
    ) -> Result<RenderResult, RenderError> {
        let mut result = RenderResult::default();
        self.terminal
            .draw(|frame| {
                let area = frame.area();
                result.width = area.width;
                result.height = area.height;
                render_node(frame, area, &description.root, &mut result.widget_states);
            })
            .map_err(|error| RenderError::Backend(error.to_string()))?;
        Ok(result)
    }
}

fn render_layout(
    frame: &mut Frame<'_>,
    area: Rect,
    direction: Direction,
    children: &[LayoutChild],
    spacing: u16,
    updates: &mut Vec<WidgetStateUpdate>,
) {
    let constraints = children.iter().map(|child| match child.constraint {
        Constraint::Length { value } => NativeConstraint::Length(value),
        Constraint::Min { value } => NativeConstraint::Min(value),
        Constraint::Max { value } => NativeConstraint::Max(value),
        Constraint::Percentage { value } => NativeConstraint::Percentage(value),
        Constraint::Fill { value } => NativeConstraint::Fill(value),
    });
    let areas = Layout::new(direction, constraints)
        .spacing(spacing)
        .split(area);
    for (child, area) in children.iter().zip(areas.iter()) {
        render_node(frame, *area, &child.node, updates);
    }
}

fn render_node(
    frame: &mut Frame<'_>,
    area: Rect,
    node: &Node,
    updates: &mut Vec<WidgetStateUpdate>,
) {
    match node {
        Node::Row { children, spacing } => {
            render_layout(
                frame,
                area,
                Direction::Horizontal,
                children,
                *spacing,
                updates,
            );
        }
        Node::Column { children, spacing } => {
            render_layout(
                frame,
                area,
                Direction::Vertical,
                children,
                *spacing,
                updates,
            );
        }
        Node::Block {
            child,
            title,
            border,
            padding,
            style,
        } => {
            let (borders, border_type) = match border {
                Border::Plain => (Borders::ALL, BorderType::Plain),
                Border::Rounded => (Borders::ALL, BorderType::Rounded),
                Border::Double => (Borders::ALL, BorderType::Double),
                Border::None => (Borders::NONE, BorderType::Plain),
            };
            let mut block = Block::default()
                .borders(borders)
                .border_type(border_type)
                .padding(Padding::new(
                    padding.left,
                    padding.right,
                    padding.top,
                    padding.bottom,
                ))
                .style(native_style(style));
            if let Some(title) = title {
                block = block.title(title.as_str());
            }
            let inner = block.inner(area);
            frame.render_widget(block, area);
            render_node(frame, inner, child, updates);
        }
        Node::Paragraph { lines, wrap, style } => {
            let text = Text::from(lines.iter().map(native_line).collect::<Vec<_>>());
            let mut paragraph = Paragraph::new(text).style(native_style(style));
            if *wrap {
                paragraph = paragraph.wrap(Wrap { trim: false });
            }
            frame.render_widget(paragraph, area);
        }
        Node::List {
            id,
            items,
            selected,
            offset,
            style,
            highlight_style,
        } => {
            let items = items
                .iter()
                .map(|line| ListItem::new(native_line(line)))
                .collect::<Vec<_>>();
            let list = List::new(items)
                .style(native_style(style))
                .highlight_style(native_style(highlight_style));
            let mut state = ListState::default()
                .with_selected(selected.map(|value| value as usize))
                .with_offset(*offset as usize);
            frame.render_stateful_widget(list, area, &mut state);
            updates.push(WidgetStateUpdate {
                id: id.clone(),
                offset: state.offset() as u32,
                selected: state.selected().map(|value| value as u32),
            });
        }
    }
}

fn native_line(line: &TextLine) -> Line<'_> {
    Line::from(
        line.iter()
            .map(|span| Span::styled(span.text.as_str(), native_style(&span.style)))
            .collect::<Vec<_>>(),
    )
}

fn native_style(style: &Style) -> NativeStyle {
    let mut native = NativeStyle::default();
    if let Some(fg) = style.fg {
        native = native.fg(native_color(fg));
    }
    if let Some(bg) = style.bg {
        native = native.bg(native_color(bg));
    }
    for (enabled, modifier) in [
        (style.bold, Modifier::BOLD),
        (style.dim, Modifier::DIM),
        (style.italic, Modifier::ITALIC),
        (style.underlined, Modifier::UNDERLINED),
        (style.reversed, Modifier::REVERSED),
        (style.crossed_out, Modifier::CROSSED_OUT),
    ] {
        if enabled {
            native = native.add_modifier(modifier);
        }
    }
    native
}

fn native_color(color: Color) -> NativeColor {
    match color {
        Color::Black => NativeColor::Black,
        Color::Red => NativeColor::Red,
        Color::Green => NativeColor::Green,
        Color::Yellow => NativeColor::Yellow,
        Color::Blue => NativeColor::Blue,
        Color::Magenta => NativeColor::Magenta,
        Color::Cyan => NativeColor::Cyan,
        Color::Gray => NativeColor::Gray,
        Color::DarkGray => NativeColor::DarkGray,
        Color::LightRed => NativeColor::LightRed,
        Color::LightGreen => NativeColor::LightGreen,
        Color::LightYellow => NativeColor::LightYellow,
        Color::LightBlue => NativeColor::LightBlue,
        Color::LightMagenta => NativeColor::LightMagenta,
        Color::LightCyan => NativeColor::LightCyan,
        Color::White => NativeColor::White,
    }
}
