import { assertEquals, assertRejects } from "@std/assert";
import { createFrame, type TerminalOptions } from "@ratatui-js/core";
import { DenoAdapter, NativeError } from "../../packages/deno/mod.ts";

const scenario = Deno.env.get("RATATUI_JS_CUSTOM_SCENARIO")!;
const fd = Number(Deno.env.get("RATATUI_JS_CUSTOM_FD"));
const outputFd = Number(Deno.env.get("RATATUI_JS_CUSTOM_OUTPUT_FD"));
const stream = scenario === "tty" ? "tty" : fd;
const options: TerminalOptions = {
  input: stream,
  output: scenario === "cleanup" ? outputFd : stream,
};
const adapter = new DenoAdapter(Deno.args[0]);
const libc = Deno.dlopen(
  Deno.build.os === "darwin" ? "/usr/lib/libSystem.B.dylib" : "libc.so.6",
  {
    write: { parameters: ["i32", "buffer", "usize"], result: "isize" },
    close: { parameters: ["i32"], result: "i32" },
    dup: { parameters: ["i32"], result: "i32" },
  },
);
const marker = libc.symbols.dup(outputFd);
function write(value: string) {
  const bytes = new TextEncoder().encode(value);
  assertEquals(
    libc.symbols.write(marker, bytes, BigInt(bytes.length)),
    BigInt(bytes.length),
  );
}
try {
  if (scenario === "render-failure") {
    const error = await assertRejects(() => adapter.open(options), NativeError);
    assertEquals(error.description.operation, "create renderer");
  } else if (scenario === "invalid") {
    const closed = libc.symbols.dup(fd);
    assertEquals(libc.symbols.close(closed), 0);
    await assertRejects(
      () => adapter.open({ ...options, input: closed }),
      NativeError,
    );
    for (
      const invalid of [-3, 0.5, NaN, Infinity, 2_147_483_648, null, "bad", {}]
    ) {
      await assertRejects(
        () => adapter.open({ ...options, input: invalid } as TerminalOptions),
        TypeError,
      );
    }
    await (await adapter.open(options)).close();
  } else if (scenario === "rollback") {
    const error = await assertRejects(
      () => adapter.open({ ...options, enhancedKeyboard: true }),
      NativeError,
    );
    assertEquals(error.description.code, "unsupportedCapability");
    await (await adapter.open(options)).close();
  } else {
    const extended = scenario === "extended";
    const driver = await adapter.open({
      ...options,
      mouseCapture: extended,
      bracketedPaste: extended,
      focusReporting: extended,
      enhancedKeyboard: extended,
    });
    assertEquals(libc.symbols.close(fd), 0);
    const frame = createFrame({
      type: "paragraph",
      lines: [[{ text: "CUSTOM_READY" }]],
    });
    assertEquals((await driver.render(frame)).width, 80);
    if (scenario === "cleanup") {
      write("CUSTOM_CLEANUP");
      await driver.nextEvent().catch(() => null);
      const error = await assertRejects(() => driver.close(), AggregateError);
      assertEquals(error.errors[0].description.code, "shutdown");
      const poisoned = await assertRejects(
        () => adapter.open(options),
        NativeError,
      );
      assertEquals(poisoned.description.code, "terminalPoisoned");
    } else {
      assertEquals(await driver.nextEvent(), {
        type: "resize",
        width: 90,
        height: 30,
      });
      assertEquals((await driver.render(frame)).width, 90);
      write("CUSTOM_RESIZED");
      if (extended) {
        const fixture = JSON.parse(
          await Deno.readTextFile(
            new URL("../fixtures/extended-input.json", import.meta.url),
          ),
        );
        for (const expected of fixture.events) {
          assertEquals(await driver.nextEvent(), expected);
        }
      } else {
        assertEquals(await driver.nextEvent(), {
          type: "key",
          key: { type: "character", value: "q" },
          kind: "press",
          modifiers: [],
        });
      }
      const waiting = driver.nextEvent();
      await driver.close();
      assertEquals(await waiting, null);
      await driver.close();
      await (await adapter.open({ input: marker, output: marker })).close();
    }
  }
} finally {
  libc.symbols.close(marker);
  libc.close();
}
