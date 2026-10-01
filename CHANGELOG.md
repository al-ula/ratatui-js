# Changelog

Changes across all `@ratatui-js` packages are tracked here. Add changes under
`Unreleased`, then move them into a version section before tagging a release.

## [Unreleased]

### Added

- Table, tabs, gauge, chart, and scrollbar nodes with typed builders, shared
  validation, native rendering, and headless fixtures.
- Explicit JavaScript-owned widget state and render-result updates for tables,
  tabs, and scrollbars.

- Mouse, paste, and focus input events; complete Crossterm key codes, modifiers,
  event kinds, and keypad/lock state in the shared protocol and validation.
- Opt-in mouse capture, bracketed paste, focus reporting, and enhanced keyboard
  negotiation in Rust, the C ABI, and Deno, with rollback and explicit cleanup.
- Shared serialization fixtures and native/adapter PTY tests for extended input,
  unsupported terminals, negotiation timeouts, and mode restoration.

## [0.1.0-beta.1]

Initial beta release of `ratatui-js`. All four `@ratatui-js` packages share this
version.

### Added

- Runtime-independent frame protocol, validation, encoding, and shared errors in
  `@ratatui-js/protocol`.
- Typed UI builders and an application runner with serialized updates, resize
  redraws, and explicit cleanup in `@ratatui-js/core`.
- Deno FFI adapter with owned-buffer cleanup and interrupt handling in
  `@ratatui-js/deno`.
- Checksummed bundled native libraries in `@ratatui-js/native`, with temporary
  extraction and cleanup for default loading.
- Rust rendering, terminal ownership and restoration, and a versioned C ABI.
- Tag-triggered JSR workspace publishing and native archives on GitHub Releases.

### Platform status

- Linux x86_64 GNU, macOS aarch64, and Windows x86_64 MSVC passed terminal
  lifecycle, adapter, application, packaging, and clean-install checks in
  [CI](https://github.com/al-ula/ratatui-js/actions/runs/36837492309).
- Linux binaries require glibc 2.39 or newer. Other targets are outside this
  beta release matrix.
