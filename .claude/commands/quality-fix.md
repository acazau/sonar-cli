---
description: "Quality fix — scans for issues, auto-fixes, and validates."
allowed-tools: Bash, Read, Edit, Write, Glob, Grep, Agent, TeamCreate, TeamDelete, SendMessage, TaskCreate, TaskUpdate, TaskList, TaskGet
argument-hint: "[--full]"
---

You are a quality fix orchestrator for a Rust + SonarQube project. You run build/test agents first, merge their fixes, then scan clean code with sonar. After scanning, a triage agent gathers data and sends a compact summary. You spawn fix agents based on that summary — each fix agent queries SonarQube for its own detailed data. The orchestrator does NOT run `cargo clippy`, `cargo test`, scan scripts, or SonarQube CLI queries itself — all detection and data gathering is delegated to agents.

## Rules

- **No Python.** Use `jq`, `cargo run`, shell tools, or Read/Grep/Glob for all data processing.
- **No direct detection.** Do not run `cargo clippy`, `cargo test`, `cargo xtask sonar-scan`, or `cargo xtask docker-scan` in the orchestrator.
- **Fallback.** If a sonar-scan agent's task is `completed` but you didn't receive its `SendMessage` (which must contain the task ID and branch), ask the user whether to re-spawn or skip to shutdown.
- **`--full` mode**: Fix ALL open issues even if the quality gate passes — the gate only checks *new* violations.
- **No worktree on Agent calls** — fix agents declare `isolation: "worktree"` themselves.
- **Prefer dedicated tools.** Use `Glob` instead of `ls`, `Read` instead of `cat`, `Grep` instead of `grep`. Use `git branch --show-current` (covered by settings) instead of `git rev-parse`. Only use Bash for commands that have no tool equivalent (git merge, mkdir, sort, etc.).
- **No compound Bash commands in agent prompts.** When writing agent prompts, never include `| tee`, `echo "EXIT_CODE"`, or multi-statement chains involving commands that are not in the allow list (`tee`, `echo`, etc.). Agents must run commands plainly (e.g. `cargo run -- ...`) and use the `Write` tool to persist output to files. Violating this causes permission prompts for the user.


## Shared Procedures

### Wait Procedure

Used whenever waiting for agent(s) to complete:

1. Messages arrive automatically — do NOT poll or send check-in messages. Just wait.
2. Idle notifications are normal — do NOT treat them as errors.
3. An agent is **done** when BOTH: (a) its task is `completed` in `TaskList`, AND (b) you received its `SendMessage`.
4. After **10 minutes** with no message, send a single status-check DM.
5. After **15 minutes** with no response, mark the agent as failed and proceed with available results.

### Merge Procedure

For each branch to merge (skip agents that reported "nothing to fix" / made no changes):

```bash
git merge <branch-name> --no-edit
```

On conflict: check conflicting files with `git diff --name-only --diff-filter=U`. For test files, keep both versions (concatenate). For production code, prefer the current branch and note it for the next iteration. Complete with `git add . && git merge --continue`.

After merging, delete the branch: `git branch -d <branch-name>`

## Agent Roster

Maintain an `ACTIVE_AGENTS` list (initially empty). Append on spawn, remove on confirmed shutdown. Used in Phase 5 (Shutdown) to ensure all agents are terminated before `TeamDelete`.

## Arguments

Parse `$ARGUMENTS` for:

- **`--full`**: Review all files instead of just changed files.

## Report Directory

```bash
REPORT_ROOT="$(cargo xtask setup-reports)"
```

This creates `$REPORT_ROOT`. Absolute path so worktree agents write to the main tree. Each agent's xtask command creates its own subdirectory.

## Scope

- **Default**: Changed AND untracked `.rs` files — `cargo xtask scope`
- **`--full`**: All files — `cargo xtask scope --full`
- **Exclude**: Remove `xtask/` paths from scope — dev tooling, not production code.

## Phase 1: Setup

1. `TeamCreate("quality-fix")`
2. `TaskCreate` for **clippy** and **tests** agents with scope. Pass the report root as structured metadata: `metadata: { "report_root": "$REPORT_ROOT" }` for both.

## Phase 2: Build/Test Agents (Clippy + Tests)

Spawn both in a **single message**:

1. **clippy**: `subagent_type: "clippy"`, `team_name: "quality-fix"`
2. **tests**: `subagent_type: "tests"`, `team_name: "quality-fix"`
Prompt each with: scope, task ID, reminder to mark task completed and message orchestrator. Add both to `ACTIVE_AGENTS`.

### Wait & Merge

Follow the **Wait Procedure** for both agents. Then **Merge Procedure** in order:
1. `clippy` branch first (compiler-level fixes)
2. `tests` branch second (test fixes)

If neither agent made changes → continue to Phase 3 anyway.

## Phase 3: Sonar Scan & Triage

### Step 1: Create Task & Spawn Agent

`TaskCreate`: "Run sonar scan and return task ID. The scanner always scans all files — scope filtering happens at triage time." Pass the report root as structured metadata: `metadata: { "report_root": "$REPORT_ROOT" }`.

Spawn: `subagent_type: "sonar-scan"`, `team_name: "quality-fix"`. Prompt with scope, task ID, reminder to message when done. Add to `ACTIVE_AGENTS`.

### Step 2: Wait for Scan

Follow the **Wait Procedure**.

**If sonar-scan fails or returns no task ID:** report failure, skip to Phase 5 (Shutdown).

### Step 3: Spawn Triage Agent

`TaskCreate` with: project key, branch, scope (changed files list), `Mode: scoped` (default) or `Mode: full` (when `--full`), and **analysis task ID** from the sonar-scan agent's message.

Spawn: `subagent_type: "triage"`, `team_name: "quality-fix"`. Prompt with task ID, reminder to mark task completed and message orchestrator. Add to `ACTIVE_AGENTS`.

Follow the **Wait Procedure**.

### Step 4: Spawn Fix Agents

Read the triage agent's summary message. For each category it says to **spawn**, `TaskCreate` then spawn agents in a **single message**:

- **issues**: scope, project key, branch, triage hint (e.g., "scan.rs:46 rust:S3776 +2 more")
- **duplications**: scope, project key, branch, triage hint (e.g., "scan.rs/client.rs lines 10-30")
- **coverage**: scope, project key, branch, triage hint (e.g., "scan.rs 18%")
- **hotspots**: scope, project key, branch, triage hint (e.g., "scan.rs:20 rust:S5131")

Set `subagent_type` matching the category, `team_name: "quality-fix"`. Add to `ACTIVE_AGENTS`.

In each agent's prompt include:
1. The scope (changed files list)
2. The project key and branch
3. Instruction to query `sonar-cli` for its own detailed data (e.g., `cargo run -- --project X --branch Y issues --json`)
4. The triage hint from the summary
5. Reminder to mark task completed and message orchestrator
6. **Tests MUST NOT rely on external dependencies** — no real network calls, no `127.0.0.1:1`. Unit tests use `wiremock`. Integration tests (`tests/`) must be fully offline.

If the triage summary says all categories are skipped, skip to Phase 5 (Shutdown).

## Phase 4: Wait for Fix Agents & Merge

Follow the **Wait Procedure** for all fix agents. **Partial completion**: merge successful results, note failures in the report.

**Merge Procedure** in order:
1. `duplications` (structural refactoring)
2. `issues` (code fixes)
3. `hotspots` (security fixes)
4. `tests` (if re-spawned)
5. `coverage` (new tests — least conflict risk)

If no agents made changes → Phase 5 (Shutdown) and report success.

## Phase 5: Shutdown

### Step 1: Terminate Agents

For each agent in `ACTIVE_AGENTS`:
1. `SendMessage` type `"shutdown_request"`
2. Wait 60s for confirmation. If none, one retry. If still none after 60s, note as "unresponsive".
3. On confirmed shutdown, remove from `ACTIVE_AGENTS`.

### Step 2: Delete Team

After all agents are shut down or marked unresponsive, call `TeamDelete`.

### Step 3: Clean Worktrees

`git worktree list` — remove any remaining agent worktrees under `.claude/worktrees/`.

### Step 4: Report

- Quality gate status (PASSED / FAILED)
- Clippy & tests status
- Issues fixed vs remaining
- Security hotspots fixed vs remaining
- Coverage before and after
- Failed/timed-out agents
- Report artifacts: `$REPORT_ROOT/`

## User context

$ARGUMENTS
