import { consoleModes } from "./console_modes.ts";
import { assertEquals } from "@std/assert";
import {
  column,
  EXIT,
  list,
  paragraph,
  startApplication,
} from "@ratatui-js/core";
import { DenoAdapter, onInterrupt } from "../../packages/deno/mod.ts";
const [library, scenario] = Deno.args;
if (!library) throw new Error("Pass library path");
const modes = consoleModes();
const driver = await new DenoAdapter(library).open({
  alternateScreen: scenario !== "no-alternate",
});
const errors: unknown[] = [];
const detach = onInterrupt(driver, (error) => errors.push(error));
const items = ["one", "two", "three"].map((text) => [{ text }]);
let ready = false;
const app = startApplication(driver, {
  initialModel: { selected: 0, offset: 0 },
  view: (model) =>
    column([
      {
        constraint: { kind: "length", value: 1 },
        node: paragraph([[{ text: "PTY frame" }]]),
      },
      {
        constraint: { kind: "fill", value: 1 },
        node: list("items", items, model),
      },
    ]),
  update: (model, event) => {
    if (event.type !== "key") return model;
    if (event.key.type === "character" && event.key.value === "q") return EXIT;
    if (event.key.type === "down") {
      return { ...model, selected: Math.min(2, model.selected + 1) };
    }
    if (event.key.type === "up") {
      return { ...model, selected: Math.max(0, model.selected - 1) };
    }
    return model;
  },
  rendered: (model, result) => {
    if (!ready) {
      assertEquals([result.width, result.height], [80, 24]);
      ready = true;
      console.log("PTY_READY\r");
    }
    if (result.width === 90) {
      assertEquals(result.height, 30);
      console.log("PTY_RESIZED\r");
    }
    return { ...model, offset: result.widgetStates[0]!.offset };
  },
});
try {
  if (scenario === "close-wait") {
    await new Promise((resolve) => setTimeout(resolve, 150));
    await app.update((model) => ({ ...model, selected: 1 }));
    app.exit();
  }
  const model = await app.done;
  if (scenario !== "interrupt") assertEquals(model.selected, 1);
} finally {
  detach();
}
assertEquals(errors, []);

modes.verify();
