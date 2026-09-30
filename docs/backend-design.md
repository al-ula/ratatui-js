# Native terminal backend design

**The Rust Crossterm session milestone is implemented. C ABI and Deno bindings
remain unimplemented.** This document records the backend design. Linux PTY
integration is verified; macOS and Windows are not. ABI signatures and Deno
bindings remain separate milestones described in [native ABI](native-abi.md).

## Goals and scope

- Render the existing protocol on a real terminal using Crossterm.
- Read normalized keyboard and resize events without blocking rendering.
- Make terminal ownership, partial initialization, and shutdown explicit.
- Keep the existing headless renderer and fixtures independent of terminal I/O.
- Support Linux, macOS, and Windows as a design goal; claim support only after
  each platform passes lifecycle and integration tests.

No runtime backend selection, JS callbacks from Rust, application loop, mouse,
paste, focus events, or custom terminal parser is included in this milestone.

## Backend choice and crate boundaries

Use `CrosstermBackend` for the first real-terminal implementation. Ratatui
recommends Crossterm for general-purpose applications. Termion, Termwiz, and
Termina do not currently provide a project-specific reason to add more backends.

```text
packages/core              Application state and scheduling (future runner)
packages/deno              Native loading, FFI calls, buffer ownership (future)
native/ffi                 C ABI, handles, panic containment (future)
native/crossterm           Terminal ownership, input, synchronization (existing)
native/core                Validation and Renderer<B: Backend> (existing)
```

The `native/crossterm` crate depends on `native/core` and enables Ratatui's
`crossterm` feature. Core continues to disable default features. Prefer
Ratatui's Crossterm re-export so terminal operations and input use the same
version as the backend. Check the dependency graph for incompatible duplicates
before implementation.

The crate name identifies the backend; a session is the lifecycle object it
owns. Do not add a generic session crate or backend-selection layer until
another backend creates a concrete need for one.

The session owns `Renderer<CrosstermBackend<Stdout>>`. Any core lifecycle
accessors needed for cursor restoration or invalidation should be small,
explicit methods, not a second rendering interface or unrestricted shared
terminal access.

## Rust session surface

The intended operations are synchronous Rust operations; adapters decide how to
execute blocking calls without blocking their runtime:

| Operation             | Behavior                                                         |
| --------------------- | ---------------------------------------------------------------- |
| `open(options)`       | Initialize and return an owned session, or roll back             |
| `render_json(bytes)`  | Validate and draw; return the existing `RenderResult`            |
| `poll_event(timeout)` | Return `Event`, `Timeout`, or `Closed`, or an error              |
| `close()`             | Stop input, finish admitted rendering, restore terminal settings |

Only `alternateScreen` is configurable initially, defaulting to true, matching
`TerminalOptions`. Queue capacity and reader polling interval are internal
constants, not public API. Capabilities advertise keyboard and resize support,
not guaranteed support for every modifier or key-event kind.

Session errors are separate from core frame errors. Required categories include
terminal busy, invalid options, session closed, concurrent event wait, terminal
I/O, input-reader failure, and shutdown failure. Preserve core error codes and
paths when forwarding frame errors. Define the cross-language error schema
before ABI implementation rather than forcing lifecycle failures into
`invalidFrame`.

## Terminal ownership and opening

The library permits one real-terminal session at a time. A native ownership
guard is acquired before changing terminal settings. Headless tests do not
acquire it. This guard covers sessions in one loaded library instance; it cannot
coordinate unrelated terminal libraries or independently loaded copies. The host
must not simultaneously read terminal input or alter terminal modes.

Initially require terminal-connected stdin and stdout. Redirected streams are
rejected before changing modes; custom file descriptors and `/dev/tty` output
selection are deferred. Reject pre-existing Crossterm raw mode rather than
claiming ownership of another caller's mode.

Opening proceeds in this order:

1. Validate options and terminal availability; claim ownership.
2. Enable raw mode.
3. Enter alternate screen if requested, hide the cursor, reset
   attributes/colors, and clear the display. Ratatui initializes blank diff
   buffers but does not clear existing physical screen contents; opening
   establishes that baseline.
4. Create the renderer and its initial terminal buffers.
5. Start the sole input-reader thread.
6. Publish the open session to the caller.

Use explicit Crossterm operations instead of `ratatui::init()` or `try_init()`.
The shared library must not install its own process-wide panic hook or
interrupt/termination handler. Crossterm provides its terminal and resize
infrastructure, including platform-specific signal handling.

An initialization guard records attempted mode changes. A failed write can have
partial effects, so rollback must not rely only on operations reporting success.
On failure, stop any started reader, attempt every applicable restoration step,
and return both the original failure and rollback failures. If restoration
fails, retain a poisoned ownership slot and reject new sessions until process
restart; do not silently treat the terminal as safely reusable.

## State and synchronization

```text
Creating -> Open -> Closing -> Closed
                                 |
                          destroyed by caller
```

Closed means operations have stopped, not that restoration necessarily
succeeded. The final close outcome is stored and returned on subsequent closes.
Destruction belongs to the ABI handle layer; it is not another callable session
operation.

Use three focused synchronization domains:

- A renderer mutex serializes drawing and terminal restoration.
- A state/event mutex holds lifecycle state, event queue, input failure, and the
  outstanding event-wait flag; condition variables wake consumers/producers.
- A close mutex elects one shutdown caller; other callers await its outcome.

Never acquire the renderer mutex or join the reader while holding the
state/event mutex. Rendering may briefly inspect state while holding the
renderer mutex. Event waits release the state/event mutex through
condition-variable waits and never acquire the renderer mutex. No locks are held
across calls into JS.

Rendering acquires the renderer mutex, then checks state to admit the operation.
An operation admitted before closing may finish. A render still waiting for the
mutex when state becomes Closing is rejected, even if its call began earlier.
All frame validation still finishes before drawing. Completion means backend
output completed, not that work was queued.

A backend draw failure may leave output and diff buffers inconsistent. Mark the
session rendering-failed and reject subsequent renders until close; do not retry
against potentially stale buffers. Invalid frames do not fault the session.

## Input reader and event polling

Use one dedicated reader calling Crossterm `poll()` and `read()` on the same
thread. Do not mix this with `EventStream` or other host input readers. The
reader does not update application state or trigger renders.

Proposed starting constants are a 50 ms input poll interval and a FIFO queue of
256 normalized events. These bound idle cancellation checks and native queue
memory; they are not performance guarantees. Queue fullness applies
interruptible backpressure rather than silently dropping keys. Close wakes a
producer blocked on a full queue. Closing therefore does not depend on JS
consuming more events.

Normalize only events expressible by `TerminalEvent`:

- Preserve supported character/function/named keys, kinds, and modifiers.
- Preserve resize dimensions; actual drawing dimensions come from
  `RenderResult`.
- Ignore unsupported event variants and unmappable key codes rather than
  inventing protocol values. Test the mapping explicitly.
- Do not enable mouse, paste, focus, or enhanced keyboard modes initially.
  Press/repeat/release availability depends on the terminal and platform;
  ordinary keyboard support does not promise release events.

Event polling claims the single-wait flag under the state/event mutex and
releases it on every exit, including failures. A second outstanding poll returns
an error. A zero timeout is nonblocking. Finite waits use a monotonic deadline
and recheck conditions after every wake, including spurious wakes.

Check closing/closed before returning queued events: close discards queued input
and wakes the current wait with `Closed`. Otherwise, a sticky reader failure
takes precedence over queued input and returns an error; the application must
close. No input failure is disguised as a timeout or normal closure. Rendering
remains available after an input failure until the host closes the session.

## Shutdown and cleanup

The elected close caller:

1. Changes Open to Closing, cancels the reader, clears input, and wakes all
   event and queue-space waits under the state/event mutex.
2. Releases that mutex and joins the input reader. The reader exits after its
   current bounded poll or an interrupted queue wait.
3. Acquires the renderer mutex, allowing any admitted render to finish, and
   drops renderer resources before explicit restoration. Ratatui's terminal
   destructor can write to the backend; these writes must precede our final
   cleanup/flush.
4. Attempts cursor/style restoration, alternate-screen exit if applicable,
   output flush, and raw-mode restoration, even if earlier steps fail.
5. Records the complete close outcome, transitions to Closed, and releases
   ownership only if restoration succeeded.

The poll interval bounds reader cancellation checks, not total close duration:
OS output or an admitted render can still block. Never free resources merely
because a shutdown deadline expired.

`Drop` performs best-effort cleanup as a fallback; explicit close is required to
observe errors. Do not promise restoration after SIGKILL or abort. FFI panic
containment must fault affected operations and preserve cleanup access, not
resume normal rendering by blindly clearing poisoned mutexes.

With alternate screen disabled, shutdown does not erase or reconstruct the
host's previous screen contents. Opening clears the primary display and
rendering overwrites it; this is a fullscreen viewport, not an inline UI.
Document this distinction from terminal-mode cleanup.

## FFI and adapter integration

The proposed ABI delegates terminal work to this session instead of maintaining
another lifecycle. Event, timeout, closed, and error outcomes remain distinct.
Closing never destroys the handle. The caller must finish all outstanding calls
before destruction; output buffers must be freed before library unload.

The Deno adapter serializes render submissions, rejects a second `nextEvent()`,
and uses nonblocking FFI bindings for potentially blocking operations. It maps
native timeouts to another wait and closed to `null`, and copies/frees native
buffers in `finally`. Close must not wait behind the outstanding input call in a
single JS operation queue: it invokes native close to wake that call first, then
awaits outstanding operations before destroying the handle or unloading.

Interrupt handling stays at the adapter/application boundary. Raw-mode Ctrl-C is
normally a key event; OS interrupts are a separate cleanup path. Neither should
cause the native library to terminate the host process.

## Verification and implementation gates

1. **Session design review:** agree on terminal ownership, initialization
   rollback, close semantics, queue backpressure, and initial platform scope.
2. **Session implementation:** unit tests for key conversion, timeout behavior,
   duplicate waits, full-queue cancellation, render admission, concurrent
   closes, reader failure, and every partial-initialization/cleanup failure. Use
   focused test seams for terminal operations, not a replacement layout
   emulator.
3. **Real-terminal milestone:** a Rust example renders a frame, responds to
   resize, and closes on `q`. PTY tests verify screen output, input, raw-mode
   restoration, cursor/alternate-screen cleanup, and failure paths. Windows
   needs equivalent console/ConPTY coverage before support is claimed.
4. **ABI milestone:** finalize error/status and buffer layouts; exercise calls
   from C, including rendering while polling, close while polling, and buffer
   release.
5. **Deno milestone:** test nonblocking waits, concurrent shutdown, interrupt
   cleanup, load/version failures, and safe unload through the real shared
   library.

Existing core golden fixtures must continue passing throughout. No application
runner or additional backend is required to complete the session milestone.

## References

- [Ratatui backend options](https://ratatui.rs/concepts/backends/)
- [Ratatui backend comparison](https://ratatui.rs/concepts/backends/comparison/)
- [Ratatui 0.30.2 initialization behavior](https://docs.rs/ratatui/0.30.2/ratatui/fn.try_init.html)
