/** Real terminal adapter for Deno. Requires --allow-ffi for the native library. */
import type {
  PlatformAdapter,
  TerminalDriver,
  TerminalOptions,
} from "@ratatui-js/core";
import {
  decodeError,
  decodeEvent,
  decodeRenderResult,
  encodeFrame,
  type ErrorDescription,
  type FrameDescription,
  PROTOCOL_VERSION,
  type RenderResult,
  type TerminalEvent,
} from "@ratatui-js/protocol";

export const ABI_VERSION = 1;
const symbols = {
  rt_abi_version: { parameters: [], result: "u32" },
  rt_protocol_version: { parameters: [], result: "u32" },
  rt_create: {
    parameters: ["u32", "u32", "u32", "buffer", "buffer"],
    result: "u32",
    nonblocking: true,
  },
  rt_render: {
    parameters: ["pointer", "buffer", "usize", "buffer", "buffer"],
    result: "u32",
    nonblocking: true,
  },
  rt_poll_event: {
    parameters: ["pointer", "u32", "buffer", "buffer"],
    result: "u32",
    nonblocking: true,
  },
  rt_close: {
    parameters: ["pointer", "buffer"],
    result: "u32",
    nonblocking: true,
  },
  rt_destroy: {
    parameters: ["pointer", "buffer"],
    result: "u32",
    nonblocking: true,
  },
  rt_bytes_free: { parameters: ["pointer", "usize"], result: "void" },
} as const satisfies Deno.ForeignLibraryInterface;
type Library = Deno.DynamicLibrary<typeof symbols>;

export class NativeError extends Error {
  constructor(readonly description: ErrorDescription) {
    super(description.message);
    this.name = "NativeError";
  }
}

export function nativeLibraryPath(): URL {
  const { os, arch } = Deno.build;
  if (
    !["linux", "darwin", "windows"].includes(os) ||
    !["x86_64", "aarch64"].includes(arch)
  ) {
    throw new Error(`Unsupported native target: ${os}-${arch}`);
  }
  const file = os === "windows"
    ? "ratatui_js_ffi.dll"
    : os === "darwin"
    ? "libratatui_js_ffi.dylib"
    : "libratatui_js_ffi.so";
  return new URL(`./native/${os}-${arch}/${file}`, import.meta.url);
}

/** An explicit library path supports local builds; packaged builds resolve by target. */
export class DenoAdapter implements PlatformAdapter {
  constructor(readonly libraryPath: string | URL = nativeLibraryPath()) {}

  async open(options: TerminalOptions = {}): Promise<TerminalDriver> {
    if (
      options.alternateScreen !== undefined &&
      typeof options.alternateScreen !== "boolean"
    ) {
      throw new TypeError("alternateScreen must be boolean");
    }
    // Deno pointer inspection requires unrestricted FFI permission, even when
    // dlopen itself is permitted for a specific path. Check before raw mode.
    if ((await Deno.permissions.query({ name: "ffi" })).state !== "granted") {
      throw new Error(
        "Native pointer access requires --allow-ffi (without a path restriction)",
      );
    }
    const library = Deno.dlopen(this.libraryPath, symbols);
    let handle: Deno.PointerValue = null;
    const error = new Uint8Array(16);
    try {
      if (library.symbols.rt_abi_version() !== ABI_VERSION) {
        throw new NativeError({
          code: "unsupportedAbi",
          message: "Unsupported native ABI",
        });
      }
      if (library.symbols.rt_protocol_version() !== PROTOCOL_VERSION) {
        throw new NativeError({
          code: "unsupportedProtocol",
          message: "Unsupported native protocol",
        });
      }
      const storage = new BigUint64Array(1);
      const status = await library.symbols.rt_create(
        ABI_VERSION,
        PROTOCOL_VERSION,
        options.alternateScreen === false ? 0 : 1,
        storage,
        error,
      );
      handle = Deno.UnsafePointer.create(storage[0]!);
      const result = takeBytes(library, error);
      checkStatus(status, result, [0]);
      if (!handle) throw new TypeError("Native create returned no handle");
      return new DenoDriver(library, handle);
    } catch (failure) {
      // A malformed successful create must still release any returned handle.
      if (handle) {
        const cleanup = new Uint8Array(16);
        try {
          await library.symbols.rt_destroy(handle, cleanup);
        } finally {
          takeBytes(library, cleanup);
        }
      }
      library.close();
      throw failure;
    }
  }
}

/** Copies native memory and frees the exact descriptor even if decoding fails. */
function takeBytes(library: Library, storage: Uint8Array): unknown {
  const view = new DataView(
    storage.buffer,
    storage.byteOffset,
    storage.byteLength,
  );
  const address = view.getBigUint64(0, true);
  const length = view.getBigUint64(8, true);
  const pointer = Deno.UnsafePointer.create(address);
  if (!pointer) {
    if (length !== 0n) {
      throw new TypeError("Native null buffer has nonzero length");
    }
    return undefined;
  }
  try {
    if (length === 0n || length > 16_777_216n) {
      throw new TypeError("Invalid native output length");
    }
    const copy = new Uint8Array(Number(length));
    new Deno.UnsafePointerView(pointer).copyInto(copy);
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(copy));
  } finally {
    library.symbols.rt_bytes_free(pointer, length);
    storage.fill(0);
  }
}
function checkStatus(
  status: number,
  error: unknown,
  allowed: readonly number[],
): void {
  if (status === 3) throw new NativeError(decodeError(error));
  if (error !== undefined || !allowed.includes(status)) {
    throw new TypeError(`Invalid native status ${status}`);
  }
}

class DenoDriver implements TerminalDriver {
  readonly capabilities = {
    protocolVersion: PROTOCOL_VERSION,
    keyboard: true,
    resize: true,
  };
  #closing = false;
  #close: Promise<void> | undefined;
  #renders: Promise<unknown> = Promise.resolve();
  #waiting: Promise<TerminalEvent | null> | undefined;
  constructor(readonly library: Library, readonly handle: Deno.PointerValue) {}

  render(frame: FrameDescription): Promise<RenderResult> {
    if (this.#closing) {
      return Promise.reject(
        new NativeError({ code: "closed", message: "Session is closed" }),
      );
    }
    // Encode now to snapshot mutable caller input before queued native work.
    let bytes: Uint8Array;
    try {
      bytes = encodeFrame(frame);
    } catch (error) {
      return Promise.reject(error);
    }
    const render = this.#renders.then(async () => {
      if (this.#closing) {
        throw new NativeError({ code: "closed", message: "Session is closed" });
      }
      const output = new Uint8Array(16), error = new Uint8Array(16);
      const status = await this.library.symbols.rt_render(
        this.handle,
        bytes,
        BigInt(bytes.length),
        output,
        error,
      );
      let value: unknown, failure: unknown;
      try {
        value = takeBytes(this.library, output);
      } finally {
        failure = takeBytes(this.library, error);
      }
      checkStatus(status, failure, [0]);
      return decodeRenderResult(value);
    });
    this.#renders = render.catch(() => {});
    return render;
  }
  nextEvent(): Promise<TerminalEvent | null> {
    if (this.#waiting) {
      return Promise.reject(
        new NativeError({
          code: "concurrentEventWait",
          message: "An event wait is outstanding",
        }),
      );
    }
    if (this.#closing) return Promise.resolve(null);
    const waiting = this.#poll();
    this.#waiting = waiting;
    // Clear without creating an unhandled rejection from a detached finally.
    waiting.then(() => {
      this.#waiting = undefined;
    }, () => {
      this.#waiting = undefined;
    });
    return waiting;
  }
  async #poll(): Promise<TerminalEvent | null> {
    while (!this.#closing) {
      const output = new Uint8Array(16), error = new Uint8Array(16);
      const status = await this.library.symbols.rt_poll_event(
        this.handle,
        100,
        output,
        error,
      );
      let value: unknown, failure: unknown;
      try {
        value = takeBytes(this.library, output);
      } finally {
        failure = takeBytes(this.library, error);
      }
      checkStatus(status, failure, [0, 1, 2]);
      if (status === 0) return decodeEvent(value);
      if (value !== undefined) throw new TypeError("Unexpected poll output");
      if (status === 2) return null;
    }
    return null;
  }
  close(): Promise<void> {
    if (this.#close) return this.#close;
    this.#closing = true;
    this.#close = this.#shutdown();
    return this.#close;
  }
  async #shutdown(): Promise<void> {
    const failures: unknown[] = [];
    const error = new Uint8Array(16);
    try {
      // Wake the native poll before waiting on JavaScript operations.
      try {
        const status = await this.library.symbols.rt_close(this.handle, error);
        checkStatus(status, takeBytes(this.library, error), [0]);
      } catch (failure) {
        failures.push(failure);
      }
      await Promise.allSettled([this.#renders, this.#waiting]);
      try {
        const status = await this.library.symbols.rt_destroy(
          this.handle,
          error,
        );
        checkStatus(status, takeBytes(this.library, error), [0]);
      } catch (failure) {
        failures.push(failure);
      }
    } finally {
      this.library.close();
    }
    if (failures.length) {
      throw new AggregateError(failures, "Terminal restoration failed");
    }
  }
}

/** Registers graceful interrupt cleanup; call the returned function to detach. */
export function onInterrupt(
  driver: TerminalDriver,
  report: (error: unknown) => void,
): () => void {
  const listener = () => {
    driver.close().catch(report);
  };
  const signals: Deno.Signal[] = Deno.build.os === "windows"
    ? ["SIGINT"]
    : ["SIGINT", "SIGTERM"];
  const registered: Deno.Signal[] = [];
  try {
    for (const signal of signals) {
      Deno.addSignalListener(signal, listener);
      registered.push(signal);
    }
  } catch (error) {
    for (const signal of registered) {
      Deno.removeSignalListener(signal, listener);
    }
    throw error;
  }
  return () => {
    for (const signal of registered) {
      Deno.removeSignalListener(signal, listener);
    }
  };
}
