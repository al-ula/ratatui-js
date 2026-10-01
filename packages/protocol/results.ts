import type { ErrorDescription, RenderResult, TerminalEvent } from "./types.ts";

const codes = new Set([
  "invalidJson",
  "invalidFrame",
  "unsupportedProtocol",
  "io",
  "terminalBusy",
  "terminalPoisoned",
  "notTerminal",
  "rawModeActive",
  "closed",
  "renderingFailed",
  "concurrentEventWait",
  "invalidTimeout",
  "input",
  "panic",
  "initialization",
  "shutdown",
  "invalidArgument",
  "unsupportedAbi",
  "unsupportedCapability",
]);
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("Expected a native result object");
  }
  return value as Record<string, unknown>;
}
function integer(value: unknown, maximum = 65535): void {
  if (
    !Number.isInteger(value) || (value as number) < 0 ||
    (value as number) > maximum
  ) {
    throw new TypeError("Invalid native integer");
  }
}
export function decodeError(value: unknown, depth = 0): ErrorDescription {
  if (depth > 32) throw new TypeError("Native error nesting limit exceeded");
  const error = record(value);
  if (!codes.has(error.code as string) || typeof error.message !== "string") {
    throw new TypeError("Invalid native error");
  }
  for (const field of ["path", "operation"]) {
    if (error[field] !== undefined && typeof error[field] !== "string") {
      throw new TypeError(`Invalid error ${field}`);
    }
  }
  if (error.cause !== undefined) decodeError(error.cause, depth + 1);
  if (error.cleanup !== undefined) {
    if (!Array.isArray(error.cleanup)) {
      throw new TypeError("Invalid cleanup failures");
    }
    for (const item of error.cleanup) {
      const failure = record(item);
      if (
        typeof failure.operation !== "string" ||
        typeof failure.message !== "string"
      ) {
        throw new TypeError("Invalid cleanup failure");
      }
    }
  }
  if (error.code === "initialization" && (!error.cause || !error.cleanup)) {
    throw new TypeError("Missing initialization details");
  }
  if (error.code === "shutdown" && !error.cleanup) {
    throw new TypeError("Missing shutdown details");
  }
  return value as ErrorDescription;
}
export function decodeRenderResult(value: unknown): RenderResult {
  const result = record(value);
  integer(result.width);
  integer(result.height);
  if (!Array.isArray(result.widgetStates)) {
    throw new TypeError("Invalid widget states");
  }
  for (const item of result.widgetStates) {
    const state = record(item);
    if (typeof state.id !== "string") throw new TypeError("Invalid widget id");
    integer(state.offset, 0xffffffff);
    if (state.selected !== undefined) integer(state.selected, 0xffffffff);
  }
  return value as RenderResult;
}
function flags(value: unknown, allowed: readonly string[], name: string): void {
  if (
    !Array.isArray(value) || value.some((item) => !allowed.includes(item)) ||
    new Set(value).size !== value.length
  ) {
    throw new TypeError(`Invalid ${name}`);
  }
}
const modifiers = ["shift", "control", "alt", "super", "hyper", "meta"];

export function decodeEvent(value: unknown): TerminalEvent {
  const event = record(value);
  if (event.type === "resize") {
    integer(event.width);
    integer(event.height);
  } else if (event.type === "paste") {
    if (
      typeof event.text !== "string" ||
      /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u
        .test(event.text)
    ) {
      throw new TypeError("Invalid paste text");
    }
  } else if (event.type === "focus") {
    if (typeof event.focused !== "boolean") {
      throw new TypeError("Invalid focus event");
    }
  } else if (event.type === "mouse") {
    integer(event.column);
    integer(event.row);
    flags(event.modifiers, modifiers, "mouse modifiers");
    const kind = record(event.kind);
    if (["down", "up", "drag"].includes(kind.type as string)) {
      if (!["left", "right", "middle"].includes(kind.button as string)) {
        throw new TypeError("Invalid mouse button");
      }
    } else if (
      !["moved", "scrollUp", "scrollDown", "scrollLeft", "scrollRight"]
        .includes(kind.type as string) || kind.button !== undefined
    ) {
      throw new TypeError("Invalid mouse kind");
    }
  } else if (event.type === "key") {
    if (!["press", "repeat", "release"].includes(event.kind as string)) {
      throw new TypeError("Invalid key event");
    }
    flags(event.modifiers, modifiers, "key modifiers");
    if (event.state !== undefined) {
      flags(event.state, ["keypad", "capsLock", "numLock"], "key state");
    }
    const key = record(event.key);
    if (key.type === "character") {
      if (
        typeof key.value !== "string" || [...key.value].length !== 1 ||
        /^[\uD800-\uDFFF]$/u.test(key.value)
      ) {
        throw new TypeError("Invalid key character");
      }
    } else if (key.type === "function") {
      integer(key.value, 255);
    } else if (key.type === "media") {
      if (
        ![
          "play",
          "pause",
          "playPause",
          "reverse",
          "stop",
          "fastForward",
          "rewind",
          "trackNext",
          "trackPrevious",
          "record",
          "lowerVolume",
          "raiseVolume",
          "muteVolume",
        ].includes(key.value as string)
      ) throw new TypeError("Invalid media key");
    } else if (key.type === "modifier") {
      if (
        ![
          "leftShift",
          "leftControl",
          "leftAlt",
          "leftSuper",
          "leftHyper",
          "leftMeta",
          "rightShift",
          "rightControl",
          "rightAlt",
          "rightSuper",
          "rightHyper",
          "rightMeta",
          "isoLevel3Shift",
          "isoLevel5Shift",
        ].includes(key.value as string)
      ) throw new TypeError("Invalid modifier key");
    } else if (
      ![
        "enter",
        "escape",
        "backspace",
        "tab",
        "backTab",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageUp",
        "pageDown",
        "insert",
        "delete",
        "null",
        "capsLock",
        "scrollLock",
        "numLock",
        "printScreen",
        "pause",
        "menu",
        "keypadBegin",
      ].includes(key.type as string)
    ) {
      throw new TypeError("Invalid key code");
    }
  } else throw new TypeError("Invalid event type");
  return value as TerminalEvent;
}
