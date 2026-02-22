---
description: Run a SonarQube analysis scan via Docker
allowed-tools: Bash(./scripts/scan.sh*), Bash(docker info*)
argument-hint: "[extra -D properties or options]"
---

Run a SonarQube analysis scan using sonar-scanner via Docker.

## Instructions

1. **Verify Docker is available**: Run `docker info` to confirm Docker is running. If not, tell the user and stop.

2. **Run the scan**: Execute `./scripts/scan.sh` passing any extra `-D` properties from `$ARGUMENTS` as arguments.

3. **Return the task ID**: Look for a line containing `task?id=` in the scanner output and extract the task ID. Report it back to the user.

## User context

$ARGUMENTS
