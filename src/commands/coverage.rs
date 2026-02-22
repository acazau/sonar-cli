use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::helpers::{self, FileCoverage};

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    min_coverage: Option<f64>,
    sort: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let files = match client.get_files_coverage(project).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to get coverage: {e}");
            return 1;
        }
    };

    let mut coverage: Vec<FileCoverage> = files
        .into_iter()
        .filter_map(|f| {
            let path = helpers::extract_path(&f.key, project);
            let cov: f64 = f
                .measures
                .iter()
                .find(|m| m.metric == "coverage")
                .and_then(|m| m.value.as_ref())
                .and_then(|v| v.parse().ok())
                .unwrap_or(100.0);
            let uncovered: u32 = helpers::parse_measure(&f.measures, "uncovered_lines");
            let lines_to_cover: u32 = helpers::parse_measure(&f.measures, "lines_to_cover");

            if let Some(min) = min_coverage {
                if cov >= min {
                    return None;
                }
            }

            Some(FileCoverage {
                file: path,
                coverage_percent: cov,
                uncovered_lines: uncovered,
                lines_to_cover,
            })
        })
        .collect();

    match sort.unwrap_or("coverage") {
        "uncovered" => coverage.sort_by(|a, b| b.uncovered_lines.cmp(&a.uncovered_lines)),
        "file" => coverage.sort_by(|a, b| a.file.cmp(&b.file)),
        _ => coverage.sort_by(|a, b| {
            a.coverage_percent
                .partial_cmp(&b.coverage_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        }),
    }

    output::print_coverage(&coverage, project, json);
    0
}
