import { assertEquals, assertThrows } from "@std/assert";
import { decodeError, decodeEvent, decodeRenderResult } from "./results.ts";
Deno.test("shared lifecycle fixtures", async () => {
  const fixtures = JSON.parse(
    await Deno.readTextFile("tests/fixtures/errors.json"),
  );
  for (const fixture of fixtures) assertEquals(decodeError(fixture), fixture);
  assertThrows(() => decodeError({ code: "unknown", message: "error" }));
  assertThrows(() => decodeError({ code: "initialization", message: "error" }));
});
Deno.test("reject malformed native results", () => {
  assertThrows(() =>
    decodeRenderResult({ width: -1, height: 24, widgetStates: [] })
  );
  assertThrows(() =>
    decodeEvent({
      type: "key",
      key: { type: "character", value: "ab" },
      kind: "press",
      modifiers: [],
    })
  );
});

Deno.test("all native input fixtures decode without changing meaning", async () => {
  const fixtures = JSON.parse(
    await Deno.readTextFile("tests/fixtures/events.json"),
  );
  for (const fixture of fixtures) assertEquals(decodeEvent(fixture), fixture);
});

Deno.test("reject malformed extended input and unknown key flags", () => {
  const key = {
    type: "key",
    key: { type: "character", value: "q" },
    kind: "press",
    modifiers: [],
  };
  const mouse = {
    type: "mouse",
    kind: { type: "down", button: "left" },
    column: 0,
    row: 0,
    modifiers: [],
  };
  for (
    const event of [
      { type: "focus", focused: 1 },
      { type: "focus" },
      { type: "paste", text: null },
      { type: "paste", text: "\ud800" },
      { ...mouse, column: -1 },
      { ...mouse, row: 65536 },
      { ...mouse, row: 1.5 },
      { ...mouse, kind: { type: "down" } },
      { ...mouse, kind: { type: "drag", button: "unknown" } },
      { ...mouse, kind: { type: "scrollUp", button: "left" } },
      { ...mouse, kind: { type: "unknown" } },
      { ...mouse, modifiers: ["unknown"] },
      { ...mouse, modifiers: ["alt", "alt"] },
      { ...key, modifiers: ["unknown"] },
      { ...key, state: ["unknown"] },
      { ...key, state: null },
      { ...key, key: { type: "character", value: "\ud800" } },
      { ...key, key: { type: "media", value: "unknown" } },
      { ...key, key: { type: "modifier", value: "unknown" } },
      { ...key, key: { type: "function", value: 256 } },
      { ...key, kind: "unknown" },
    ]
  ) assertThrows(() => decodeEvent(event), TypeError);
});
