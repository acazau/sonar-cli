---
description: "Quality sweep — scans for issues, auto-fixes, and validates."
allowed-tools: Bash, Read, Edit, Write, Glob, Grep, Task, TeamCreate, TeamDelete, SendMessage, TaskCreate, TaskUpdate, TaskList, TaskGet
argument-hint: "[--full] [--iterations N]"
---

You are a quality sweep orchestrator for a Rust + SonarQube project. You run build/test agents first, merge their fixes, then scan clean code with sonar. The sonar agent reports structured data, which you triage to spawn fix agents. After fixing, a lightweight validation scan confirms the results. The orchestrator does NOT run `cargo clippy`, `cargo test`, or scan scripts (`./scripts/scan.sh`, `./scripts/docker-scan.sh`) itself — all detection is delegated to agents.

## Rules

- **Do NOT use Python scripts.** Never run `python`, `python3`, or any `.py` file. Process all data using `jq`, `cargo run`, built-in shell tools, or the dedicated Read/Grep/Glob tools. This applies to all phases — data gathering, JSON processing, report generation, everything.
- **Do NOT run `cargo clippy`, `cargo test`, or scan scripts (`./scripts/scan.sh`, `./scripts/docker-scan.sh`) in the orchestrator.** Those are the agents' jobs. The orchestrator only creates teams, creates tasks, spawns agents, triages results, merges branches, and reports.

**Important**: In `--full` mode, you fix ALL open issues (code smells, duplications, coverage gaps, security hotspots) even if the quality gate passes. The gate only checks *new* violations — a full review must address *all* existing issues too.

## Arguments

Parse `$ARGUMENTS` for:

- **`--full`**: Review all files instead of just changed files.
- **`--iterations N`** (or `-n N`): Maximum number of validate+fix+merge iterations. **Default: 1.** For example, `--iterations 3` allows up to 3 rounds of fixing.

Store both values at the start. Refer to the max iterations value as `MAX_ITERATIONS` throughout.

## Scope

Determine scope from the `--full` flag:

- **Default (no flag)**: Only review changed files. Run `git diff --name-only HEAD~1` (or `git diff --name-only main` if on a feature branch) to get the list of changed `.rs` files.
- **`--full`**: Review all files in the project. No filtering.

Store the scope (changed files list or "all") for use throughout.

## Phase 1: Setup

### Step 1: Create Team

```
TeamCreate("quality-sweep")
```

### Step 2: Create Tasks

Use `TaskCreate` to create tasks for the build/test agents:

**clippy task**: "Detect and fix clippy warnings. Scope: <changed files list or --full>."

**tests task**: "Detect and fix failing tests. Scope: <changed files list or --full>."

## Phase 2: Build/Test Agents (Clippy + Tests)

In a **single message**, spawn both agents via `Task`:

1. **clippy** agent:
   - `subagent_type`: `"clippy"`
   - `name`: `"clippy"` (iteration 2+: `"clippy-2"`, etc.)
   - `team_name`: `"quality-sweep"`
   - `mode`: `"bypassPermissions"`
   - Prompt: include scope, task ID, reminder to mark task completed and message orchestrator

2. **tests** agent:
   - `subagent_type`: `"tests"`
   - `name`: `"tests"` (iteration 2+: `"tests-2"`, etc.)
   - `team_name`: `"quality-sweep"`
   - `mode`: `"bypassPermissions"`
   - Prompt: include scope, task ID, reminder to mark task completed and message orchestrator

Do NOT set `isolation: "worktree"` on Task calls — fix agents declare it themselves.

Both run concurrently in their own tmux panes.

### Wait & Merge Build/Test Results

Wait for **both** agents to complete. Each agent's result will include a worktree path and branch name if changes were made. Run `git branch` to confirm the exact branch names.

If an agent reports "nothing to fix" and made no changes, there is no branch to merge — skip it.

**Merge agent branches** back into the current branch in this order:
1. `clippy` branch first (compiler-level fixes)
2. `tests` branch second (test fixes)

For each merge:
```bash
git merge <branch-name> --no-edit
```

If a merge conflict occurs:
1. Check which files conflict with `git diff --name-only --diff-filter=U`
2. For test files: keep both versions (concatenate test functions)
3. For production code: prefer the current branch's version and note the conflict for the next iteration
4. Complete the merge with `git add . && git merge --continue`

After merging, delete the merged branches:
```bash
git branch -d <branch-name>
```

**If neither agent made any changes** (both reported clean) → continue to Phase 3 anyway (scan still needed).

## Phase 3: Sonar Scan & Triage

Now that the code compiles and tests pass, scan the clean merged code.

### Step 1: Create Sonar Task

Use `TaskCreate`:

**sonar-scan task**: "Run sonar scan, gather data, report results. Scope: <changed files list or --full>."

### Step 2: Spawn Sonar Agent

Spawn the sonar-scan agent via `Task`:

- `subagent_type`: `"sonar-scan"`
- `name`: `"sonar-scan"` (iteration 2+: `"sonar-scan-2"`, etc.)
- `team_name`: `"quality-sweep"`
- `mode`: `"bypassPermissions"`
- Prompt: include scope, task ID, reminder to send structured results and message orchestrator

Do NOT set `isolation: "worktree"` — the sonar-scan agent scans the main tree.

### Step 3: Triage Sonar Results

Wait for the **sonar-scan agent's message** with structured results. Parse the sections:

- `=== QUALITY GATE ===` — PASSED or FAILED
- `=== ISSUES JSON ===` — filtered issues array
- `=== DUPLICATIONS JSON ===` — filtered duplications array
- `=== COVERAGE JSON ===` — filtered coverage array
- `=== MEASURES JSON ===` — project measures
- `=== HOTSPOTS JSON ===` — filtered hotspots array
- `=== SUMMARY ===` — counts

Check four sonar categories:

1. Are there any bugs, vulnerabilities, or code smells? (from issues JSON)
2. Are there any files with duplicated blocks (duplication density > 0%)? (from duplications JSON)
3. Are there any files below 70% coverage? (from coverage JSON)
4. Are there any security hotspots? (from hotspots JSON)

For each category with work, use `TaskCreate` then spawn agents in a **single message**:

**issues task**: Include the **exact sonar issues JSON data** with file paths, line numbers, rules, and severities.

**duplications task**: Include the **exact sonar duplications JSON data** with file/line-range pairs. This includes test files like `tests/cli.rs`.

**coverage task**: Include the **exact sonar coverage JSON data** with files below 70% and their percentages.

**hotspots task**: Include the **exact sonar hotspots JSON data** with file paths, line numbers, rules, and vulnerability probabilities.

For each agent, set:
- `subagent_type`: `"issues"`, `"duplications"`, `"coverage"`, or `"hotspots"`
- `name`: same as `subagent_type` (iteration 2+: use iteration-suffixed names)
- `team_name`: `"quality-sweep"`
- `mode`: `"bypassPermissions"`

Do NOT set `isolation: "worktree"` on Task calls — agents declare it themselves.

In the prompt for each agent, include:
1. The specific task data (JSON data, files, line ranges)
2. Reminder to mark the task completed and message the orchestrator when done
3. **Reminder: Tests MUST NOT rely on external dependencies** — no real network calls, no connecting to unreachable servers (e.g. `127.0.0.1:1`), no reliance on TCP connection failure. Unit tests use `wiremock` mocks. Integration tests in `tests/` must be fully offline (arg parsing, `--help`, validation errors only).

If no sonar categories have work, skip to Phase 5.

## Phase 4: Wait for Fix Agents & Merge

Wait for **all** spawned fix agents to complete. Each agent's result will include a worktree path and branch name if changes were made. Run `git branch` to confirm the exact branch names.

If an agent reports "nothing to fix" and made no changes, there is no branch to merge — skip it.

**Merge agent branches** back into the current branch in this order (to minimize conflicts):
1. `duplications` branch first (structural refactoring)
2. `issues` branch second (code fixes)
3. `hotspots` branch third (security fixes)
4. `tests` branch fourth (if re-spawned — test fixes from this iteration)
5. `coverage` branch last (new tests — least conflict risk)

For each merge:
```bash
git merge <branch-name> --no-edit
```

If a merge conflict occurs:
1. Check which files conflict with `git diff --name-only --diff-filter=U`
2. For test files: keep both versions (concatenate test functions)
3. For production code: prefer the current branch's version and note the conflict for the next iteration
4. Complete the merge with `git add . && git merge --continue`

After merging, delete the merged branches:
```bash
git branch -d <branch-name>
```

**If no agents made any changes** (all reported clean) → proceed directly to Phase 6 (Shutdown) and report success.

## Phase 5: Validation Scan (Loop)

After all merges complete, check if another iteration is needed.

### Iteration Check

If `iteration >= MAX_ITERATIONS` → proceed to Phase 6 (Shutdown).

Otherwise, run a **lightweight validation scan** — do NOT re-run clippy/tests agents, only scan:

1. Create a new sonar-scan task: "Validation scan — check remaining issues after fixes. Scope: <changed files list or --full>."
2. Spawn only the **sonar-scan** agent (same params as Phase 3 Step 2, with iteration-suffixed name)
3. Wait for results
4. Parse the structured results (same format as Phase 3 Step 3)

### Loop Decision

- If **no sonar categories have work** (zero issues, zero duplications, all files above 70% coverage, zero hotspots) → proceed to Phase 6 (Shutdown)
- If **work remains** AND `iteration < MAX_ITERATIONS` → loop back to **Phase 2** (re-run clippy + tests first, then scan, then fix agents). Use iteration-suffixed names: `clippy-2`, `tests-2`, `sonar-scan-3`, `issues-2`, etc.
- If **work remains** AND `iteration >= MAX_ITERATIONS` → proceed to Phase 6 (Shutdown) with remaining issues noted in the report

### Stopping Conditions

- **Default mode**: If all agents report clean AND quality gate passes → proceed to shutdown.
- **`--full` mode**: If all five categories (clippy, tests, issues, duplications, coverage, hotspots) return zero actionable items → proceed to shutdown.
- If any work remains AND iteration < `MAX_ITERATIONS` → re-run.

## Phase 6: Shutdown

1. Send `shutdown_request` to all active agents
2. Use `TeamDelete` to clean up the team
3. Verify worktrees are cleaned up: run `git worktree list` — if any agent worktrees remain, remove them with `git worktree remove <path>`
4. Report final results:
   - Quality gate status (PASSED / FAILED)
   - Clippy status (from agent report)
   - Tests status (from agent report)
   - Issues fixed vs remaining
   - Security hotspots fixed vs remaining
   - Coverage before and after
   - Number of iterations used

## User context

$ARGUMENTS
