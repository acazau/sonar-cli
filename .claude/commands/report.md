---
description: Generate a SonarQube project health report
allowed-tools: Bash(cargo run:*), Read, Glob, Grep
argument-hint: "[focus area, e.g. coverage, critical issues, branch name]"
---

Generate a comprehensive SonarQube project health report.

## Instructions

1. **Discover the CLI**: Run `cargo run -- --help` to see all available subcommands and global flags. Then run `cargo run -- <subcommand> -h` for each reporting-related subcommand to understand its flags and options. Use what you learn to construct the correct commands — do not assume flag names or subcommand signatures.

2. **Detect the branch**: Run `git branch --show-current` and pass `--branch <name>` to all subcommands so data matches the current branch.

3. **Determine scope**: If `$ARGUMENTS` specifies a focus area (e.g., "coverage only", "critical issues", "branch develop"), tailor the commands accordingly. If a branch is explicitly specified, use that instead of the detected branch. Otherwise, run every reporting subcommand you discovered in step 1 to build a full report.

4. **Gather data**: Run each relevant subcommand with `--json` and `--branch` flags. Use the flags and options you discovered via `-h` to pass the right project key and any other applicable options.

4. **Synthesize the report**: Combine the outputs into a structured summary covering quality gate status, key metrics, issues breakdown by severity, coverage analysis, security hotspots, duplications, and a prioritized list of recommended action items. Adapt sections based on what data is actually available.

## User context

$ARGUMENTS
