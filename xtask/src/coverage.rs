use std::path::PathBuf;
use std::process::{Command, Stdio};

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

pub fn test_report(args: &crate::ReportRootArgs) {
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
