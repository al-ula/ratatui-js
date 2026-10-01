import { consoleModes } from "./console_modes.ts";
import { assertEquals, assertRejects } from "@std/assert";
import { createFrame } from "@ratatui-js/core";
import {
  DenoAdapter,
  NativeError,
  onInterrupt,
} from "../../packages/deno/mod.ts";
const [library, scenario] = Deno.args;
if (!library) throw new Error("Pass a native library path");
const modes = consoleModes();
const adapter = new DenoAdapter(library);
if (
  scenario === "unsupported-keyboard" || scenario === "keyboard-probe-timeout"
) {
  const error = await assertRejects(
    () => adapter.open({ enhancedKeyboard: true }),
    NativeError,
  );
  assertEquals(
    error.description.code,
    scenario === "unsupported-keyboard" ? "unsupportedCapability" : "io",
  );
  // A successful second open proves failed negotiation rolled back and released ownership.
}
const extended = scenario === "extended";
const driver = await new DenoAdapter(library).open({
  alternateScreen: scenario !== "no-alternate",
  mouseCapture: extended,
  bracketedPaste: extended,
  focusReporting: extended,
  enhancedKeyboard: extended,
});
const busy = await assertRejects(
  () => new DenoAdapter(library).open({}),
  NativeError,
);
assertEquals(busy.description.code, "terminalBusy");
const failures: unknown[] = [];
const detach = onInterrupt(driver, (error) => failures.push(error));
const frame = createFrame({
  type: "paragraph",
  lines: [[{ text: "PTY frame" }]],
});
async function nextInputEvent() {
  // ConPTY may report the existing dimensions before the host sends input.
  while (true) {
    const event = await driver.nextEvent();
    if (event?.type === "resize" && event.width === 80 && event.height === 24) {
      continue;
    }
    return event;
  }
}
try {
  assertEquals((await driver.render(frame)).width, 80);
  const pending = nextInputEvent();
  await assertRejects(() => driver.nextEvent());
  // Waiting for native input must allow timers and native rendering to progress.
  await new Promise((resolve) => setTimeout(resolve, 150));
  assertEquals((await driver.render(frame)).height, 24);
  if (
    scenario === "unsupported-keyboard" || scenario === "keyboard-probe-timeout"
  ) {
    await driver.close();
    assertEquals(await pending, null);
  } else if (extended) {
    assertEquals(driver.capabilities, {
      protocolVersion: 1,
      keyboard: true,
      resize: true,
      mouse: true,
      paste: true,
      focus: true,
      enhancedKeyboard: true,
    });
    console.log("PTY_READY\r");
    const fixture = JSON.parse(
      await Deno.readTextFile(
        new URL("../fixtures/extended-input.json", import.meta.url),
      ),
    );
    for (let index = 0; index < fixture.events.length; index++) {
      assertEquals(
        index === 0 ? await pending : await nextInputEvent(),
        fixture.events[index],
      );
    }
  } else if (scenario === "close-wait") {
    await Promise.all([driver.close(), driver.close()]);
    assertEquals(await pending, null);
    await assertRejects(() => driver.render(frame));
  } else {
    console.log("PTY_READY\r");
    let event = await pending;
    while (event) {
      if (event.type === "resize") {
        assertEquals([event.width, event.height], [90, 30]);
        assertEquals((await driver.render(frame)).height, 30);
        console.log("PTY_RESIZED\r");
      } else if (
        event.type === "key" && event.key.type === "character" &&
        event.key.value === "q"
      ) {
        break;
      }
      event = await nextInputEvent();
    }
    if (scenario === "interrupt") assertEquals(event, null);
  }
} finally {
  try {
    await driver.close();
  } finally {
    detach();
  }
}
assertEquals(failures, []);

modes.verify();
