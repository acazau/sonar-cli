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
    pub p: usize,
    pub ps: usize,
    pub issues: Vec<SonarIssue>,
    #[serde(default)]
    pub components: Vec<SonarComponent>,
}

/// Component information
#[derive(Debug, Clone, Deserialize)]
pub struct SonarComponent {
    pub key: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub qualifier: Option<String>,
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
    pub const PENDING: &str = "PENDING";
    pub const IN_PROGRESS: &str = "IN_PROGRESS";
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

/// Issue type values
pub mod issue_type {
    pub const CODE_SMELL: &str = "CODE_SMELL";
    pub const BUG: &str = "BUG";
    pub const VULNERABILITY: &str = "VULNERABILITY";
    pub const SECURITY_HOTSPOT: &str = "SECURITY_HOTSPOT";
}

/// Response from component tree measures API
#[derive(Debug, Clone, Deserialize)]
pub struct ComponentTreeResponse {
    pub paging: Option<Paging>,
    #[serde(rename = "baseComponent")]
    pub base_component: Option<TreeComponent>,
    pub components: Vec<TreeComponent>,
}

/// Paging information
#[derive(Debug, Clone, Deserialize)]
pub struct Paging {
    #[serde(rename = "pageIndex")]
    pub page_index: usize,
    #[serde(rename = "pageSize")]
    pub page_size: usize,
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
    #[serde(rename = "projectName")]
    #[serde(default)]
    pub project_name: Option<String>,
}

/// Response from the security hotspots search API
#[derive(Debug, Clone, Deserialize)]
pub struct HotspotsResponse {
    pub paging: Paging,
    pub hotspots: Vec<SecurityHotspot>,
    #[serde(default)]
    pub components: Vec<HotspotComponent>,
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

/// Component information for hotspots
#[derive(Debug, Clone, Deserialize)]
pub struct HotspotComponent {
    pub key: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub qualifier: Option<String>,
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
