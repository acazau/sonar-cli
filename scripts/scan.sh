#!/usr/bin/env bash
set -euo pipefail

CLIPPY_REPORT="${SONAR_CLIPPY_REPORT:-clippy-report.json}"
COVERAGE_REPORT="${SONAR_COVERAGE_REPORT:-coverage.xml}"

REPORT_FLAGS=()
[[ -f "$CLIPPY_REPORT" ]]   && REPORT_FLAGS+=(--clippy-report "$CLIPPY_REPORT")
[[ -f "$COVERAGE_REPORT" ]] && REPORT_FLAGS+=(--coverage-report "$COVERAGE_REPORT")

cargo run -- --project sonar-cli scan \
  "${REPORT_FLAGS[@]}" \
  --no-scm \
  --skip-unchanged \
  --exclusions "**/*.json" \
  --sources "src,tests,scripts" \
  "$@"
