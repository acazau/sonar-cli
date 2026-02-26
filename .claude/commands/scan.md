---
description: Run a SonarQube analysis scan via native sonar-scanner
allowed-tools: Bash(./scripts/scan.sh*), Bash(sonar-scanner*)
argument-hint: "[extra -D properties or options]"
---

Run a SonarQube analysis scan using native sonar-scanner.

## Instructions

1. **Run the scan**: Execute `./scripts/scan.sh` passing any extra `-D` properties from `$ARGUMENTS` as arguments.

3. **Return the task ID**: Look for a line containing `task?id=` in the scanner output and extract the task ID. Report it back to the user.

## User context

$ARGUMENTS
