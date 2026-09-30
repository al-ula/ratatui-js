# Rendering protocol v1

The initial transport is UTF-8 JSON. Protocol versioning is independent of the
future C ABI version. A frame is:

```json
{
  "protocolVersion": 1,
  "root": {
    "type": "paragraph",
    "lines": [[{ "text": "Hello" }]]
  }
}
```

`packages/protocol/types.ts` and `native/core/src/protocol.rs` define the shared
schema. Unknown fields and enum values are rejected. Optional fields may be
omitted, but not set to `null`. Native validation is authoritative; JavaScript
validation provides earlier application errors.

## Layout

- `row` lays out children horizontally; `column` vertically.
- Each child supplies `constraint` and `node`.
- Constraints map directly to Ratatui: `length`, `min`, `max`, `percentage`, and
  positive-weight `fill`. Each constraint uses `{ "kind": "…", "value": n }`.
- Cell counts and spacing are unsigned 16-bit integers. Percentage is 0–100.
- Spacing defaults to zero. Ratatui resolves conflicts when constraints do not
  fit; this is not a CSS flexbox implementation.
- Empty layouts are allowed. Areas are clipped to the terminal's dimensions.

## Blocks and text

- `block` draws a border and an optional title around one `child`.
- Border defaults to `plain`; alternatives are `rounded`, `double`, and `none`.
- Padding is an object with optional `left`, `right`, `top`, and `bottom` cell
  counts, defaulting to zero. The child receives the remaining inner area.
- Paragraph `lines` are arrays of span arrays. Each span has `text` and optional
  `style`. Use multiple explicit lines rather than embedded newline characters.
- Paragraph wrapping defaults to false. When true, use Ratatui wrapping with
  `trim: false`. Without wrapping, excess text is clipped.
- Unicode grapheme width and wide-cell handling belong to Ratatui, not TS.
- Styles support the named colors and additive boolean modifiers listed in the
  TS schema. Omitted/false modifiers add nothing; they do not remove inherited
  modifiers. Span styles patch widget styles.

## Lists

- List `items` are single-line span arrays.
- Each list requires a nonempty `id`, unique among lists in the frame.
- `selected` is an optional zero-based index; omit it for no selection.
- `offset` defaults to zero and must be inside the list. Empty lists require
  offset zero and no selection.
- Default selection highlighting is reversed text. An explicit `highlightStyle`
  replaces this default.
- Ratatui may change the offset to keep selection visible. Render results return
  every list's actual offset and optional selection, in tree traversal order.

```json
{
  "width": 80,
  "height": 24,
  "widgetStates": [{ "id": "tasks", "offset": 1, "selected": 2 }]
}
```

Rendering completion means the backend draw/output operation completed; it does
not mean a queued render was merely accepted.

## Validation and failures

- Maximum encoded frame size: 1,048,576 bytes, including JSON overhead.
- Maximum node depth: 32, counting the root as depth one.
- Maximum node count: 4,096.
- Maximum entries per collection: 4,096.
- Maximum total text spans: 16,384.
- Maximum aggregate UTF-8 text bytes, including titles and IDs: 1,048,576.
- Reject all Unicode control characters (`Cc`), including ESC, C0, DEL, and C1.
- Rust requires valid UTF-8/Unicode. TS rejects lone UTF-16 surrogates rather
  than allowing silent replacement during encoding.
- Duplicate native struct fields are rejected; do not use duplicate JSON keys.

The payload byte limit applies to the actual input bytes. Direct Rust callers
also receive structural/text validation, without serializing their in-memory
frames to enforce a wire-size limit.

All validation finishes before drawing. Rejected frames leave the previously
rendered buffer unchanged. A backend I/O error can happen after output starts
and cannot promise transactional rollback.

Current error codes are `invalidJson`, `invalidFrame`, `unsupportedProtocol`,
and `io`. Descriptions include a message and, when available, a field path.
Malformed JSON/UTF-8 is distinct from well-formed JSON that fails schema
validation.

## Input contract

TS types define normalized key and resize events for the upcoming adapters.
`native/crossterm` reads these events for Rust callers and serializes the same
event shape. Key events contain a structured key code, press/repeat/release
kind, and modifiers; availability depends on the terminal and platform. Events
or key codes/modifiers outside this contract are ignored rather than
misrepresented. Mouse, paste, focus, and enhanced keyboard-mode negotiation are
deferred.
