# sonar-cli

Standalone CLI for SonarQube — query issues, quality gates, metrics, coverage, and more.

sonar-cli is a **reporting-only** tool. It queries a SonarQube server for project data but does not run scans itself. Use `sonar-scanner` (via Docker or native install) to submit analyses, then use sonar-cli to inspect the results.

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

sonar-cli reads configuration from command-line flags or environment variables. A `.env` file in the project root is loaded automatically via `dotenvy`.

| Flag | Env Variable | Default | Description |
|------|-------------|---------|-------------|
| `--url` | `SONAR_HOST_URL` | `http://localhost:9000` | SonarQube server URL |
| `--token` | `SONAR_TOKEN` | | Authentication token |
| `--project` | `SONAR_PROJECT_KEY` | | Project key |
| `--branch` | `SONAR_BRANCH` | | Branch name |
| `--json` | | | Output as JSON |
| `--timeout` | | `30` | Request timeout in seconds |
| `-v` | | | Verbose logging |

## Commands

### Server commands (no `--project` required)

```bash
# Check server health
sonar-cli health

# List projects
sonar-cli projects
sonar-cli projects --search my-app

# Search quality rules
sonar-cli rules
sonar-cli rules --language java --severity CRITICAL

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
```

### Analysis commands

```bash
# Wait for a background analysis task to complete
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

## SonarQube Server Setup

This project includes a `docker-compose.yml` that runs SonarQube with the [community-branch-plugin](https://github.com/mc1arke/sonarqube-community-branch-plugin), enabling analysis of feature branches (not just `main`).

```bash
docker compose up -d
```

SonarQube will be available at `http://localhost:9000` (default credentials: `admin`/`admin`).

### First-time setup

1. Log in and create a project (Projects → Create Project → Manually)
2. Generate an authentication token (My Account → Security → Generate Token)
3. Create a `.env` file in the project root:

   ```
   SONAR_HOST_URL=http://localhost:9000
   SONAR_TOKEN=<your-token>
   SONAR_PROJECT_KEY=<your-project-key>
   ```

### Running a scan

**Native (default)** — requires `sonar-scanner` installed locally:

```bash
brew install sonar-scanner
./scripts/scan.sh
```

**Docker (reference)** — uses the `sonarsource/sonar-scanner-cli` image:

```bash
./scripts/docker-scan.sh
```

### Branch analysis

Branch analysis is automatic — the `/scan` command detects the current git branch and passes it to the scanner via `-Dsonar.branch.name`. Results are then queryable per branch:

```bash
sonar-cli --project my-proj --branch feature-x issues
sonar-cli --project my-proj --branch feature-x quality-gate
```

## Claude Quality Sweep Workflow

This project includes a Claude Code agent (`/quality-fix`) that scans for issues and auto-fixes them using parallel agents in isolated git worktrees.

- `/quality-fix` — scan changed files only (default)
- `/quality-fix --full` — scan all files (tech debt cleanup)

```
┌──────────────────────────────────────────────────────────┐
│                /quality-fix [--full]                       │
└────────────────────┬─────────────────────────────────────┘
                     │
                     ▼
             ┌───────────────┐
             │ Determine     │── --full ──→ All .rs files
             │ Scope         │── default ─→ git diff changed files
             └───────┬───────┘
                     │
        ═════════════╪═════════════
        ║  PHASE 1: VALIDATE      ║
        ═════════════╪═════════════
                     ▼
             ┌───────────────┐  ≤3 trivial?
             │ Clippy        │─────────────→ Fix inline
             └───────┬───────┘
                     │ pass
                     ▼
             ┌───────────────┐  ≤2 trivial?
             │ Tests         │─────────────→ Fix inline
             └───────┬───────┘
                     │ pass
                     ▼
             ┌───────────────┐
             │ Coverage      │ cargo llvm-cov → coverage.xml
             └───────┬───────┘
                     │
                     ▼
             ┌───────────────┐
             │ Sonar Scan    │ → /scan
             └───────┬───────┘
                     │
                     ▼
             ┌───────────────┐
             │ Report        │ → /report
             └───────┬───────┘
                     │
                     ▼
              ╔═════════════╗
              ║ Gate passed?║
              ╚══════╤══════╝
            yes      │     no
         ┌───────────┴──────────┐
         ▼                      ▼
  ┌────────────┐   ═════════════╪═══════════════
  │ Done       │   ║  PHASE 2: TRIAGE & FIX    ║
  └────────────┘   ═════════════╪═══════════════
                                ▼
                   ┌────────────────────────────┐
                   │ Gather JSON data           │
                   │ duplications/issues/cov    │
                   │ (filtered to scope)        │
                   └───────────┬────────────────┘
                               │
                               ▼
                   ┌────────────────────────────┐
                   │ TeamCreate "quality-fix"    │
                   │ TaskCreate per category    │
                   └───────────┬────────────────┘
                               │
                ┌──────────────┼──────────────┐
                │              │              │
                ▼              ▼              ▼
       ┌──────────────┐ ┌───────────┐ ┌────────────┐
       │ WORKTREE A   │ │WORKTREE B │ │ WORKTREE C │
       │              │ │           │ │            │
       │              │ │           │ │            │
       │ duplications │ │ issues    │ │ coverage   │
       │              │ │ Bugs,     │ │ Add tests  │
       │ Extract      │ │ smells,   │ │ for files  │
       │ helpers      │ │ vulns     │ │ below 70%  │
       └──────┬───────┘ └─────┬─────┘ └─────┬──────┘
              │               │              │
              └───────┬───────┴──────┬───────┘
                      ▼              │
         ┌─────────────────────────┐ │
         │ Merge branches in order │◄┘
         │ 1. duplications (structural)
         │ 2. issues (code fixes)
         │ 3. coverage (tests)
         └────────────┬────────────┘
                      │
         ═════════════╪═════════════
         ║  PHASE 3: RE-VALIDATE   ║
         ═════════════╪═════════════
                      │
                      ▼
               ╔═════════════╗
               ║ Gate passed?║
               ╚══════╤══════╝
             yes      │      no
         ┌────────────┴───────────┐
         ▼                        ▼
  ┌──────────────┐        ╔═════════════╗
  │ Shutdown     │        ║ iter < 3?   ║
  │ team & done  │        ╚══════╤══════╝
  └──────────────┘       yes     │    no
                     ┌───────────┴──────────┐
                     ▼                      ▼
              Loop back to          ┌──────────────┐
              PHASE 2               │ Shutdown     │
                                    │ & report     │
                                    │ remaining    │
                                    └──────────────┘
```

## Development

```bash
cargo build          # Debug build
cargo test           # Run tests (19 tests)
cargo clippy         # Lint
cargo fmt            # Format
```

## License

MIT
