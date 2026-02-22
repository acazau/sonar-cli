//! Shared helper types and functions for SonarQube data processing

use serde::Serialize;

use crate::client::{SonarQubeClient, SonarQubeError};
use crate::types::*;

/// Extended SonarQube data for downstream use
#[derive(Debug, Clone, Serialize)]
pub struct ExtendedSonarData {
    pub duplications: Vec<FileDuplication>,
    pub coverage_gaps: Vec<FileCoverage>,
}

/// Duplication info for a single file
#[derive(Debug, Clone, Serialize)]
pub struct FileDuplication {
    pub file: String,
    pub duplicated_lines: u32,
    pub duplicated_density: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<DuplicationBlockDetail>,
}

/// Detail of a single duplication block
#[derive(Debug, Clone, Serialize)]
pub struct DuplicationBlockDetail {
    pub from_line: u32,
    pub size: u32,
    pub duplicated_in: String,
    pub duplicated_in_line: u32,
}

/// Coverage info for a single file
#[derive(Debug, Clone, Serialize)]
pub struct FileCoverage {
    pub file: String,
    pub coverage_percent: f64,
    pub uncovered_lines: u32,
    pub lines_to_cover: u32,
}

/// Extract file path from component key (strips `project:` prefix)
pub fn extract_path(component: &str, project_key: &str) -> String {
    if let Some(path) = component.strip_prefix(&format!("{}:", project_key)) {
        path.to_string()
    } else {
        component.to_string()
    }
}

/// Parse a measure value from a list of measures
pub fn parse_measure<T: std::str::FromStr + Default>(measures: &[Measure], metric_name: &str) -> T {
    measures
        .iter()
        .find(|m| m.metric == metric_name)
        .and_then(|m| m.value.as_ref())
        .and_then(|v| v.parse().ok())
        .unwrap_or_default()
}

/// Fetch extended data (duplications + coverage per file)
pub async fn fetch_extended_data(
    client: &SonarQubeClient,
    project_key: &str,
) -> Result<ExtendedSonarData, SonarQubeError> {
    let files_with_dups = client
        .get_files_with_duplications(project_key)
        .await
        .unwrap_or_default();

    let mut duplications = Vec::new();
    for file in files_with_dups {
        if let Some(mut dup) = convert_to_duplication(&file, project_key) {
            if let Ok(dup_response) = client.get_duplications(&file.key).await {
                dup.blocks = extract_duplication_blocks(&dup_response, &file.key);
            }
            duplications.push(dup);
        }
    }

    let mut coverage_gaps: Vec<FileCoverage> = client
        .get_files_coverage(project_key)
        .await
        .map(|files| {
            files
                .into_iter()
                .filter_map(|f| convert_to_coverage(&f, project_key))
                .collect()
        })
        .unwrap_or_default();

    coverage_gaps.sort_by(|a, b| {
        a.coverage_percent
            .partial_cmp(&b.coverage_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(ExtendedSonarData {
        duplications,
        coverage_gaps,
    })
}

fn convert_to_duplication(file: &TreeComponent, project_key: &str) -> Option<FileDuplication> {
    let path = extract_path(&file.key, project_key);
    let dup_lines: u32 = parse_measure(&file.measures, "duplicated_lines");
    let dup_density: f64 = parse_measure(&file.measures, "duplicated_lines_density");

    if dup_lines > 0 {
        Some(FileDuplication {
            file: path,
            duplicated_lines: dup_lines,
            duplicated_density: dup_density,
            blocks: Vec::new(),
        })
    } else {
        None
    }
}

fn convert_to_coverage(file: &TreeComponent, project_key: &str) -> Option<FileCoverage> {
    let path = extract_path(&file.key, project_key);
    let coverage: f64 = file
        .measures
        .iter()
        .find(|m| m.metric == "coverage")
        .and_then(|m| m.value.as_ref())
        .and_then(|v| v.parse().ok())
        .unwrap_or(100.0);
    let uncovered_lines: u32 = parse_measure(&file.measures, "uncovered_lines");
    let lines_to_cover: u32 = parse_measure(&file.measures, "lines_to_cover");

    let has_gap = uncovered_lines > 0 || coverage < 80.0;
    has_gap.then_some(FileCoverage {
        file: path,
        coverage_percent: coverage,
        uncovered_lines,
        lines_to_cover,
    })
}

fn extract_duplication_blocks(
    response: &DuplicationsResponse,
    current_file_key: &str,
) -> Vec<DuplicationBlockDetail> {
    let mut blocks = Vec::new();

    for dup in &response.duplications {
        let current_block = dup.blocks.iter().find(|b| {
            response
                .files
                .get(&b.file_ref)
                .map(|f| f.key == current_file_key)
                .unwrap_or(false)
        });

        for other_block in &dup.blocks {
            let other_file = response.files.get(&other_block.file_ref);
            if let (Some(curr), Some(other)) = (current_block, other_file) {
                if other.key == current_file_key && other_block.file_ref == curr.file_ref {
                    continue;
                }
                blocks.push(DuplicationBlockDetail {
                    from_line: curr.from,
                    size: curr.size,
                    duplicated_in: other.name.clone().unwrap_or_else(|| other.key.clone()),
                    duplicated_in_line: other_block.from,
                });
            }
        }
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Measure;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use crate::client::{SonarQubeConfig, SonarQubeClient};

    async fn try_mock_server() -> Option<MockServer> {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return None,
        };
        Some(MockServer::builder().listener(listener).start().await)
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("my-project:src/main.rs", "my-project"), "src/main.rs");
        assert_eq!(extract_path("other:path.rs", "my-project"), "other:path.rs");
    }

    #[test]
    fn test_extract_path_no_prefix() {
        // Component key without colon separator returns unchanged
        assert_eq!(extract_path("standalone", "my-project"), "standalone");
    }

    #[test]
    fn test_parse_measure_found() {
        let measures = vec![
            Measure { metric: "coverage".to_string(), value: Some("85.5".to_string()), period: None },
            Measure { metric: "bugs".to_string(), value: Some("3".to_string()), period: None },
        ];
        let coverage: f64 = parse_measure(&measures, "coverage");
        assert!((coverage - 85.5).abs() < 0.001);
        let bugs: u32 = parse_measure(&measures, "bugs");
        assert_eq!(bugs, 3);
    }

    #[test]
    fn test_parse_measure_not_found() {
        let measures: Vec<Measure> = vec![];
        let val: u32 = parse_measure(&measures, "nonexistent");
        assert_eq!(val, 0);
    }

    #[test]
    fn test_parse_measure_invalid_value() {
        let measures = vec![
            Measure { metric: "coverage".to_string(), value: Some("not-a-number".to_string()), period: None },
        ];
        let val: f64 = parse_measure(&measures, "coverage");
        // Default for f64 is 0.0
        assert_eq!(val, 0.0_f64);
    }

    #[test]
    fn test_parse_measure_none_value() {
        let measures = vec![
            Measure { metric: "coverage".to_string(), value: None, period: None },
        ];
        let val: u32 = parse_measure(&measures, "coverage");
        assert_eq!(val, 0);
    }

    #[tokio::test]
    async fn test_fetch_extended_data_no_dups_no_gaps() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        let tree_response = serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "my-proj:src/main.rs",
                    "path": "src/main.rs",
                    "measures": [
                        {"metric": "duplicated_lines", "value": "0"},
                        {"metric": "duplicated_lines_density", "value": "0.0"},
                        {"metric": "duplicated_blocks", "value": "0"},
                        {"metric": "coverage", "value": "100.0"},
                        {"metric": "uncovered_lines", "value": "0"},
                        {"metric": "lines_to_cover", "value": "10"}
                    ]
                }
            ]
        });

        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tree_response))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let client = SonarQubeClient::new(config).unwrap();
        let result = fetch_extended_data(&client, "my-proj").await;
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(data.duplications.is_empty());
        assert!(data.coverage_gaps.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_extended_data_with_dup() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        let tree_response = serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "my-proj:src/client.rs",
                    "path": "src/client.rs",
                    "measures": [
                        {"metric": "duplicated_lines", "value": "10"},
                        {"metric": "duplicated_lines_density", "value": "5.0"},
                        {"metric": "duplicated_blocks", "value": "1"},
                        {"metric": "coverage", "value": "60.0"},
                        {"metric": "uncovered_lines", "value": "40"},
                        {"metric": "lines_to_cover", "value": "100"}
                    ]
                }
            ]
        });

        let dups_response = serde_json::json!({
            "duplications": [],
            "files": {}
        });

        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(tree_response))
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/duplications/show"))
            .respond_with(ResponseTemplate::new(200).set_body_json(dups_response))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let client = SonarQubeClient::new(config).unwrap();
        let result = fetch_extended_data(&client, "my-proj").await;
        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.duplications.len(), 1);
        assert_eq!(data.duplications[0].file, "src/client.rs");
        assert_eq!(data.duplications[0].duplicated_lines, 10);
    }
}
