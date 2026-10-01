# Native releases

## Verification matrix

| Target              | Rust   | Deno  | Terminal coverage                     | Status           |
| ------------------- | ------ | ----- | ------------------------------------- | ---------------- |
| Linux x86_64 GNU    | 1.98.0 | 2.9.6 | Rust/C/Deno/application PTYs          | Locally verified |
| macOS aarch64       | 1.98.0 | 2.9.6 | Unix PTY suite configured in CI       | Awaiting CI      |
| Windows x86_64 MSVC | 1.98.0 | 2.9.6 | Console/ConPTY suite configured in CI | Awaiting CI      |

Only Linux x86_64 GNU is currently verified. Linux release builds use glibc 2.39
or newer (Ubuntu 24.04 CI); musl is outside this release matrix. The local GNU
artifact requires GLIBC_2.39 according to its ELF version metadata. macOS and
Windows must pass their own suites before being advertised as supported. Other
architectures resolve artifact names but have no release or support claim.
Earlier Rust/Deno versions have not been tested.

CI runs checks, unit tests, formatting, lint, Clippy, native integration,
release packaging, and clean-cache installation checks independently on all
three platforms. An uploaded CI bundle is a candidate artifact until that
platform's verification has been reviewed. Windows ConPTY mode restoration is
asserted inside each child, since its host cannot inspect the child's console
handles. Resize uses `ResizePseudoConsole`; interrupt tests use a helper
attached to the child console to generate a real console control event.

## Build and install

From a clean checkout, with Rust, Deno, Python 3, and a C compiler installed:

```sh
deno task check
deno task test
deno task lint
cargo test --manifest-path native/Cargo.toml --locked
cargo clippy --manifest-path native/Cargo.toml --all-targets --locked -- -D warnings
python3 tests/integration/run.py
python3 scripts/package-native.py --verified
python3 tests/integration/package.py
```

On Windows use `python` and run in an x64 Visual Studio developer environment.
PTY/ConPTY tests require actual terminal signal access; restricted sandboxes can
prevent resize delivery. The packaging script refuses to overwrite an existing
bundle. Use a fresh `--output` directory when rebuilding.

The archive in `dist/` includes the native library and bundled binary bytes in
the JSR native wrapper, all four local workspace packages, both licenses in each
package, examples, the C header, version/toolchain metadata, and `SHA256SUMS`.
Extract it, verify the checksums, and run inside the extracted directory:

```sh
deno run --allow-ffi --allow-write examples/application.ts
```

Press ↑/↓ to select a list item and `q` to quit. Resizing redraws;
SIGINT/SIGTERM perform cleanup on Unix. Windows registers SIGINT. `--allow-ffi`
must be unrestricted because Deno requires it for pointer inspection. Native
libraries are trusted executable code. No network permission is needed for the
extracted local workspace examples.

The clean-install test extracts the archive into a temporary directory, verifies
all checksums and licenses, starts with an empty `DENO_DIR`, and exercises the
packaged target resolver through a real terminal lifecycle. The
`@ratatui-js/native` JSR wrapper embeds checksummed binary bytes generated from
verified archives. The adapter materializes the target binary into a private
temporary directory before `dlopen`, then removes it after shutdown and library
unload. Default loading needs temporary-directory write permission; an explicit
local path avoids extraction and that permission. No extra runtime fetch or npm
registry publication is needed. Source checkouts contain an empty artifact map;
release staging generates it before publication, keeping compiled binaries out
of Git.

## Before publishing

- Review all supported-platform CI runs, including mode restoration and
  interrupts. Do not promote an unverified platform to the support matrix.
- Keep crate/package versions, ABI/protocol metadata, and release notes aligned.
  ABI and protocol versions change only when their contracts change.
- Review the generated archive and checksum manifest; distribute both license
  texts and the public header with every native artifact.
- Dry-run package exports and included files with `deno publish --dry-run` after
  staging verified binaries into `packages/native/artifacts.ts`. Native bytes
  are included in the JSR wrapper; standalone C artifacts also ship in archives.
- Verify the documented example and default bundled native loading from a clean
  extracted bundle before publishing.

## Tag-only publishing

The packages remain unpublished. Create and link all four JSR packages to this
repository: `@ratatui-js/protocol`, `@ratatui-js/core`, `@ratatui-js/native`,
and `@ratatui-js/deno`. Linking enables the workflow's GitHub OIDC
authentication (`id-token: write`); no stored JSR token or publishing secret is
needed. GitHub release attachments use the workflow's built-in, run-scoped
`github.token`. Linking does not itself publish the repository.

Only pushing a version tag such as `v0.1.0-beta.1` or `v0.1.0` triggers
`release.yml`. Prerelease tags create GitHub prereleases and are not marked as
the latest release. Ordinary pushes, pull requests, and manual check runs never
publish. The release workflow checks every crate/package version and the
changelog against the tag, runs the full three-platform CI suite, merges
checksum-verified native archives into the JSR wrapper, checks the publish
payload, publishes the workspace, and creates a GitHub release with the
standalone archives and that version's changelog notes. Generated native bytes
explain the workflow's `--allow-dirty`: they are staged after checking out the
tag.

Track changes for all four packages in [`CHANGELOG.md`](../CHANGELOG.md) under
`## [Unreleased]` as work lands. Before tagging, move those notes into a section
such as `## [0.2.0] - 2026-10-01`, leaving `Unreleased` for future changes. The
date is optional; the version must match the tag without its `v` prefix. Use
third-level headings such as `### Added`, `### Changed`, and `### Fixed` to
group entries, naming the affected packages when helpful. Keep past version
sections.

The release gate rejects missing, duplicate, or empty notes for the tagged
version before publishing. Only that version's section becomes the GitHub
Release body; `Unreleased` and older versions are excluded. Preview the body
locally with:

```sh
python3 scripts/release_notes.py v0.1.0-beta.1 --output /tmp/ratatui-js-release-notes.md
```

Update versions through the relevant package-manager CLIs and commit changes
before tagging. Do not tag until all advertised target results have been
reviewed. No tag or remote publication is performed by local development checks.
