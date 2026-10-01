import { assertEquals, assertRejects } from "@std/assert";
import { createFrame } from "@ratatui-js/core";
import { DenoAdapter, onInterrupt } from "../../packages/deno/mod.ts";
const [library, scenario] = Deno.args;
if (!library) throw new Error("Pass a native library path");
const driver = await new DenoAdapter(library).open({
  alternateScreen: scenario !== "no-alternate",
});
const failures: unknown[] = [];
const detach = onInterrupt(driver, (error) => failures.push(error));
const frame = createFrame({
  type: "paragraph",
  lines: [[{ text: "PTY frame" }]],
});
try {
  assertEquals((await driver.render(frame)).width, 80);
  const pending = driver.nextEvent();
  await assertRejects(() => driver.nextEvent());
  // Waiting for native input must allow timers and native rendering to progress.
  await new Promise((resolve) => setTimeout(resolve, 150));
  assertEquals((await driver.render(frame)).height, 24);
  if (scenario === "close-wait") {
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
      } else if (event.key.type === "character" && event.key.value === "q") {
        break;
      }
      event = await driver.nextEvent();
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
