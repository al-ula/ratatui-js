import { PROTOCOL_VERSION, validateFrame } from "@ratatui-js/protocol";
import type {
  FrameDescription,
  RenderResult,
  TerminalCapabilities,
  TerminalDriver,
  TerminalEvent,
} from "../mod.ts";

/** An in-memory driver for application tests, not a Ratatui rendering emulator. */
export class FakeDriver implements TerminalDriver {
  readonly capabilities: TerminalCapabilities = {
    protocolVersion: PROTOCOL_VERSION,
    keyboard: true,
    resize: true,
  };
  readonly renderedFrames: FrameDescription[] = [];

  #closed = false;
  #events: TerminalEvent[] = [];
  #waiter: ((event: TerminalEvent | null) => void) | undefined;

  constructor(readonly width = 80, readonly height = 24) {}

  render(frame: FrameDescription): Promise<RenderResult> {
    if (this.#closed) return Promise.reject(new Error("Terminal is closed"));
    validateFrame(frame);
    this.renderedFrames.push(structuredClone(frame));
    return Promise.resolve({
      width: this.width,
      height: this.height,
      widgetStates: [],
    });
  }

  nextEvent(): Promise<TerminalEvent | null> {
    if (this.#closed) return Promise.resolve(null);
    if (this.#waiter) {
      return Promise.reject(new Error("An event wait is already outstanding"));
    }
    const event = this.#events.shift();
    if (event) return Promise.resolve(event);
    return new Promise((resolve) => {
      this.#waiter = resolve;
    });
  }

  pushEvent(event: TerminalEvent): void {
    if (this.#closed) throw new Error("Terminal is closed");
    if (this.#waiter) {
      const resolve = this.#waiter;
      this.#waiter = undefined;
      resolve(event);
    } else {
      this.#events.push(event);
    }
  }

  close(): Promise<void> {
    this.#closed = true;
    this.#events = [];
    const resolve = this.#waiter;
    this.#waiter = undefined;
    resolve?.(null);
    return Promise.resolve();
  }
}
