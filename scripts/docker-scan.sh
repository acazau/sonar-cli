#!/usr/bin/env bash
set -euo pipefail

: "${SONAR_HOST_URL:?Set SONAR_HOST_URL}" "${SONAR_TOKEN:?Set SONAR_TOKEN}"

docker run --rm --network=host \
  -e SONAR_HOST_URL -e SONAR_TOKEN \
  -e GIT_CONFIG_COUNT=1 \
  -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/usr/src \
  -v "$(pwd):/usr/src" \
  sonarsource/sonar-scanner-cli \
  -Dsonar.projectKey="sonar-cli" \
  -Dsonar.branch.name="$(git branch --show-current)" \
  -Dsonar.rust.cobertura.reportPaths="${SONAR_COVERAGE_REPORT:-coverage.xml}" \
  -Dsonar.rust.clippy.reportPaths="${SONAR_CLIPPY_REPORT:-clippy-report.json}" \
  "$@"
