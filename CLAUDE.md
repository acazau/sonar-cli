# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

sonar-cli is a standalone Rust CLI for SonarQube — query issues, quality gates, metrics, coverage, and more. It is a reporting-only tool (no scan execution). It was extracted from the dagent project. Binary-only crate (no library target).

## Build & Test Commands

```bash
cargo build                          # Debug build
cargo build --release                # Release build → target/release/sonar-cli
cargo test                           # Run all tests (19 tests)
cargo test client::tests             # Run tests in a specific module
cargo test test_health_check_success # Run a single test by name
cargo test -- --nocapture            # Run tests with stdout visible
cargo clippy                         # Lint
cargo fmt                            # Format code
```

## Architecture

The codebase follows a layered architecture:

- **CLI layer** (`src/main.rs`) — Clap v4 derive-based command parsing with global flags (`--url`, `--token`, `--project`, `--branch`, `--json`, `--timeout`, `-v`). All global flags support env vars (`SONAR_HOST_URL`, `SONAR_TOKEN`, `SONAR_PROJECT_KEY`, `SONAR_BRANCH`). Routes to command handlers and converts their `i32` returns into exit codes.

- **Client layer** (`src/client.rs`) — `SonarQubeClient` with builder-pattern config (`SonarQubeConfig::new().with_token().with_project()`). Handles basic auth (token as username, empty password), automatic pagination (page size 100, max 100 pages), branch-aware API calls, and timeout. This is the largest module (~770 lines).

- **Command layer** (`src/commands/`) — Each command is a separate module (health, issues, quality_gate, measures, coverage, duplications, hotspots, projects, history, rules, source, wait). Each exposes an async function returning `i32` exit code.

- **Output layer** (`src/output.rs`) — Dual output modes: human-readable text tables or JSON (`--json` flag). Commands call output functions rather than printing directly.

- **Helpers layer** (`src/helpers.rs`) — Shared types (`FileCoverage`, `FileDuplication`, `ExtendedSonarData`) and utility functions (`extract_path`, `parse_measure`, `fetch_extended_data`) used by coverage, duplications, and output modules.

- **Types/Domain layer** (`src/types.rs`) — Serde models for all SonarQube API responses. `SonarQubeError` enum with variants: `Http`, `Api`, `Deserialize`, `Config`, `Timeout`, `Analysis`.

## Key Patterns

- All async using Tokio runtime (`#[tokio::main]`, `#[tokio::test]`)
- Tests use `wiremock` for HTTP mocking with a helper `try_mock_server()` that gracefully skips tests if port binding fails
- `#[cfg(test)]` inline test modules within source files
- `serial_test` crate for tests requiring sequential execution
- Exit code convention: 0 = success, 1 = error

## Environment Variables

| Variable | Purpose |
|---|---|
| `SONAR_HOST_URL` / `SONAR_URL` | SonarQube server URL (default: `http://localhost:9000`) |
| `SONAR_TOKEN` | Authentication token |
| `SONAR_PROJECT_KEY` | Project identifier |
| `SONAR_BRANCH` | Branch name for analysis |
| `RUST_LOG` | Tracing log level filter |

## Testing Rules

- **Tests MUST NOT rely on external dependencies** — no real network calls, no connecting to unreachable servers (e.g. `127.0.0.1:1`), no reliance on TCP connection failure. Unit tests use `wiremock` mock servers for all HTTP testing. Integration tests in `tests/` must be fully offline: only test arg parsing, `--help` output, and validation errors (e.g. missing `--project`).
- If a script fails due to missing env vars, ask the user.

## Code Review

To run a code review, use the `/code-review` slash command:
- `/code-review` — reviews changed files only
- `/code-review --full` — reviews all files

The slash command runs in the main session (as orchestrator), performing the sonar scan and data gathering centrally, then spawning parallel detect+fix agents as teammates. Each agent owns its full cycle — clippy and tests agents run their own `cargo` commands, while sonar-based agents (issues, duplications, coverage) receive JSON data from the orchestrator.

Do **not** use the `/scan` or `/report` skills directly for code reviews. Those are individual steps that the code-review pipeline calls internally.

## Compiler Warnings

The codebase intentionally has dead-code warnings for unused API response fields and type constants kept for API completeness. These are expected.
