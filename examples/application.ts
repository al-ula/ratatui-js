import {
  block,
  column,
  EXIT,
  list,
  paragraph,
  runApplication,
} from "@ratatui-js/core";
import { DenoAdapter, onInterrupt } from "@ratatui-js/deno";
const items = [
  "Native rendering",
  "Keyboard input",
  "Resize redraw",
  "Graceful shutdown",
].map((text) => [{ text }]);
const driver = await new DenoAdapter(Deno.args[0]).open({});
const failures: unknown[] = [];
const detach = onInterrupt(driver, (error) => failures.push(error));
try {
  await runApplication(driver, {
    initialModel: { selected: 0, offset: 0 },
    view: (model) =>
      block(
        column([
          {
            constraint: { kind: "fill", value: 1 },
            node: list("features", items, {
              ...model,
              highlightStyle: { reversed: true },
            }),
          },
          {
            constraint: { kind: "length", value: 1 },
            node: paragraph([[{ text: "↑/↓ select • q quit" }]]),
          },
        ]),
        { title: "Deno + Ratatui", border: "rounded" },
      ),
    update: (model, event) => {
      if (event.type !== "key" || event.kind === "release") return model;
      const key = event.key;
      if (key.type === "character" && key.value === "q") return EXIT;
      if (key.type === "up") {
        return { ...model, selected: Math.max(0, model.selected - 1) };
      }
      if (key.type === "down") {
        return {
          ...model,
          selected: Math.min(items.length - 1, model.selected + 1),
        };
      }
      return model;
    },
    rendered: (model, result) => ({
      ...model,
      offset: result.widgetStates.find((state) =>
        state.id === "features"
      )?.offset ?? model.offset,
    }),
  });
} finally {
  detach();
}
if (failures.length) {
  throw new AggregateError(failures, "Interrupt cleanup failed");
}
