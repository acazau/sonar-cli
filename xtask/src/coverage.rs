use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(clap::Args)]
pub struct TestReportArgs {
    /// Root report directory (creates tests/coverage.xml inside)
    #[arg(long)]
    pub report_root: String,
    /// Timeout in seconds (default: 600). Exit code 124 on timeout.
    #[arg(long, default_value = "600")]
    pub timeout: u64,
}

pub fn build_test_report_command(root: &str) -> Command {
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

pub fn test_report_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("tests").join("coverage.xml")
}

pub fn test_report(args: &TestReportArgs) {
    let output_path = test_report_path(&args.report_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create test report dir");
    }
    let mut cmd = build_test_report_command(&args.report_root);
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let mut child = cmd.spawn().expect("failed to run cargo llvm-cov");
    let timeout = Duration::from_secs(args.timeout);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let abs = std::fs::canonicalize(&output_path)
                    .unwrap_or_else(|_| output_path.clone());
                println!("{}", abs.display());
                std::process::exit(status.code().unwrap_or(1));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    eprintln!(
                        "Error: cargo llvm-cov timed out after {} seconds",
                        args.timeout
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    std::process::exit(124);
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Error: failed to wait for cargo llvm-cov: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_vec(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn test_build_test_report_command_args() {
        let cmd = build_test_report_command("/tmp/reports");
        assert_eq!(
            args_vec(&cmd),
            vec!["llvm-cov", "--cobertura", "--output-path", "/tmp/reports/tests/coverage.xml"]
        );
    }

    #[test]
    fn test_test_report_path() {
        let path = test_report_path("/tmp/reports");
        assert_eq!(path, PathBuf::from("/tmp/reports/tests/coverage.xml"));
    }
}
