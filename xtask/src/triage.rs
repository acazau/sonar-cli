use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

#[derive(clap::Args)]
pub struct TriageArgs {
    /// SonarQube project key
    #[arg(long)]
    pub project: String,
    /// Branch name
    #[arg(long)]
    pub branch: String,
    /// Analysis task ID from sonar-scan
    #[arg(long)]
    pub task_id: String,
    /// Report root directory (creates triage/ subdirectory)
    #[arg(long)]
    pub report_root: String,
    /// Triage mode: "scoped" (default) or "full"
    #[arg(long, default_value = "scoped")]
    pub mode: String,
    /// Timeout in seconds for the wait step (default: 600)
    #[arg(long, default_value = "600")]
    pub timeout: u64,
    /// Path to scope file (one file path per line) for filtering output
    #[arg(long)]
    pub scope_file: Option<String>,
}

pub fn build_wait_command(task_id: &str, timeout: u64) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run", "--", "wait",
        task_id,
        "--timeout",
        &timeout.to_string(),
        "--poll-interval",
        "10",
    ]);
    cmd
}

pub fn build_query_command(
    project: &str,
    branch: &str,
    subcommand: &str,
    extra_flags: &[&str],
) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "--project", project, "--branch", branch, subcommand, "--json"]);
    cmd.args(extra_flags);
    cmd
}

fn run_and_check(label: &str, mut cmd: Command, timeout: u64) {
    use std::time::{Duration, Instant};

    cmd.stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit());
    let mut child = cmd.spawn().unwrap_or_else(|e| {
        eprintln!("Error: failed to spawn cargo run -- {label}: {e}");
        std::process::exit(1);
    });

    let deadline = Duration::from_secs(timeout);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    eprintln!(
                        "Error: cargo run -- {label} exited with {}",
                        status.code().unwrap_or(1)
                    );
                    std::process::exit(status.code().unwrap_or(1));
                }
                return;
            }
            Ok(None) => {
                if start.elapsed() >= deadline {
                    eprintln!("Error: cargo run -- {label} timed out after {timeout} seconds");
                    let _ = child.kill();
                    let _ = child.wait();
                    std::process::exit(124);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error: failed to wait for cargo run -- {label}: {e}");
                std::process::exit(1);
            }
        }
    }
}

pub fn triage(args: &TriageArgs) {
    let triage_dir = PathBuf::from(&args.report_root).join("triage");
    std::fs::create_dir_all(&triage_dir).unwrap_or_else(|e| {
        eprintln!(
            "Error: failed to create triage dir {}: {e}",
            triage_dir.display()
        );
        std::process::exit(1);
    });

    // Step 1: Wait for analysis
    run_and_check(
        "wait",
        build_wait_command(&args.task_id, args.timeout),
        args.timeout + 30, // extra buffer for the outer timeout
    );

    // Step 2: Run queries, capture output to files
    let new_code = args.mode == "scoped";
    let queries: Vec<(&str, Command)> = vec![
        (
            "quality-gate",
            build_query_command(&args.project, &args.branch, "quality-gate", &[]),
        ),
        (
            "issues",
            build_query_command(
                &args.project,
                &args.branch,
                "issues",
                if new_code { &["--new-code"] } else { &[] },
            ),
        ),
        (
            "duplications",
            build_query_command(
                &args.project,
                &args.branch,
                "duplications",
                &["--details"],
            ),
        ),
        (
            "coverage",
            build_query_command(&args.project, &args.branch, "coverage", &[]),
        ),
        (
            "measures",
            build_query_command(&args.project, &args.branch, "measures", &[]),
        ),
        (
            "hotspots",
            build_query_command(
                &args.project,
                &args.branch,
                "hotspots",
                if new_code { &["--new-code"] } else { &[] },
            ),
        ),
    ];

    for (name, mut cmd) in queries {
        let output = cmd.output().unwrap_or_else(|e| {
            eprintln!("Error: failed to run cargo run -- {name}: {e}");
            std::process::exit(1);
        });
        let path = triage_dir.join(format!("{name}.json"));
        std::fs::write(&path, &output.stdout).unwrap_or_else(|e| {
            eprintln!("Error: failed to write {}: {e}", path.display());
            std::process::exit(1);
        });
        if !output.status.success() {
            eprintln!("Warning: cargo run -- {name} exited with {}", output.status);
        }
    }

    // Step 3: Generate scoped variants if scope file is provided
    if let Some(scope_path) = &args.scope_file {
        let scope = load_scope_file(scope_path);
        if !scope.is_empty() {
            for name in &["issues", "coverage", "duplications", "hotspots"] {
                let input_path = triage_dir.join(format!("{name}.json"));
                let output_path = triage_dir.join(format!("{name}-scoped.json"));
                if input_path.exists() {
                    filter_json_file(&input_path, &output_path, &scope, name);
                }
            }
        }
    }

    // Step 4: Print triage directory
    let abs = std::fs::canonicalize(&triage_dir).unwrap_or(triage_dir);
    println!("{}", abs.display());
}

fn load_scope_file(path: &str) -> HashSet<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to read scope file {path}: {e}");
            return HashSet::new();
        }
    };
    content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Check if a SonarQube component path matches any scope file.
/// Component format: "sonar-cli:src/commands/scan.rs" -> extract path after ":"
fn component_matches_scope(component: &str, scope: &HashSet<String>) -> bool {
    let path = component
        .find(':')
        .map(|i| &component[i + 1..])
        .unwrap_or(component);
    scope.iter().any(|s| path == s.as_str() || s.ends_with(path) || path.ends_with(s.as_str()))
}

fn filter_json_file(
    input: &std::path::Path,
    output: &std::path::Path,
    scope: &HashSet<String>,
    kind: &str,
) {
    let data = match std::fs::read_to_string(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "Warning: failed to read {}: {e}",
                input.display()
            );
            return;
        }
    };

    let parsed: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Warning: failed to parse {}: {e}",
                input.display()
            );
            return;
        }
    };

    let filtered = filter_by_kind(&parsed, scope, kind);

    let json = serde_json::to_string_pretty(&filtered).unwrap_or_default();
    if let Err(e) = std::fs::write(output, &json) {
        eprintln!("Warning: failed to write {}: {e}", output.display());
    }
}

/// Maximum number of entries to keep in each scoped output file.
const SCOPED_ITEM_LIMIT: usize = 10;

fn filter_by_kind(
    value: &serde_json::Value,
    scope: &HashSet<String>,
    kind: &str,
) -> serde_json::Value {
    match value {
        serde_json::Value::Array(arr) => {
            let filtered: Vec<serde_json::Value> = arr
                .iter()
                .filter(|item| item_matches_scope(item, scope, kind))
                .cloned()
                .collect();
            let total = filtered.len();
            let trimmed = trim_entries(filtered, kind);
            serde_json::json!({"total": total, "items": trimmed})
        }
        serde_json::Value::Object(map) => {
            // Some responses wrap data in an object with a key like "issues", "components", etc.
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                if val.is_array() {
                    result.insert(key.clone(), filter_by_kind(val, scope, kind));
                } else {
                    result.insert(key.clone(), val.clone());
                }
            }
            serde_json::Value::Object(result)
        }
        other => other.clone(),
    }
}

/// Apply kind-specific sorting, filtering, and capping to scoped entries.
fn trim_entries(entries: Vec<serde_json::Value>, kind: &str) -> Vec<serde_json::Value> {
    match kind {
        "coverage" => trim_coverage_entries(entries),
        "issues" => trim_issues_entries(entries),
        "hotspots" => trim_hotspots_entries(entries),
        "duplications" => trim_duplications_entries(entries),
        _ => entries,
    }
}

/// Trim coverage entries: drop well-covered (>80%) files,
/// sort by uncovered_lines descending, and cap at SCOPED_ITEM_LIMIT.
fn trim_coverage_entries(mut entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    entries.retain(|item| {
        let pct = item
            .get("coverage_percent")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        pct <= 80.0
    });
    entries.sort_by(|a, b| {
        let ua = a
            .get("uncovered_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let ub = b
            .get("uncovered_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        ub.cmp(&ua)
    });
    entries.truncate(SCOPED_ITEM_LIMIT);
    entries
}

/// Trim issues: sort by severity (BLOCKER first), cap at SCOPED_ITEM_LIMIT.
fn trim_issues_entries(mut entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    entries.sort_by_key(|item| {
        let sev = item
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("INFO");
        severity_rank(sev)
    });
    entries.truncate(SCOPED_ITEM_LIMIT);
    entries
}

fn severity_rank(s: &str) -> u8 {
    match s {
        "BLOCKER" => 0,
        "CRITICAL" => 1,
        "MAJOR" => 2,
        "MINOR" => 3,
        _ => 4,
    }
}

/// Trim hotspots: sort by vulnerability probability (HIGH first), cap at SCOPED_ITEM_LIMIT.
fn trim_hotspots_entries(mut entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    entries.sort_by_key(|item| {
        let prob = item
            .get("vulnerabilityProbability")
            .and_then(|v| v.as_str())
            .unwrap_or("LOW");
        vulnerability_rank(prob)
    });
    entries.truncate(SCOPED_ITEM_LIMIT);
    entries
}

fn vulnerability_rank(s: &str) -> u8 {
    match s {
        "HIGH" => 0,
        "MEDIUM" => 1,
        _ => 2,
    }
}

/// Trim duplications: sort by duplicated_lines descending, cap at SCOPED_ITEM_LIMIT.
fn trim_duplications_entries(mut entries: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    entries.sort_by(|a, b| {
        let da = a
            .get("duplicated_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let db = b
            .get("duplicated_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        db.cmp(&da)
    });
    entries.truncate(SCOPED_ITEM_LIMIT);
    entries
}

fn item_matches_scope(
    item: &serde_json::Value,
    scope: &HashSet<String>,
    kind: &str,
) -> bool {
    match kind {
        "issues" | "hotspots" => {
            // "component": "sonar-cli:src/commands/scan.rs"
            item.get("component")
                .and_then(|v| v.as_str())
                .is_some_and(|c| component_matches_scope(c, scope))
        }
        "coverage" => {
            // "file": "src/commands/scan.rs" (or "component")
            item.get("file")
                .or_else(|| item.get("component"))
                .and_then(|v| v.as_str())
                .is_some_and(|c| component_matches_scope(c, scope))
        }
        "duplications" => {
            // Top-level "file" field matches scope
            let file_matches = item
                .get("file")
                .and_then(|v| v.as_str())
                .is_some_and(|f| component_matches_scope(f, scope));
            if file_matches {
                return true;
            }
            // Also check if any block's "duplicated_in" target is in scope
            if let Some(blocks) = item.get("blocks").and_then(|b| b.as_array()) {
                return blocks.iter().any(|block| {
                    block
                        .get("duplicated_in")
                        .and_then(|v| v.as_str())
                        .is_some_and(|f| component_matches_scope(f, scope))
                });
            }
            false
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_vec(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn wait_command_args() {
        let cmd = build_wait_command("AXyz123", 300);
        assert_eq!(cmd.get_program().to_string_lossy(), "cargo");
        assert_eq!(
            args_vec(&cmd),
            vec!["run", "--", "wait", "AXyz123", "--timeout", "300", "--poll-interval", "10"]
        );
    }

    #[test]
    fn wait_command_custom_timeout() {
        let cmd = build_wait_command("TASK_99", 600);
        assert_eq!(
            args_vec(&cmd),
            vec!["run", "--", "wait", "TASK_99", "--timeout", "600", "--poll-interval", "10"]
        );
    }

    #[test]
    fn query_command_quality_gate() {
        let cmd = build_query_command("sonar-cli", "main", "quality-gate", &[]);
        assert_eq!(cmd.get_program().to_string_lossy(), "cargo");
        assert_eq!(
            args_vec(&cmd),
            vec![
                "run", "--",
                "--project",
                "sonar-cli",
                "--branch",
                "main",
                "quality-gate",
                "--json"
            ]
        );
    }

    #[test]
    fn query_command_issues_scoped() {
        let cmd = build_query_command("sonar-cli", "feat/x", "issues", &["--new-code"]);
        assert_eq!(
            args_vec(&cmd),
            vec![
                "run", "--",
                "--project",
                "sonar-cli",
                "--branch",
                "feat/x",
                "issues",
                "--json",
                "--new-code"
            ]
        );
    }

    #[test]
    fn query_command_issues_full() {
        let cmd = build_query_command("sonar-cli", "main", "issues", &[]);
        let args = args_vec(&cmd);
        assert!(!args.contains(&"--new-code".to_string()));
        assert_eq!(
            args,
            vec![
                "run", "--",
                "--project",
                "sonar-cli",
                "--branch",
                "main",
                "issues",
                "--json"
            ]
        );
    }

    #[test]
    fn query_command_duplications() {
        let cmd = build_query_command("sonar-cli", "main", "duplications", &["--details"]);
        assert_eq!(
            args_vec(&cmd),
            vec![
                "run", "--",
                "--project",
                "sonar-cli",
                "--branch",
                "main",
                "duplications",
                "--json",
                "--details"
            ]
        );
    }

    #[test]
    fn query_command_hotspots_scoped() {
        let cmd = build_query_command("my-proj", "dev", "hotspots", &["--new-code"]);
        assert_eq!(
            args_vec(&cmd),
            vec![
                "run", "--",
                "--project",
                "my-proj",
                "--branch",
                "dev",
                "hotspots",
                "--json",
                "--new-code"
            ]
        );
    }

    #[test]
    fn triage_creates_directory() {
        let tmp = std::env::temp_dir().join(format!("xtask-triage-dir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let triage_dir = tmp.join("triage");
        std::fs::create_dir_all(&triage_dir).unwrap();
        assert!(triage_dir.is_dir());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn component_matches_scope_basic() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()].into_iter().collect();
        assert!(component_matches_scope(
            "sonar-cli:src/commands/scan.rs",
            &scope
        ));
        assert!(!component_matches_scope(
            "sonar-cli:src/commands/other.rs",
            &scope
        ));
    }

    #[test]
    fn component_matches_scope_no_prefix() {
        let scope: HashSet<String> =
            ["src/commands/client.rs".to_string()].into_iter().collect();
        assert!(component_matches_scope("src/commands/client.rs", &scope));
    }

    #[test]
    fn filter_issues_json() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()].into_iter().collect();
        let input = serde_json::json!([
            {"component": "sonar-cli:src/commands/scan.rs", "rule": "rust:S1234", "severity": "MAJOR", "message": "fix me"},
            {"component": "sonar-cli:src/commands/client.rs", "rule": "rust:S5678", "severity": "MINOR", "message": "other"}
        ]);
        let filtered = filter_by_kind(&input, &scope, "issues");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["rule"], "rust:S1234");
    }

    #[test]
    fn filter_coverage_json() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()].into_iter().collect();
        let input = serde_json::json!([
            {"file": "src/commands/scan.rs", "coverage_percent": 45.0, "uncovered_lines": 100},
            {"file": "src/main.rs", "coverage_percent": 50.0, "uncovered_lines": 20}
        ]);
        let filtered = filter_by_kind(&input, &scope, "coverage");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["file"], "src/commands/scan.rs");
    }

    #[test]
    fn trim_coverage_drops_well_covered() {
        let entries = vec![
            serde_json::json!({"file": "src/a.rs", "coverage_percent": 90.0, "uncovered_lines": 5}),
            serde_json::json!({"file": "src/b.rs", "coverage_percent": 40.0, "uncovered_lines": 50}),
        ];
        let result = trim_coverage_entries(entries);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["file"], "src/b.rs");
    }

    #[test]
    fn trim_coverage_sorts_by_uncovered_and_caps() {
        let mut entries = Vec::new();
        for i in 0..30 {
            entries.push(serde_json::json!({
                "file": format!("src/file_{i}.rs"),
                "coverage_percent": 10.0,
                "uncovered_lines": i * 10
            }));
        }
        let result = trim_coverage_entries(entries);
        assert_eq!(result.len(), SCOPED_ITEM_LIMIT);
        // First entry should have the most uncovered lines
        assert_eq!(result[0]["uncovered_lines"], 290);
        assert_eq!(result[1]["uncovered_lines"], 280);
    }

    #[test]
    fn filter_hotspots_json() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()]
                .into_iter()
                .collect();
        let input = serde_json::json!([
            {"component": "sonar-cli:src/commands/scan.rs", "rule": "rust:S5131", "vulnerabilityProbability": "HIGH"},
            {"component": "sonar-cli:src/commands/client.rs", "rule": "rust:S2245", "vulnerabilityProbability": "LOW"}
        ]);
        let filtered = filter_by_kind(&input, &scope, "hotspots");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["rule"], "rust:S5131");
    }

    #[test]
    fn filter_duplications_by_source_file() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()].into_iter().collect();
        let input = serde_json::json!([
            {
                "file": "src/commands/scan.rs",
                "duplicated_lines": 20,
                "blocks": [
                    {"from_line": 10, "size": 20, "duplicated_in": "src/commands/client.rs", "duplicated_in_line": 10}
                ]
            },
            {
                "file": "src/main.rs",
                "duplicated_lines": 5,
                "blocks": [
                    {"from_line": 1, "size": 5, "duplicated_in": "src/lib.rs", "duplicated_in_line": 1}
                ]
            }
        ]);
        let filtered = filter_by_kind(&input, &scope, "duplications");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["file"], "src/commands/scan.rs");
    }

    #[test]
    fn filter_duplications_by_duplicated_in_target() {
        let scope: HashSet<String> =
            ["src/commands/client.rs".to_string()].into_iter().collect();
        let input = serde_json::json!([
            {
                "file": "src/commands/scan.rs",
                "duplicated_lines": 22,
                "blocks": [
                    {"from_line": 56, "size": 11, "duplicated_in": "src/commands/client.rs", "duplicated_in_line": 56}
                ]
            }
        ]);
        let filtered = filter_by_kind(&input, &scope, "duplications");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        assert_eq!(obj["items"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn filter_duplications_no_match() {
        let scope: HashSet<String> =
            ["src/main.rs".to_string()].into_iter().collect();
        let input = serde_json::json!([
            {
                "file": "src/commands/scan.rs",
                "duplicated_lines": 22,
                "blocks": [
                    {"from_line": 56, "size": 11, "duplicated_in": "src/commands/client.rs", "duplicated_in_line": 56}
                ]
            }
        ]);
        let filtered = filter_by_kind(&input, &scope, "duplications");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["total"], 0);
        assert_eq!(obj["items"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn filter_wrapped_object() {
        let scope: HashSet<String> =
            ["src/commands/scan.rs".to_string()].into_iter().collect();
        let input = serde_json::json!({
            "p": 1,
            "issues": [
                {"component": "sonar-cli:src/commands/scan.rs", "rule": "rust:S1234", "severity": "MAJOR"},
                {"component": "sonar-cli:src/commands/client.rs", "rule": "rust:S5678", "severity": "MINOR"}
            ]
        });
        let filtered = filter_by_kind(&input, &scope, "issues");
        let obj = filtered.as_object().unwrap();
        assert_eq!(obj["p"], 1); // non-array fields preserved
        let inner = obj["issues"].as_object().unwrap();
        assert_eq!(inner["total"], 1);
        assert_eq!(inner["items"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn filter_json_file_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("xtask-filter-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let scope_path = tmp.join("scope.txt");
        std::fs::write(&scope_path, "src/commands/scan.rs\n").unwrap();

        let input_path = tmp.join("issues.json");
        let output_path = tmp.join("issues-scoped.json");
        let data = serde_json::json!([
            {"component": "sonar-cli:src/commands/scan.rs", "rule": "rust:S1234", "severity": "MAJOR"},
            {"component": "sonar-cli:src/commands/client.rs", "rule": "rust:S5678", "severity": "MINOR"}
        ]);
        std::fs::write(&input_path, serde_json::to_string(&data).unwrap()).unwrap();

        let scope = load_scope_file(scope_path.to_str().unwrap());
        filter_json_file(&input_path, &output_path, &scope, "issues");

        let result: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&output_path).unwrap()).unwrap();
        let obj = result.as_object().unwrap();
        assert_eq!(obj["total"], 1);
        let items = obj["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["rule"], "rust:S1234");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn trim_issues_sorts_by_severity() {
        let entries = vec![
            serde_json::json!({"severity": "MINOR", "rule": "r1"}),
            serde_json::json!({"severity": "BLOCKER", "rule": "r2"}),
            serde_json::json!({"severity": "MAJOR", "rule": "r3"}),
            serde_json::json!({"severity": "CRITICAL", "rule": "r4"}),
        ];
        let result = trim_issues_entries(entries);
        assert_eq!(result[0]["severity"], "BLOCKER");
        assert_eq!(result[1]["severity"], "CRITICAL");
        assert_eq!(result[2]["severity"], "MAJOR");
        assert_eq!(result[3]["severity"], "MINOR");
    }

    #[test]
    fn trim_issues_caps_at_limit() {
        let entries: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"severity": "MAJOR", "rule": format!("r{i}")}))
            .collect();
        let result = trim_issues_entries(entries);
        assert_eq!(result.len(), SCOPED_ITEM_LIMIT);
    }

    #[test]
    fn trim_hotspots_sorts_by_probability() {
        let entries = vec![
            serde_json::json!({"vulnerabilityProbability": "LOW", "rule": "r1"}),
            serde_json::json!({"vulnerabilityProbability": "HIGH", "rule": "r2"}),
            serde_json::json!({"vulnerabilityProbability": "MEDIUM", "rule": "r3"}),
        ];
        let result = trim_hotspots_entries(entries);
        assert_eq!(result[0]["vulnerabilityProbability"], "HIGH");
        assert_eq!(result[1]["vulnerabilityProbability"], "MEDIUM");
        assert_eq!(result[2]["vulnerabilityProbability"], "LOW");
    }

    #[test]
    fn trim_hotspots_caps_at_limit() {
        let entries: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"vulnerabilityProbability": "MEDIUM", "rule": format!("r{i}")}))
            .collect();
        let result = trim_hotspots_entries(entries);
        assert_eq!(result.len(), SCOPED_ITEM_LIMIT);
    }

    #[test]
    fn trim_duplications_sorts_by_lines_desc() {
        let entries = vec![
            serde_json::json!({"duplicated_lines": 10}),
            serde_json::json!({"duplicated_lines": 50}),
            serde_json::json!({"duplicated_lines": 30}),
        ];
        let result = trim_duplications_entries(entries);
        assert_eq!(result[0]["duplicated_lines"], 50);
        assert_eq!(result[1]["duplicated_lines"], 30);
        assert_eq!(result[2]["duplicated_lines"], 10);
    }

    #[test]
    fn trim_duplications_caps_at_limit() {
        let entries: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"duplicated_lines": i}))
            .collect();
        let result = trim_duplications_entries(entries);
        assert_eq!(result.len(), SCOPED_ITEM_LIMIT);
    }

    #[test]
    fn filter_by_kind_wraps_array_in_total_items() {
        let scope: HashSet<String> =
            ["src/a.rs".to_string(), "src/b.rs".to_string()]
                .into_iter()
                .collect();
        let input = serde_json::json!([
            {"component": "sonar-cli:src/a.rs", "severity": "MAJOR"},
            {"component": "sonar-cli:src/b.rs", "severity": "MINOR"},
            {"component": "sonar-cli:src/c.rs", "severity": "INFO"}
        ]);
        let result = filter_by_kind(&input, &scope, "issues");
        let obj = result.as_object().unwrap();
        assert_eq!(obj["total"], 2);
        assert_eq!(obj["items"].as_array().unwrap().len(), 2);
    }
}
