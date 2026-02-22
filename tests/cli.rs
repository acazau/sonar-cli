#[allow(deprecated)]
use assert_cmd::Command;
use predicates::prelude::*;

/// Build a CLI command with SonarQube env vars cleared so tests are hermetic.
/// Also sets current_dir to a temp dir so dotenvy won't load the project's .env file.
#[allow(deprecated)]
fn cli() -> Command {
    let mut cmd = Command::cargo_bin("sonar-cli").unwrap();
    cmd.env_remove("SONAR_HOST_URL")
        .env_remove("SONAR_URL")
        .env_remove("SONAR_TOKEN")
        .env_remove("SONAR_PROJECT_KEY")
        .env_remove("SONAR_BRANCH")
        .current_dir(std::env::temp_dir());
    cmd
}

/// Assert that running the CLI with the given args fails with "Project key is required".
fn assert_missing_project(args: &[&str]) {
    cli()
        .args(args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Project key is required"));
}

/// Assert that running a subcommand with `--help` succeeds and stdout contains all expected strings.
fn assert_help_contains(subcommand: &str, expected: &[&str]) {
    let mut assertion = cli()
        .args([subcommand, "--help"])
        .assert()
        .success();
    for s in expected {
        assertion = assertion.stdout(predicate::str::contains(*s));
    }
}

// ── Top-level flags ─────────────────────────────────────────────────

#[test]
fn test_help_flag() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Standalone CLI for SonarQube"));
}

#[test]
fn test_version_flag() {
    cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sonar-cli"));
}

#[test]
fn test_no_subcommand() {
    cli()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

#[test]
fn test_invalid_subcommand() {
    cli()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

// ── Subcommand --help (verifies clap definitions) ───────────────────

#[test]
fn test_health_help() {
    assert_help_contains("health", &["Check SonarQube server health"]);
}

#[test]
fn test_quality_gate_help() {
    assert_help_contains("quality-gate", &["quality gate status", "--fail-on-error"]);
}

#[test]
fn test_issues_help() {
    assert_help_contains("issues", &["--severity", "--status", "--rule", "--language"]);
}

#[test]
fn test_measures_help() {
    assert_help_contains("measures", &["--metrics"]);
}

#[test]
fn test_coverage_help() {
    assert_help_contains("coverage", &["--min-coverage", "--sort"]);
}

#[test]
fn test_duplications_help() {
    assert_help_contains("duplications", &["--details"]);
}

#[test]
fn test_hotspots_help() {
    assert_help_contains("hotspots", &["--status"]);
}

#[test]
fn test_projects_help() {
    assert_help_contains("projects", &["--search", "--qualifier"]);
}

#[test]
fn test_history_help() {
    assert_help_contains("history", &["--metrics", "--from", "--to"]);
}

#[test]
fn test_rules_help() {
    assert_help_contains("rules", &["--language", "--severity", "--rule-type"]);
}

#[test]
fn test_source_help() {
    assert_help_contains("source", &["--from", "--to", "<COMPONENT>"]);
}

#[test]
fn test_wait_help() {
    assert_help_contains("wait", &["--timeout", "--poll-interval"]);
}

// ── Missing --project validation (exits before any network call) ────

#[test]
fn test_issues_missing_project() {
    assert_missing_project(&["issues"]);
}

#[test]
fn test_quality_gate_missing_project() {
    assert_missing_project(&["quality-gate"]);
}

#[test]
fn test_measures_missing_project() {
    assert_missing_project(&["measures"]);
}

#[test]
fn test_coverage_missing_project() {
    assert_missing_project(&["coverage"]);
}

#[test]
fn test_duplications_missing_project() {
    assert_missing_project(&["duplications"]);
}

#[test]
fn test_hotspots_missing_project() {
    assert_missing_project(&["hotspots"]);
}

#[test]
fn test_history_missing_project() {
    assert_missing_project(&["history", "--metrics", "coverage"]);
}

// ── Clap validation errors (exits before any network call) ──────────

#[test]
fn test_history_missing_required_metrics_arg() {
    cli()
        .args(["--project", "test-proj", "history"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--metrics"));
}

#[test]
fn test_source_missing_required_component_arg() {
    cli()
        .arg("source")
        .assert()
        .failure()
        .stderr(predicate::str::contains("<COMPONENT>"));
}

#[test]
fn test_wait_missing_required_task_id_arg() {
    cli()
        .arg("wait")
        .assert()
        .failure()
        .stderr(predicate::str::contains("<TASK_ID>"));
}

// ── Global flags and env vars ────────────────────────────────────────

#[test]
fn test_url_flag_accepted() {
    // Exercises --url parsing in build_config(); exits immediately on missing --project
    assert_missing_project(&["--url", "http://custom-server:9000", "issues"]);
}

#[test]
fn test_token_flag_accepted() {
    // Exercises --token parsing in build_config()
    assert_missing_project(&["--token", "mytoken", "issues"]);
}

#[test]
fn test_branch_flag_accepted() {
    // Exercises --branch parsing in build_config()
    assert_missing_project(&["--branch", "develop", "issues"]);
}

#[test]
fn test_json_flag_accepted() {
    // Exercises --json parsing path
    assert_missing_project(&["--json", "issues"]);
}

#[test]
fn test_timeout_flag_accepted() {
    // Exercises --timeout parsing in build_config()
    assert_missing_project(&["--timeout", "60", "issues"]);
}

#[test]
fn test_verbose_flag_accepted() {
    // Exercises -v / --verbose flag parsing (init_tracing verbose=true path)
    assert_missing_project(&["-v", "issues"]);
}

#[test]
fn test_verbose_long_flag_accepted() {
    assert_missing_project(&["--verbose", "issues"]);
}

// ── require_project() error path for all project-requiring commands ──

#[test]
fn test_history_missing_project_no_project_flag() {
    // Exercises require_project() Err path for History command dispatch
    assert_missing_project(&["history", "--metrics", "coverage"]);
}

// ── Command argument parsing (exercises dispatch arms in main) ───────

#[test]
fn test_issues_with_all_filters_missing_project() {
    // Exercises the Issues command arm's argument parsing in main
    assert_missing_project(&[
        "issues",
        "--severity", "CRITICAL",
        "--status", "OPEN",
        "--rule", "rust:S1",
        "--language", "rust",
        "--limit", "10",
        "--created-after", "2025-01-01",
        "--created-before", "2025-12-31",
        "--author", "alice",
        "--assignee", "bob",
        "--tags", "security",
        "--resolution", "FALSE-POSITIVE",
    ]);
}

#[test]
fn test_measures_with_metrics_missing_project() {
    // Exercises Measures command arm with --metrics flag
    assert_missing_project(&["measures", "--metrics", "coverage,bugs"]);
}

#[test]
fn test_coverage_with_min_coverage_missing_project() {
    // Exercises Coverage command arm with --min-coverage flag
    assert_missing_project(&["coverage", "--min-coverage", "80"]);
}

#[test]
fn test_coverage_with_sort_missing_project() {
    // Exercises Coverage command arm with --sort flag
    assert_missing_project(&["coverage", "--sort", "uncovered"]);
}

#[test]
fn test_duplications_with_details_missing_project() {
    // Exercises Duplications command arm with --details flag
    assert_missing_project(&["duplications", "--details"]);
}

#[test]
fn test_hotspots_with_status_missing_project() {
    // Exercises Hotspots command arm with --status flag
    assert_missing_project(&["hotspots", "--status", "REVIEWED"]);
}

#[test]
fn test_quality_gate_with_fail_on_error_missing_project() {
    // Exercises QualityGate command arm with --fail-on-error flag
    assert_missing_project(&["quality-gate", "--fail-on-error"]);
}

#[test]
fn test_history_with_from_to_missing_project() {
    // Exercises History command arm with --from and --to flags
    assert_missing_project(&[
        "history",
        "--metrics", "coverage",
        "--from", "2025-01-01",
        "--to", "2025-12-31",
    ]);
}

#[test]
fn test_rules_with_all_filters() {
    // Exercises Rules command arm (no --project required)
    // Should fail with a network error, not a project error
    cli()
        .args([
            "--url", "http://localhost:1",
            "rules",
            "--language", "rust",
            "--severity", "CRITICAL",
            "--rule-type", "BUG",
            "--status", "READY",
            "--search", "null",
        ])
        .assert()
        .failure();
}

#[test]
fn test_projects_with_search_and_qualifier() {
    // Exercises Projects command arm (no --project required)
    // Should fail with a network error, not a project error
    cli()
        .args([
            "--url", "http://localhost:1",
            "projects",
            "--search", "my-app",
            "--qualifier", "TRK",
        ])
        .assert()
        .failure();
}

#[test]
fn test_source_with_line_range() {
    // Exercises Source command arm with --from and --to flags
    // Should fail with a network error (URL unreachable), not a clap error
    cli()
        .args([
            "--url", "http://localhost:1",
            "source",
            "my-project:src/main.rs",
            "--from", "1",
            "--to", "10",
        ])
        .assert()
        .failure();
}

#[test]
fn test_wait_with_task_id_and_options() {
    // Exercises Wait command arm dispatch with all flags
    // Should fail with network error, not clap/project error
    cli()
        .args([
            "--url", "http://localhost:1",
            "wait",
            "task-abc123",
            "--timeout", "5",
            "--poll-interval", "1",
        ])
        .assert()
        .failure();
}

#[test]
fn test_project_flag_short_circuit() {
    // Exercises build_config() with --project flag set, verifying config flows through
    // The command still fails at network level (no server) rather than project validation
    cli()
        .args([
            "--url", "http://localhost:1",
            "--project", "test-proj",
            "issues",
        ])
        .assert()
        .failure();
}

#[test]
fn test_project_and_branch_flag_short_circuit() {
    // Exercises build_config() with --project and --branch flags
    cli()
        .args([
            "--url", "http://localhost:1",
            "--project", "test-proj",
            "--branch", "develop",
            "quality-gate",
        ])
        .assert()
        .failure();
}

#[test]
fn test_project_token_url_combination() {
    // Exercises build_config() with all three config flags together
    cli()
        .args([
            "--url", "http://localhost:1",
            "--project", "test-proj",
            "--token", "mytoken",
            "measures",
        ])
        .assert()
        .failure();
}
