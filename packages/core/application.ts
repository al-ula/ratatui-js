import {
  createFrame,
  type RenderResult,
  type TerminalDriver,
  type TerminalEvent,
  type UiNode,
} from "./mod.ts";

export const EXIT = Symbol("application exit");
type MaybePromise<Value> = Value | Promise<Value>;
export interface ApplicationDefinition<Model> {
  readonly initialModel: Model;
  readonly view: (model: Model) => MaybePromise<UiNode>;
  readonly update: (
    model: Model,
    event: TerminalEvent,
  ) => MaybePromise<Model | typeof EXIT>;
  /** Apply native list offsets/selections to the JS model. No automatic redraw. */
  readonly rendered?: (
    model: Model,
    result: RenderResult,
  ) => MaybePromise<Model>;
}
export interface Application<Model> {
  /** Resolves with the final model after terminal cleanup, or rejects on failure. */
  readonly done: Promise<Model>;
  /** Updates serialize with event callbacks. Queued changes share a redraw. */
  update(
    change: (model: Model) => MaybePromise<Model | typeof EXIT>,
  ): Promise<void>;
  /** Wakes the runner; cleanup wakes any outstanding native event wait. */
  exit(): void;
}
interface Change<Model> {
  apply: (model: Model) => MaybePromise<Model | typeof EXIT>;
  resolve: () => void;
  reject: (error: unknown) => void;
}

/** Owns the supplied driver until done settles. Models always live in JS. */
export function startApplication<Model>(
  driver: TerminalDriver,
  definition: ApplicationDefinition<Model>,
): Application<Model> {
  let model = definition.initialModel;
  let exiting = false;
  let failed = false;
  let failure: unknown;
  let wake: (() => void) | undefined;
  const queue: Change<Model>[] = [];
  function exit(): void {
    exiting = true;
    wake?.();
  }
  function fail(error: unknown): void {
    if (!failed) {
      failed = true;
      failure = error;
    }
    exit();
  }
  function update(apply: Change<Model>["apply"]): Promise<void> {
    if (exiting) return Promise.reject(new Error("Application exited"));
    return new Promise((resolve, reject) => {
      queue.push({ apply, resolve, reject });
      wake?.();
    });
  }
  async function draw(): Promise<void> {
    const result = await driver.render(
      createFrame(await definition.view(model)),
    );
    if (definition.rendered) model = await definition.rendered(model, result);
  }
  // Begin event reading after the runner's initial draw starts. The event pump
  // awaits each admitted update, bounding input independently of rendering.
  async function readEvents(): Promise<void> {
    try {
      while (!exiting) {
        const event = await driver.nextEvent();
        if (!event) {
          exit();
          break;
        }
        if (exiting) break;
        await update((current) => definition.update(current, event));
      }
    } catch (error) {
      if (!exiting) fail(error);
    }
  }
  async function run(): Promise<Model> {
    let events: Promise<void> | undefined;
    const cleanupFailures: unknown[] = [];
    try {
      await draw();
      events = readEvents();
      while (!exiting) {
        if (!queue.length) {
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
          wake = undefined;
        }
        let dirty = false;
        while (queue.length && !exiting) {
          const change = queue.shift()!;
          try {
            const next = await change.apply(model);
            if (next === EXIT) exit();
            else {
              model = next;
              dirty = true;
            }
            change.resolve();
          } catch (error) {
            change.reject(error);
            throw error;
          }
        }
        // A resize goes through update even if it returns the same model;
        // every admitted update invalidates the frame. New changes arriving
        // during rendering are drained together before the next draw.
        if (dirty && !exiting) await draw();
      }
    } catch (error) {
      fail(error);
    } finally {
      exit();
      for (const change of queue.splice(0)) {
        change.reject(failed ? failure : new Error("Application exited"));
      }
      try {
        await driver.close();
      } catch (error) {
        cleanupFailures.push(error);
      }
      await events;
    }
    if (cleanupFailures.length) {
      throw new AggregateError(
        failed ? [failure, ...cleanupFailures] : cleanupFailures,
        "Application cleanup failed",
      );
    }
    if (failed) throw failure;
    return model;
  }
  const done = run();
  return { done, update, exit };
}

/** Convenience wrapper for applications driven entirely by terminal events. */
export function runApplication<Model>(
  driver: TerminalDriver,
  definition: ApplicationDefinition<Model>,
): Promise<Model> {
  return startApplication(driver, definition).done;
}
