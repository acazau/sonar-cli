use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{Command, Stdio};

pub fn test_failures_path(root: &str) -> PathBuf {
    PathBuf::from(root).join("tests").join("test-failures.json")
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TestFailure {
    pub test: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestFailuresScoped {
    pub total: usize,
    pub items: Vec<TestFailure>,
}

/// Parse test failures and compilation errors from `cargo test` output.
///
/// Handles two categories:
/// 1. **Test failures** — from the `failures:` summary block
/// 2. **Compilation errors** — `error[E0xxx]: message` lines with ` --> file:line:col`
pub fn parse_test_failures(output: &str) -> Vec<TestFailure> {
    let mut failures = Vec::new();

    // Extract compilation errors (error[Exxxx] lines)
    parse_compile_errors(output, &mut failures);

    // Extract test runtime failures
    parse_runtime_failures(output, &mut failures);

    failures
}

fn extract_error_message(trimmed: &str) -> Option<String> {
    if let Some(rest) = trimmed.strip_prefix("error[") {
        rest.find("]: ").map(|pos| rest[pos + 3..].to_string())
    } else if let Some(rest) = trimmed.strip_prefix("error: ") {
        if rest.starts_with("could not compile")
            || rest.starts_with("aborting")
            || rest.starts_with("test failed")
        {
            None
        } else {
            Some(rest.to_string())
        }
    } else {
        None
    }
}

fn extract_location(lines: &[&str], i: usize) -> String {
    if i + 1 < lines.len() {
        let next = lines[i + 1].trim();
        if let Some(loc) = next.strip_prefix("--> ") {
            loc.to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

fn format_test_name(message: &str, location: &str) -> String {
    if location.is_empty() {
        format!("compile error: {message}")
    } else {
        format!("compile error at {location}")
    }
}

fn extract_test_stdout_section<'a>(all_lines: &[&'a str], start_idx: usize) -> (String, Vec<&'a str>, usize) {
    let line = all_lines[start_idx];
    if let Some(test_name) = line
        .strip_prefix("---- ")
        .and_then(|s| s.strip_suffix(" stdout ----"))
    {
        let mut msg_lines = Vec::new();
        let mut i = start_idx + 1;
        while i < all_lines.len() {
            let inner = all_lines[i];
            if inner.starts_with("---- ") || inner.trim() == "failures:" {
                break;
            }
            msg_lines.push(inner);
            i += 1;
        }
        (test_name.to_string(), msg_lines, i)
    } else {
        (String::new(), Vec::new(), start_idx + 1)
    }
}

fn extract_test_messages(output: &str) -> std::collections::HashMap<String, String> {
    let mut messages = std::collections::HashMap::new();
    let all_lines: Vec<&str> = output.lines().collect();
    let mut i = 0;

    while i < all_lines.len() {
        if all_lines[i].starts_with("---- ") && all_lines[i].contains(" stdout ----") {
            let (test_name, msg_lines, next_i) = extract_test_stdout_section(&all_lines, i);
            let msg = extract_assertion_message(&msg_lines);
            if !msg.is_empty() {
                messages.insert(test_name, msg);
            }
            i = next_i;
        } else {
            i += 1;
        }
    }

    messages
}

fn collect_summary_failures(
    output: &str,
    messages: &std::collections::HashMap<String, String>,
) -> Vec<TestFailure> {
    let mut failures = Vec::new();
    let mut failures_count = 0;
    let mut in_summary = false;

    for line in output.lines() {
        if line.trim() == "failures:" {
            failures_count += 1;
            if failures_count >= 2 {
                in_summary = true;
            }
            continue;
        }
        if in_summary {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("test result:") {
                break;
            }
            let test_name = trimmed.to_string();
            let message = messages.get(&test_name).cloned().unwrap_or_default();
            failures.push(TestFailure {
                test: test_name,
                message,
            });
        }
    }

    failures
}

fn parse_compile_errors(output: &str, failures: &mut Vec<TestFailure>) {
    let lines: Vec<&str> = output.lines().collect();
    for i in 0..lines.len() {
        let trimmed = lines[i].trim();
        let Some(message) = extract_error_message(trimmed) else {
            continue;
        };

        let location = extract_location(&lines, i);
        let test_name = format_test_name(&message, &location);

        failures.push(TestFailure {
            test: test_name,
            message,
        });
    }
}

fn parse_runtime_failures(output: &str, failures: &mut Vec<TestFailure>) {
    let messages = extract_test_messages(output);
    let runtime_failures = collect_summary_failures(output, &messages);
    failures.extend(runtime_failures);
}

fn extract_assertion_message(lines: &[&str]) -> String {
    for line in lines {
        let trimmed = line.trim();
        if trimmed.contains("panicked at") {
            // Extract the message after "panicked at '"
            if let Some(start) = trimmed.find("panicked at '") {
                let after = &trimmed[start + 13..];
                if let Some(end) = after.rfind('\'') {
                    return after[..end].to_string();
                }
                return after.to_string();
            }
            // Rust 2024 format: panicked at <file>:<line>:<col>:\n<message>
            if let Some(start) = trimmed.find("panicked at ") {
                return trimmed[start + 12..].to_string();
            }
        }
        if trimmed.starts_with("assertion") {
            return trimmed.to_string();
        }
    }
    String::new()
}

pub fn test_failures(args: &crate::ReportRootArgs) {
    let output_path = test_failures_path(&args.report_root);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("failed to create test failures dir");
    }

    let mut cmd = Command::new("cargo");
    cmd.args(["test"]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let output = cmd.output().expect("failed to run cargo test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    let failures = parse_test_failures(&combined);
    let scoped = TestFailuresScoped {
        total: failures.len(),
        items: failures.into_iter().take(10).collect(),
    };

    let json = serde_json::to_string_pretty(&scoped).expect("failed to serialize test failures");
    std::fs::write(&output_path, json).expect("failed to write test failures file");

    let abs = std::fs::canonicalize(&output_path).unwrap_or_else(|_| output_path.clone());
    println!("{}", abs.display());

    if scoped.total > 0 || !output.status.success() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failures_path_construction() {
        let path = test_failures_path("/tmp/reports");
        assert_eq!(
            path,
            PathBuf::from("/tmp/reports/tests/test-failures.json")
        );
    }

    #[test]
    fn test_parse_no_failures() {
        let output = "running 5 tests\ntest result: ok. 5 passed; 0 failed;\n";
        let failures = parse_test_failures(output);
        assert!(failures.is_empty());
    }

    #[test]
    fn test_parse_with_failures() {
        let output = r#"running 3 tests
test commands::scan::tests::test_ok ... ok
test commands::scan::tests::test_post_entry ... FAILED
test commands::scan::tests::test_other ... FAILED

failures:

---- commands::scan::tests::test_post_entry stdout ----
thread 'test_post_entry' panicked at 'assertion failed: expected 5, got 3'

---- commands::scan::tests::test_other stdout ----
thread 'test_other' panicked at 'not yet implemented'

failures:
    commands::scan::tests::test_post_entry
    commands::scan::tests::test_other

test result: FAILED. 1 passed; 2 failed;
"#;
        let failures = parse_test_failures(output);
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].test, "commands::scan::tests::test_post_entry");
        assert_eq!(failures[0].message, "assertion failed: expected 5, got 3");
        assert_eq!(failures[1].test, "commands::scan::tests::test_other");
        assert_eq!(failures[1].message, "not yet implemented");
    }

    #[test]
    fn test_parse_caps_at_10() {
        let mut output = String::from("failures:\n\n");
        for i in 0..15 {
            output.push_str(&format!("---- test_{i} stdout ----\nthread 'test_{i}' panicked at 'fail {i}'\n\n"));
        }
        output.push_str("failures:\n");
        for i in 0..15 {
            output.push_str(&format!("    test_{i}\n"));
        }
        output.push_str("\ntest result: FAILED.\n");
        let failures = parse_test_failures(&output);
        assert_eq!(failures.len(), 15);
        let scoped = TestFailuresScoped {
            total: failures.len(),
            items: failures.into_iter().take(10).collect(),
        };
        assert_eq!(scoped.total, 15);
        assert_eq!(scoped.items.len(), 10);
    }

    #[test]
    fn test_parse_compile_errors() {
        let output = r#"error[E0308]: mismatched types
 --> src/commands/scan.rs:42:5
  |
42 |     5u32
  |     ^^^^ expected `String`, found `u32`

error[E0433]: failed to resolve: use of undeclared type
 --> src/commands/client.rs:10:5

error: could not compile `sonar-cli` (lib) due to 2 previous errors
"#;
        let failures = parse_test_failures(output);
        assert_eq!(failures.len(), 2);
        assert_eq!(
            failures[0].test,
            "compile error at src/commands/scan.rs:42:5"
        );
        assert_eq!(failures[0].message, "mismatched types");
        assert_eq!(
            failures[1].test,
            "compile error at src/commands/client.rs:10:5"
        );
        assert_eq!(failures[1].message, "failed to resolve: use of undeclared type");
    }

    #[test]
    fn test_parse_compile_error_without_location() {
        let output = "error: macro expansion ignores token `{`\n";
        let failures = parse_test_failures(output);
        assert_eq!(failures.len(), 1);
        assert_eq!(
            failures[0].test,
            "compile error: macro expansion ignores token `{`"
        );
    }

    #[test]
    fn test_parse_mixed_compile_and_runtime() {
        let output = r#"error[E0599]: no method named `foo`
 --> src/lib.rs:5:10

failures:

---- commands::tests::test_bar stdout ----
thread 'test_bar' panicked at 'assertion failed'

failures:
    commands::tests::test_bar

test result: FAILED.
"#;
        let failures = parse_test_failures(output);
        assert_eq!(failures.len(), 2);
        assert_eq!(failures[0].test, "compile error at src/lib.rs:5:10");
        assert_eq!(failures[1].test, "commands::tests::test_bar");
    }

    #[test]
    fn test_parse_assertion_message_formats() {
        // Rust 2024 format without quotes
        let output = r#"failures:

---- my_test stdout ----
thread 'my_test' panicked at src/lib.rs:10:5:
assertion `left == right` failed
  left: 1
 right: 2

failures:
    my_test

test result: FAILED.
"#;
        let failures = parse_test_failures(output);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].test, "my_test");
        assert!(!failures[0].message.is_empty());
    }
}
