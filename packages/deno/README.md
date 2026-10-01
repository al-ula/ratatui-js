# Deno terminal adapter

Build the library with
`cargo build --manifest-path native/Cargo.toml -p
ratatui-js-ffi --locked`. Run
from the repository root in a terminal:

```sh
deno run --allow-ffi examples/terminal.ts native/target/debug/libratatui_js_ffi.so
```

On macOS use `libratatui_js_ffi.dylib`; on Windows use `ratatui_js_ffi.dll`.
These targets are not yet verified. Linux x86_64 has real PTY coverage.

`DenoAdapter` accepts a local path or URL. Without one it materializes the
target binary embedded in the `@ratatui-js/native` JSR wrapper into a temporary
directory. This requires `--allow-write` in addition to `--allow-ffi`. It
verifies the checksum before writing and removes the directory after destroying
the handle and unloading the library. Local source checkouts use an explicit
path because binary bytes are generated during release staging. Only 64-bit
x86_64 and aarch64 targets are accepted. ABI and protocol versions are checked
before session creation. Pointer inspection requires unrestricted `--allow-ffi`;
path-restricted FFI permission is rejected before terminal modes change. A
native library is trusted executable code.

One event wait is allowed. Native timeouts retry; closed sessions return `null`.
Rendering is serialized and snapshots the frame before queuing. Close wakes
polling before awaiting active operations, destroys the session, then unloads
the library. Close is idempotent, including its failure outcome. `NativeError`
exposes the original structured `description`. Restoration failures reject close
with an `AggregateError`.

Use `onInterrupt(driver, report)` to handle SIGINT/SIGTERM (SIGINT on Windows).
Retain and call the returned detach function in `finally`, after awaiting
`driver.close()`. Report interrupt cleanup failures; never rely on finalizers.
The host must not read terminal input or change terminal modes concurrently.
