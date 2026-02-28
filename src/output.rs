//! Output formatting — human-readable and JSON

use crate::helpers::{FileCoverage, FileDuplication};
use crate::types::{
    AnalysisTask, MeasureHistory, MeasuresResponse,
    ProjectInfo, QualityGateResponse,
    RuleInfo, SecurityHotspot, SonarIssue, SourceLine,
};

/// Print value as JSON to stdout
pub fn print_json<T: serde::Serialize + ?Sized>(value: &T) {
    match serde_json::to_string_pretty(value) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Failed to serialize JSON: {e}"),
    }
}

/// Format health check output
pub fn print_health(status: &str, url: &str, json: bool) {
    if json {
        print_json(&serde_json::json!({
            "url": url,
            "status": status,
            "healthy": status == "UP",
        }));
    } else {
        let icon = if status == "UP" { "OK" } else { "FAIL" };
        println!("[{icon}] SonarQube at {url} — status: {status}");
    }
}

/// Format quality gate output
pub fn print_quality_gate(response: &QualityGateResponse, project: &str, json: bool) {
    if json {
        print_json(response);
        return;
    }

    let status = &response.project_status.status;
    let icon = match status.as_str() {
        "OK" => "PASSED",
        "WARN" => "WARNING",
        _ => "FAILED",
    };
    println!("Quality Gate: [{icon}] {status}  (project: {project})");

    if !response.project_status.conditions.is_empty() {
        println!();
        println!("  {:<30} {:<10} {:<10} Threshold", "Metric", "Status", "Value");
        println!("  {}", "-".repeat(70));
        for cond in &response.project_status.conditions {
            let value = cond.actual_value.as_deref().unwrap_or("-");
            let threshold = cond.error_threshold.as_deref().unwrap_or("-");
            let comparator = cond.comparator.as_deref().unwrap_or("");
            println!(
                "  {:<30} {:<10} {:<10} {comparator} {threshold}",
                cond.metric_key, cond.status, value
            );
        }
    }
}

/// Format issues output
pub fn print_issues(issues: &[SonarIssue], project: &str, json: bool) {
    if json {
        print_json(issues);
        return;
    }

    println!("{} issues found (project: {project})", issues.len());
    if issues.is_empty() {
        return;
    }

    println!();
    for issue in issues {
        let line_str = issue
            .line
            .or(issue.text_range.as_ref().map(|r| r.start_line))
            .map(|l| format!(":{l}"))
            .unwrap_or_default();

        let file = issue
            .component
            .split(':')
            .nth(1)
            .unwrap_or(&issue.component);

        println!(
            "  [{:<8}] [{:<8}] {file}{line_str}",
            issue.severity, issue.issue_type
        );
        println!("           {}", issue.message);
        if !issue.tags.is_empty() {
            println!("           tags: {}", issue.tags.join(", "));
        }
        println!();
    }
}

/// Format measures output
pub fn print_measures(response: &MeasuresResponse, json: bool) {
    if json {
        print_json(response);
        return;
    }

    println!("Measures for: {}", response.component.key);
    println!();
    println!("  {:<35} Value", "Metric");
    println!("  {}", "-".repeat(50));
    for measure in &response.component.measures {
        let value = measure.value.as_deref().unwrap_or("-");
        println!("  {:<35} {value}", measure.metric);
    }
}

/// Format file coverage output
pub fn print_coverage(files: &[FileCoverage], project: &str, json: bool) {
    if json {
        print_json(files);
        return;
    }

    println!(
        "{} files with coverage data (project: {project})",
        files.len()
    );
    if files.is_empty() {
        return;
    }

    println!();
    println!(
        "  {:<50} {:>8} {:>10} {:>10}",
        "File", "Coverage", "Uncovered", "Lines"
    );
    println!("  {}", "-".repeat(82));
    for f in files {
        println!(
            "  {:<50} {:>7.1}% {:>10} {:>10}",
            f.file, f.coverage_percent, f.uncovered_lines, f.lines_to_cover
        );
    }
}

/// Format duplications output
pub fn print_duplications(files: &[FileDuplication], project: &str, json: bool, details: bool) {
    if json {
        print_json(files);
        return;
    }

    println!(
        "{} files with duplications (project: {project})",
        files.len()
    );
    if files.is_empty() {
        return;
    }

    println!();
    println!(
        "  {:<50} {:>8} {:>10}",
        "File", "Lines", "Density"
    );
    println!("  {}", "-".repeat(72));
    for f in files {
        println!(
            "  {:<50} {:>8} {:>9.1}%",
            f.file, f.duplicated_lines, f.duplicated_density
        );
        if details && !f.blocks.is_empty() {
            for block in &f.blocks {
                println!(
                    "    L{}-{} duplicated in {} L{}",
                    block.from_line,
                    block.from_line + block.size,
                    block.duplicated_in,
                    block.duplicated_in_line
                );
            }
        }
    }
}

/// Format hotspots output
pub fn print_hotspots(hotspots: &[SecurityHotspot], project: &str, json: bool) {
    if json {
        print_json(hotspots);
        return;
    }

    println!(
        "{} security hotspots (project: {project})",
        hotspots.len()
    );
    if hotspots.is_empty() {
        return;
    }

    println!();
    for hs in hotspots {
        let file = hs
            .component
            .split(':')
            .nth(1)
            .unwrap_or(&hs.component);
        let line_str = hs.line.map(|l| format!(":{l}")).unwrap_or_default();

        println!(
            "  [{:<6}] [{:<12}] {file}{line_str}",
            hs.vulnerability_probability, hs.security_category
        );
        println!("           {}", hs.message);
        println!("           rule: {}", hs.rule_key);
        println!();
    }
}

/// Format projects output
pub fn print_projects(projects: &[ProjectInfo], json: bool) {
    if json {
        print_json(projects);
        return;
    }

    println!("{} projects found", projects.len());
    if projects.is_empty() {
        return;
    }

    println!();
    println!(
        "  {:<40} {:<40} {:<10} Last Analysis",
        "Key", "Name", "Visibility"
    );
    println!("  {}", "-".repeat(105));
    for p in projects {
        let vis = p.visibility.as_deref().unwrap_or("-");
        let last = p.last_analysis_date.as_deref().unwrap_or("-");
        println!("  {:<40} {:<40} {:<10} {}", p.key, p.name, vis, last);
    }
}

/// Format measures history output
pub fn print_history(measures: &[MeasureHistory], project: &str, json: bool) {
    if json {
        print_json(measures);
        return;
    }

    println!("Measures history for: {project}");
    if measures.is_empty() {
        println!("  No history data found.");
        return;
    }

    for measure in measures {
        println!();
        println!("  Metric: {}", measure.metric);
        println!("  {:<25} Value", "Date");
        println!("  {}", "-".repeat(40));
        for point in &measure.history {
            let value = point.value.as_deref().unwrap_or("-");
            println!("  {:<25} {}", point.date, value);
        }
    }
}

/// Format rules output
pub fn print_rules(rules: &[RuleInfo], json: bool) {
    if json {
        print_json(rules);
        return;
    }

    println!("{} rules found", rules.len());
    if rules.is_empty() {
        return;
    }

    println!();
    println!(
        "  {:<40} {:<35} {:<10} {:<15} Language",
        "Key", "Name", "Severity", "Type"
    );
    println!("  {}", "-".repeat(110));
    for r in rules {
        let sev = r.severity.as_deref().unwrap_or("-");
        let rt = r.rule_type.as_deref().unwrap_or("-");
        let lang = r.lang_name.as_deref().or(r.lang.as_deref()).unwrap_or("-");
        let name_truncated = if r.name.len() > 33 {
            format!("{}...", &r.name[..30])
        } else {
            r.name.clone()
        };
        println!(
            "  {:<40} {:<35} {:<10} {:<15} {}",
            r.key, name_truncated, sev, rt, lang
        );
    }
}

/// Format source code output
pub fn print_source(lines: &[SourceLine], json: bool) {
    if json {
        print_json(lines);
        return;
    }

    for line in lines {
        println!("{:>6} | {}", line.line, line.code);
    }
}

/// Format wait result output
pub fn print_wait_result(task: &AnalysisTask, json: bool) {
    if json {
        print_json(task);
        return;
    }

    println!("Analysis task: {}", task.id);
    println!("  Status:      {}", task.status);
    println!("  Submitted:   {}", task.submitted_at);
    if let Some(ref executed) = task.executed_at {
        println!("  Completed:   {executed}");
    }
    if let Some(ref analysis_id) = task.analysis_id {
        println!("  Analysis ID: {analysis_id}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{DuplicationBlockDetail, FileCoverage, FileDuplication};
    use crate::types::{Measure, MeasuresComponent, ProjectStatus, QualityGateCondition, TextRange};

    fn sample_issue() -> SonarIssue {
        SonarIssue {
            key: "abc".to_string(),
            rule: "rust:S3776".to_string(),
            severity: "CRITICAL".to_string(),
            component: "proj:src/main.rs".to_string(),
            project: "proj".to_string(),
            line: Some(42),
            text_range: None,
            message: "Complexity too high".to_string(),
            issue_type: "CODE_SMELL".to_string(),
            status: "OPEN".to_string(),
            resolution: None,
            debt: Some("6min".to_string()),
            effort: Some("6min".to_string()),
            tags: vec!["brain-overload".to_string()],
        }
    }

    fn sample_quality_gate() -> QualityGateResponse {
        QualityGateResponse {
            project_status: ProjectStatus {
                status: "OK".to_string(),
                conditions: vec![QualityGateCondition {
                    status: "OK".to_string(),
                    metric_key: "new_bugs".to_string(),
                    comparator: Some("GT".to_string()),
                    error_threshold: Some("0".to_string()),
                    actual_value: Some("0".to_string()),
                }],
            },
        }
    }

    fn sample_measures_response() -> MeasuresResponse {
        MeasuresResponse {
            component: MeasuresComponent {
                key: "proj".to_string(),
                measures: vec![
                    Measure { metric: "bugs".to_string(), value: Some("0".to_string()), period: None },
                    Measure { metric: "coverage".to_string(), value: None, period: None },
                ],
            },
        }
    }

    fn sample_hotspot() -> SecurityHotspot {
        SecurityHotspot {
            key: "hs1".to_string(),
            component: "proj:src/main.rs".to_string(),
            project: "proj".to_string(),
            security_category: "sql-injection".to_string(),
            vulnerability_probability: "HIGH".to_string(),
            status: "TO_REVIEW".to_string(),
            line: Some(10),
            message: "Review this".to_string(),
            rule_key: "rust:S2077".to_string(),
            text_range: None,
        }
    }

    fn sample_project() -> ProjectInfo {
        ProjectInfo {
            key: "sonar-cli".to_string(),
            name: "sonar-cli".to_string(),
            qualifier: Some("TRK".to_string()),
            visibility: Some("public".to_string()),
            last_analysis_date: Some("2026-01-01".to_string()),
        }
    }

    fn sample_history() -> MeasureHistory {
        MeasureHistory {
            metric: "coverage".to_string(),
            history: vec![
                crate::types::HistoryValue {
                    date: "2026-01-01T00:00:00+0000".to_string(),
                    value: Some("75.0".to_string()),
                },
                crate::types::HistoryValue {
                    date: "2026-01-02T00:00:00+0000".to_string(),
                    value: None,
                },
            ],
        }
    }

    fn sample_rule() -> RuleInfo {
        RuleInfo {
            key: "rust:S3776".to_string(),
            name: "Cognitive Complexity should not be too high — this is a very long rule name indeed".to_string(),
            severity: Some("CRITICAL".to_string()),
            rule_type: Some("CODE_SMELL".to_string()),
            lang: Some("rust".to_string()),
            status: Some("READY".to_string()),
            lang_name: Some("Rust".to_string()),
        }
    }

    fn sample_analysis_task() -> AnalysisTask {
        AnalysisTask {
            id: "task-123".to_string(),
            task_type: "REPORT".to_string(),
            status: "SUCCESS".to_string(),
            submitted_at: "2026-01-01T00:00:00+0000".to_string(),
            executed_at: Some("2026-01-01T00:00:01+0000".to_string()),
            analysis_id: Some("analysis-456".to_string()),
            error_message: None,
        }
    }

    // --- print_health ---

    #[test]
    fn test_print_health_up_text() {
        print_health("UP", "http://localhost:9000", false);
    }

    #[test]
    fn test_print_health_down_text() {
        print_health("DOWN", "http://localhost:9000", false);
    }

    #[test]
    fn test_print_health_up_json() {
        print_health("UP", "http://localhost:9000", true);
    }

    #[test]
    fn test_print_health_unreachable_json() {
        print_health("UNREACHABLE", "http://localhost:9000", true);
    }

    // --- print_quality_gate ---

    #[test]
    fn test_print_quality_gate_ok_text() {
        print_quality_gate(&sample_quality_gate(), "proj", false);
    }

    #[test]
    fn test_print_quality_gate_warn_text() {
        let mut gate = sample_quality_gate();
        gate.project_status.status = "WARN".to_string();
        print_quality_gate(&gate, "proj", false);
    }

    #[test]
    fn test_print_quality_gate_error_text() {
        let mut gate = sample_quality_gate();
        gate.project_status.status = "ERROR".to_string();
        print_quality_gate(&gate, "proj", false);
    }

    #[test]
    fn test_print_quality_gate_json() {
        print_quality_gate(&sample_quality_gate(), "proj", true);
    }

    #[test]
    fn test_print_quality_gate_no_conditions_text() {
        let gate = QualityGateResponse {
            project_status: ProjectStatus {
                status: "OK".to_string(),
                conditions: vec![],
            },
        };
        print_quality_gate(&gate, "proj", false);
    }

    // --- print_issues ---

    #[test]
    fn test_print_issues_text() {
        print_issues(&[sample_issue()], "proj", false);
    }

    #[test]
    fn test_print_issues_json() {
        print_issues(&[sample_issue()], "proj", true);
    }

    #[test]
    fn test_print_issues_empty() {
        print_issues(&[], "proj", false);
    }

    #[test]
    fn test_print_issues_no_line() {
        let mut issue = sample_issue();
        issue.line = None;
        issue.text_range = Some(TextRange {
            start_line: 5,
            end_line: 10,
            start_offset: Some(0),
            end_offset: Some(10),
        });
        print_issues(&[issue], "proj", false);
    }

    #[test]
    fn test_print_issues_no_tags() {
        let mut issue = sample_issue();
        issue.tags = vec![];
        print_issues(&[issue], "proj", false);
    }

    // --- print_measures ---

    #[test]
    fn test_print_measures_text() {
        print_measures(&sample_measures_response(), false);
    }

    #[test]
    fn test_print_measures_json() {
        print_measures(&sample_measures_response(), true);
    }

    // --- print_coverage ---

    #[test]
    fn test_print_coverage_text() {
        let files = vec![
            FileCoverage {
                file: "src/main.rs".to_string(),
                coverage_percent: 75.0,
                uncovered_lines: 10,
                lines_to_cover: 40,
            },
        ];
        print_coverage(&files, "proj", false);
    }

    #[test]
    fn test_print_coverage_json() {
        let files = vec![
            FileCoverage {
                file: "src/main.rs".to_string(),
                coverage_percent: 0.0,
                uncovered_lines: 50,
                lines_to_cover: 50,
            },
        ];
        print_coverage(&files, "proj", true);
    }

    #[test]
    fn test_print_coverage_empty() {
        print_coverage(&[], "proj", false);
    }

    // --- print_duplications ---

    #[test]
    fn test_print_duplications_text_no_blocks() {
        let files = vec![FileDuplication {
            file: "src/client.rs".to_string(),
            duplicated_lines: 10,
            duplicated_density: 5.0,
            blocks: vec![],
        }];
        print_duplications(&files, "proj", false, false);
    }

    #[test]
    fn test_print_duplications_text_with_blocks() {
        let files = vec![FileDuplication {
            file: "src/client.rs".to_string(),
            duplicated_lines: 10,
            duplicated_density: 5.0,
            blocks: vec![DuplicationBlockDetail {
                from_line: 16,
                size: 10,
                duplicated_in: "src/issues.rs".to_string(),
                duplicated_in_line: 10,
            }],
        }];
        print_duplications(&files, "proj", false, true);
    }

    #[test]
    fn test_print_duplications_json() {
        let files = vec![FileDuplication {
            file: "src/client.rs".to_string(),
            duplicated_lines: 10,
            duplicated_density: 5.0,
            blocks: vec![],
        }];
        print_duplications(&files, "proj", true, true);
    }

    #[test]
    fn test_print_duplications_empty() {
        print_duplications(&[], "proj", false, false);
    }

    // --- print_hotspots ---

    #[test]
    fn test_print_hotspots_text() {
        print_hotspots(&[sample_hotspot()], "proj", false);
    }

    #[test]
    fn test_print_hotspots_json() {
        print_hotspots(&[sample_hotspot()], "proj", true);
    }

    #[test]
    fn test_print_hotspots_empty() {
        print_hotspots(&[], "proj", false);
    }

    #[test]
    fn test_print_hotspots_no_line() {
        let mut hs = sample_hotspot();
        hs.line = None;
        print_hotspots(&[hs], "proj", false);
    }

    // --- print_projects ---

    #[test]
    fn test_print_projects_text() {
        print_projects(&[sample_project()], false);
    }

    #[test]
    fn test_print_projects_json() {
        print_projects(&[sample_project()], true);
    }

    #[test]
    fn test_print_projects_empty() {
        print_projects(&[], false);
    }

    #[test]
    fn test_print_projects_no_visibility() {
        let mut p = sample_project();
        p.visibility = None;
        p.last_analysis_date = None;
        print_projects(&[p], false);
    }

    // --- print_history ---

    #[test]
    fn test_print_history_text() {
        print_history(&[sample_history()], "proj", false);
    }

    #[test]
    fn test_print_history_json() {
        print_history(&[sample_history()], "proj", true);
    }

    #[test]
    fn test_print_history_empty() {
        print_history(&[], "proj", false);
    }

    // --- print_rules ---

    #[test]
    fn test_print_rules_text() {
        print_rules(&[sample_rule()], false);
    }

    #[test]
    fn test_print_rules_json() {
        print_rules(&[sample_rule()], true);
    }

    #[test]
    fn test_print_rules_empty() {
        print_rules(&[], false);
    }

    #[test]
    fn test_print_rules_short_name() {
        let mut rule = sample_rule();
        rule.name = "Short name".to_string();
        rule.severity = None;
        rule.rule_type = None;
        rule.lang = None;
        rule.lang_name = None;
        print_rules(&[rule], false);
    }

    // --- print_source ---

    #[test]
    fn test_print_source_text() {
        let lines = vec![
            SourceLine { line: 1, code: "fn main() {}".to_string() },
            SourceLine { line: 2, code: "".to_string() },
        ];
        print_source(&lines, false);
    }

    #[test]
    fn test_print_source_json() {
        let lines = vec![
            SourceLine { line: 1, code: "fn main() {}".to_string() },
        ];
        print_source(&lines, true);
    }

    #[test]
    fn test_print_source_empty() {
        print_source(&[], false);
    }

    // --- print_wait_result ---

    #[test]
    fn test_print_wait_result_text_full() {
        print_wait_result(&sample_analysis_task(), false);
    }

    #[test]
    fn test_print_wait_result_json() {
        print_wait_result(&sample_analysis_task(), true);
    }

    #[test]
    fn test_print_wait_result_no_optional_fields() {
        let task = AnalysisTask {
            id: "t1".to_string(),
            task_type: "REPORT".to_string(),
            status: "SUCCESS".to_string(),
            submitted_at: "2026-01-01T00:00:00+0000".to_string(),
            executed_at: None,
            analysis_id: None,
            error_message: None,
        };
        print_wait_result(&task, false);
    }
}
