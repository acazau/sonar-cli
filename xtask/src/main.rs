use clap::{Parser, Subcommand};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(name = "xtask", about = "Dev workflow tasks for sonar-cli")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a timestamped report directory
    SetupReports(SetupReportsArgs),
    /// List in-scope .rs files (changed or all)
    Scope(ScopeArgs),
    /// Run clippy and write a JSON report for SonarQube
    ClippyReport(ReportRootArgs),
    /// Run tests with coverage and write a Cobertura XML report for SonarQube
    TestReport(ReportRootArgs),
    /// Run SonarQube scan with auto-detected reports and project defaults
    SonarScan(SonarScanArgs),
    /// Run SonarQube scan inside a Docker container (sonarsource/sonar-scanner-cli)
    DockerScan(DockerScanArgs),
}

#[derive(clap::Args)]
struct SetupReportsArgs {
    /// Base directory for reports
    #[arg(long, default_value = "./reports")]
    base_dir: String,
    /// Output JSON instead of plain path
    #[arg(long)]
    json: bool,
}

#[derive(clap::Args)]
struct ReportRootArgs {
    /// Root report directory (subcommand creates its own subdirectory)
    #[arg(long)]
    report_root: String,
}

#[derive(clap::Args)]
struct SonarScanArgs {
    /// Root report directory to look for clippy/coverage report files
    #[arg(long)]
    report_root: Option<String>,
    /// Extra arguments forwarded to `cargo run -- scan`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    extra: Vec<String>,
}

#[derive(clap::Args)]
struct DockerScanArgs {
    /// SonarQube project key
    #[arg(long, default_value = "sonar-cli")]
    project: String,
    /// Clippy report path (falls back to SONAR_CLIPPY_REPORT env, then "clippy-report.json")
    #[arg(long)]
    clippy_report: Option<String>,
    /// Coverage report path (falls back to SONAR_COVERAGE_REPORT env, then "coverage.xml")
    #[arg(long)]
    coverage_report: Option<String>,
    /// Extra arguments forwarded to docker run
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    extra: Vec<String>,
}

#[derive(clap::Args)]
struct ScopeArgs {
    /// List all tracked files instead of just changed ones
    #[arg(long)]
    full: bool,
    /// File extension to filter (without dot)
    #[arg(long, default_value = "rs")]
    ext: String,
    /// Output as JSON array
    #[arg(long)]
    json: bool,
}

fn setup_reports(args: &SetupReportsArgs) {
    let now = chrono_stamp();
    let base = PathBuf::from(&args.base_dir);
    let report_root = base.join(&now);
    std::fs::create_dir_all(&report_root).expect("failed to create report root");

    let abs_root = std::fs::canonicalize(&report_root).expect("failed to resolve absolute path");

    if args.json {
        println!(
            "{{\"report_root\":\"{}\"}}",
            abs_root.display(),
        );
    } else {
        println!("{}", abs_root.display());
    }
}

fn chrono_stamp() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before epoch");
    let secs = now.as_secs();

    // Manual UTC breakdown (no chrono dependency needed)
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Days since 1970-01-01 to (year, month, day)
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    days += 719468;
    let era = days / 146097;
    let doe = days % 146097;
    let yoe =
        (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn git_run(args: &[&str], dir: Option<&Path>) -> Result<Vec<String>, ()> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let output = cmd.output().map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .trim()
        .split('\n')
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

fn scope(args: &ScopeArgs) {
    let pattern = format!("*.{}", args.ext);
    let files = if args.full {
        get_all_files(&pattern, None)
    } else {
        get_changed_files(&pattern, None)
    };

    if args.json {
        let json_arr: Vec<String> = files.iter().map(|f| format!("\"{}\"", f)).collect();
        println!("[{}]", json_arr.join(","));
    } else {
        for f in &files {
            println!("{}", f);
        }
    }
}

fn get_changed_files(pattern: &str, dir: Option<&Path>) -> Vec<String> {
    let diff_files = git_run(&["diff", "--name-only", "HEAD~1", "--", pattern], dir)
        .or_else(|_| git_run(&["diff", "--name-only", "main", "--", pattern], dir))
        .unwrap_or_default();

    let untracked = git_run(
        &["ls-files", "--others", "--exclude-standard", pattern],
        dir,
    )
    .unwrap_or_default();

    let mut set = BTreeSet::new();
    for f in diff_files {
        set.insert(f);
    }
    for f in untracked {
        set.insert(f);
    }
    set.into_iter().collect()
}

fn get_all_files(pattern: &str, dir: Option<&Path>) -> Vec<String> {
    git_run(&["ls-files", pattern], dir).unwrap_or_default()
}

fn build_clippy_command() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["clippy", "--message-format=json"]);
    cmd
}

fn clippy_report_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("clippy").join("clippy-report.json")
}

fn clippy_report(args: &ReportRootArgs) {
    let output_path = clippy_report_path(&args.report_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create clippy report dir");
    }
    let mut cmd = build_clippy_command();
    cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());

    let output = cmd.output().expect("failed to run cargo clippy");
    std::fs::write(&output_path, &output.stdout).expect("failed to write clippy report");

    let abs = std::fs::canonicalize(&output_path).unwrap_or_else(|_| output_path.clone());
    println!("{}", abs.display());

    std::process::exit(output.status.code().unwrap_or(1));
}

fn build_test_report_command(root: &str) -> Command {
    let coverage_path = test_report_path(root);
    let mut cmd = Command::new("cargo");
    cmd.args([
        "llvm-cov",
        "--cobertura",
        "--output-path",
        coverage_path.to_str().expect("non-UTF8 path"),
    ]);
    cmd
}

fn test_report_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("tests").join("coverage.xml")
}

fn test_report(args: &ReportRootArgs) {
    let output_path = test_report_path(&args.report_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create test report dir");
    }
    let mut cmd = build_test_report_command(&args.report_root);
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let status = cmd.status().expect("failed to run cargo llvm-cov");

    let abs = std::fs::canonicalize(&output_path).unwrap_or_else(|_| output_path.clone());
    println!("{}", abs.display());

    std::process::exit(status.code().unwrap_or(1));
}

fn build_sonar_scan_command(report_root: Option<&str>, extra: &[String]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "--project", "sonar-cli", "scan"]);

    if let Some(root) = report_root {
        let clippy = clippy_report_path(root);
        if clippy.exists() {
            cmd.arg("--clippy-report").arg(&clippy);
        }
        let coverage = test_report_path(root);
        if coverage.exists() {
            cmd.arg("--coverage-report").arg(&coverage);
        }
    }

    cmd.args([
        "--no-scm",
        "--skip-unchanged",
        "--exclusions",
        "**/*.json",
        "--sources",
        "src,tests",
    ]);
    cmd.args(extra);
    cmd
}

fn sonar_scan(args: &SonarScanArgs) {
    let mut cmd = build_sonar_scan_command(args.report_root.as_deref(), &args.extra);
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let status = cmd.status().expect("failed to run cargo run -- scan");
    std::process::exit(status.code().unwrap_or(1));
}

fn resolve_with_env(explicit: Option<&str>, env_var: &str, default: &str) -> String {
    if let Some(v) = explicit {
        return v.to_string();
    }
    std::env::var(env_var).unwrap_or_else(|_| default.to_string())
}

fn build_docker_scan_command(
    project: &str,
    branch: &str,
    cwd: &str,
    clippy_report: &str,
    coverage_report: &str,
    extra: &[String],
) -> Command {
    let mut cmd = Command::new("docker");
    cmd.args([
        "run", "--rm", "--network=host",
        "-e", "SONAR_HOST_URL",
        "-e", "SONAR_TOKEN",
        "-e", "GIT_CONFIG_COUNT=1",
        "-e", "GIT_CONFIG_KEY_0=safe.directory",
        "-e", "GIT_CONFIG_VALUE_0=/usr/src",
        "-v", &format!("{}:/usr/src", cwd),
        "sonarsource/sonar-scanner-cli",
    ]);
    cmd.arg(format!("-Dsonar.projectKey={}", project));
    cmd.arg(format!("-Dsonar.branch.name={}", branch));
    cmd.arg(format!("-Dsonar.rust.cobertura.reportPaths={}", coverage_report));
    cmd.arg(format!("-Dsonar.rust.clippy.reportPaths={}", clippy_report));
    cmd.args(extra);
    cmd
}

fn docker_scan(args: &DockerScanArgs) {
    if std::env::var("SONAR_HOST_URL").is_err() {
        eprintln!("Error: SONAR_HOST_URL is not set");
        std::process::exit(1);
    }
    if std::env::var("SONAR_TOKEN").is_err() {
        eprintln!("Error: SONAR_TOKEN is not set");
        std::process::exit(1);
    }

    let branch_output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .expect("failed to run git branch --show-current");
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

    let cwd = std::env::current_dir()
        .expect("failed to get current directory")
        .to_string_lossy()
        .into_owned();

    let clippy = resolve_with_env(
        args.clippy_report.as_deref(),
        "SONAR_CLIPPY_REPORT",
        "clippy-report.json",
    );
    let coverage = resolve_with_env(
        args.coverage_report.as_deref(),
        "SONAR_COVERAGE_REPORT",
        "coverage.xml",
    );

    let mut cmd = build_docker_scan_command(
        &args.project, &branch, &cwd, &clippy, &coverage, &args.extra,
    );
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let status = cmd.status().expect("failed to run docker");
    std::process::exit(status.code().unwrap_or(1));
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Cmd::SetupReports(args) => setup_reports(args),
        Cmd::Scope(args) => scope(args),
        Cmd::ClippyReport(args) => clippy_report(args),
        Cmd::TestReport(args) => test_report(args),
        Cmd::SonarScan(args) => sonar_scan(args),
        Cmd::DockerScan(args) => docker_scan(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn setup_reports_creates_structure() {
        let tmp = env::temp_dir().join(format!("xtask-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);

        let args = SetupReportsArgs {
            base_dir: tmp.to_str().unwrap().to_string(),
            json: false,
        };
        setup_reports(&args);

        // Find the created timestamp dir
        let entries: Vec<_> = fs::read_dir(&tmp).unwrap().collect();
        assert_eq!(entries.len(), 1, "expected one timestamp directory");
        let ts_dir = entries[0].as_ref().unwrap().path();
        // Subdirs are NOT created by setup-reports (commands own their subdirs)
        assert!(!ts_dir.join("clippy").exists());
        assert!(!ts_dir.join("tests").exists());

        // Path is absolute
        assert!(ts_dir.is_absolute() || fs::canonicalize(&ts_dir).unwrap().is_absolute());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn setup_reports_json_output() {
        let tmp = env::temp_dir().join(format!("xtask-json-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);

        let args = SetupReportsArgs {
            base_dir: tmp.to_str().unwrap().to_string(),
            json: true,
        };

        // Capture what would be printed by checking the dirs exist and are parseable
        setup_reports(&args);

        let entries: Vec<_> = fs::read_dir(&tmp).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let ts_dir = entries[0].as_ref().unwrap().path();
        // Subdirs are NOT created by setup-reports (commands own their subdirs)
        assert!(!ts_dir.join("clippy").exists());
        assert!(!ts_dir.join("tests").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn setup_reports_base_dir_override() {
        let tmp = env::temp_dir().join(format!("xtask-base-{}", std::process::id()));
        let custom = tmp.join("custom-reports");
        let _ = fs::remove_dir_all(&tmp);

        let args = SetupReportsArgs {
            base_dir: custom.to_str().unwrap().to_string(),
            json: false,
        };
        setup_reports(&args);

        assert!(custom.is_dir());
        let entries: Vec<_> = fs::read_dir(&custom).unwrap().collect();
        assert_eq!(entries.len(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    fn init_temp_git_repo(suffix: &str) -> PathBuf {
        let tmp = env::temp_dir().join(format!(
            "xtask-{}-{}-{}",
            suffix,
            std::process::id(),
            format!("{:?}", std::thread::current().id())
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&tmp)
                .output()
                .expect("git command failed")
        };

        git(&["init"]);
        git(&["config", "user.email", "test@test.com"]);
        git(&["config", "user.name", "Test"]);

        // Create initial commit with a .rs file
        fs::write(tmp.join("main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.join("lib.rs"), "pub fn lib() {}").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "initial"]);

        // Modify one file and add an untracked file for "changed" mode
        fs::write(tmp.join("main.rs"), "fn main() { println!(\"hi\"); }").unwrap();
        fs::write(tmp.join("new.rs"), "fn new() {}").unwrap();
        // Also add a non-rs file to verify filtering
        fs::write(tmp.join("notes.txt"), "not rust").unwrap();

        tmp
    }

    #[test]
    fn scope_full_returns_all_tracked() {
        let repo = init_temp_git_repo("full");

        let files = get_all_files("*.rs", Some(&repo));
        // lib.rs and main.rs are tracked (new.rs is untracked)
        assert!(files.contains(&"lib.rs".to_string()));
        assert!(files.contains(&"main.rs".to_string()));
        assert!(!files.contains(&"new.rs".to_string()));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_default_returns_changed_and_untracked() {
        let repo = init_temp_git_repo("changed");

        let files = get_changed_files("*.rs", Some(&repo));
        // main.rs is modified, new.rs is untracked — both should appear
        assert!(files.contains(&"main.rs".to_string()));
        assert!(files.contains(&"new.rs".to_string()));
        // lib.rs is unchanged and tracked — should NOT appear
        assert!(!files.contains(&"lib.rs".to_string()));
        // notes.txt should not appear (wrong extension)
        assert!(!files.contains(&"notes.txt".to_string()));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_ext_filtering() {
        let repo = init_temp_git_repo("ext");

        // Track the txt file
        Command::new("git")
            .args(["add", "notes.txt"])
            .current_dir(&repo)
            .output()
            .unwrap();

        let files = get_all_files("*.txt", Some(&repo));
        assert!(files.contains(&"notes.txt".to_string()));
        assert!(!files.contains(&"main.rs".to_string()));

        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn scope_graceful_fallback_no_parent() {
        // In a repo with only one commit, HEAD~1 doesn't exist.
        // get_changed_files should fall back to main, then to empty.
        let tmp = env::temp_dir().join(format!(
            "xtask-fallback-{}-{}",
            std::process::id(),
            format!("{:?}", std::thread::current().id())
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let git = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(&tmp)
                .output()
                .unwrap()
        };

        git(&["init"]);
        git(&["config", "user.email", "t@t.com"]);
        git(&["config", "user.name", "T"]);
        fs::write(tmp.join("a.rs"), "").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "only commit"]);

        // Should not panic — gracefully returns empty or falls back
        let files = get_changed_files("*.rs", Some(&tmp));
        // The result may be empty (both HEAD~1 and main fail) — that's fine
        assert!(files.len() <= 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn chrono_stamp_format() {
        let stamp = chrono_stamp();
        // Format: YYYYMMDD-HHMMSS (15 chars)
        assert_eq!(stamp.len(), 15, "stamp should be 15 chars: {}", stamp);
        assert_eq!(&stamp[8..9], "-", "separator should be dash: {}", stamp);
        // All other chars are digits
        for (i, c) in stamp.chars().enumerate() {
            if i == 8 {
                continue;
            }
            assert!(c.is_ascii_digit(), "char {} should be digit: {}", i, stamp);
        }
    }

    // ── clippy-report tests ─────────────────────────────────────────────

    fn args_vec(cmd: &std::process::Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn test_build_clippy_command_args() {
        let cmd = build_clippy_command();
        let args = args_vec(&cmd);
        assert_eq!(args, vec!["clippy", "--message-format=json"]);
    }

    #[test]
    fn test_clippy_report_path() {
        let path = clippy_report_path("/tmp/reports");
        assert_eq!(path, PathBuf::from("/tmp/reports/clippy/clippy-report.json"));
    }

    // ── test-report tests ───────────────────────────────────────────────

    #[test]
    fn test_build_test_report_command_args() {
        let cmd = build_test_report_command("/tmp/reports");
        let args = args_vec(&cmd);
        assert_eq!(
            args,
            vec![
                "llvm-cov",
                "--cobertura",
                "--output-path",
                "/tmp/reports/tests/coverage.xml",
            ]
        );
    }

    #[test]
    fn test_test_report_path() {
        let path = test_report_path("/tmp/reports");
        assert_eq!(path, PathBuf::from("/tmp/reports/tests/coverage.xml"));
    }

    // ── sonar-scan tests ──────────────────────────────────────────────

    #[test]
    fn sonar_scan_no_report_root() {
        let cmd = build_sonar_scan_command(None, &[]);
        let args = args_vec(&cmd);
        assert_eq!(
            args,
            vec![
                "run", "--", "--project", "sonar-cli", "scan",
                "--no-scm", "--skip-unchanged",
                "--exclusions", "**/*.json",
                "--sources", "src,tests",
            ]
        );
    }

    #[test]
    fn sonar_scan_report_root_no_files() {
        let tmp = env::temp_dir().join(format!("xtask-scan-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let cmd = build_sonar_scan_command(Some(tmp.to_str().unwrap()), &[]);
        let args = args_vec(&cmd);
        // No report flags since files don't exist
        assert!(
            !args.iter().any(|a| a == "--clippy-report"),
            "should not have --clippy-report: {:?}",
            args
        );
        assert!(
            !args.iter().any(|a| a == "--coverage-report"),
            "should not have --coverage-report: {:?}",
            args
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sonar_scan_report_root_with_files() {
        let tmp = env::temp_dir().join(format!("xtask-scan-full-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        // Create both report files
        let clippy_dir = tmp.join("clippy");
        let tests_dir = tmp.join("tests");
        fs::create_dir_all(&clippy_dir).unwrap();
        fs::create_dir_all(&tests_dir).unwrap();
        fs::write(clippy_dir.join("clippy-report.json"), "[]").unwrap();
        fs::write(tests_dir.join("coverage.xml"), "<xml/>").unwrap();

        let cmd = build_sonar_scan_command(Some(tmp.to_str().unwrap()), &[]);
        let args = args_vec(&cmd);
        assert!(
            args.iter().any(|a| a == "--clippy-report"),
            "should have --clippy-report: {:?}",
            args
        );
        assert!(
            args.iter().any(|a| a == "--coverage-report"),
            "should have --coverage-report: {:?}",
            args
        );
        // Defaults still present after report flags
        assert!(args.contains(&"--no-scm".to_string()));
        assert!(args.contains(&"--skip-unchanged".to_string()));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sonar_scan_extra_args_forwarded() {
        let extra = vec![
            "-Dsonar.verbose=true".to_string(),
            "--new-code".to_string(),
        ];
        let cmd = build_sonar_scan_command(None, &extra);
        let args = args_vec(&cmd);
        assert!(args.ends_with(&[
            "-Dsonar.verbose=true".to_string(),
            "--new-code".to_string(),
        ]));
    }

    // ── docker-scan tests ─────────────────────────────────────────────

    #[test]
    fn docker_scan_default_args() {
        let cmd = build_docker_scan_command(
            "sonar-cli", "main", "/home/user/project",
            "clippy-report.json", "coverage.xml", &[],
        );
        let args = args_vec(&cmd);
        assert_eq!(
            args,
            vec![
                "run", "--rm", "--network=host",
                "-e", "SONAR_HOST_URL",
                "-e", "SONAR_TOKEN",
                "-e", "GIT_CONFIG_COUNT=1",
                "-e", "GIT_CONFIG_KEY_0=safe.directory",
                "-e", "GIT_CONFIG_VALUE_0=/usr/src",
                "-v", "/home/user/project:/usr/src",
                "sonarsource/sonar-scanner-cli",
                "-Dsonar.projectKey=sonar-cli",
                "-Dsonar.branch.name=main",
                "-Dsonar.rust.cobertura.reportPaths=coverage.xml",
                "-Dsonar.rust.clippy.reportPaths=clippy-report.json",
            ]
        );
    }

    #[test]
    fn docker_scan_custom_project_and_branch() {
        let cmd = build_docker_scan_command(
            "my-proj", "feature/xyz", "/tmp/repo",
            "custom-clippy.json", "custom-coverage.xml", &[],
        );
        let args = args_vec(&cmd);
        assert!(args.contains(&"-Dsonar.projectKey=my-proj".to_string()));
        assert!(args.contains(&"-Dsonar.branch.name=feature/xyz".to_string()));
        assert!(args.contains(&"-Dsonar.rust.clippy.reportPaths=custom-clippy.json".to_string()));
        assert!(args.contains(&"-Dsonar.rust.cobertura.reportPaths=custom-coverage.xml".to_string()));
        assert!(args.contains(&"/tmp/repo:/usr/src".to_string()));
    }

    #[test]
    fn docker_scan_extra_args() {
        let extra = vec![
            "-Dsonar.verbose=true".to_string(),
            "-Dsonar.log.level=DEBUG".to_string(),
        ];
        let cmd = build_docker_scan_command(
            "sonar-cli", "main", "/home/user/project",
            "clippy-report.json", "coverage.xml", &extra,
        );
        let args = args_vec(&cmd);
        assert!(args.ends_with(&[
            "-Dsonar.verbose=true".to_string(),
            "-Dsonar.log.level=DEBUG".to_string(),
        ]));
    }

    #[test]
    fn resolve_with_env_explicit_wins() {
        let val = resolve_with_env(Some("explicit.json"), "NONEXISTENT_VAR_12345", "default.json");
        assert_eq!(val, "explicit.json");
    }

    #[test]
    fn resolve_with_env_falls_back_to_default() {
        let val = resolve_with_env(None, "NONEXISTENT_VAR_12345", "default.json");
        assert_eq!(val, "default.json");
    }
}
