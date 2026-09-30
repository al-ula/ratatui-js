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
      const constraint = object(child.constraint, `${childPath}.constraint`, [
        "kind",
        "value",
      ]);
      if (
        !["length", "min", "max", "percentage", "fill"].includes(
          constraint.kind as string,
        )
      ) {
        invalid(`${childPath}.constraint.kind`, "unknown constraint");
      }
      const amount = integer(
        constraint.value,
        `${childPath}.constraint.value`,
        constraint.kind === "percentage" ? 100 : 65_535,
      );
      if (constraint.kind === "fill" && amount === 0) {
        invalid(`${childPath}.constraint.value`, "fill must be positive");
      }
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
    const id = text(list.id, `${path}.id`, state);
    if (id.length === 0 || state.ids.has(id)) {
      invalid(`${path}.id`, "expected a nonempty, unique widget ID");
    }
    state.ids.add(id);
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
