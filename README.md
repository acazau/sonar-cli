# sonar-cli

Standalone CLI for SonarQube — scan, query issues, quality gates, metrics, coverage, and more.

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
# Binary at target/release/sonar-cli
```

## Configuration

sonar-cli reads configuration from command-line flags or a stored config file (`sonar-cli auth login`).

| Flag | Default | Description |
|------|---------|-------------|
| `--url` | *(required)* | SonarQube server URL |
| `--token` | | Authentication token |
| `--project` | `SONAR_PROJECT_KEY` env | Project key |
| `--branch` | `SONAR_BRANCH` env | Branch name |
| `--json` | | Output as JSON |
| `--timeout` | `30` | Request timeout in seconds |
| `-v` | | Verbose logging |

### Credential management

```bash
# Store credentials (saved globally — no env vars needed for subsequent commands)
sonar-cli auth login --url https://sonar.example.com --token squ_abc123

# Check stored credentials
sonar-cli auth status

# Remove stored credentials
sonar-cli auth logout
```

Priority: CLI flags > config file > defaults.

## Commands

### Server commands (no `--project` required)

```bash
# Check server health
sonar-cli health

# List projects
sonar-cli projects
sonar-cli projects --search my-app
sonar-cli projects --qualifier VW   # list portfolios (TRK=projects, VW=portfolios, APP=applications)

# Search quality rules
sonar-cli rules
sonar-cli rules --language java --severity CRITICAL
sonar-cli rules --search "null pointer"
sonar-cli rules --rule-type BUG --status READY

# View source code
sonar-cli source my-project:src/main.rs
sonar-cli source my-project:src/main.rs --from 1 --to 50
```

### Project commands (require `--project`)

```bash
# Quality gate status
sonar-cli --project my-proj quality-gate
sonar-cli --project my-proj quality-gate --fail-on-error

# Issues
sonar-cli --project my-proj issues
sonar-cli --project my-proj issues --severity CRITICAL
sonar-cli --project my-proj issues --status RESOLVED --language java
sonar-cli --project my-proj issues --created-after 2025-01-01 --limit 50
sonar-cli --project my-proj issues --issue-type BUG --rule java:S1234
sonar-cli --project my-proj issues --resolution FIXED --tags security
sonar-cli --project my-proj issues --author jdoe --assignee unassigned
sonar-cli --project my-proj issues --created-before 2025-06-01 --new-code

# Metrics
sonar-cli --project my-proj measures
sonar-cli --project my-proj measures --metrics coverage,bugs,ncloc

# Metric trends
sonar-cli --project my-proj history --metrics coverage
sonar-cli --project my-proj history --metrics coverage,bugs --from 2025-01-01

# Coverage
sonar-cli --project my-proj coverage
sonar-cli --project my-proj coverage --min-coverage 80 --sort uncovered

# Duplications
sonar-cli --project my-proj duplications
sonar-cli --project my-proj duplications --details

# Security hotspots
sonar-cli --project my-proj hotspots
sonar-cli --project my-proj hotspots --status REVIEWED
sonar-cli --project my-proj hotspots --new-code
```

### Analysis commands

#### CLI scanner (default)

```bash
# Run sonar-scanner using stored credentials (no env vars needed)
sonar-cli --project my-proj scan
sonar-cli --project my-proj scan --wait
sonar-cli --project my-proj scan --clippy-report clippy.json --coverage-report coverage.xml
sonar-cli --project my-proj scan --no-scm --skip-unchanged
sonar-cli --project my-proj scan --exclusions "**/*_test.go,**/vendor/**"
sonar-cli --project my-proj scan --sources src,lib
sonar-cli --project my-proj scan --wait --wait-timeout 600 --poll-interval 10
sonar-cli --project my-proj scan --wait -- -Dsonar.verbose=true
```

#### .NET scanner (`--scanner dotnet`)

Orchestrates a 4-phase flow: `dotnet sonarscanner begin` → `dotnet build` → `dotnet test` → `dotnet sonarscanner end`.

```bash
sonar-cli --project my-proj scan --scanner dotnet --solution MyApp.sln
sonar-cli --project my-proj scan --scanner dotnet --solution MyApp.sln --wait
sonar-cli --project my-proj scan --scanner dotnet --solution MyApp.sln --skip-tests
sonar-cli --project my-proj scan --scanner dotnet --solution MyApp.sln \
  --opencover-report coverage.xml --lcov-report lcov.info --run-id 42
```

#### Wait for background analysis

```bash
sonar-cli wait <TASK_ID>
sonar-cli wait <TASK_ID> --timeout 600 --poll-interval 10
```

## JSON output

All commands support `--json` for machine-readable output:

```bash
sonar-cli --project my-proj quality-gate --json
sonar-cli --project my-proj issues --json | jq '.[] | select(.severity == "CRITICAL")'
```

## CI usage

Use `--fail-on-error` with `quality-gate` to fail CI pipelines when the quality gate doesn't pass:

```bash
sonar-cli --project my-proj quality-gate --fail-on-error
```

Exit codes: `0` = success, `1` = error or quality gate failed.

## Claude Quality Sweep Workflow

This project includes a Claude Code agent (`/quality-fix`) that auto-fixes code quality issues using parallel agents in isolated git worktrees.

- `/quality-fix` — scan changed files only (default)
- `/quality-fix --full` — scan all files (tech debt cleanup)
- `/quality-fix --iterations N` — run up to N fix cycles (default: 1)

```
┌──────────────────────────────────────────────────────────┐
│           /quality-fix [--full] [--iterations N]         │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
             ┌───────────────┐
             │ Phase 1:      │
             │ Setup         │── TeamCreate, scope, report dir
             └───────┬───────┘
                     │
        ┌────────────────────────────┐
        │   Fix Cycle (up to N)      │
        │                            │
        │  ══════════════════════    │
        │  PHASE 2: BUILD & TEST    │
        │  ══════════════════════    │
        │         │                  │
        │    ┌────┴────┐             │
        │    ▼         ▼             │
        │ ┌───────┐ ┌───────┐       │
        │ │clippy │ │ tests │       │
        │ │(wktree)│ │(wktree)│      │
        │ └───┬───┘ └───┬───┘       │
        │     └────┬─────┘           │
        │          ▼                 │
        │   Merge clippy → tests    │
        │          │                 │
        │  ══════════════════════    │
        │  PHASE 3: SCAN & TRIAGE   │
        │  ══════════════════════    │
        │          ▼                 │
        │   ┌─────────────┐         │
        │   │  sonar-scan │→ task ID│
        │   └──────┬──────┘         │
        │          ▼                 │
        │   ┌─────────────┐         │
        │   │   triage    │         │
        │   └──────┬──────┘         │
        │          │                 │
        │  ══════════════════════    │
        │  PHASE 4: FIX AGENTS      │
        │  ══════════════════════    │
        │          │                 │
        │   ┌──────┴──────────┐     │
        │   ▼    ▼     ▼     ▼     │
        │ ┌────┐┌────┐┌───┐┌─────┐ │
        │ │issu││dups││cov││hotsp│ │
        │ │(wt)││(wt)││(wt)││(wt) │ │
        │ └─┬──┘└─┬──┘└─┬─┘└──┬──┘ │
        │   └─────┴─────┴─────┘     │
        │          ▼                 │
        │   Merge: dups → issues    │
        │    → hotspots → coverage  │
        │          │                 │
        │   (loop if iterations      │
        │    remain & issues found)  │
        └────────────┬───────────────┘
                     │
        ═════════════╪═════════════
        ║   PHASE 5: SHUTDOWN      ║
        ═════════════╪═════════════
                     ▼
              Final report
```

## Development

```bash
cargo build          # Debug build
cargo test           # Run tests
cargo clippy         # Lint
cargo fmt            # Format
```

## License

MIT
