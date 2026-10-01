import {
  type ErrorCode,
  FRAME_LIMITS,
  type FrameDescription,
  PROTOCOL_VERSION,
} from "./types.ts";

/** A frame rejected before it reaches a platform adapter. */
export class FrameError extends Error {
  override readonly name = "FrameError";

  constructor(
    readonly code: ErrorCode,
    message: string,
    readonly path?: string,
  ) {
    super(message);
  }
}

function invalid(path: string, message: string): never {
  throw new FrameError("invalidFrame", `${path}: ${message}`, path);
}

function object(
  value: unknown,
  path: string,
  allowed: readonly string[],
): Record<string, unknown> {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    invalid(path, "expected an object");
  }
  const record = value as Record<string, unknown>;
  for (const key of Object.keys(record)) {
    if (!allowed.includes(key)) invalid(`${path}.${key}`, "unknown field");
  }
  return record;
}

function integer(value: unknown, path: string, max = 65_535): number {
  if (
    typeof value !== "number" || !Number.isInteger(value) || value < 0 ||
    value > max
  ) {
    invalid(path, `expected an integer between 0 and ${max}`);
  }
  return value;
}

function boolean(value: unknown, path: string): void {
  if (typeof value !== "boolean") invalid(path, "expected a boolean");
}

const COLORS = new Set([
  "black",
  "red",
  "green",
  "yellow",
  "blue",
  "magenta",
  "cyan",
  "gray",
  "darkGray",
  "lightRed",
  "lightGreen",
  "lightYellow",
  "lightBlue",
  "lightMagenta",
  "lightCyan",
  "white",
]);
const MODIFIERS = [
  "bold",
  "dim",
  "italic",
  "underlined",
  "reversed",
  "crossedOut",
];

function style(value: unknown, path: string): void {
  const record = object(value, path, ["fg", "bg", ...MODIFIERS]);
  for (const field of ["fg", "bg"]) {
    if (record[field] !== undefined && !COLORS.has(record[field] as string)) {
      invalid(`${path}.${field}`, "unknown color");
    }
  }
  for (const field of MODIFIERS) {
    if (record[field] !== undefined) boolean(record[field], `${path}.${field}`);
  }
}

function collection(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) invalid(path, "expected an array");
  if (value.length > FRAME_LIMITS.collectionItems) {
    invalid(path, "too many items");
  }
  return value;
}

interface ValidationState {
  nodes: number;
  spans: number;
  textBytes: number;
  readonly ids: Set<string>;
}

const encoder = new TextEncoder();

function text(value: unknown, path: string, state: ValidationState): string {
  if (typeof value !== "string") invalid(path, "expected a string");
  if (!value.isWellFormed()) invalid(path, "text must contain valid Unicode");
  if (/\p{Cc}/u.test(value)) {
    invalid(path, "control characters are not allowed; use explicit lines");
  }
  state.textBytes += encoder.encode(value).length;
  if (state.textBytes > FRAME_LIMITS.bytes) invalid(path, "too much text");
  return value;
}

function line(value: unknown, path: string, state: ValidationState): void {
  const spans = collection(value, path);
  for (const [index, value] of spans.entries()) {
    if (++state.spans > FRAME_LIMITS.spans) invalid(path, "too many spans");
    const spanPath = `${path}[${index}]`;
    const span = object(value, spanPath, ["text", "style"]);
    text(span.text, `${spanPath}.text`, state);
    if (span.style !== undefined) style(span.style, `${spanPath}.style`);
  }
}

function constraint(value: unknown, path: string): void {
  const item = object(value, path, ["kind", "value"]);
  if (
    !["length", "min", "max", "percentage", "fill"].includes(
      item.kind as string,
    )
  ) invalid(`${path}.kind`, "unknown constraint");
  const amount = integer(
    item.value,
    `${path}.value`,
    item.kind === "percentage" ? 100 : 65535,
  );
  if (item.kind === "fill" && amount === 0) {
    invalid(`${path}.value`, "fill must be positive");
  }
}

function finite(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    invalid(path, "expected a finite number");
  }
  return value;
}

function pair(value: unknown, path: string): readonly [number, number] {
  if (!Array.isArray(value) || value.length !== 2) {
    invalid(path, "expected two coordinates");
  }
  return [finite(value[0], `${path}[0]`), finite(value[1], `${path}[1]`)];
}

function widgetId(value: unknown, path: string, state: ValidationState): void {
  const id = text(value, path, state);
  if (id.length === 0 || state.ids.has(id)) {
    invalid(path, "expected a nonempty, unique widget ID");
  }
  state.ids.add(id);
}

function selection(value: unknown, path: string, length: number): void {
  if (value !== undefined && integer(value, path, 0xffffffff) >= length) {
    invalid(path, "selection is outside the content");
  }
}

function offset(value: unknown, path: string, length: number): void {
  if (
    value !== undefined &&
    integer(value, path, 0xffffffff) >= Math.max(1, length)
  ) invalid(path, "offset is outside the content");
}

function axis(value: unknown, path: string, state: ValidationState): void {
  const item = object(value, path, ["bounds", "title", "labels", "style"]);
  const bounds = pair(item.bounds, `${path}.bounds`);
  if (bounds[0] >= bounds[1] || !Number.isFinite(bounds[1] - bounds[0])) {
    invalid(`${path}.bounds`, "expected increasing bounds with a finite range");
  }
  if (item.title !== undefined) text(item.title, `${path}.title`, state);
  if (item.labels !== undefined) {
    for (
      const [index, label] of collection(item.labels, `${path}.labels`)
        .entries()
    ) line(label, `${path}.labels[${index}]`, state);
  }
  if (item.style !== undefined) style(item.style, `${path}.style`);
}

function node(
  value: unknown,
  path: string,
  depth: number,
  state: ValidationState,
): void {
  if (depth > FRAME_LIMITS.depth) {
    invalid(path, "maximum nesting depth exceeded");
  }
  if (++state.nodes > FRAME_LIMITS.nodes) invalid(path, "too many nodes");

  // Check the discriminant before applying variant-specific field validation.
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    invalid(path, "expected a node object");
  }
  const type = (value as Record<string, unknown>).type;
  if (type === "row" || type === "column") {
    const layout = object(value, path, ["type", "children", "spacing"]);
    if (layout.spacing !== undefined) {
      integer(layout.spacing, `${path}.spacing`);
    }
    const children = collection(layout.children, `${path}.children`);
    for (const [index, value] of children.entries()) {
      const childPath = `${path}.children[${index}]`;
      const child = object(value, childPath, ["constraint", "node"]);
      constraint(child.constraint, `${childPath}.constraint`);
      node(child.node, `${childPath}.node`, depth + 1, state);
    }
    return;
  }

  if (type === "block") {
    const block = object(value, path, [
      "type",
      "child",
      "title",
      "border",
      "padding",
      "style",
    ]);
    if (block.title !== undefined) text(block.title, `${path}.title`, state);
    if (
      block.border !== undefined &&
      !["plain", "rounded", "double", "none"].includes(block.border as string)
    ) {
      invalid(`${path}.border`, "unknown border");
    }
    if (block.padding !== undefined) {
      const padding = object(block.padding, `${path}.padding`, [
        "left",
        "right",
        "top",
        "bottom",
      ]);
      for (const field of ["left", "right", "top", "bottom"]) {
        if (padding[field] !== undefined) {
          integer(padding[field], `${path}.padding.${field}`);
        }
      }
    }
    if (block.style !== undefined) style(block.style, `${path}.style`);
    node(block.child, `${path}.child`, depth + 1, state);
    return;
  }

  if (type === "paragraph") {
    const paragraph = object(value, path, ["type", "lines", "wrap", "style"]);
    const lines = collection(paragraph.lines, `${path}.lines`);
    for (const [index, value] of lines.entries()) {
      line(value, `${path}.lines[${index}]`, state);
    }
    if (paragraph.wrap !== undefined) boolean(paragraph.wrap, `${path}.wrap`);
    if (paragraph.style !== undefined) style(paragraph.style, `${path}.style`);
    return;
  }

  if (type === "list") {
    const list = object(value, path, [
      "type",
      "id",
      "items",
      "selected",
      "offset",
      "style",
      "highlightStyle",
    ]);
    widgetId(list.id, `${path}.id`, state);
    const items = collection(list.items, `${path}.items`);
    for (const [index, value] of items.entries()) {
      line(value, `${path}.items[${index}]`, state);
    }
    if (list.selected !== undefined) {
      const selected = integer(
        list.selected,
        `${path}.selected`,
        4_294_967_295,
      );
      if (selected >= items.length) {
        invalid(`${path}.selected`, "selection is outside the list");
      }
    }
    if (list.offset !== undefined) {
      const offset = integer(list.offset, `${path}.offset`, 4_294_967_295);
      if (offset >= Math.max(1, items.length)) {
        invalid(`${path}.offset`, "offset is outside the list");
      }
    }
    if (list.style !== undefined) style(list.style, `${path}.style`);
    if (list.highlightStyle !== undefined) {
      style(list.highlightStyle, `${path}.highlightStyle`);
    }
    return;
  }

  if (type === "table") {
    const item = object(value, path, [
      "type",
      "id",
      "rows",
      "widths",
      "header",
      "selected",
      "offset",
      "columnSpacing",
      "style",
      "highlightStyle",
    ]);
    widgetId(item.id, `${path}.id`, state);
    const widths = collection(item.widths, `${path}.widths`);
    for (const [index, width] of widths.entries()) {
      constraint(width, `${path}.widths[${index}]`);
    }
    const row = (value: unknown, rowPath: string) => {
      const cells = collection(value, rowPath);
      if (cells.length !== widths.length) {
        invalid(rowPath, "cell count must match widths");
      }
      for (const [index, cell] of cells.entries()) {
        line(cell, `${rowPath}[${index}]`, state);
      }
    };
    const rows = collection(item.rows, `${path}.rows`);
    for (const [index, value] of rows.entries()) {
      row(value, `${path}.rows[${index}]`);
    }
    if (item.header !== undefined) row(item.header, `${path}.header`);
    selection(item.selected, `${path}.selected`, rows.length);
    offset(item.offset, `${path}.offset`, rows.length);
    if (item.columnSpacing !== undefined) {
      integer(item.columnSpacing, `${path}.columnSpacing`);
    }
    if (item.style !== undefined) style(item.style, `${path}.style`);
    if (item.highlightStyle !== undefined) {
      style(item.highlightStyle, `${path}.highlightStyle`);
    }
    return;
  }
  if (type === "tabs") {
    const item = object(value, path, [
      "type",
      "id",
      "titles",
      "selected",
      "style",
      "highlightStyle",
    ]);
    widgetId(item.id, `${path}.id`, state);
    const titles = collection(item.titles, `${path}.titles`);
    for (const [index, title] of titles.entries()) {
      line(title, `${path}.titles[${index}]`, state);
    }
    selection(item.selected, `${path}.selected`, titles.length);
    if (item.style !== undefined) style(item.style, `${path}.style`);
    if (item.highlightStyle !== undefined) {
      style(item.highlightStyle, `${path}.highlightStyle`);
    }
    return;
  }
  if (type === "gauge") {
    const item = object(value, path, [
      "type",
      "ratio",
      "label",
      "style",
      "gaugeStyle",
    ]);
    const ratio = finite(item.ratio, `${path}.ratio`);
    if (ratio < 0 || ratio > 1) {
      invalid(`${path}.ratio`, "ratio must be between 0 and 1");
    }
    if (item.label !== undefined) line([item.label], `${path}.label`, state);
    if (item.style !== undefined) style(item.style, `${path}.style`);
    if (item.gaugeStyle !== undefined) {
      style(item.gaugeStyle, `${path}.gaugeStyle`);
    }
    return;
  }
  if (type === "chart") {
    const item = object(value, path, [
      "type",
      "datasets",
      "xAxis",
      "yAxis",
      "style",
    ]);
    axis(item.xAxis, `${path}.xAxis`, state);
    axis(item.yAxis, `${path}.yAxis`, state);
    for (
      const [index, value] of collection(item.datasets, `${path}.datasets`)
        .entries()
    ) {
      const datasetPath = `${path}.datasets[${index}]`;
      const dataset = object(value, datasetPath, [
        "name",
        "data",
        "graphType",
        "style",
      ]);
      if (dataset.name !== undefined) {
        text(dataset.name, `${datasetPath}.name`, state);
      }
      if (
        dataset.graphType !== undefined &&
        !["line", "scatter", "bar"].includes(dataset.graphType as string)
      ) invalid(`${datasetPath}.graphType`, "unknown graph type");
      for (
        const [index, point] of collection(dataset.data, `${datasetPath}.data`)
          .entries()
      ) pair(point, `${datasetPath}.data[${index}]`);
      if (dataset.style !== undefined) {
        style(dataset.style, `${datasetPath}.style`);
      }
    }
    if (item.style !== undefined) style(item.style, `${path}.style`);
    return;
  }
  if (type === "scrollbar") {
    const item = object(value, path, [
      "type",
      "id",
      "contentLength",
      "position",
      "viewportContentLength",
      "orientation",
      "style",
    ]);
    widgetId(item.id, `${path}.id`, state);
    const length = integer(
      item.contentLength,
      `${path}.contentLength`,
      0xffffffff,
    );
    offset(item.position, `${path}.position`, length);
    if (item.viewportContentLength !== undefined) {
      integer(
        item.viewportContentLength,
        `${path}.viewportContentLength`,
        0xffffffff,
      );
    }
    if (
      item.orientation !== undefined &&
      !["verticalRight", "verticalLeft", "horizontalBottom", "horizontalTop"]
        .includes(item.orientation as string)
    ) invalid(`${path}.orientation`, "unknown orientation");
    if (item.style !== undefined) style(item.style, `${path}.style`);
    return;
  }

  invalid(`${path}.type`, "unknown node type");
}

/** Validate a frame, including the encoded payload size. */
export function validateFrame(
  value: unknown,
): asserts value is FrameDescription {
  const frame = object(value, "$", ["protocolVersion", "root"]);
  integer(frame.protocolVersion, "$.protocolVersion", 4_294_967_295);
  if (frame.protocolVersion !== PROTOCOL_VERSION) {
    throw new FrameError(
      "unsupportedProtocol",
      `Expected protocol ${PROTOCOL_VERSION}, received ${frame.protocolVersion}`,
    );
  }
  node(frame.root, "$.root", 1, {
    nodes: 0,
    spans: 0,
    textBytes: 0,
    ids: new Set(),
  });
  if (encoder.encode(JSON.stringify(value)).length > FRAME_LIMITS.bytes) {
    invalid("$", "encoded frame is too large");
  }
}

/** Encode a validated frame into bytes borrowed by a native render call. */
export function encodeFrame(frame: FrameDescription): Uint8Array {
  validateFrame(frame);
  return encoder.encode(JSON.stringify(frame));
}
