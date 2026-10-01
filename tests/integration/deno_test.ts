import { assertEquals, assertRejects } from "@std/assert";
import { DenoAdapter, NativeError } from "../../packages/deno/mod.ts";
const root = new URL("../../native/target/debug/", import.meta.url);
Deno.test("missing library and invalid options reject", async () => {
  await assertRejects(() =>
    new DenoAdapter(new URL("missing.so", root)).open({})
  );
  await assertRejects(() =>
    new DenoAdapter("unused").open({ alternateScreen: 1 as unknown as boolean })
  );
});
for (
  const [name, code] of [["abi", "unsupportedAbi"], [
    "protocol",
    "unsupportedProtocol",
  ]]
) {
  Deno.test(`${name} version mismatch rejects before terminal creation`, async () => {
    const error = await assertRejects(
      () => new DenoAdapter(new URL(`${name}.so`, root)).open({}),
      NativeError,
    );
    assertEquals(error.description.code, code);
  });
}
Deno.test("real library rejects redirected terminal", async () => {
  const filename = Deno.build.os === "darwin"
    ? "libratatui_js_ffi.dylib"
    : Deno.build.os === "windows"
    ? "ratatui_js_ffi.dll"
    : "libratatui_js_ffi.so";
  const error = await assertRejects(
    () => new DenoAdapter(new URL(filename, root)).open({}),
    NativeError,
  );
  assertEquals(error.description.code, "notTerminal");
});
