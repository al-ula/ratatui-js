import { assertEquals, assertRejects } from "@std/assert";
import { materializeArtifact } from "../../packages/native/mod.ts";
const artifact = {
  filename: "libratatui_js_ffi.so",
  base64: "aGVsbG8=",
  sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
  abiVersion: 1,
  protocolVersion: 1,
};
Deno.test("bundled native bytes verify and owned files are removed idempotently", async () => {
  const native = await materializeArtifact(artifact);
  assertEquals(await Deno.readTextFile(native.path), "hello");
  await Promise.all([native.dispose(), native.dispose()]);
  await assertRejects(() => Deno.stat(native.path), Deno.errors.NotFound);
});
Deno.test("checksums and filenames reject before native materialization", async () => {
  await assertRejects(
    () => materializeArtifact({ ...artifact, sha256: "0".repeat(64) }),
    Error,
    "checksum mismatch",
  );
  await assertRejects(
    () => materializeArtifact({ ...artifact, filename: "../library.so" }),
    TypeError,
  );
});
