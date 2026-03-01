#!/usr/bin/env bash
set -euo pipefail

cargo run -- --project sonar-cli scan \
  --clippy-report "${SONAR_CLIPPY_REPORT:-clippy-report.json}" \
  --coverage-report "${SONAR_COVERAGE_REPORT:-coverage.xml}" \
  --no-scm \
  --skip-unchanged \
  --exclusions "**/*.json" \
  --sources "src,tests,scripts" \
  "$@"
