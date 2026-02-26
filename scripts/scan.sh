#!/usr/bin/env bash
set -euo pipefail

# Load environment variables from .env
set -a
source "$(dirname "$0")/../.env"
set +a

# Run SonarQube scanner natively (requires: brew install sonar-scanner)
sonar-scanner \
  -Dsonar.host.url="${SONAR_HOST_URL}" \
  -Dsonar.token="${SONAR_TOKEN}" \
  -Dsonar.projectKey="${SONAR_PROJECT_KEY}" \
  -Dsonar.projectBaseDir="$(pwd)" \
  -Dsonar.branch.name="$(git branch --show-current)" \
  -Dsonar.rust.cobertura.reportPaths=coverage.xml \
  "$@"
