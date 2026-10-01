# Bundled native library

This JSR wrapper contains checksummed binary bytes for verified release targets.
The source checkout has an empty artifact map. Release staging generates it from
tested archives with `scripts/prepare-jsr.py`; compiled binaries are not
committed to the repository.

`@ratatui-js/deno` uses this wrapper automatically when no explicit library path
is supplied. It verifies SHA-256, writes an owned local library into a private
temporary directory, and loads it through FFI. After closing the session,
finishing calls, freeing buffers, and unloading the library, it removes that
directory. The default path needs `--allow-ffi --allow-write`; a local explicit
path needs only `--allow-ffi`. Embedded assets do not need a runtime network
fetch.

A JSR module URL cannot be passed to the OS dynamic loader. Embedding the bytes
keeps native delivery in JSR while still supplying the local file needed by
`Deno.dlopen`. See [release instructions](../../docs/releases.md) for tag-only
publishing and the verified-target matrix.
