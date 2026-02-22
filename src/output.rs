//! Output formatting — human-readable and JSON

use crate::helpers::{FileCoverage, FileDuplication};
use crate::types::*;

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
