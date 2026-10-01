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

Enable additional input modes when opening:

```ts
const driver = await new DenoAdapter(libraryPath).open({
  mouseCapture: true,
  bracketedPaste: true,
  focusReporting: true,
  enhancedKeyboard: true,
});
```

These options default to false. `enhancedKeyboard` requires a successful
terminal support probe and rejects unsupported terminals with
`NativeError.description.code === "unsupportedCapability"`; a probe timeout
returns `io`. Crossterm does not support enhanced-mode negotiation on Windows.
The other modes depend on terminal support and may emit no events. Capabilities
`mouse`, `paste`, `focus`, and `enhancedKeyboard` indicate enabled modes.
`nextEvent()` returns the complete key/resize/mouse/paste/focus union; check
`event.type` before accessing its fields. Paste text is preserved verbatim and
must be sanitized before putting it into a frame.

Requested modes are restored on close and initialization failure. Leave
mouse/paste/focus modes disabled before opening and give the session exclusive
ownership of terminal modes. Pre-existing enhanced keyboard flags are preserved
with a push/pop pair. See
[the input contract](../../docs/protocol.md#input-contract) for exact payloads
and limitations. Linux PTYs cover extended event delivery, mode cleanup,
unsupported support responses, and negotiation timeouts; these new cases have
not yet been verified on macOS or Windows.

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

On Unix, select terminal streams explicitly when stdin/stdout are redirected:

```ts
const driver = await new DenoAdapter(libraryPath).open({
  input: "tty",
  output: "tty",
});
```

`input` and `output` independently accept `"standard"` (the default), `"tty"`
(open `/dev/tty`), or a nonnegative Unix file descriptor. Both must be
terminals; input must be readable and output writable. Descriptors must be
blocking and remain valid until `open()` resolves. Native code duplicates them
with close-on-exec, so the caller retains ownership and can close its originals
after opening. Do not read the selected input or alter its modes/descriptor
flags concurrently. Duplicates share file status flags with the originals.

Raw mode and exact saved termios restoration use the selected input. Commands,
rendering, and dimensions use the selected output; keyboard support responses
must arrive on the selected input. Resize detection on custom streams polls the
selected output dimensions every 50 ms, coalescing intermediate sizes. Parsing
and keyboard negotiation use the session's sole input source. Close cancels the
reader before restoration; initialization failures also restore attempted modes.
Custom input sequences are limited to 1 MiB; oversized sequences fail input with
an `input` error. The adapter releases all duplicated handles when it destroys
the session during close. Restoration errors poison ownership, preventing later
sessions even for another terminal. One session is permitted per adapter
module/native library.

Windows supports standard console streams; explicit descriptors and `"tty"`
reject with `unsupportedCapability`. Unix custom streams are tested on Linux;
macOS uses the same Unix implementation but has not been verified locally. The
adapter requires a library exporting the additive ABI 1 `rt_create_with_streams`
entrypoint; rebuild older local libraries before use.
