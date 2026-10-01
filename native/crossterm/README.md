# ratatui-js-crossterm

Native Crossterm sessions for the existing ratatui-js JSON rendering protocol.
This is a Rust library, not the future C ABI or Deno adapter. Linux
extended-input behavior is verified with PTYs; extended input on macOS and
Windows has not yet been verified.

## API

- `Session::open(SessionOptions)`: require terminal stdin/stdout, acquire
  exclusive ownership, enable raw mode, optionally enter alternate screen, and
  start input.
- `render_json(&[u8])`: synchronous validated drawing, returning `RenderResult`.
- `poll_event(Duration)`: one outstanding wait; returns `Event`, `Timeout`, or
  `Closed`. Zero duration is nonblocking. Errors are not normal closure.
- `close()`: wake event waits, stop input, finish admitted rendering, and
  restore modes. Repeated/concurrent closes return the same outcome, including
  failures.

`SessionOptions::default()` enables alternate screen. Sharing `Arc<Session>`
allows independent rendering, polling, and closing. Render calls serialize; an
input wait never holds the renderer lock. Calls waiting to render when close
begins are rejected. Output I/O failure or render panic disables future
rendering until close; invalid frames do not.

The bounded input queue preserves FIFO events with interruptible backpressure.
Key, resize, mouse, paste, and focus events preserve their native meaning;
unknown modifier/state bits cause the entire event to be ignored.
`SessionOptions` offers opt-in `mouse_capture`, `bracketed_paste`,
`focus_reporting`, and `enhanced_keyboard` booleans, all false by default.
Enhanced keyboard mode probes support before starting input; unsupported
terminals reject opening, and query errors roll back raw mode. The keyboard flag
stack is pushed and popped; other requested input modes are disabled on shutdown
or initialization rollback. Ordinary keyboard support does not guarantee
repeat/release events. Crossterm does not support enhanced keyboard negotiation
on Windows. See the [input contract](../../docs/protocol.md#input-contract) for
event shapes and platform behavior.

## Ownership and cleanup

Only one session per loaded library instance may own the terminal. The host must
not concurrently read terminal input or alter its modes; independently loaded
libraries cannot be coordinated by this ownership guard. Existing Crossterm raw
mode is rejected rather than borrowed. Mouse, paste, and focus modes must
initially be disabled by the host; there is no portable prior-state query.
Pre-existing enhanced keyboard flags are preserved with a push/pop pair.

Explicitly close on both success and failure paths to observe cleanup errors.
`Drop` is a best-effort fallback. No global panic hook, interrupt handler, or
host termination policy is installed by this crate; Crossterm supplies its own
terminal and resize infrastructure. Raw-mode Ctrl-C is normally input, not an
automatic shutdown command. Abrupt termination cannot guarantee restoration.

All applicable cleanup steps are attempted. Restoration failures poison terminal
ownership: new sessions are rejected until process restart. Failed
initialization reports the original error plus any rollback failures. Disabling
alternate screen clears and overwrites the primary display on opening; closing
does not restore earlier screen contents. The initial viewport is fullscreen,
not inline.

## Example and tests

From the workspace root:

```sh
cargo run --manifest-path native/Cargo.toml -p ratatui-js-crossterm --example hello --locked
cargo test --manifest-path native/Cargo.toml --locked
```

The example redraws on resize and exits on unmodified `q`. Unit tests use
focused terminal-operation and input seams; headless rendering still uses
Ratatui's real `TestBackend`. Opt-in Linux PTY tests require Python 3:

```sh
cargo test --manifest-path native/Cargo.toml -p ratatui-js-crossterm --test terminal_pty --locked real_terminal_lifecycle -- --ignored --nocapture
```

See [backend design](../../docs/backend-design.md) for the concurrency and
lifecycle contract.
