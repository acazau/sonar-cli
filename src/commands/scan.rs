use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::scanner::{self, ScannerConfig};

pub async fn run(
    config: SonarQubeConfig,
    source_dir: PathBuf,
    sources: Vec<String>,
    tests: Vec<String>,
    exclusions: Vec<String>,
    coverage_report: Option<String>,
    scanner_path: String,
    docker: bool,
    docker_image: Option<String>,
    wait: bool,
    extra_props: Vec<String>,
    json: bool,
) -> i32 {
    let project = match config.project_key.as_deref() {
        Some(p) => p.to_string(),
        None => {
            eprintln!("Project key is required. Use --project or set SONAR_PROJECT_KEY.");
            return 1;
        }
    };

    // Validate scanner availability
    if !docker {
        if let Err(e) = scanner::validate_scanner(&scanner_path).await {
            eprintln!("{e}");
            return 1;
        }
    }

    // Parse extra properties
    let mut extra_properties = HashMap::new();
    for prop in &extra_props {
        if let Some((key, value)) = prop.split_once('=') {
            extra_properties.insert(key.to_string(), value.to_string());
        } else {
            eprintln!("Invalid property format: {prop} (expected key=value)");
            return 1;
        }
    }

    let scan_config = ScannerConfig {
        client: config.clone(),
        use_docker: docker,
        scanner_image: docker_image.unwrap_or_else(|| "sonarsource/sonar-scanner-cli".into()),
        scanner_path,
        source_dir: source_dir.clone(),
        sources,
        tests,
        exclusions,
        coverage_report_path: coverage_report,
        extra_properties,
        wait_for_completion: wait,
        wait_timeout: Duration::from_secs(300),
        wait_poll_interval: Duration::from_secs(5),
    };

    if !json {
        let mode = if docker { "Docker" } else { "direct" };
        eprintln!(
            "Running sonar-scanner ({mode}) for project '{project}' in {}...",
            source_dir.display()
        );
    }

    let task_id = match scanner::run_scan(&scan_config).await {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Scan failed: {e}");
            return 1;
        }
    };

    if !json {
        if let Some(ref id) = task_id {
            eprintln!("Analysis submitted — task ID: {id}");
        } else {
            eprintln!("Analysis submitted (no task ID captured)");
        }
    }

    if wait {
        if let Some(ref id) = task_id {
            let client = match SonarQubeClient::new(config.clone()) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create client: {e}");
                    return 1;
                }
            };

            if !json {
                eprintln!("Waiting for analysis to complete...");
            }

            match client
                .wait_for_analysis(
                    id,
                    scan_config.wait_timeout,
                    scan_config.wait_poll_interval,
                )
                .await
            {
                Ok(task) => {
                    output::print_wait_result(&task, json);

                    // Show quality gate and issues summary
                    if !json {
                        eprintln!();
                        if let Ok(qg) = client.get_quality_gate(&project).await {
                            output::print_quality_gate(&qg, &project, false);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Analysis failed: {e}");
                    return 1;
                }
            }
        }
    }

    0
}
