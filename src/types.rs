//! SonarQube API response types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// SonarQube issue from the API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SonarIssue {
    pub key: String,
    pub rule: String,
    pub severity: String,
    pub component: String,
    pub project: String,
    pub line: Option<u32>,
    #[serde(rename = "textRange")]
    pub text_range: Option<TextRange>,
    pub message: String,
    #[serde(rename = "type")]
    pub issue_type: String,
    pub status: String,
    #[serde(default)]
    pub resolution: Option<String>,
    #[serde(default)]
    pub debt: Option<String>,
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Text range for an issue
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextRange {
    #[serde(rename = "startLine")]
    pub start_line: u32,
    #[serde(rename = "endLine")]
    pub end_line: u32,
    #[serde(rename = "startOffset")]
    pub start_offset: Option<u32>,
    #[serde(rename = "endOffset")]
    pub end_offset: Option<u32>,
}

/// Response from the issues search API
#[derive(Debug, Clone, Deserialize)]
pub struct IssuesResponse {
    pub total: usize,
    pub issues: Vec<SonarIssue>,
}

/// Quality gate status
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QualityGateResponse {
    #[serde(rename = "projectStatus")]
    pub project_status: ProjectStatus,
}

/// Project status from quality gate
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectStatus {
    pub status: String,
    #[serde(default)]
    pub conditions: Vec<QualityGateCondition>,
}

/// Individual quality gate condition
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct QualityGateCondition {
    pub status: String,
    #[serde(rename = "metricKey")]
    pub metric_key: String,
    #[serde(default)]
    pub comparator: Option<String>,
    #[serde(rename = "errorThreshold")]
    pub error_threshold: Option<String>,
    #[serde(rename = "actualValue")]
    pub actual_value: Option<String>,
}

/// Project measures (metrics)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeasuresResponse {
    pub component: MeasuresComponent,
}

/// Component with measures
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeasuresComponent {
    pub key: String,
    #[serde(default)]
    pub measures: Vec<Measure>,
}

/// Individual metric measure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Measure {
    pub metric: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub period: Option<MeasurePeriod>,
}

/// Measure period (for new code)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeasurePeriod {
    pub value: String,
}

/// Analysis status response
#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisResponse {
    pub task: AnalysisTask,
}

/// Analysis task details
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AnalysisTask {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub status: String,
    #[serde(rename = "submittedAt")]
    pub submitted_at: String,
    #[serde(rename = "executedAt")]
    pub executed_at: Option<String>,
    #[serde(rename = "analysisId")]
    pub analysis_id: Option<String>,
    #[serde(rename = "errorMessage")]
    pub error_message: Option<String>,
}

/// Task status values
pub mod task_status {
    pub const SUCCESS: &str = "SUCCESS";
    pub const FAILED: &str = "FAILED";
    pub const CANCELED: &str = "CANCELED";
}

/// Issue severity values
pub mod severity {
    pub const INFO: &str = "INFO";
    pub const MINOR: &str = "MINOR";
    pub const MAJOR: &str = "MAJOR";
    pub const CRITICAL: &str = "CRITICAL";
    pub const BLOCKER: &str = "BLOCKER";

    /// All severity levels in ascending order
    pub const ALL: &[&str] = &[INFO, MINOR, MAJOR, CRITICAL, BLOCKER];

    /// Returns the ordinal of a severity level (higher = more severe)
    pub fn ordinal(sev: &str) -> usize {
        match sev {
            INFO => 0,
            MINOR => 1,
            MAJOR => 2,
            CRITICAL => 3,
            BLOCKER => 4,
            _ => 0,
        }
    }
}

/// Response from component tree measures API
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentTreeResponse {
    pub paging: Option<Paging>,
    pub components: Vec<TreeComponent>,
}

/// Paging information
#[derive(Debug, Clone, Deserialize)]
pub struct Paging {
    pub total: usize,
}

/// Component in tree response with measures
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TreeComponent {
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub qualifier: Option<String>,
    #[serde(default)]
    pub measures: Vec<Measure>,
}

/// Response from duplications API
#[derive(Debug, Clone, Deserialize)]
pub struct DuplicationsResponse {
    #[serde(default)]
    pub duplications: Vec<Duplication>,
    #[serde(default)]
    pub files: HashMap<String, DuplicationFile>,
}

/// A duplication group (set of duplicated blocks)
#[derive(Debug, Clone, Deserialize)]
pub struct Duplication {
    pub blocks: Vec<DuplicationBlock>,
}

/// A single duplicated block
#[derive(Debug, Clone, Deserialize)]
pub struct DuplicationBlock {
    #[serde(rename = "_ref")]
    pub file_ref: String,
    pub from: u32,
    pub size: u32,
}

/// File information in duplications response
#[derive(Debug, Clone, Deserialize)]
pub struct DuplicationFile {
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
}

/// Response from the security hotspots search API
#[derive(Debug, Clone, Deserialize)]
pub struct HotspotsResponse {
    pub paging: Paging,
    pub hotspots: Vec<SecurityHotspot>,
}

/// Security hotspot from the API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityHotspot {
    pub key: String,
    pub component: String,
    pub project: String,
    #[serde(rename = "securityCategory")]
    pub security_category: String,
    #[serde(rename = "vulnerabilityProbability")]
    pub vulnerability_probability: String,
    pub status: String,
    #[serde(default)]
    pub line: Option<u32>,
    pub message: String,
    #[serde(rename = "ruleKey")]
    pub rule_key: String,
    #[serde(rename = "textRange")]
    #[serde(default)]
    pub text_range: Option<TextRange>,
}

/// Response from the components/search API (projects listing)
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectsSearchResponse {
    pub paging: Paging,
    pub components: Vec<ProjectInfo>,
}

/// Project information from the components/search API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProjectInfo {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub qualifier: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
    #[serde(rename = "lastAnalysisDate")]
    #[serde(default)]
    pub last_analysis_date: Option<String>,
}

/// Response from the measures/search_history API
#[derive(Debug, Clone, Deserialize)]
pub struct MeasuresHistoryResponse {
    pub paging: Paging,
    pub measures: Vec<MeasureHistory>,
}

/// A metric's history data
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeasureHistory {
    pub metric: String,
    pub history: Vec<HistoryValue>,
}

/// A single historical data point
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoryValue {
    pub date: String,
    #[serde(default)]
    pub value: Option<String>,
}

/// Response from the rules/search API
#[derive(Debug, Clone, Deserialize)]
pub struct RulesSearchResponse {
    pub total: usize,
    pub rules: Vec<RuleInfo>,
}

/// Rule information from the rules/search API
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleInfo {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(rename = "type")]
    #[serde(default)]
    pub rule_type: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(rename = "langName")]
    #[serde(default)]
    pub lang_name: Option<String>,
}

/// A line of source code (constructed from API responses)
#[derive(Debug, Clone, Serialize)]
pub struct SourceLine {
    pub line: usize,
    pub code: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_issue() {
        let json = r#"{
            "key": "AYtest123",
            "rule": "rust:S1135",
            "severity": "INFO",
            "component": "project:src/main.rs",
            "project": "project",
            "line": 42,
            "message": "Complete the task associated to this TODO comment.",
            "type": "CODE_SMELL",
            "status": "OPEN",
            "tags": ["todo"]
        }"#;

        let issue: SonarIssue = serde_json::from_str(json).unwrap();
        assert_eq!(issue.key, "AYtest123");
        assert_eq!(issue.severity, "INFO");
        assert_eq!(issue.line, Some(42));
    }

    #[test]
    fn test_deserialize_quality_gate() {
        let json = r#"{
            "projectStatus": {
                "status": "OK",
                "conditions": [
                    {
                        "status": "OK",
                        "metricKey": "new_bugs",
                        "comparator": "GT",
                        "errorThreshold": "0",
                        "actualValue": "0"
                    }
                ]
            }
        }"#;

        let response: QualityGateResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.project_status.status, "OK");
        assert_eq!(response.project_status.conditions.len(), 1);
    }

    #[test]
    fn test_severity_ordinal() {
        assert_eq!(severity::ordinal("INFO"), 0);
        assert_eq!(severity::ordinal("MINOR"), 1);
        assert_eq!(severity::ordinal("MAJOR"), 2);
        assert_eq!(severity::ordinal("CRITICAL"), 3);
        assert_eq!(severity::ordinal("BLOCKER"), 4);
    }
}
