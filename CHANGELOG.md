# Changelog

Changes across all `@ratatui-js` packages are tracked here. Add changes under
`Unreleased`, then move them into a version section before tagging a release.

## [Unreleased]

### Fixed

- Enforce LF line endings so formatting checks agree across platforms.
- Stop Unix PTY test harnesses at EOF after child exit, avoiding false timeouts
  on macOS while retaining output and terminal restoration checks.
- Check restored Unix terminal modes before the session leader exits and macOS
  revokes the terminal.
- Align fixture DLL export declarations with definitions for MSVC compilation.

## [0.1.0-beta.1]

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

- Linux x86_64 GNU is verified locally. macOS aarch64 and Windows x86_64 MSVC
  verification is configured in CI and awaits review.
