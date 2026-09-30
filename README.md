# ratatui-js

Ratatui rendering for JavaScript applications, designed in three layers:

```text
Pure TypeScript API → platform FFI adapter → native Rust renderer
```

JSR scope: `@ratatui-js`. Deno will be the first adapter; the protocol and core
library do not depend on a JavaScript runtime.

## Current status

The foundation and first native terminal backend are implemented:

- `native/core`: validated JSON frames rendered through Ratatui, with headless
  `TestBackend` tests.
- `native/crossterm`: real-terminal sessions, keyboard/resize input, serialized
  rendering, and explicit shutdown with terminal restoration.
- `packages/protocol`: frame types, validation, and UTF-8 encoding.
- `packages/core`: adapter contracts and validated frame creation.
- Shared fixtures checking layout, borders, Unicode, wrapping, lists, and
  errors.

**There is no C ABI implementation or Deno FFI adapter yet.** These packages are
not published. The application runner and UI builders will follow the adapter.
Real-terminal integration has been verified on Linux with PTYs; macOS and
Windows have not been verified.

## Development

Verified locally with Rust 1.98.0 and Deno 2.9.6. Earlier toolchain versions
have not been tested. Dependencies are managed through package-manager CLIs and
lockfiles.

```sh
deno task check
deno task test
deno task lint
deno task fmt

cargo test --manifest-path native/Cargo.toml --locked
cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings
cargo fmt --manifest-path native/Cargo.toml --all --check
```

The TypeScript tests need only read permission for `tests/fixtures`; no terminal
access or native library is required.

## Native terminal example

Run in a terminal; press `q` to quit. Resizing redraws the frame.

```sh
cargo run --manifest-path native/Cargo.toml -p ratatui-js-crossterm --example hello --locked
```

Add `-- --no-alternate-screen` to clear and draw on the primary screen. Both
stdin and stdout must be terminals; the host must not concurrently read input or
change modes. See [Crossterm session usage](native/crossterm/README.md) for
lifecycle details.

Opt-in Linux PTY tests require Python 3 and test input, resize, concurrent
close, Drop cleanup, mode restoration, and rejection of already-owned raw mode:

```sh
cargo test --manifest-path native/Cargo.toml -p ratatui-js-crossterm --test terminal_pty --locked real_terminal_lifecycle -- --ignored --nocapture
```

## Frame construction

Inside this workspace:

```ts
import { createFrame } from "@ratatui-js/core";
import { encodeFrame } from "@ratatui-js/protocol";

const frame = createFrame({
  type: "paragraph",
  lines: [[{ text: "Hello from TypeScript", style: { bold: true } }]],
});

const bytes = encodeFrame(frame);
```

An eventual adapter submits those bytes to the native renderer in one call.

See [architecture](docs/architecture.md), [protocol](docs/protocol.md),
[backend design](docs/backend-design.md), and the
[proposed native ABI](docs/native-abi.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option. Release packaging must include both license texts with published
packages and native artifacts.
