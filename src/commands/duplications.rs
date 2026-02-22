use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::helpers;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    details: bool,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match helpers::fetch_extended_data(&client, project).await {
        Ok(data) => {
            output::print_duplications(&data.duplications, project, json, details);
            0
        }
        Err(e) => {
            eprintln!("Failed to get duplications: {e}");
            1
        }
    }
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

    fn component_tree_no_dups() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "my-proj:src/main.rs",
                    "path": "src/main.rs",
                    "measures": [
                        {"metric": "duplicated_lines", "value": "0"},
                        {"metric": "duplicated_lines_density", "value": "0.0"},
                        {"metric": "duplicated_blocks", "value": "0"}
                    ]
                }
            ]
        })
    }

    fn coverage_tree_response() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "my-proj:src/main.rs",
                    "path": "src/main.rs",
                    "measures": [
                        {"metric": "coverage", "value": "100.0"},
                        {"metric": "uncovered_lines", "value": "0"},
                        {"metric": "lines_to_cover", "value": "10"}
                    ]
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_duplications_no_dups() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        // First call: get_files_with_duplications (dup metrics)
        // Second call: get_files_coverage (coverage metrics)
        // Both use /api/measures/component_tree
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(component_tree_no_dups()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", false, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(component_tree_no_dups()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", true, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_api_error() {
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
        let exit = run(config, "my-proj", false, false).await;
        // fetch_extended_data swallows the error with unwrap_or_default, so still 0
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_api_error_json() {
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
        let exit = run(config, "my-proj", true, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_with_dups() {
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
        // Test with details=true, json=false (text output)
        let exit = run(config, "my-proj", true, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_with_dups_json() {
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
        // Test with details=false, json=true
        let exit = run(config, "my-proj", false, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_duplications_coverage_fallback() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        // Return coverage data when queried for coverage metrics
        Mock::given(method("GET"))
            .and(path("/api/measures/component_tree"))
            .respond_with(ResponseTemplate::new(200).set_body_json(coverage_tree_response()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", false, true).await;
        assert_eq!(exit, 0);
    }
}
