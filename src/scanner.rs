//! SonarQube scanner execution
//!
//! Supports direct `sonar-scanner` execution (default) and Docker mode (opt-in).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Serialize;

use crate::client::{SonarQubeClient, SonarQubeConfig, SonarQubeError};
use crate::coverage::{convert_cobertura_to_sonarqube, is_cobertura_format};
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

/// Scanner configuration
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub client: SonarQubeConfig,
    pub use_docker: bool,
    pub scanner_image: String,
    pub scanner_path: String,
    pub source_dir: PathBuf,
    pub sources: Vec<String>,
    pub tests: Vec<String>,
    pub exclusions: Vec<String>,
    pub coverage_report_path: Option<String>,
    pub extra_properties: HashMap<String, String>,
    pub wait_for_completion: bool,
    pub wait_timeout: Duration,
    pub wait_poll_interval: Duration,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            client: SonarQubeConfig::from_env(),
            use_docker: false, // Direct execution by default
            scanner_image: "sonarsource/sonar-scanner-cli".to_string(),
            scanner_path: "sonar-scanner".to_string(),
            source_dir: PathBuf::from("."),
            sources: vec!["src".to_string()],
            tests: Vec::new(),
            exclusions: Vec::new(),
            coverage_report_path: None,
            extra_properties: HashMap::new(),
            wait_for_completion: false,
            wait_timeout: Duration::from_secs(300),
            wait_poll_interval: Duration::from_secs(5),
        }
    }
}

/// Run a scan and return the task ID (if any)
pub async fn run_scan(config: &ScannerConfig) -> Result<Option<String>, SonarQubeError> {
    let project_key = config
        .client
        .project_key
        .as_deref()
        .ok_or_else(|| SonarQubeError::Config("project key is required for scanning".into()))?;

    let work_dir = &config.source_dir;

    // Convert Cobertura coverage if needed
    if let Some(ref coverage_path) = config.coverage_report_path {
        let input_path = work_dir.join(coverage_path);
        if input_path.exists() && is_cobertura_format(&input_path) {
            let output_path = work_dir.join("coverage-sonar.xml");
            convert_cobertura_to_sonarqube(&input_path, &output_path, work_dir)
                .map_err(|e| SonarQubeError::Analysis(format!("Coverage conversion failed: {e}")))?;
            tracing::info!("Converted Cobertura coverage to SonarQube format");
        }
    }

    let output = if config.use_docker {
        run_docker_scanner(config, project_key, work_dir).await?
    } else {
        run_direct_scanner(config, project_key, work_dir).await?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(SonarQubeError::Analysis(format!(
            "Scanner failed:\nstdout: {stdout}\nstderr: {stderr}"
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(extract_task_id(&stdout))
}

/// Validate that sonar-scanner is available on PATH
pub async fn validate_scanner(scanner_path: &str) -> Result<String, SonarQubeError> {
    let output = tokio::process::Command::new(scanner_path)
        .arg("--version")
        .output()
        .await
        .map_err(|e| {
            SonarQubeError::Config(format!(
                "sonar-scanner not found at '{}': {}\n\
                 Install it from https://docs.sonarsource.com/sonarqube/latest/analyzing-source-code/scanners/sonarscanner/\n\
                 Or use --docker to run via Docker instead.",
                scanner_path, e
            ))
        })?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Build scanner arguments
fn build_scanner_args(config: &ScannerConfig, project_key: &str) -> Vec<String> {
    let mut args = vec![
        format!("-Dsonar.projectKey={}", project_key),
        format!("-Dsonar.sources={}", config.sources.join(",")),
    ];

    if !config.tests.is_empty() {
        args.push(format!("-Dsonar.tests={}", config.tests.join(",")));
    }
    if !config.exclusions.is_empty() {
        args.push(format!(
            "-Dsonar.exclusions={}",
            config.exclusions.join(",")
        ));
    }
    if config.coverage_report_path.is_some() {
        args.push("-Dsonar.coverageReportPaths=coverage-sonar.xml".to_string());
    }
    for (key, value) in &config.extra_properties {
        args.push(format!("-D{}={}", key, value));
    }
    args
}

/// Run scanner via Docker
async fn run_docker_scanner(
    config: &ScannerConfig,
    project_key: &str,
    work_dir: &Path,
) -> Result<std::process::Output, SonarQubeError> {
    let scanner_args = build_scanner_args(config, project_key);
    let container_name = format!("sonar-cli-{}", uuid::Uuid::new_v4());

    let mut cmd = tokio::process::Command::new("docker");
    cmd.arg("run")
        .arg("--rm")
        .arg(format!("--name={}", container_name))
        .arg("--network=host")
        .arg("-e")
        .arg("GIT_CONFIG_COUNT=1")
        .arg("-e")
        .arg("GIT_CONFIG_KEY_0=safe.directory")
        .arg("-e")
        .arg("GIT_CONFIG_VALUE_0=/usr/src")
        .arg("-e")
        .arg(format!("SONAR_HOST_URL={}", config.client.url))
        .arg("-e")
        .arg(format!(
            "SONAR_TOKEN={}",
            config.client.token.as_deref().unwrap_or("")
        ))
        .arg("-v")
        .arg(format!("{}:/usr/src", work_dir.display()))
        .arg(&config.scanner_image);

    for arg in &scanner_args {
        cmd.arg(arg);
    }

    let timeout = config.wait_timeout;
    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(result) => result.map_err(|e| SonarQubeError::Http(format!("Failed to run docker: {e}"))),
        Err(_) => {
            let _ = tokio::process::Command::new("docker")
                .args(["kill", &container_name])
                .output()
                .await;
            Err(SonarQubeError::Analysis(format!(
                "Docker scanner timed out after {}s",
                timeout.as_secs()
            )))
        }
    }
}

/// Run scanner directly (without Docker)
async fn run_direct_scanner(
    config: &ScannerConfig,
    project_key: &str,
    work_dir: &Path,
) -> Result<std::process::Output, SonarQubeError> {
    let scanner_args = build_scanner_args(config, project_key);

    let mut cmd = tokio::process::Command::new(&config.scanner_path);
    cmd.arg(format!("-Dsonar.host.url={}", config.client.url))
        .arg(format!("-Dsonar.projectBaseDir={}", work_dir.display()));

    if let Some(ref token) = config.client.token {
        cmd.arg(format!("-Dsonar.token={}", token));
    }

    for arg in &scanner_args {
        cmd.arg(arg);
    }

    cmd.current_dir(work_dir).output().await.map_err(|e| {
        SonarQubeError::Http(format!("Failed to run sonar-scanner: {e}"))
    })
}

/// Extract task ID from scanner output
fn extract_task_id(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .find(|line| line.contains("task?id="))
        .and_then(|line| line.split("task?id=").nth(1))
        .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
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
    has_gap.then(|| FileCoverage {
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
    fn test_extract_task_id() {
        let output = "INFO: Analysis report uploaded to server\nINFO: task?id=AYtest123 \nINFO: Done";
        assert_eq!(extract_task_id(output), Some("AYtest123".to_string()));
    }

    #[test]
    fn test_extract_task_id_not_found() {
        assert_eq!(extract_task_id("no task here"), None);
    }

    #[test]
    fn test_extract_path() {
        assert_eq!(extract_path("my-project:src/main.rs", "my-project"), "src/main.rs");
        assert_eq!(extract_path("other:path.rs", "my-project"), "other:path.rs");
    }

    #[test]
    fn test_build_scanner_args() {
        let config = ScannerConfig {
            sources: vec!["src".into(), "lib".into()],
            tests: vec!["tests".into()],
            exclusions: vec!["**/target/**".into()],
            ..Default::default()
        };
        let args = build_scanner_args(&config, "my-project");
        assert!(args.contains(&"-Dsonar.projectKey=my-project".to_string()));
        assert!(args.contains(&"-Dsonar.sources=src,lib".to_string()));
        assert!(args.contains(&"-Dsonar.tests=tests".to_string()));
        assert!(args.contains(&"-Dsonar.exclusions=**/target/**".to_string()));
    }
}
