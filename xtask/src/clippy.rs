use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn build_clippy_command() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.args(["clippy", "--message-format=json"]);
    cmd
}

pub fn clippy_report_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("clippy").join("clippy-report.json")
}

pub fn clippy_report(args: &crate::ReportRootArgs) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args_vec(cmd: &Command) -> Vec<String> {
        cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn test_build_clippy_command_args() {
        let cmd = build_clippy_command();
        assert_eq!(args_vec(&cmd), vec!["clippy", "--message-format=json"]);
    }

    #[test]
    fn test_clippy_report_path() {
        let path = clippy_report_path("/tmp/reports");
        assert_eq!(path, PathBuf::from("/tmp/reports/clippy/clippy-report.json"));
    }
}
