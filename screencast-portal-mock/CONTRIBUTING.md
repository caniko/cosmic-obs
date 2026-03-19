# Contributing

Thanks for your interest in contributing!

## Getting started

You need a Linux system with `dbus-daemon` available (the test harness spawns
private bus instances).

```bash
# Run the tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy -- -D warnings
```

## Style

- **Static dispatch by default**: use `impl Trait` / `<T: Trait>`.
  Only use `dyn Trait` when the type is caller-chosen at runtime, with a comment
  explaining why.
- **No `unwrap()`** in library code (`src/`). Use `?`, `expect()` with a
  justification message, or `thiserror` enums.
- **`#[must_use]`** on functions returning `Result` or meaningful values.
- Keep functions under ~30 lines where practical.
- No dead code, unused imports, or commented-out blocks.

## Pull requests

1. Fork the repo and create a feature branch.
2. Make sure `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test` all pass.
3. Open a pull request with a clear description of the change.
