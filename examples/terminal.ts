import { createFrame } from "@ratatui-js/core";
import { DenoAdapter, onInterrupt } from "@ratatui-js/deno";
const driver = await new DenoAdapter(Deno.args[0]).open({});
const failures: unknown[] = [];
const detach = onInterrupt(driver, (error) => failures.push(error));
const frame = createFrame({
  type: "block",
  title: "Deno + Ratatui",
  border: "rounded",
  child: {
    type: "paragraph",
    lines: [[{ text: "Press q to quit; resize to redraw." }]],
  },
});
try {
  await driver.render(frame);
  for (;;) {
    const event = await driver.nextEvent();
    if (
      !event ||
      (event.type === "key" && event.key.type === "character" &&
        event.key.value === "q")
    ) break;
    if (event.type === "resize") await driver.render(frame);
  }
} finally {
  try {
    await driver.close();
  } finally {
    detach();
  }
}
if (failures.length) {
  throw new AggregateError(failures, "Interrupt cleanup failed");
}
