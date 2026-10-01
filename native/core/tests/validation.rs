use ratatui::{backend::TestBackend, buffer::Buffer};
use ratatui_js_core::{
    ErrorCode, FrameDescription, MAX_COLLECTION_ITEMS, MAX_DEPTH, MAX_FRAME_BYTES, MAX_NODES,
    MAX_SPANS, Renderer,
};
use serde_json::{Value, json};

fn decode(root: Value) -> Result<FrameDescription, ratatui_js_core::RenderError> {
    FrameDescription::decode(
        &serde_json::to_vec(&json!({
            "protocolVersion": 1,
            "root": root,
        }))
        .unwrap(),
    )
}

#[test]
fn schema_errors_are_distinct_from_json_syntax_errors() {
    for root in [
        json!({"type": "paragraph"}),
        json!({"type": "paragraph", "lines": [], "wrap": "yes"}),
        json!({"type": "paragraph", "lines": [], "style": {"fg": null}}),
        json!({"type": "paragraph", "lines": [], "style": {"bold": 1}}),
        json!({"type": "paragraph", "lines": [[{"text": "a", "extra": true}]]}),
        json!({"type": "block", "child": {"type": "paragraph", "lines": []}, "padding": {"unknown": 1}}),
        json!({"type": "row", "children": [], "spacing": 0.5}),
        json!({"type": "row", "children": [], "spacing": 65536}),
        json!({"type": "list", "id": "", "items": []}),
    ] {
        let error = decode(root).unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidFrame, "{error}");
        assert!(error.description().path.is_some());
    }
}

#[test]
fn every_control_character_class_is_rejected() {
    for text in ["\0", "\t", "\r", "\u{7f}", "\u{9b}"] {
        assert_eq!(
            decode(json!({"type": "paragraph", "lines": [[{"text": text}]]}))
                .unwrap_err()
                .code(),
            ErrorCode::InvalidFrame
        );
    }
}

#[test]
fn layout_nesting_limit_is_inclusive() {
    let mut node = json!({"type": "paragraph", "lines": []});
    for _ in 1..MAX_DEPTH {
        node = json!({"type": "row", "children": [{
            "constraint": {"kind": "fill", "value": 1},
            "node": node,
        }]});
    }
    decode(node.clone()).unwrap();
    assert_eq!(
        decode(json!({"type": "row", "children": [{
            "constraint": {"kind": "fill", "value": 1}, "node": node,
        }]}))
        .unwrap_err()
        .code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn collection_and_node_limits_are_inclusive() {
    let child = json!({
        "constraint": {"kind": "fill", "value": 1},
        "node": {"type": "paragraph", "lines": []},
    });
    decode(json!({"type": "row", "children": vec![child.clone(); MAX_NODES - 1]})).unwrap();
    assert_eq!(
        decode(json!({"type": "row", "children": vec![child; MAX_NODES]}))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidFrame
    );
    assert_eq!(
        decode(json!({"type": "paragraph", "lines": vec![json!([]); MAX_COLLECTION_ITEMS + 1]}))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn total_span_limit_is_inclusive() {
    let line = vec![json!({"text": ""}); MAX_COLLECTION_ITEMS];
    let mut lines = vec![line; MAX_SPANS / MAX_COLLECTION_ITEMS];
    decode(json!({"type": "paragraph", "lines": lines})).unwrap();
    lines.push(vec![json!({"text": ""})]);
    assert_eq!(
        decode(json!({"type": "paragraph", "lines": lines}))
            .unwrap_err()
            .code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn wire_byte_limit_counts_actual_input_not_expanded_defaults() {
    let mut bytes = br#"{"protocolVersion":1,"root":{"type":"paragraph","lines":[]}}"#.to_vec();
    bytes.resize(MAX_FRAME_BYTES, b' ');
    FrameDescription::decode(&bytes).unwrap();
    bytes.push(b' ');
    assert_eq!(
        FrameDescription::decode(&bytes).unwrap_err().code(),
        ErrorCode::InvalidFrame
    );
}

#[test]
fn direct_rust_frames_are_validated_before_drawing() {
    let mut renderer = Renderer::new(TestBackend::new(4, 1)).unwrap();
    let mut frame = decode(json!({"type": "paragraph", "lines": [[{"text": "good"}]]})).unwrap();
    renderer.render(&frame).unwrap();
    frame.protocol_version = 2;
    assert_eq!(
        renderer.render(&frame).unwrap_err().code(),
        ErrorCode::UnsupportedProtocol
    );
    renderer
        .terminal()
        .backend()
        .assert_buffer(&Buffer::with_lines(["good"]));
}

#[test]
fn direct_rust_numeric_widget_inputs_are_validated_before_drawing() {
    use ratatui_js_core::Node;
    let mut renderer = Renderer::new(TestBackend::new(4, 1)).unwrap();
    let mut gauge = decode(json!({"type": "gauge", "ratio": 0.5})).unwrap();
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1.1] {
        if let Node::Gauge { ratio, .. } = &mut gauge.root {
            *ratio = value;
        }
        assert_eq!(
            renderer.render(&gauge).unwrap_err().code(),
            ErrorCode::InvalidFrame
        );
    }
    let mut chart = decode(json!({"type": "chart", "xAxis": {"bounds": [0, 1]}, "yAxis": {"bounds": [0, 1]}, "datasets": [{"data": [[0, 0]]}]})).unwrap();
    if let Node::Chart { datasets, .. } = &mut chart.root {
        datasets[0].data[0][0] = f64::NAN;
    }
    assert_eq!(
        renderer.render(&chart).unwrap_err().code(),
        ErrorCode::InvalidFrame
    );
    if let Node::Chart {
        datasets, x_axis, ..
    } = &mut chart.root
    {
        datasets[0].data[0][0] = 0.0;
        x_axis.bounds = [0.0, f64::INFINITY];
    }
    assert_eq!(
        renderer.render(&chart).unwrap_err().code(),
        ErrorCode::InvalidFrame
    );
}
