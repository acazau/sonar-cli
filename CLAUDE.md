# CLAUDE.md

Standalone Rust CLI for SonarQube (reporting only, no scan execution). Binary-only crate.

## Testing Rules

- **No external dependencies in tests.** Unit tests use `wiremock` with `try_mock_server()`. Integration tests (`tests/cli.rs`) are fully offline: arg parsing, `--help`, validation errors only.
- If a script fails due to missing env vars, ask the user.

## Notes

- Dead-code warnings for unused API response fields are intentional.
- Do NOT use Rust macros (`macro_rules!`, proc macros). Use regular functions instead.
