"""Verify checksums, licenses, and native loading from a clean extracted bundle."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import zipfile

root = Path(__file__).resolve().parents[2]
bundles = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "dist"
archives = list(bundles.glob("*.zip"))
assert len(archives) == 1, "Expect exactly one freshly built host release archive"
with tempfile.TemporaryDirectory(prefix="ratatui-js-install-") as temporary:
    install = Path(temporary)
    with zipfile.ZipFile(archives[0]) as archive:
        archive.extractall(install)
    bundle = next(path for path in install.iterdir() if path.is_dir())
    for line in (bundle / "SHA256SUMS").read_text().splitlines():
        digest, filename = line.split("  ", 1)
        assert hashlib.sha256((bundle / filename).read_bytes()).hexdigest() == digest, filename
    for package in ["protocol", "core", "native", "deno"]:
        for license in ["LICENSE-MIT", "LICENSE-APACHE"]:
            assert (bundle / "packages" / package / license).read_bytes() == (root / license).read_bytes()
    # No existing Deno cache or explicit library path. Local workspace exports
    # and the adapter's target resolver must be sufficient to run this program.
    smoke = bundle / "smoke.ts"
    smoke.write_text('''
import { createFrame } from "@ratatui-js/core";
import { DenoAdapter, onInterrupt } from "@ratatui-js/deno";
const scenario = Deno.args[0];
const driver = await new DenoAdapter().open({alternateScreen: scenario !== "no-alternate"});
const failures: unknown[] = [];
const detach = onInterrupt(driver, error => failures.push(error));
const frame = createFrame({type: "paragraph", lines: [[{text: "PTY frame"}]]});
try {
  const result = await driver.render(frame);
  if (result.width !== 80 || result.height !== 24) throw new Error("Wrong dimensions");
  const waiting = driver.nextEvent();
  await new Promise(resolve => setTimeout(resolve, 150));
  await driver.render(frame);
  if (scenario === "close-wait") {
    await driver.close();
    if (await waiting !== null) throw new Error("Poll was not closed");
  } else {
    console.log("PTY_READY\\r");
    let event = await waiting;
    while (event) {
      if (event.type === "resize") {
        const result = await driver.render(frame);
        if (result.width !== 90 || result.height !== 30) throw new Error("Resize failed");
        console.log("PTY_RESIZED\\r");
      } else if (event.key.type === "character" && event.key.value === "q") break;
      event = await driver.nextEvent();
    }
  }
} finally {
  try { await driver.close(); } finally { detach(); }
}
if (failures.length) throw new AggregateError(failures, "Interrupt cleanup failed");
''')
    environment = dict(os.environ)
    environment["DENO_DIR"] = str(install / "clean-deno-cache")
    harness = root / "tests/integration" / ("windows_console.py" if sys.platform == "win32" else "terminal_pty.py")
    if sys.platform == "win32":
        # The bundle is self-contained, but mode checks live only in the test fixture.
        shutil.copy2(root / "tests/integration/console_modes.ts", bundle / "console_modes.ts")
        smoke.write_text('import { consoleModes } from "./console_modes.ts";\nconst modes = consoleModes();\n' + smoke.read_text() + '\nmodes.verify();\n')
    subprocess.run([sys.executable, str(harness), shutil.which("deno"), "run", "--allow-ffi", "--allow-write", str(smoke)],
                   cwd=bundle, env=environment, check=True)
print("Packaged installation: passed")
