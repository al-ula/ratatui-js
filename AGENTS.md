# Project rules

## Code for humans

- Favor readable, straightforward code over cleverness.
- Use descriptive names, small focused units, and explicit control flow.
- Keep abstractions proportional to current needs; avoid speculative infrastructure.
- Explain non-obvious decisions and invariants, not what obvious code does.

## Prioritize correctness

- Correctness takes precedence over performance shortcuts and convenience.
- Make ownership, error handling, concurrency, and cleanup explicit, especially across FFI.
- Validate boundary inputs and test failure paths as well as successful behavior.
- Verify changes with relevant tests and checks; report anything not verified.
- Optimize only when measurements justify it, preserving correctness and readability.

## Manage dependencies through the CLI

- Use the relevant package manager CLI to add, update, or remove dependencies.
- Do not manually edit dependency declarations or lockfiles to manage versions.
- Use the latest stable releases available when adding or updating dependencies; verify current versions rather than relying on memory.
- Generate and update lockfiles through the package manager for reproducible builds.
- Discuss compatibility constraints before selecting an older version.
