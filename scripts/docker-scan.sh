#!/usr/bin/env bash
set -euo pipefail

# Load environment variables from .env
set -a
source "$(dirname "$0")/../.env"
set +a

# Run SonarQube scanner via Docker
docker run --rm --network=host \
  -e SONAR_HOST_URL="${SONAR_HOST_URL}" -e SONAR_TOKEN \
  -e GIT_CONFIG_COUNT=1 \
  -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/usr/src \
  -v "$(pwd):/usr/src" \
  sonarsource/sonar-scanner-cli \
  -Dsonar.projectKey="$SONAR_PROJECT_KEY" \
  -Dsonar.branch.name="$(git branch --show-current)" \
  -Dsonar.rust.cobertura.reportPaths=coverage.xml \
  "$@"
