# Native ABI v1

Implemented by `native/ffi`, wrapping `native/crossterm`. The public header is
[`ratatui_js.h`](../native/ffi/include/ratatui_js.h). ABI and JSON protocol
versions are independent; both are currently 1. Query `rt_abi_version` and
`rt_protocol_version` before creation. `rt_create` validates both versions and
flags before terminal initialization.

## Surface

The FFI crate builds a shared library and expose:

| Operation        | Purpose                                                       |
| ---------------- | ------------------------------------------------------------- |
| `rt_abi_version` | Return the C ABI version without opening a terminal           |
| `rt_create`      | Negotiate protocol/configuration and return an opaque session |
| `rt_render`      | Borrow frame bytes and return an owned render-result buffer   |
| `rt_poll_event`  | Wait with a timeout and return an owned event buffer          |
| `rt_close`       | Stop input, wake waiters, and restore terminal settings       |
| `rt_destroy`     | Release a closed session after outstanding calls finish       |
| `rt_bytes_free`  | Release an output/error buffer allocated by the library       |

Use C-compatible fixed-width status values and pointer/`size_t` buffer pairs.
Rust types and borrowed frames must never appear in the public ABI.

Polling must distinguish an event, timeout, closed session, and error. Output
buffers and error buffers are separate. Errors include a structured code/message
and an optional field path; no global or thread-local last-error mechanism.

## Ownership

- Input bytes remain owned by the caller and are borrowed only during a call.
- Any queued work must copy its input before the call returns.
- Output/error bytes are owned by native code until released with
  `rt_bytes_free`.
- Adapters copy or decode output before freeing it and always free in `finally`.
- Never free native memory with a JS/runtime allocator or free the same buffer
  twice. Adapters cannot mutate or forge buffer metadata.
- Session handles are opaque and invalid after destruction.
- A library cannot be unloaded while calls, handles, or output buffers remain.

## Session states

```text
Creating → Open → Closing → Closed → Destroyed
```

Creation failures restore any terminal settings already changed. Initially one
real-terminal owner is allowed per process; headless renderer tests are
separate.

Closed sessions reject rendering and return closure from event polling. Close is
idempotent and wakes event waits. Destruction is not idempotent: a destroyed
handle must never be used again.

Render calls serialize in the session; close waits for admitted rendering and
rejects new rendering. Event waiting must not retain the rendering lock.
Platform wrappers prevent overlapping destruction or unload and reject a second
outstanding event wait.

## Errors and cleanup

- Guard ABI entry points against unwindable Rust panics. Abort-level failures
  cannot be caught by this mechanism.
- Return error details through explicitly owned buffers.
- Report restoration failures, even if other shutdown steps succeeded. An
  idempotent close does not imply terminal restoration succeeded.
- JS uses explicit cleanup rather than relying on finalizers.
- Interrupt handling will be integrated at the adapter level and tested with a
  terminal/PTY. Abrupt termination such as SIGKILL cannot guarantee restoration.

## Signatures and statuses

The header is authoritative. Functions return a `uint32_t`: 0 success/event, 1
timeout, 2 closed, 3 error. `RtBytes` is `{uint8_t *data; size_t len;}`; all
output pointers are required and must be aligned and disjoint. Output storage
must contain no unreleased buffers. A null error output returns status 3 without
running the operation. Null arguments are rejected; arbitrary invalid addresses
cannot be safely validated by a C library. Creation flags currently accept only
bit 0, alternate screen. Timeout is an unsigned 32-bit millisecond count. Zero
checks immediately.

Destroy consumes the handle even when cleanup reports failure. Call close and
wait for all operations before destroy; destruction must never overlap any other
operation. Free exact native buffer pairs once, after readers finish. Empty
null/zero pairs may be freed. No C struct contains Rust-owned types. The Rust
build uses unwind panics; abort-level failures cannot be contained.

Structured lifecycle errors follow
[the protocol schema](protocol.md#lifecycle-errors), including nested
initialization causes and all restoration failures.
