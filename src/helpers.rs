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

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("my-project:src/main.rs", "my-project"), "src/main.rs");
        assert_eq!(extract_path("other:path.rs", "my-project"), "other:path.rs");
    }
}
