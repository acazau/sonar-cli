# CLAUDE.md

Standalone Rust CLI for SonarQube. Binary-only crate.

## Rules

- Do NOT add `// NOSONAR`, `#[allow(...)]` (including `dead_code`, `unused_*`, `clippy::*`), or any suppression attributes/comments — fix the root cause.
- Do not change public API signatures unless the fix requires it.
- Do not delete or `#[ignore]` existing passing tests.
- Do NOT use Rust macros (`macro_rules!`, proc macros). Use regular functions instead.
- Do NOT use `unsafe` blocks. Fix the root cause instead (e.g., pass values as parameters rather than mutating global state).

## Testing Rules

- **No external dependencies in tests.** Unit tests use `wiremock` with `try_mock_server()`. Integration tests (`tests/cli.rs`) are fully offline: arg parsing, `--help`, validation errors only.
- If a script fails due to missing env vars, ask the user.
