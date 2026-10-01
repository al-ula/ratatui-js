use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    style::{Color, Modifier},
};
use ratatui_js_core::{ErrorCode, FrameDescription, MAX_DEPTH, MAX_FRAME_BYTES, Renderer};
use serde_json::{Value, json};

const FIXTURES: &str = include_str!("../../../tests/fixtures/frames.json");

fn renderer(width: u16, height: u16) -> Renderer<TestBackend> {
    Renderer::new(TestBackend::new(width, height)).unwrap()
}

#[test]
fn shared_frames_render_expected_cells_and_state() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    for case in fixtures["valid"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let width = case["width"].as_u64().unwrap() as u16;
        let height = case["height"].as_u64().unwrap() as u16;
        let mut renderer = renderer(width, height);
        let result = renderer
            .render_json(&serde_json::to_vec(&case["frame"]).unwrap())
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!((result.width, result.height), (width, height), "{name}");
        assert_eq!(
            serde_json::to_value(result.widget_states).unwrap(),
            case["expectedStates"],
            "{name}"
        );
        let lines = case["expectedLines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap())
            .collect::<Vec<_>>();
        let expected = Buffer::with_lines(lines);
        let actual = renderer.terminal().backend().buffer();
        assert_eq!(actual.area, expected.area, "{name}");
        for (index, (actual, expected)) in actual.content.iter().zip(&expected.content).enumerate()
        {
            assert_eq!(actual.symbol(), expected.symbol(), "{name}, cell {index}");
        }
    }
}

#[test]
fn rejected_frames_do_not_change_the_rendered_buffer() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let mut renderer = renderer(12, 3);
    renderer
        .render_json(&serde_json::to_vec(&fixtures["valid"][0]["frame"]).unwrap())
        .unwrap();
    let before = renderer.terminal().backend().buffer().clone();
    for case in fixtures["invalid"].as_array().unwrap() {
        let error = renderer
            .render_json(&serde_json::to_vec(&case["frame"]).unwrap())
            .unwrap_err();
        assert_eq!(
            serde_json::to_value(error.code()).unwrap(),
            case["code"],
            "{}: {error}",
            case["name"]
        );
        assert_eq!(
            renderer.terminal().backend().buffer(),
            &before,
            "{}",
            case["name"]
        );
    }
}

#[test]
fn span_styles_and_default_selection_style_are_applied() {
    let mut renderer = renderer(4, 2);
    let frame = json!({"protocolVersion": 1, "root": {"type": "column", "children": [
        {"constraint": {"kind": "length", "value": 1}, "node": {"type": "paragraph", "lines": [[
            {"text": "a", "style": {"fg": "green", "bold": true}}, {"text": "b"}
        ]]}},
        {"constraint": {"kind": "fill", "value": 1}, "node": {"type": "list", "id": "list", "selected": 0, "items": [[{"text": "c"}]]}}
    ]}});
    renderer
        .render_json(&serde_json::to_vec(&frame).unwrap())
        .unwrap();
    let buffer = renderer.terminal().backend().buffer();
    assert_eq!(buffer[(0, 0)].fg, Color::Green);
    assert!(buffer[(0, 0)].modifier.contains(Modifier::BOLD));
    assert_eq!(buffer[(1, 0)].fg, Color::Reset);
    assert!(!buffer[(1, 0)].modifier.contains(Modifier::BOLD));
    assert!(buffer[(0, 1)].modifier.contains(Modifier::REVERSED));
}

#[test]
fn nesting_is_bounded_and_the_limit_is_inclusive() {
    let mut node = json!({"type": "paragraph", "lines": []});
    for _ in 1..MAX_DEPTH {
        node = json!({"type": "block", "child": node});
    }
    let valid = json!({"protocolVersion": 1, "root": node});
    FrameDescription::decode(&serde_json::to_vec(&valid).unwrap()).unwrap();
    let invalid = json!({"protocolVersion": 1, "root": {"type": "block", "child": valid["root"]}});
    assert_eq!(
        FrameDescription::decode(&serde_json::to_vec(&invalid).unwrap())
            .unwrap_err()
            .code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn invalid_json_utf8_and_oversized_payloads_are_rejected() {
    for bytes in [
        b"{".as_slice(),
        b"\xff".as_slice(),
        br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[[{"text":"\ud800"}]]}}"#
            .as_slice(),
    ] {
        assert_eq!(
            FrameDescription::decode(bytes).unwrap_err().code(),
            ErrorCode::InvalidJson
        );
    }
    assert_eq!(
        FrameDescription::decode(&vec![b' '; MAX_FRAME_BYTES + 1])
            .unwrap_err()
            .code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn duplicate_struct_fields_are_rejected() {
    let bytes = br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[],"lines":[]}}"#;
    assert_eq!(
        FrameDescription::decode(bytes).unwrap_err().code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn zero_sized_and_overpadded_areas_do_not_panic() {
    let bytes = br#"{"protocolVersion":1,"root":{"type":"block","padding":{"left":65535,"top":65535},"child":{"type":"list","id":"list","items":[]}}}"#;
    for (width, height) in [(0, 0), (0, 1), (1, 0), (1, 1), (4, 4)] {
        let result = renderer(width, height).render_json(bytes).unwrap();
        assert_eq!((result.width, result.height), (width, height));
        assert_eq!(result.widget_states.len(), 1);
    }
}

#[test]
fn a_new_frame_clears_previous_content() {
    let mut renderer = renderer(4, 1);
    renderer
        .render_json(
            br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[[{"text":"abcd"}]]}}"#,
        )
        .unwrap();
    renderer
        .render_json(
            br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[[{"text":"x"}]]}}"#,
        )
        .unwrap();
    renderer
        .terminal()
        .backend()
        .assert_buffer(&Buffer::with_lines(["x   "]));
}

#[test]
fn additional_widgets_handle_zero_sized_and_overpadded_areas() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    for case in fixtures["valid"].as_array().unwrap() {
        if !["table", "tabs", "gauge", "chart", "scrollbar"]
            .contains(&case["frame"]["root"]["type"].as_str().unwrap())
        {
            continue;
        }
        for (width, height) in [(0, 0), (0, 1), (1, 0), (1, 1), (2, 2)] {
            let mut renderer = renderer(width, height);
            renderer
                .render_json(&serde_json::to_vec(&case["frame"]).unwrap())
                .unwrap();
            let frame = json!({"protocolVersion": 1, "root": {
                "type": "block", "padding": {"left": 65535, "top": 65535},
                "child": case["frame"]["root"]
            }});
            renderer
                .render_json(&serde_json::to_vec(&frame).unwrap())
                .unwrap();
        }
    }
}

#[test]
fn state_is_supplied_by_each_frame_and_never_retained_by_the_renderer() {
    let mut renderer = renderer(4, 1);
    let root = json!({"type": "table", "id": "table", "widths": [{"kind": "fill", "value": 1}],
        "rows": [[[ {"text": "one"} ]], [[ {"text": "two"} ]]], "selected": 1});
    let mut frame = json!({"protocolVersion": 1, "root": root});
    let result = renderer
        .render_json(&serde_json::to_vec(&frame).unwrap())
        .unwrap();
    assert_eq!(result.widget_states[0].offset, 1);
    frame["root"].as_object_mut().unwrap().remove("selected");
    let result = renderer
        .render_json(&serde_json::to_vec(&frame).unwrap())
        .unwrap();
    assert_eq!(result.widget_states[0].offset, 0);
    assert_eq!(result.widget_states[0].selected, None);
    renderer
        .terminal()
        .backend()
        .assert_buffer(&Buffer::with_lines(["one "]));
}

#[test]
fn additional_widget_styles_reach_the_cell_buffer() {
    for (root, width, height, x, y, color, reversed) in [
        (
            json!({"type": "table", "id": "table", "rows": [[[ {"text": "a"} ]]], "widths": [{"kind": "fill", "value": 1}], "selected": 0, "style": {"fg": "red"}, "highlightStyle": {"fg": "green"}}),
            4,
            1,
            0,
            0,
            Color::Green,
            false,
        ),
        (
            json!({"type": "tabs", "id": "tabs", "titles": [[{"text": "a"}]], "selected": 0, "style": {"fg": "blue"}}),
            4,
            1,
            1,
            0,
            Color::Blue,
            true,
        ),
        (
            json!({"type": "gauge", "ratio": 1, "label": {"text": "a", "style": {"fg": "yellow"}}, "gaugeStyle": {"fg": "green"}}),
            3,
            1,
            1,
            0,
            Color::Yellow,
            false,
        ),
        (
            json!({"type": "chart", "xAxis": {"bounds": [0, 1]}, "yAxis": {"bounds": [0, 1]}, "datasets": [{"data": [[0, 0]], "style": {"fg": "cyan"}}]}),
            3,
            3,
            0,
            2,
            Color::Cyan,
            false,
        ),
        (
            json!({"type": "scrollbar", "id": "scroll", "contentLength": 10, "style": {"fg": "magenta"}}),
            4,
            4,
            3,
            0,
            Color::Magenta,
            false,
        ),
    ] {
        let mut renderer = renderer(width, height);
        renderer
            .render_json(&serde_json::to_vec(&json!({"protocolVersion": 1, "root": root})).unwrap())
            .unwrap();
        let cell = &renderer.terminal().backend().buffer()[(x, y)];
        assert_eq!(cell.fg, color, "{root}");
        assert_eq!(
            cell.modifier.contains(Modifier::REVERSED),
            reversed,
            "{root}"
        );
    }
}

#[test]
fn richer_colors_and_each_modifier_override_reach_rendered_cells() {
    for (name, modifier) in [
        ("bold", Modifier::BOLD),
        ("dim", Modifier::DIM),
        ("italic", Modifier::ITALIC),
        ("underlined", Modifier::UNDERLINED),
        ("reversed", Modifier::REVERSED),
        ("crossedOut", Modifier::CROSSED_OUT),
    ] {
        let mut renderer = renderer(4, 1);
        let frame = json!({"protocolVersion": 1, "root": {
            "type": "paragraph", "style": {"fg": {"rgb": [0, 128, 255]}, "bg": {"indexed": 255}, name: true},
            "lines": [[
                {"text": "a"},
                {"text": "b", "style": {name: false, "fg": {"indexed": 0}}},
                {"text": "c", "style": {name: true, "bg": {"rgb": [255, 0, 128]}}}
            ]]
        }});
        renderer
            .render_json(&serde_json::to_vec(&frame).unwrap())
            .unwrap();
        let buffer = renderer.terminal().backend().buffer();
        assert_eq!(buffer[(0, 0)].fg, Color::Rgb(0, 128, 255));
        assert_eq!(buffer[(0, 0)].bg, Color::Indexed(255));
        assert!(buffer[(0, 0)].modifier.contains(modifier), "{name}");
        assert_eq!(buffer[(1, 0)].fg, Color::Indexed(0));
        assert_eq!(buffer[(1, 0)].bg, Color::Indexed(255));
        assert!(!buffer[(1, 0)].modifier.contains(modifier), "{name}");
        assert_eq!(buffer[(2, 0)].fg, Color::Rgb(0, 128, 255));
        assert_eq!(buffer[(2, 0)].bg, Color::Rgb(255, 0, 128));
        assert!(buffer[(2, 0)].modifier.contains(modifier), "{name}");
        // Removal in one span must not affect the adjacent span or padding.
        assert!(buffer[(3, 0)].modifier.contains(modifier), "{name}");
    }
}

#[test]
fn child_widgets_remove_block_modifiers_and_can_add_them_back() {
    for parent_enabled in [true, false] {
        let mut renderer = renderer(5, 3);
        let frame = json!({"protocolVersion": 1, "root": {
            "type": "block", "style": {"bold": parent_enabled, "fg": {"rgb": [255, 0, 0]}},
            "child": {"type": "paragraph", "style": {"bold": !parent_enabled}, "lines": [[
                {"text": "a"}, {"text": "b", "style": {"bold": parent_enabled, "fg": {"indexed": 42}}}
            ]]}
        }});
        renderer
            .render_json(&serde_json::to_vec(&frame).unwrap())
            .unwrap();
        let buffer = renderer.terminal().backend().buffer();
        assert_eq!(
            buffer[(1, 1)].modifier.contains(Modifier::BOLD),
            !parent_enabled
        );
        assert_eq!(
            buffer[(2, 1)].modifier.contains(Modifier::BOLD),
            parent_enabled
        );
        assert_eq!(buffer[(1, 1)].fg, Color::Rgb(255, 0, 0));
        assert_eq!(buffer[(2, 1)].fg, Color::Indexed(42));
    }
}

#[test]
fn selection_highlights_can_remove_content_modifiers() {
    for root in [
        json!({"type":"list", "id":"l", "items":[[{"text":"a", "style":{"bold":true}}]], "selected":0, "style":{"italic":true}, "highlightStyle":{"bold":false,"italic":false,"fg":{"rgb":[1,2,3]}}}),
        json!({"type":"table", "id":"t", "rows":[[[{"text":"a", "style":{"bold":true}}]]], "widths":[{"kind":"fill","value":1}], "selected":0, "style":{"italic":true}, "highlightStyle":{"bold":false,"italic":false,"fg":{"rgb":[1,2,3]}}}),
        json!({"type":"tabs", "id":"t", "titles":[[{"text":"a", "style":{"bold":true}}]], "selected":0, "style":{"italic":true}, "highlightStyle":{"bold":false,"italic":false,"fg":{"rgb":[1,2,3]}}}),
    ] {
        let mut renderer = renderer(4, 1);
        renderer
            .render_json(&serde_json::to_vec(&json!({"protocolVersion":1,"root":root})).unwrap())
            .unwrap();
        let cell = renderer
            .terminal()
            .backend()
            .buffer()
            .content
            .iter()
            .find(|cell| cell.symbol() == "a")
            .unwrap();
        assert_eq!(cell.fg, Color::Rgb(1, 2, 3), "{root}");
        assert!(
            !cell.modifier.intersects(Modifier::BOLD | Modifier::ITALIC),
            "{root}"
        );
    }
}

#[test]
fn shared_style_schemas_accept_valid_values_and_reject_invalid_values_before_drawing() {
    let fixtures: Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/styles.json")).unwrap();
    let mut renderer = renderer(1, 1);
    for case in fixtures["valid"].as_array().unwrap() {
        let style: ratatui_js_core::Style = serde_json::from_value(case["style"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(style).unwrap(),
            case["style"],
            "{}",
            case["name"]
        );
        renderer.render_json(&serde_json::to_vec(&json!({
            "protocolVersion": 1,
            "root": {"type": "paragraph", "lines": [[{"text": "x"}]], "style": case["style"]}
        })).unwrap()).unwrap();
    }
    let before = renderer.terminal().backend().buffer().clone();
    for case in fixtures["invalid"].as_array().unwrap() {
        let error = renderer.render_json(&serde_json::to_vec(&json!({
            "protocolVersion": 1,
            "root": {"type": "paragraph", "lines": [[{"text": "y"}]], "style": case["style"]}
        })).unwrap()).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidFrame, "{}", case["name"]);
        assert!(error.description().path.is_some(), "{}", case["name"]);
        assert_eq!(
            renderer.terminal().backend().buffer(),
            &before,
            "{}",
            case["name"]
        );
    }
}
