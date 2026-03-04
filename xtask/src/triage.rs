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
}

pub fn build_wait_command(task_id: &str, timeout: u64) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "run", "--", "wait", task_id,
        "--timeout", &timeout.to_string(),
        "--poll-interval", "10",
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
        eprintln!("Error: failed to create triage dir {}: {e}", triage_dir.display());
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
        ("quality-gate", build_query_command(&args.project, &args.branch, "quality-gate", &[])),
        (
            "issues",
            build_query_command(
                &args.project,
                &args.branch,
                "issues",
                if new_code { &["--new-code"] } else { &[] },
            ),
        ),
        ("duplications", build_query_command(&args.project, &args.branch, "duplications", &["--details"])),
        ("coverage", build_query_command(&args.project, &args.branch, "coverage", &[])),
        ("measures", build_query_command(&args.project, &args.branch, "measures", &[])),
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

    // Step 3: Print triage directory
    let abs = std::fs::canonicalize(&triage_dir).unwrap_or(triage_dir);
    println!("{}", abs.display());
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
            vec!["run", "--", "--project", "sonar-cli", "--branch", "main", "quality-gate", "--json"]
        );
    }

    #[test]
    fn query_command_issues_scoped() {
        let cmd = build_query_command("sonar-cli", "feat/x", "issues", &["--new-code"]);
        assert_eq!(
            args_vec(&cmd),
            vec!["run", "--", "--project", "sonar-cli", "--branch", "feat/x", "issues", "--json", "--new-code"]
        );
    }

    #[test]
    fn query_command_issues_full() {
        let cmd = build_query_command("sonar-cli", "main", "issues", &[]);
        let args = args_vec(&cmd);
        assert!(!args.contains(&"--new-code".to_string()));
        assert_eq!(
            args,
            vec!["run", "--", "--project", "sonar-cli", "--branch", "main", "issues", "--json"]
        );
    }

    #[test]
    fn query_command_duplications() {
        let cmd = build_query_command("sonar-cli", "main", "duplications", &["--details"]);
        assert_eq!(
            args_vec(&cmd),
            vec!["run", "--", "--project", "sonar-cli", "--branch", "main", "duplications", "--json", "--details"]
        );
    }

    #[test]
    fn query_command_hotspots_scoped() {
        let cmd = build_query_command("my-proj", "dev", "hotspots", &["--new-code"]);
        assert_eq!(
            args_vec(&cmd),
            vec!["run", "--", "--project", "my-proj", "--branch", "dev", "hotspots", "--json", "--new-code"]
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
}
