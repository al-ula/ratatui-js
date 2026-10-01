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
