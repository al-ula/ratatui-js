# Roadmap

The next goal is a Deno application that renders through the native library,
handles keyboard and resize events, and restores the terminal on exit.
Milestones are ordered by their dependencies; unchecked items are planned work.

## Implemented foundation

- [x] Runtime-independent frame types, validation, and UTF-8 encoding in
      `packages/protocol`.
- [x] Adapter contracts, validated frame creation, and a testing fake driver in
      `packages/core`.
- [x] Generic Ratatui rendering and headless cell-buffer tests in `native/core`.
- [x] Terminal ownership, keyboard/resize input, serialized rendering, and
      explicit shutdown in `native/crossterm`.
- [x] A Rust terminal example and Linux PTY lifecycle coverage.

JavaScript can open a real terminal through the C ABI and Deno adapter. The
application runner and UI builders are implemented. Linux integration is
verified through Rust, C, and Deno; macOS and Windows remain unverified.

## Implementation order

| Milestone                                   | Depends on                           | Completion outcome                                                |
| ------------------------------------------- | ------------------------------------ | ----------------------------------------------------------------- |
| Shared lifecycle error schema               | Existing protocol and session errors | Rust and TypeScript agree on structured failures                  |
| Native C ABI                                | Shared error schema                  | C callers can safely own, render, poll, and close a session       |
| Deno adapter                                | Native C ABI                         | JavaScript can render and consume events through the real library |
| Application runner and UI builders          | Deno adapter                         | An application can update state, redraw, and clean up reliably    |
| Platform verification and release packaging | ABI and adapter integration          | Published artifacts work on each explicitly supported target      |

## Shared lifecycle error schema

- [x] Define stable error codes and payloads for terminal ownership, unavailable
      terminals, closed sessions, duplicate event waits, input failures,
      rendering failures, panics, initialization rollback, and shutdown
      failures.
- [x] Preserve existing frame error codes and field paths when forwarding
      errors.
- [x] Represent the original initialization failure and all cleanup failures
      without reducing them to a generic message.
- [x] Define matching Rust serialization and TypeScript types, with shared
      fixtures covering each category.

Complete when serialization tests demonstrate agreement across both languages
and the schema is documented alongside the [protocol](protocol.md) and
[native ABI](native-abi.md).

## Native C ABI

- [x] Add `native/ffi` as a shared-library crate wrapping the existing Crossterm
      session.
- [x] Finalize fixed-width statuses, opaque handles, pointer/length buffers, and
      a C header. Keep ABI versioning independent of protocol versioning.
- [x] Implement `rt_abi_version`, `rt_create`, `rt_render`, `rt_poll_event`,
      `rt_close`, `rt_destroy`, and `rt_bytes_free`.
- [x] Validate boundary inputs and configuration before changing terminal modes.
- [x] Keep event, timeout, closed, and error results distinct; return explicitly
      owned output and error buffers.
- [x] Contain unwindable panics at ABI entry points and retain access to cleanup
      after a failed operation.
- [x] Specify when handles and buffers may be released, including outstanding
      calls and concurrent render, poll, and close operations.
- [x] Add C integration tests for successful calls, rejected inputs, version
      negotiation, buffer release, rendering while polling, and closing while
      polling.

Complete when a C program exercises the public header and shared library through
a real terminal lifecycle, and failure tests verify error reporting and cleanup.
Existing renderer and session tests must continue passing.

## Deno adapter

- [x] Add `packages/deno` implementing `PlatformAdapter` and `TerminalDriver`.
- [x] Load the correct native artifact and reject unsupported ABI/protocol
      versions before opening a session.
- [x] Use nonblocking FFI bindings for potentially blocking operations,
      serialize renders, and reject a second outstanding event wait.
- [x] Decode and validate native results, copy owned bytes, and free
      output/error buffers in `finally`.
- [x] Retry native event timeouts and map native closure to `null`.
- [x] Make close idempotent: wake polling first, await outstanding calls,
      destroy the handle, and unload only after all buffers are released.
- [x] Integrate interrupt cleanup at the adapter/application boundary and report
      terminal restoration failures.
- [x] Add an example and integration tests using the actual shared library for
      rendering, input, resize, concurrent shutdown, load/version failures, and
      interrupt cleanup.

Complete when a Deno program renders a frame, receives keyboard and resize
events, and exits with terminal modes restored. Tests must show that an event
wait does not block JavaScript or prevent rendering and close.

## Application runner and UI builders

- [x] Define a runtime-independent runner API for model updates, event handling,
      frame construction, and explicit exit.
- [x] Implement initial drawing, serialized model updates, resize invalidation,
      redraw coalescing, and cleanup in `finally`.
- [x] Make returned list state available to the application while keeping model
      ownership in JavaScript.
- [x] Add focused builders for the existing row, column, block, paragraph, and
      list protocol nodes.
- [x] Use the fake driver to test update ordering, redraw scheduling, closure,
      and failures in application callbacks or driver operations.
- [x] Add a Deno example covering state updates, list selection, resize, and
      exit through the runner and real adapter.

Complete when the example handles its full application lifecycle through the
public API and tests verify cleanup on both normal exit and failure.

## Platform verification and release packaging

This work can proceed after ABI and adapter integration. Each platform needs its
own verification before it is advertised as supported.

- [ ] Extend Linux PTY coverage through the C ABI and Deno adapter.
- [ ] Verify terminal lifecycle and adapter behavior on macOS with PTY coverage.
- [ ] Add equivalent Windows console/ConPTY coverage, including input, resize,
      concurrent shutdown, and mode restoration.
- [ ] Add CI for TypeScript checks/tests/lint/formatting, Rust tests/Clippy/
      formatting, and supported-platform integration tests.
- [ ] Define the supported target and toolchain matrix and build native
      artifacts for each release target.
- [ ] Package native artifacts with version metadata, checksums, and both
      license texts; verify loading from a clean Deno environment.
- [ ] Prepare package exports, installation instructions, runnable examples, and
      release checks before publishing the JSR packages and native artifacts.

Complete when every advertised target passes the integration suite and a clean
installation can run the documented example using the packaged native library.

## Later scope

Bun and Node adapters follow the Deno adapter. Additional widgets, mouse, paste,
focus events, enhanced keyboard modes, custom terminal descriptors, and
alternate native backends should be separate proposals driven by concrete
application needs.

See [architecture](architecture.md), [backend design](backend-design.md), and
[native ABI](native-abi.md) for the ownership and lifecycle contracts behind
these milestones.
