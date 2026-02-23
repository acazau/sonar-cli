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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn try_mock_server() -> Option<MockServer> {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(l) => l,
            Err(_) => return None,
        };
        Some(MockServer::builder().listener(listener).start().await)
    }

    fn coverage_tree_body(coverage_val: &str) -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "my-proj:src/main.rs",
                    "path": "src/main.rs",
                    "measures": [
                        {"metric": "coverage", "value": coverage_val},
                        {"metric": "uncovered_lines", "value": "20"},
                        {"metric": "lines_to_cover", "value": "100"}
                    ]
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_coverage_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_body("75.0")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_coverage_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_body("40.0")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, None, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_coverage_with_min_filter() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_body("95.0")))
            .mount(&mock_server)
            .await;

        // min_coverage=80 should filter out files with coverage >= 80 (95.0 gets filtered)
        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", Some(80.0), None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_coverage_sort_by_uncovered() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_body("50.0")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, Some("uncovered"), false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_coverage_sort_by_file() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_body("50.0")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, Some("file"), false).await;
        assert_eq!(exit, 0);
    }

    fn coverage_tree_multi_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 3},
            "components": [
                {
                    "key": "my-proj:src/main.rs",
                    "path": "src/main.rs",
                    "measures": [
                        {"metric": "coverage", "value": "30.0"},
                        {"metric": "uncovered_lines", "value": "70"},
                        {"metric": "lines_to_cover", "value": "100"}
                    ]
                },
                {
                    "key": "my-proj:src/lib.rs",
                    "path": "src/lib.rs",
                    "measures": [
                        {"metric": "coverage", "value": "90.0"},
                        {"metric": "uncovered_lines", "value": "5"},
                        {"metric": "lines_to_cover", "value": "50"}
                    ]
                },
                {
                    "key": "my-proj:src/utils.rs",
                    "path": "src/utils.rs",
                    "measures": [
                        {"metric": "coverage", "value": "60.0"},
                        {"metric": "uncovered_lines", "value": "20"},
                        {"metric": "lines_to_cover", "value": "50"}
                    ]
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_coverage_default_sort_multiple_files() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_multi_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        // Default sort (None → "coverage") with 3 files exercises the sort comparator
        let exit = run(config, "my-proj", None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_coverage_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, None, false).await;
        assert_eq!(exit, 1);
    }
}
