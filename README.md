# ratatui-js

Ratatui rendering for JavaScript applications, designed in three layers:

```text
Pure TypeScript API → platform FFI adapter → native Rust renderer
```

JSR scope: `@ratatui-js`. Deno will be the first adapter; the protocol and core
library do not depend on a JavaScript runtime.

## Current status

The terminal stack is implemented, including a runtime-independent application
runner and typed UI builders:

- `packages/protocol`: frame validation, encoding, results, and shared errors.
- `packages/core`: driver contracts, application lifecycle, and UI builders.
- `packages/deno`: nonblocking native FFI, owned-buffer cleanup, and interrupts.
- `packages/native`: checksummed bundled native assets for JSR.
- `native/core`: validated Ratatui rendering and headless tests.
- `native/crossterm`: terminal ownership, input, and explicit restoration.
- `native/ffi`: versioned C ABI with a public header and integration tests.

Linux x86_64 GNU has verified Rust, C, Deno, and application PTY coverage. macOS
and Windows coverage is configured in CI and remains unverified. Packages are
not yet published. See the [roadmap](docs/roadmap.md) and
[release matrix](docs/releases.md).

## Deno application

Build and run from the repository root in a terminal:

```sh
cargo build --manifest-path native/Cargo.toml -p ratatui-js-ffi --locked
deno run --allow-ffi examples/application.ts native/target/debug/libratatui_js_ffi.so
```

Use ↑/↓ to select a list item and `q` to quit. Resizing redraws. Unix interrupts
restore terminal settings; the application awaits explicit cleanup. Deno needs
unrestricted `--allow-ffi` for native pointer inspection.

Build an installable bundle after integration checks with
`python3 scripts/package-native.py --verified`. Inside the extracted archive,
run `deno run --allow-ffi --allow-write examples/application.ts` without a
library path. See [installation and release checks](docs/releases.md). JSR
publishing runs only for version tags; ordinary pushes and pull requests only
run checks.

See the [changelog](CHANGELOG.md) for changes across all packages. Each GitHub
Release includes the corresponding version's changelog notes.

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

The TypeScript unit tests need only read permission for `tests/fixtures`. Native
integration tests also require a C compiler, Python 3, FFI permission, and real
PTY/console access: `python3 tests/integration/run.py`.

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

The Deno adapter submits those bytes to the native renderer in one call.

See [architecture](docs/architecture.md), [protocol](docs/protocol.md),
[roadmap](docs/roadmap.md), [backend design](docs/backend-design.md), and the
[native ABI](docs/native-abi.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option. Release packaging must include both license texts with published
packages and native artifacts.
