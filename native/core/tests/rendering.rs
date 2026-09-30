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
