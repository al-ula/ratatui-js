import { assertEquals, assertRejects } from "@std/assert";
import { createFrame, EXIT, paragraph, startApplication } from "./mod.ts";
import type { FrameDescription, RenderResult, TerminalEvent } from "./mod.ts";
import { FakeDriver } from "./testing/fake_driver.ts";
class Driver extends FakeDriver {
  closes = 0;
  override close(): Promise<void> {
    this.closes++;
    return super.close();
  }
}
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
const key: TerminalEvent = {
  type: "key",
  key: { type: "enter" },
  kind: "press",
  modifiers: [],
};
const view = (model: number) => paragraph([[{ text: String(model) }]]);
Deno.test("initial draw, queued updates coalesce, and explicit exit cleans up", async () => {
  const driver = new Driver();
  const rendered = deferred();
  let draws = 0;
  const app = startApplication(driver, {
    initialModel: 0,
    view,
    update: (model) => model,
    rendered: (model) => {
      if (++draws === 2) rendered.resolve();
      return model;
    },
  });
  await Promise.all([
    app.update((model) => model + 1),
    app.update((model) => model + 2),
  ]);
  await rendered.promise;
  app.exit();
  assertEquals(await app.done, 3);
  assertEquals(driver.renderedFrames, [
    createFrame(view(0)),
    createFrame(view(3)),
  ]);
  assertEquals(driver.closes, 1);
  await assertRejects(() => app.update((model) => model));
});
Deno.test("async model updates serialize with events and resize redraws", async () => {
  const driver = new Driver();
  const entered = deferred(), release = deferred(), resized = deferred();
  const order: string[] = [];
  const app = startApplication(driver, {
    initialModel: 0,
    view,
    update: (model, event) => {
      order.push(event.type);
      if (event.type === "resize") resized.resolve();
      return event.type === "key" ? EXIT : model;
    },
  });
  const change = app.update(async (model) => {
    order.push("start");
    entered.resolve();
    await release.promise;
    order.push("end");
    return model + 1;
  });
  await entered.promise;
  driver.pushEvent({ type: "resize", width: 90, height: 30 });
  release.resolve();
  await change;
  await resized.promise;
  driver.pushEvent(key);
  assertEquals(await app.done, 1);
  assertEquals(order, ["start", "end", "resize", "key"]);
  assertEquals(driver.renderedFrames.length >= 2, true);
  assertEquals(driver.closes, 1);
});
Deno.test("returned native list state reaches the next model update", async () => {
  class StatefulDriver extends Driver {
    override async render(frame: FrameDescription): Promise<RenderResult> {
      const result = await super.render(frame);
      return {
        ...result,
        widgetStates: [{ id: "list", offset: 2, selected: 3 }],
      };
    }
  }
  const driver = new StatefulDriver();
  let observed = 0;
  const app = startApplication(driver, {
    initialModel: 0,
    view,
    rendered: (_model, result) => result.widgetStates[0]!.offset,
    update: (model) => {
      observed = model;
      return EXIT;
    },
  });
  driver.pushEvent(key);
  assertEquals(await app.done, 2);
  assertEquals(observed, 2);
});
for (
  const point of ["view", "update", "rendered", "render", "nextEvent"] as const
) {
  Deno.test(`${point} failure restores terminal and rejects done`, async () => {
    const error = new Error(`${point} failed`);
    class FailingDriver extends Driver {
      override render(frame: FrameDescription): Promise<RenderResult> {
        return point === "render" ? Promise.reject(error) : super.render(frame);
      }
      override nextEvent(): Promise<TerminalEvent | null> {
        return point === "nextEvent"
          ? Promise.reject(error)
          : super.nextEvent();
      }
    }
    const driver = new FailingDriver();
    const app = startApplication(driver, {
      initialModel: 0,
      view: (model) => {
        if (point === "view") throw error;
        return view(model);
      },
      update: () => {
        throw error;
      },
      rendered: (model) => {
        if (point === "rendered") throw error;
        return model;
      },
    });
    driver.pushEvent(key);
    await assertRejects(() => app.done, Error, `${point} failed`);
    assertEquals(driver.closes, 1);
  });
}
Deno.test("callback and cleanup failures are both retained", async () => {
  const primary = new Error("view"), cleanup = new Error("close");
  class FailingClose extends Driver {
    override async close(): Promise<void> {
      await super.close();
      throw cleanup;
    }
  }
  const driver = new FailingClose();
  const app = startApplication(driver, {
    initialModel: 0,
    view: () => {
      throw primary;
    },
    update: (model) => model,
  });
  const error = await assertRejects(() => app.done, AggregateError);
  assertEquals(error.errors, [primary, cleanup]);
  assertEquals(driver.closes, 1);
});
Deno.test("driver closure exits while waiting", async () => {
  const driver = new Driver();
  const drawn = deferred();
  const app = startApplication(driver, {
    initialModel: 0,
    view,
    update: (model) => model,
    rendered: (model) => {
      drawn.resolve();
      return model;
    },
  });
  await drawn.promise;
  await driver.close();
  assertEquals(await app.done, 0);
});
