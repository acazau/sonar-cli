use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(clap::Args)]
pub struct SonarScanArgs {
    /// Root report directory to look for clippy/coverage report files
    #[arg(long)]
    pub report_root: Option<String>,
    /// Timeout in seconds (default: 600). Exit code 124 on timeout.
    #[arg(long, default_value = "600")]
    pub timeout: u64,
    /// Extra arguments forwarded to `cargo run -- scan`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra: Vec<String>,
}

#[derive(clap::Args)]
pub struct DockerScanArgs {
    /// SonarQube project key
    #[arg(long, default_value = "sonar-cli")]
    pub project: String,
    /// Clippy report path (falls back to SONAR_CLIPPY_REPORT env, then "clippy-report.json")
    #[arg(long)]
    pub clippy_report: Option<String>,
    /// Coverage report path (falls back to SONAR_COVERAGE_REPORT env, then "coverage.xml")
    #[arg(long)]
    pub coverage_report: Option<String>,
    /// Extra arguments forwarded to docker run
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra: Vec<String>,
}

pub fn resolve_with_env(explicit: Option<&str>, env_var: &str, default: &str) -> String {
    if let Some(v) = explicit {
        return v.to_string();
    }
    std::env::var(env_var).unwrap_or_else(|_| default.to_string())
}

pub fn build_sonar_scan_command(report_root: Option<&str>, extra: &[String]) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--", "--project", "sonar-cli", "scan"]);

    if let Some(root) = report_root {
        let clippy = crate::clippy::clippy_report_path(root);
        if clippy.exists() {
            cmd.arg("--clippy-report").arg(&clippy);
        }
        let coverage = crate::coverage::test_report_path(root);
        if coverage.exists() {
            cmd.arg("--coverage-report").arg(&coverage);
        }
    }

    cmd.args(["--no-scm", "--skip-unchanged", "--exclusions", "**/*.json", "--sources", "src,tests"]);
    cmd.args(extra);
    cmd
}

pub fn sonar_scan(args: &SonarScanArgs) {
    let mut cmd = build_sonar_scan_command(args.report_root.as_deref(), &args.extra);
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    let mut child = cmd.spawn().expect("failed to run cargo run -- scan");
    let timeout = Duration::from_secs(args.timeout);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => std::process::exit(status.code().unwrap_or(1)),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    eprintln!(
                        "Error: sonar scan timed out after {} seconds",
                        args.timeout
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    std::process::exit(124);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error: failed to wait for sonar scan: {e}");
                std::process::exit(1);
            }
        }
    }
}

pub fn build_docker_scan_command(
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

pub fn docker_scan(args: &DockerScanArgs) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    fn args_vec(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn sonar_scan_no_report_root() {
        let cmd = build_sonar_scan_command(None, &[]);
        assert_eq!(
            args_vec(&cmd),
            vec![
                "run", "--", "--project", "sonar-cli", "scan",
                "--no-scm", "--skip-unchanged", "--exclusions", "**/*.json", "--sources", "src,tests",
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
        assert!(!args.iter().any(|a| a == "--clippy-report"), "should not have --clippy-report: {:?}", args);
        assert!(!args.iter().any(|a| a == "--coverage-report"), "should not have --coverage-report: {:?}", args);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sonar_scan_report_root_with_files() {
        let tmp = env::temp_dir().join(format!("xtask-scan-full-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let clippy_dir = tmp.join("clippy");
        let tests_dir = tmp.join("tests");
        fs::create_dir_all(&clippy_dir).unwrap();
        fs::create_dir_all(&tests_dir).unwrap();
        fs::write(clippy_dir.join("clippy-report.json"), "[]").unwrap();
        fs::write(tests_dir.join("coverage.xml"), "<xml/>").unwrap();
        let cmd = build_sonar_scan_command(Some(tmp.to_str().unwrap()), &[]);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "--clippy-report"), "should have --clippy-report: {:?}", args);
        assert!(args.iter().any(|a| a == "--coverage-report"), "should have --coverage-report: {:?}", args);
        assert!(args.contains(&"--no-scm".to_string()));
        assert!(args.contains(&"--skip-unchanged".to_string()));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sonar_scan_extra_args_forwarded() {
        let extra = vec!["-Dsonar.verbose=true".to_string(), "--new-code".to_string()];
        let cmd = build_sonar_scan_command(None, &extra);
        assert!(args_vec(&cmd).ends_with(&["-Dsonar.verbose=true".to_string(), "--new-code".to_string()]));
    }

    #[test]
    fn docker_scan_default_args() {
        let cmd = build_docker_scan_command(
            "sonar-cli", "main", "/home/user/project", "clippy-report.json", "coverage.xml", &[],
        );
        assert_eq!(
            args_vec(&cmd),
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
            "my-proj", "feature/xyz", "/tmp/repo", "custom-clippy.json", "custom-coverage.xml", &[],
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
        let extra = vec!["-Dsonar.verbose=true".to_string(), "-Dsonar.log.level=DEBUG".to_string()];
        let cmd = build_docker_scan_command(
            "sonar-cli", "main", "/home/user/project", "clippy-report.json", "coverage.xml", &extra,
        );
        assert!(args_vec(&cmd).ends_with(&["-Dsonar.verbose=true".to_string(), "-Dsonar.log.level=DEBUG".to_string()]));
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
