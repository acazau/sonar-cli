use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::client::SonarQubeConfig;

/// Resolve the current git branch name.
fn detect_branch() -> Option<String> {
    Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            } else {
                None
            }
        })
}

/// Extract the analysis task ID from scanner output.
///
/// The sonar-scanner CLI prints a line like:
///   More about the report processing at http://host/api/ce/task?id=AXyz123
fn extract_task_id(line: &str) -> Option<String> {
    if let Some(pos) = line.find("task?id=") {
        let after = &line[pos + "task?id=".len()..];
        let id: String = after.chars().take_while(|c| !c.is_whitespace()).collect();
        if id.is_empty() { None } else { Some(id) }
    } else {
        None
    }
}

pub struct ScanParams {
    pub clippy_report: Option<String>,
    pub coverage_report: Option<String>,
    pub wait: bool,
    pub timeout: u64,
    pub poll_interval: u64,
    pub no_scm: bool,
    pub skip_unchanged: bool,
    pub exclusions: Option<String>,
    pub sources: Option<String>,
    pub extra: Vec<String>,
    pub json: bool,
}

/// Build the sonar-scanner Command with all -D arguments.
fn build_command(config: &SonarQubeConfig, project: &str, params: &ScanParams) -> Command {
    let branch = config.branch.clone().or_else(detect_branch);

    let mut cmd = Command::new("sonar-scanner");

    cmd.arg(format!("-Dsonar.host.url={}", config.url));
    if let Some(ref token) = config.token {
        cmd.arg(format!("-Dsonar.token={token}"));
    }
    cmd.arg(format!("-Dsonar.projectKey={project}"));

    if let Ok(cwd) = std::env::current_dir() {
        cmd.arg(format!("-Dsonar.projectBaseDir={}", cwd.display()));
    }

    if let Some(ref b) = branch {
        cmd.arg(format!("-Dsonar.branch.name={b}"));
    }

    if let Some(ref path) = params.clippy_report {
        cmd.arg(format!("-Dsonar.rust.clippy.reportPaths={path}"));
    }
    if let Some(ref path) = params.coverage_report {
        cmd.arg(format!("-Dsonar.rust.cobertura.reportPaths={path}"));
    }

    // Performance flags — only emitted when explicitly set via CLI flags.
    if params.no_scm {
        cmd.arg("-Dsonar.scm.disabled=true");
    }
    if params.skip_unchanged {
        cmd.arg("-Dsonar.scanner.skipUnchangedFiles=true");
    }
    if let Some(ref excl) = params.exclusions {
        cmd.arg(format!("-Dsonar.exclusions={excl}"));
    }
    if let Some(ref src) = params.sources {
        cmd.arg(format!("-Dsonar.sources={src}"));
    }

    for arg in &params.extra {
        cmd.arg(arg);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    cmd
}

/// Stream lines from a reader, optionally printing them, and extract the first task ID found.
fn stream_output<R: std::io::Read>(
    reader: BufReader<R>,
    verbose: bool,
    existing_task_id: Option<String>,
) -> Option<String> {
    let mut task_id = existing_task_id;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if verbose {
            eprintln!("{line}");
        }
        if task_id.is_none() {
            task_id = extract_task_id(&line);
        }
    }
    task_id
}

/// Report the task ID result to the user.
fn report_task_id(task_id: &Option<String>, json: bool) {
    if let Some(ref id) = task_id {
        if json {
            println!("{{\"taskId\":\"{id}\"}}");
        } else {
            eprintln!("Analysis task ID: {id}");
        }
    } else if !json {
        eprintln!("Warning: could not extract analysis task ID from scanner output");
    }
}

pub async fn run(config: SonarQubeConfig, project: &str, params: ScanParams) -> i32 {
    let mut cmd = build_command(&config, project, &params);

    if !params.json {
        eprintln!("Running sonar-scanner for project '{project}'...");
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to start sonar-scanner: {e}");
            eprintln!("Is sonar-scanner installed and on your PATH?");
            return 1;
        }
    };

    let mut task_id: Option<String> = None;

    if let Some(stdout) = child.stdout.take() {
        task_id = stream_output(BufReader::new(stdout), !params.json, task_id);
    }

    if let Some(stderr) = child.stderr.take() {
        task_id = stream_output(BufReader::new(stderr), !params.json, task_id);
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to wait for sonar-scanner: {e}");
            return 1;
        }
    };

    if !status.success() {
        let code = status.code().unwrap_or(1);
        eprintln!("sonar-scanner exited with code {code}");
        return code;
    }

    report_task_id(&task_id, params.json);

    if params.wait {
        if let Some(ref id) = task_id {
            return super::wait::run(config, id, params.timeout, params.poll_interval, params.json)
                .await;
        }
        eprintln!("Cannot wait: no task ID was extracted from scanner output");
        return 1;
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_task_id_from_url() {
        let line = "More about the report processing at http://localhost:9000/api/ce/task?id=AXyz123abc";
        assert_eq!(extract_task_id(line), Some("AXyz123abc".to_string()));
    }

    #[test]
    fn test_extract_task_id_no_match() {
        let line = "INFO: Analysis complete";
        assert_eq!(extract_task_id(line), None);
    }

    #[test]
    fn test_extract_task_id_trailing_whitespace() {
        let line = "task?id=ABC123  ";
        assert_eq!(extract_task_id(line), Some("ABC123".to_string()));
    }

    #[test]
    fn test_extract_task_id_empty() {
        let line = "task?id=";
        assert_eq!(extract_task_id(line), None);
    }

    // ── build_command tests ───────────────────────────────────────────────

    fn make_config(url: &str, token: Option<&str>, branch: Option<&str>) -> SonarQubeConfig {
        SonarQubeConfig {
            url: url.to_string(),
            token: token.map(|t| t.to_string()),
            timeout: std::time::Duration::from_secs(30),
            project_key: None,
            branch: branch.map(|b| b.to_string()),
        }
    }

    fn make_params(
        clippy: Option<&str>,
        coverage: Option<&str>,
        extra: Vec<&str>,
    ) -> ScanParams {
        ScanParams {
            clippy_report: clippy.map(|s| s.to_string()),
            coverage_report: coverage.map(|s| s.to_string()),
            wait: false,
            timeout: 60,
            poll_interval: 5,
            no_scm: false,
            skip_unchanged: false,
            exclusions: None,
            sources: None,
            extra: extra.into_iter().map(|s| s.to_string()).collect(),
            json: false,
        }
    }

    fn args_vec(cmd: &std::process::Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn test_build_command_basic_url_and_project() {
        let config = make_config("http://localhost:9000", None, None);
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "my-project", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.host.url=http://localhost:9000"));
        assert!(args.iter().any(|a| a == "-Dsonar.projectKey=my-project"));
    }

    #[test]
    fn test_build_command_with_token() {
        let config = make_config("http://localhost:9000", Some("mytoken123"), None);
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.token=mytoken123"));
    }

    #[test]
    fn test_build_command_without_token() {
        let config = make_config("http://localhost:9000", None, None);
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.token=")));
    }

    #[test]
    fn test_build_command_with_config_branch() {
        let config = make_config("http://localhost:9000", None, Some("feature/xyz"));
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.branch.name=feature/xyz"));
    }

    #[test]
    fn test_build_command_with_clippy_report() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = make_params(Some("/tmp/clippy.json"), None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args
            .iter()
            .any(|a| a == "-Dsonar.rust.clippy.reportPaths=/tmp/clippy.json"));
    }

    #[test]
    fn test_build_command_with_coverage_report() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = make_params(None, Some("/tmp/lcov.info"), vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args
            .iter()
            .any(|a| a == "-Dsonar.rust.cobertura.reportPaths=/tmp/lcov.info"));
    }

    #[test]
    fn test_build_command_with_extra_args() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = make_params(None, None, vec!["-Dsonar.verbose=true", "-Dsonar.foo=bar"]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.verbose=true"));
        assert!(args.iter().any(|a| a == "-Dsonar.foo=bar"));
    }

    #[test]
    fn test_build_command_with_all_options() {
        let config = make_config("http://sonar:9000", Some("tok"), Some("release"));
        let params = make_params(
            Some("/reports/clippy.json"),
            Some("/reports/lcov.info"),
            vec!["-Dsonar.extra=1"],
        );
        let cmd = build_command(&config, "all-opts", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.host.url=http://sonar:9000"));
        assert!(args.iter().any(|a| a == "-Dsonar.token=tok"));
        assert!(args.iter().any(|a| a == "-Dsonar.projectKey=all-opts"));
        assert!(args.iter().any(|a| a == "-Dsonar.branch.name=release"));
        assert!(args
            .iter()
            .any(|a| a == "-Dsonar.rust.clippy.reportPaths=/reports/clippy.json"));
        assert!(args
            .iter()
            .any(|a| a == "-Dsonar.rust.cobertura.reportPaths=/reports/lcov.info"));
        // Extra args
        assert!(args.iter().any(|a| a == "-Dsonar.extra=1"));
        // Default make_params has no performance flags → should not appear
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.scm.disabled")));
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.scanner.skipUnchangedFiles")));
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.exclusions")));
    }

    #[test]
    fn test_build_command_with_performance_flags() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = ScanParams {
            clippy_report: None,
            coverage_report: None,
            wait: false,
            timeout: 60,
            poll_interval: 5,
            no_scm: true,
            skip_unchanged: true,
            exclusions: Some("**/*.json".to_string()),
            sources: Some("src,tests,scripts".to_string()),
            extra: vec![],
            json: false,
        };
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args.iter().any(|a| a == "-Dsonar.scm.disabled=true"));
        assert!(args.iter().any(|a| a == "-Dsonar.scanner.skipUnchangedFiles=true"));
        assert!(args.iter().any(|a| a == "-Dsonar.exclusions=**/*.json"));
        assert!(args.iter().any(|a| a == "-Dsonar.sources=src,tests,scripts"));
    }

    #[test]
    fn test_build_command_without_performance_flags() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.scm.disabled")));
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.scanner.skipUnchangedFiles")));
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.exclusions")));
        assert!(!args.iter().any(|a| a.starts_with("-Dsonar.sources")));
    }

    #[test]
    fn test_build_command_includes_project_base_dir() {
        let config = make_config("http://localhost:9000", None, Some("main"));
        let params = make_params(None, None, vec![]);
        let cmd = build_command(&config, "proj", &params);
        let args = args_vec(&cmd);
        assert!(args
            .iter()
            .any(|a| a.starts_with("-Dsonar.projectBaseDir=")));
    }

    // ── stream_output tests ───────────────────────────────────────────────

    #[test]
    fn test_stream_output_extracts_task_id() {
        let data = b"INFO: Starting analysis\nMore about the report processing at http://host/api/ce/task?id=TASK001\nINFO: Done\n";
        let reader = BufReader::new(std::io::Cursor::new(data.as_ref()));
        let result = stream_output(reader, false, None);
        assert_eq!(result, Some("TASK001".to_string()));
    }

    #[test]
    fn test_stream_output_no_task_id() {
        let data = b"INFO: Starting analysis\nINFO: Done\n";
        let reader = BufReader::new(std::io::Cursor::new(data.as_ref()));
        let result = stream_output(reader, false, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_stream_output_empty_input() {
        let data: &[u8] = b"";
        let reader = BufReader::new(std::io::Cursor::new(data));
        let result = stream_output(reader, false, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_stream_output_preserves_existing_task_id() {
        let data = b"http://host/api/ce/task?id=NEW999\n";
        let reader = BufReader::new(std::io::Cursor::new(data.as_ref()));
        let result = stream_output(reader, false, Some("EXISTING_ID".to_string()));
        assert_eq!(result, Some("EXISTING_ID".to_string()));
    }

    #[test]
    fn test_stream_output_verbose_does_not_panic() {
        let data = b"INFO: line one\ntask?id=VERBOSE1\n";
        let reader = BufReader::new(std::io::Cursor::new(data.as_ref()));
        let result = stream_output(reader, true, None);
        assert_eq!(result, Some("VERBOSE1".to_string()));
    }

    #[test]
    fn test_stream_output_picks_first_task_id() {
        let data = b"task?id=FIRST\ntask?id=SECOND\n";
        let reader = BufReader::new(std::io::Cursor::new(data.as_ref()));
        let result = stream_output(reader, false, None);
        assert_eq!(result, Some("FIRST".to_string()));
    }

    // ── report_task_id tests ──────────────────────────────────────────────

    #[test]
    fn test_report_task_id_json_some() {
        report_task_id(&Some("TASK_ABC".to_string()), true);
    }

    #[test]
    fn test_report_task_id_human_some() {
        report_task_id(&Some("TASK_ABC".to_string()), false);
    }

    #[test]
    fn test_report_task_id_json_none() {
        report_task_id(&None, true);
    }

    #[test]
    fn test_report_task_id_human_none() {
        report_task_id(&None, false);
    }

    // ── detect_branch tests ───────────────────────────────────────────────

    #[test]
    fn test_detect_branch_returns_option() {
        let result = detect_branch();
        if let Some(branch) = result {
            assert!(!branch.is_empty());
        }
    }
}
