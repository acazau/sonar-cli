use serde::{Deserialize, Serialize};
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

pub fn clippy_scoped_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("clippy").join("clippy-scoped.json")
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ClippyDiagnostic {
    pub level: String,
    pub file: String,
    pub line: u64,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClippyScoped {
    pub total: usize,
    pub items: Vec<ClippyDiagnostic>,
}

pub fn parse_clippy_diagnostics(ndjson: &str) -> Vec<ClippyDiagnostic> {
    let mut diagnostics = Vec::new();
    for line in ndjson.lines() {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if val.get("reason").and_then(|v| v.as_str()) != Some("compiler-message") {
            continue;
        }
        let Some(msg) = val.get("message") else {
            continue;
        };
        let level = match msg.get("level").and_then(|v| v.as_str()) {
            Some("warning" | "error") => msg["level"].as_str().unwrap().to_string(),
            _ => continue,
        };
        // Skip summary lines (no code)
        let code = match msg.get("code").and_then(|c| c.get("code")).and_then(|c| c.as_str()) {
            Some(c) => c.to_string(),
            None => continue,
        };
        let message_text = msg
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Extract primary span file/line
        let spans = msg.get("spans").and_then(|s| s.as_array());
        let primary = spans.and_then(|arr| arr.iter().find(|s| s.get("is_primary") == Some(&serde_json::Value::Bool(true))));
        let (file, line_num) = match primary {
            Some(span) => (
                span.get("file_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                span.get("line_start")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
            ),
            None => continue,
        };

        diagnostics.push(ClippyDiagnostic {
            level,
            file,
            line: line_num,
            code,
            message: message_text,
        });
    }
    // Sort errors before warnings, then by file path
    diagnostics.sort_by(|a, b| {
        let level_ord = |l: &str| -> u8 {
            match l {
                "error" => 0,
                "warning" => 1,
                _ => 2,
            }
        };
        level_ord(&a.level)
            .cmp(&level_ord(&b.level))
            .then(a.file.cmp(&b.file))
            .then(a.line.cmp(&b.line))
    });
    diagnostics
}

pub fn clippy_report(args: &crate::ReportRootArgs) {
    let output_path = clippy_report_path(&args.report_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create clippy report dir");
    }
    let mut cmd = build_clippy_command();
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());

    let output = cmd.output().expect("failed to run cargo clippy");
    std::fs::write(&output_path, &output.stdout).expect("failed to write clippy report");

    let abs = std::fs::canonicalize(&output_path).unwrap_or_else(|_| output_path.clone());
    println!("{}", abs.display());

    // Post-process: write scoped summary
    let ndjson = String::from_utf8_lossy(&output.stdout);
    let diagnostics = parse_clippy_diagnostics(&ndjson);
    let scoped = ClippyScoped {
        total: diagnostics.len(),
        items: diagnostics.into_iter().take(10).collect(),
    };
    let scoped_path = clippy_scoped_path(&args.report_root);
    let json = serde_json::to_string_pretty(&scoped).expect("failed to serialize clippy scoped");
    std::fs::write(&scoped_path, json).expect("failed to write clippy scoped file");

    let scoped_abs = std::fs::canonicalize(&scoped_path).unwrap_or_else(|_| scoped_path.clone());
    println!("{}", scoped_abs.display());

    std::process::exit(output.status.code().unwrap_or(1));
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
    fn test_build_clippy_command_args() {
        let cmd = build_clippy_command();
        assert_eq!(args_vec(&cmd), vec!["clippy", "--message-format=json"]);
    }

    #[test]
    fn test_clippy_report_path() {
        let path = clippy_report_path("/tmp/reports");
        assert_eq!(
            path,
            PathBuf::from("/tmp/reports/clippy/clippy-report.json")
        );
    }

    #[test]
    fn test_clippy_scoped_path() {
        let path = clippy_scoped_path("/tmp/reports");
        assert_eq!(
            path,
            PathBuf::from("/tmp/reports/clippy/clippy-scoped.json")
        );
    }

    #[test]
    fn test_parse_clippy_diagnostics_extracts_warnings() {
        let ndjson = r#"{"reason":"compiler-message","message":{"level":"warning","message":"unused variable","code":{"code":"clippy::needless_borrow"},"spans":[{"file_name":"src/commands/foo.rs","line_start":42,"is_primary":true}]}}
{"reason":"compiler-message","message":{"level":"warning","message":"summary line with no code","code":null,"spans":[]}}
{"reason":"build-finished","success":true}"#;
        let items = parse_clippy_diagnostics(ndjson);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].level, "warning");
        assert_eq!(items[0].file, "src/commands/foo.rs");
        assert_eq!(items[0].line, 42);
        assert_eq!(items[0].code, "clippy::needless_borrow");
        assert_eq!(items[0].message, "unused variable");
    }

    #[test]
    fn test_parse_clippy_diagnostics_includes_errors() {
        let ndjson = r#"{"reason":"compiler-message","message":{"level":"error","message":"mismatched types","code":{"code":"E0308"},"spans":[{"file_name":"src/main.rs","line_start":10,"is_primary":true}]}}
{"reason":"compiler-message","message":{"level":"warning","message":"unused import","code":{"code":"unused_imports"},"spans":[{"file_name":"src/lib.rs","line_start":1,"is_primary":true}]}}"#;
        let items = parse_clippy_diagnostics(ndjson);
        assert_eq!(items.len(), 2);
        // Errors sorted before warnings
        assert_eq!(items[0].level, "error");
        assert_eq!(items[0].code, "E0308");
        assert_eq!(items[1].level, "warning");
        assert_eq!(items[1].code, "unused_imports");
    }

    #[test]
    fn test_parse_clippy_diagnostics_sorted_errors_first_then_by_file() {
        let ndjson = r#"{"reason":"compiler-message","message":{"level":"warning","message":"w","code":{"code":"clippy::a"},"spans":[{"file_name":"a.rs","line_start":1,"is_primary":true}]}}
{"reason":"compiler-message","message":{"level":"error","message":"e","code":{"code":"E0001"},"spans":[{"file_name":"z.rs","line_start":1,"is_primary":true}]}}"#;
        let items = parse_clippy_diagnostics(ndjson);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].level, "error");
        assert_eq!(items[0].file, "z.rs");
        assert_eq!(items[1].level, "warning");
        assert_eq!(items[1].file, "a.rs");
    }

    #[test]
    fn test_parse_clippy_diagnostics_skips_non_primary_span() {
        let ndjson = r#"{"reason":"compiler-message","message":{"level":"warning","message":"w","code":{"code":"clippy::a"},"spans":[{"file_name":"a.rs","line_start":1,"is_primary":false}]}}"#;
        let items = parse_clippy_diagnostics(ndjson);
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_parse_clippy_diagnostics_skips_notes_and_info() {
        let ndjson = r#"{"reason":"compiler-message","message":{"level":"note","message":"some note","code":{"code":"clippy::a"},"spans":[{"file_name":"a.rs","line_start":1,"is_primary":true}]}}
{"reason":"compiler-message","message":{"level":"help","message":"try this","code":{"code":"clippy::a"},"spans":[{"file_name":"a.rs","line_start":1,"is_primary":true}]}}"#;
        let items = parse_clippy_diagnostics(ndjson);
        assert_eq!(items.len(), 0);
    }

    #[test]
    fn test_parse_clippy_diagnostics_caps_at_10() {
        let mut lines = Vec::new();
        for i in 0..15 {
            lines.push(format!(
                r#"{{"reason":"compiler-message","message":{{"level":"warning","message":"w{i}","code":{{"code":"clippy::a"}},"spans":[{{"file_name":"f{i:02}.rs","line_start":1,"is_primary":true}}]}}}}"#
            ));
        }
        let ndjson = lines.join("\n");
        let items = parse_clippy_diagnostics(&ndjson);
        assert_eq!(items.len(), 15);
        // Capping is done at the scoped level, not in parse
        let scoped = ClippyScoped {
            total: items.len(),
            items: items.into_iter().take(10).collect(),
        };
        assert_eq!(scoped.total, 15);
        assert_eq!(scoped.items.len(), 10);
    }
}
