---
description: Run a SonarQube analysis scan via xtask
allowed-tools: Bash(cargo xtask:*)
argument-hint: "[extra -D properties or options]"
---

Run a SonarQube analysis scan using the xtask sonar-scan subcommand.

## Instructions

1. **Run the scan**: Execute `cargo xtask sonar-scan` passing any extra `-D` properties from `$ARGUMENTS` as trailing arguments.

3. **Return the task ID**: Look for a line containing `task?id=` in the scanner output and extract the task ID. Report it back to the user.

## User context

$ARGUMENTS
