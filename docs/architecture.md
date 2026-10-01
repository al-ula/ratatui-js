# Architecture

## Ownership

| Layer            | Responsibilities                                                              |
| ---------------- | ----------------------------------------------------------------------------- |
| Pure TS core     | Application state, event dispatch, UI construction, render scheduling         |
| Platform adapter | Binary loading, FFI declarations, serialization, native memory, safe shutdown |
| Native engine    | Terminal lifecycle, input reading, Ratatui layout/widgets, buffer diffing     |

The application is host-driven and immediate-mode. JS updates its model and
submits a complete frame. The native library does not run application logic or
call JS components through FFI. Ratatui's borrowed `Frame` never leaves Rust.

## Package boundaries

- `@ratatui-js/protocol` contains runtime-independent wire types and validation.
- `@ratatui-js/core` exposes the platform contract, frame creation, application
  runner, and UI builders.
- `@ratatui-js/deno`, and later Bun and Node adapters, implement the same
  contract.
- `native/core` is generic over a Ratatui backend.
- `native/crossterm` owns the Crossterm terminal lifecycle and input; see
  [backend design](backend-design.md).
- `native/ffi` exposes the renderer and session through a C-compatible ABI.
- `@ratatui-js/native` bundles verified library bytes for JSR; the Deno adapter
  materializes a checksummed local copy and removes it after unloading.

Workspace package names resolve locally through Deno. No published package or
native binary is needed to develop the foundation.

The testing fake driver is deliberately not a layout/rendering emulator. Golden
cell-buffer tests use the real Rust renderer and Ratatui's `TestBackend`.

## State and scheduling

JS owns selection and requested scroll offsets. Rust returns list/table offsets
and selections, tab selections, and scrollbar positions after rendering, keyed
by unique widget IDs. Applications can use that result in the next frame; hidden
native application state must not compete with JS state.

Platform drivers must serialize renders, allow one outstanding input wait, and
wake that wait when closed. An event wait must not hold the render lock. Closing
must finish pending operations before freeing a handle or unloading a library.

The pure TS application runner handles initial drawing, serialized model
updates, resize invalidation, redraw coalescing, and cleanup in `finally`. Input
reader threads are infrastructure, not a second application loop.

## Implementation boundary

The protocol, driver contracts, frame creation, headless rendering, and native
Crossterm sessions exist today. Sessions implement terminal ownership, native
input, and shutdown synchronization. The C ABI, Deno FFI adapter, application
runner, and UI builders are implemented. Real-terminal integration is verified
on Linux x86_64 GNU, macOS aarch64, and Windows x86_64 MSVC.
