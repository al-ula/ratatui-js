import { assertEquals, assertThrows } from "@std/assert";
import {
  encodeFrame,
  type ErrorCode,
  FRAME_LIMITS,
  type FrameDescription,
  FrameError,
  type UiNode,
  validateFrame,
} from "./mod.ts";

interface Fixture {
  readonly name: string;
  readonly frame: unknown;
  readonly code?: ErrorCode;
}

const fixtures: { valid: Fixture[]; invalid: Fixture[] } = JSON.parse(
  await Deno.readTextFile(
    new URL("../../tests/fixtures/frames.json", import.meta.url),
  ),
);

for (const fixture of fixtures.valid) {
  Deno.test(`shared frame: ${fixture.name}`, () => {
    const frame = fixture.frame;
    validateFrame(frame);
    assertEquals(
      JSON.parse(new TextDecoder().decode(encodeFrame(frame))),
      frame,
    );
  });
}

for (const fixture of fixtures.invalid) {
  Deno.test(`reject shared frame: ${fixture.name}`, () => {
    const error = assertThrows(() => validateFrame(fixture.frame), FrameError);
    assertEquals(error.code, fixture.code);
  });
}

function frame(root: UiNode): FrameDescription {
  return { protocolVersion: 1, root };
}

Deno.test("nesting limit is inclusive", () => {
  let root: UiNode = { type: "paragraph", lines: [] };
  for (let depth = 1; depth < FRAME_LIMITS.depth; depth++) {
    root = { type: "block", child: root };
  }
  validateFrame(frame(root));
  assertThrows(
    () => validateFrame(frame({ type: "block", child: root })),
    FrameError,
  );
});

Deno.test("lone surrogates cannot silently change during UTF-8 encoding", () => {
  const error = assertThrows(
    () =>
      encodeFrame(frame({ type: "paragraph", lines: [[{ text: "\ud800" }]] })),
    FrameError,
  );
  assertEquals(error.code, "invalidFrame");
});

Deno.test("all Unicode control characters are rejected", () => {
  for (const text of ["\u0000", "\t", "\r", "\u007f", "\u009b"]) {
    assertThrows(
      () => encodeFrame(frame({ type: "paragraph", lines: [[{ text }]] })),
      FrameError,
    );
  }
});

Deno.test("collection, node, and span budgets are enforced", () => {
  const span = { text: "" };
  assertThrows(() =>
    validateFrame(frame({
      type: "paragraph",
      lines: [Array(FRAME_LIMITS.collectionItems + 1).fill(span)],
    })), FrameError);

  assertThrows(() =>
    validateFrame(frame({
      type: "row",
      children: Array(FRAME_LIMITS.nodes).fill({
        constraint: { kind: "fill", value: 1 },
        node: { type: "paragraph", lines: [] },
      }),
    })), FrameError);

  const line = Array(FRAME_LIMITS.collectionItems).fill(span);
  assertThrows(
    () =>
      validateFrame(
        frame({ type: "paragraph", lines: [line, line, line, line, [span]] }),
      ),
    FrameError,
  );
});

Deno.test("encoded JSON overhead counts toward the byte limit", () => {
  const root: UiNode = {
    type: "paragraph",
    lines: [[{ text: "x".repeat(FRAME_LIMITS.bytes) }]],
  };
  assertThrows(() => encodeFrame(frame(root)), FrameError);
});

Deno.test("numeric fields reject non-integers and values outside their range", () => {
  for (const spacing of [NaN, Infinity, -1, 0.5, 65_536]) {
    assertThrows(
      () => validateFrame(frame({ type: "row", children: [], spacing })),
      FrameError,
    );
  }
});

Deno.test("unknown nested fields and malformed styles are rejected", () => {
  for (
    const root of [
      { type: "paragraph", lines: [[{ text: "a", extra: true }]] },
      { type: "paragraph", lines: [], style: { bold: 1 } },
      { type: "paragraph", lines: [], style: null },
      {
        type: "block",
        child: { type: "paragraph", lines: [] },
        padding: { unknown: 1 },
      },
    ]
  ) {
    assertThrows(() => validateFrame({ protocolVersion: 1, root }), FrameError);
  }
});
