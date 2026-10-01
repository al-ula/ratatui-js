# TODO

## Extended input

- [x] Add mouse, paste, and focus events to the shared protocol, TypeScript
      validation, and native event normalization.
- [x] Add opt-in terminal modes for mouse capture, bracketed paste, and focus
      reporting, with initialization rollback and shutdown restoration.
- [x] Add enhanced keyboard-mode negotiation and represent supported key codes,
      modifiers, and event kinds without silently changing their meaning.
- [x] Test event serialization, unsupported terminal capabilities, and mode
      cleanup through native and adapter integration tests.

## Additional widgets

- [x] Add table, tabs, gauge, chart, and scrollbar nodes to the shared protocol,
      validation, TypeScript builders, and native renderer.
- [x] Define explicit JS-owned state and render-result updates for new stateful
      widgets.
- [x] Add headless rendering fixtures and validation tests for each widget,
      including empty content, clipping, and invalid state.

## Richer styling

- [ ] Add RGB and indexed colors alongside the existing named colors.
- [ ] Add explicit modifier removal so child styles can override inherited
      modifiers.
- [ ] Keep TypeScript and Rust style schemas and validation aligned, and
      document inheritance and override behavior.
- [ ] Test color boundaries, invalid style values, and rendered style overrides.

## Custom terminal streams

- [ ] Add explicit input/output stream selection, including custom file
      descriptors and Unix `/dev/tty` support.
- [ ] Define stream ownership, handle lifetime, terminal checks, and platform
      support across the native session, C ABI, and adapter options.
- [ ] Preserve single-reader ownership, cancellation, initialization rollback,
      and terminal restoration for custom streams.
- [ ] Add integration tests for redirected standard streams, custom terminal
      streams, invalid handles, and cleanup failures.
