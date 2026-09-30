import { assertEquals, assertRejects, assertThrows } from "@std/assert";
import { FrameError } from "@ratatui-js/protocol";
import {
  createFrame,
  type PlatformAdapter,
  type TerminalEvent,
} from "./mod.ts";
import { FakeDriver } from "./testing/fake_driver.ts";

Deno.test("a runtime-independent adapter accepts a validated frame", async () => {
  const driver = new FakeDriver(12, 3);
  const adapter: PlatformAdapter = { open: () => Promise.resolve(driver) };
  const terminal = await adapter.open({});
  const frame = createFrame({
    type: "paragraph",
    lines: [[{ text: "hello" }]],
  });
  try {
    assertEquals(await terminal.render(frame), {
      width: 12,
      height: 3,
      widgetStates: [],
    });
    assertEquals(driver.renderedFrames, [frame]);
  } finally {
    await terminal.close();
  }
});

Deno.test("invalid UI descriptions are rejected before adapter submission", () => {
  assertThrows(
    () => createFrame({ type: "paragraph", lines: [[{ text: "\u001b[31m" }]] }),
    FrameError,
  );
});

Deno.test("closing wakes a pending event wait and is idempotent", async () => {
  const driver = new FakeDriver();
  const pending = driver.nextEvent();
  await driver.close();
  assertEquals(await pending, null);
  await driver.close();
  assertEquals(await driver.nextEvent(), null);
  await assertRejects(
    () => driver.render(createFrame({ type: "paragraph", lines: [] })),
    Error,
    "closed",
  );
});

Deno.test("only one outstanding event wait is allowed", async () => {
  const driver = new FakeDriver();
  const pending = driver.nextEvent();
  await assertRejects(() => driver.nextEvent(), Error, "already outstanding");
  await driver.close();
  assertEquals(await pending, null);
});

Deno.test("events are delivered in order to queued and waiting readers", async () => {
  const driver = new FakeDriver();
  const resize: TerminalEvent = { type: "resize", width: 100, height: 30 };
  const key: TerminalEvent = {
    type: "key",
    key: { type: "character", value: "q" },
    kind: "press",
    modifiers: [],
  };
  driver.pushEvent(resize);
  assertEquals(await driver.nextEvent(), resize);
  const pending = driver.nextEvent();
  driver.pushEvent(key);
  assertEquals(await pending, key);
  await driver.close();
});
