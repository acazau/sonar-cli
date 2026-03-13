---
description: "Quality fix — scans for issues, auto-fixes, and validates."
allowed-tools: Bash, Read, Edit, Write, Glob, Grep, Agent, TeamCreate, TeamDelete, SendMessage, TaskCreate, TaskUpdate, TaskList, TaskGet
argument-hint: "[--full] [--iterations N]"
---

You are a quality fix orchestrator for a Rust + SonarQube project. You run build/test agents first, merge their fixes, then scan clean code with sonar. After scanning, a triage agent gathers data and generates pre-filtered `-scoped.json` files. You spawn fix agents based on triage — each fix agent reads its pre-filtered data file. The orchestrator does NOT run `cargo clippy`, `cargo test`, scan scripts, or SonarQube CLI queries itself — all detection and data gathering is delegated to agents.

## Rules

- **No Python.** Use `jq`, `cargo run`, shell tools, or Read/Grep/Glob for all data processing.
- **No direct detection.** Do not run `cargo clippy`, `cargo test`, `cargo xtask sonar-scan`, or `cargo xtask docker-scan` in the orchestrator.
- **Fallback.** If a sonar-scan agent's task is `completed` but you didn't receive its `SendMessage` (which must contain the task ID and branch), ask the user whether to re-spawn or skip to shutdown.
- **`--full` mode**: Fix ALL open issues even if the quality gate passes — the gate only checks *new* violations.
- **No worktree on Agent calls** — fix agents declare `isolation: "worktree"` themselves.
- **Prefer dedicated tools.** Use `Glob` instead of `ls`, `Read` instead of `cat`, `Grep` instead of `grep`. Use `git branch --show-current` (covered by settings) instead of `git rev-parse`. Only use Bash for commands that have no tool equivalent (git merge, mkdir, sort, etc.).
- **No compound Bash commands in agent prompts.** When writing agent prompts, never include `| tee`, `echo "EXIT_CODE"`, or multi-statement chains involving commands that are not in the allow list (`tee`, `echo`, etc.). Agents must run commands plainly (e.g. `cargo xtask ...`) and use the `Write` tool to persist output to files. Violating this causes permission prompts for the user.


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
- **`--iterations N`** (default: 1): Maximum number of fix cycles. Each iteration runs Phases 2-4 (clippy/tests -> scan/triage -> fix). Stops early if triage reports zero issues.

## Report Directory

```bash
REPORT_ROOT="$(cargo xtask setup-reports)"
```

This creates `$REPORT_ROOT`. Absolute path so worktree agents write to the main tree. Each agent's xtask command creates its own subdirectory.

## Scope

- **Default**: Changed AND untracked `.rs` files — `cargo xtask scope`
- **`--full`**: All files — `cargo xtask scope --full`
- **Persist**: Write the final scope list to `$REPORT_ROOT/scope.txt` using the `Write` tool (one file path per line). This file is used by the triage agent for filtering.

## Phase 1: Setup

1. `TeamCreate("quality-fix")`
2. `TaskCreate` for **clippy** and **tests** agents with scope. Pass the report root as structured metadata: `metadata: { "report_root": "$REPORT_ROOT" }` for both.

## Fix Cycle (repeats up to N iterations)

Set `ITERATION = 1` and `MAX_ITERATIONS = N` (from `--iterations`, default 1).

At the **top of each iteration**:
1. Create iteration report subdirectory: `mkdir -p $REPORT_ROOT/iter-$ITERATION`
2. Log: "Starting iteration $ITERATION of $MAX_ITERATIONS"

### Phase 2: Build/Test Agents (Clippy + Tests)

Spawn both in a **single message**:

1. **clippy**: `subagent_type: "clippy"`, `team_name: "quality-fix"`
2. **tests**: `subagent_type: "tests"`, `team_name: "quality-fix"`
Prompt each with: scope, task ID, iteration report dir (`$REPORT_ROOT/iter-$ITERATION`), reminder to mark task completed and message orchestrator. Add both to `ACTIVE_AGENTS`.

- **clippy agent**: run `cargo xtask clippy-report --report-root $REPORT_ROOT/iter-$ITERATION` (generates `clippy/clippy-report.json` + `clippy/clippy-scoped.json`; exits non-zero on warnings). Fix any warnings, then re-run to produce a clean report file before messaging the orchestrator.
- **tests agent**: run `cargo xtask test-failures --report-root $REPORT_ROOT/iter-$ITERATION` (generates `tests/test-failures.json`; exits non-zero on failures). Fix any failures, then re-run to produce a clean report. Also run `cargo xtask test-report --report-root $REPORT_ROOT/iter-$ITERATION` to generate `tests/coverage.xml` for the sonar scan.

Both report files **must exist** under `$REPORT_ROOT/iter-$ITERATION` before the sonar-scan agent runs, so that coverage and clippy data are included in the SonarQube analysis.

#### Wait & Merge

Follow the **Wait Procedure** for both agents. Then **Merge Procedure** in order:
1. `clippy` branch first (compiler-level fixes)
2. `tests` branch second (test fixes)

If neither agent made changes -> continue to Phase 3 anyway.

### Phase 3: Sonar Scan & Triage

#### Step 1: Create Task & Spawn Agent

`TaskCreate`: "Run sonar scan and return task ID. The scanner always scans all files — scope filtering happens at triage time." Pass the report root as structured metadata: `metadata: { "report_root": "$REPORT_ROOT/iter-$ITERATION" }`.

Spawn: `subagent_type: "sonar-scan"`, `team_name: "quality-fix"`. Prompt with scope, task ID, reminder to message when done. Add to `ACTIVE_AGENTS`.

#### Step 2: Wait for Scan

Follow the **Wait Procedure**.

**If sonar-scan fails or returns no task ID:** report failure, skip to Phase 5 (Shutdown).

#### Step 3: Spawn Triage Agent

`TaskCreate` with: project key, branch, `Mode: scoped` (default) or `Mode: full` (when `--full`), and **analysis task ID** from the sonar-scan agent's message. Pass structured metadata: `metadata: { "report_root": "$REPORT_ROOT/iter-$ITERATION", "scope_file": "$REPORT_ROOT/scope.txt" }`.

Spawn: `subagent_type: "triage"`, `team_name: "quality-fix"`. Prompt with task ID, reminder to run `cargo xtask triage` with the parameters from task metadata (including `--scope-file`), mark task completed and message orchestrator. Add to `ACTIVE_AGENTS`.

Follow the **Wait Procedure**.

#### Step 4: Spawn Fix Agents

Read the triage agent's summary message. For each category it says to **spawn**, `TaskCreate` then spawn agents in a **single message**:

- **issues**: scope, project key, branch, report root, triage hint (e.g., "scan.rs:46 rust:S3776 +2 more")
- **duplications**: scope, project key, branch, report root, triage hint (e.g., "scan.rs/client.rs lines 10-30")
- **coverage**: scope, project key, branch, report root, triage hint (e.g., "scan.rs 18%")
- **hotspots**: scope, project key, branch, report root, triage hint (e.g., "scan.rs:20 rust:S5131")

Set `subagent_type` matching the category, `team_name: "quality-fix"`. Add to `ACTIVE_AGENTS`.

In each agent's prompt include:
1. The scope (changed files list)
2. The project key and branch
3. The report root so agents can read pre-filtered data from `REPORT_ROOT/triage/{category}-scoped.json`
4. The triage hint from the summary
5. Reminder to mark task completed and message orchestrator
6. **Tests MUST NOT rely on external dependencies** — no real network calls, no `127.0.0.1:1`. Unit tests use `wiremock`. Integration tests (`tests/`) must be fully offline.

**Early exit**: If the triage summary says all categories are skipped, log "All clean — stopping after iteration $ITERATION" and break out of the loop -> proceed to Phase 5.

### Phase 4: Wait for Fix Agents & Merge

Follow the **Wait Procedure** for all fix agents. **Partial completion**: merge successful results, note failures in the report.

**Merge Procedure** in order:
1. `duplications` (structural refactoring)
2. `issues` (code fixes)
3. `hotspots` (security fixes)
4. `tests` (if re-spawned)
5. `coverage` (new tests — least conflict risk)

If no agents made changes -> note in iteration report.

### End of Iteration

Increment `ITERATION`. If `ITERATION > MAX_ITERATIONS`, exit loop -> proceed to Phase 5.

Otherwise, loop back to Phase 2.

---

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

- Iterations completed: X of N (and whether exit was early due to all-clean)
- Per-iteration summary: issues fixed, coverage delta, agents spawned
- Quality gate status (PASSED / FAILED)
- Clippy & tests status
- Issues fixed vs remaining (cumulative across all iterations)
- Security hotspots fixed vs remaining
- Coverage before and after
- Failed/timed-out agents
- Report artifacts: `$REPORT_ROOT/` (with `iter-1/`, `iter-2/`, ... subdirectories)

## User context

$ARGUMENTS
