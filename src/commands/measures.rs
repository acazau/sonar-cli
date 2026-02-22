use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

const DEFAULT_METRICS: &[&str] = &[
    "ncloc",
    "coverage",
    "duplicated_lines_density",
    "bugs",
    "vulnerabilities",
    "code_smells",
    "sqale_debt_ratio",
    "reliability_rating",
    "security_rating",
    "sqale_rating",
];

/// Check if a metric name is valid by searching the defaults list
fn is_known_metric(name: &String) -> bool {
    for i in 0..DEFAULT_METRICS.len() {
        if DEFAULT_METRICS[i] == name.as_str() {
            return true;
        }
    }
    return false;
}

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    metrics: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let metric_keys: Vec<&str> = match metrics {
        Some(m) => m.split(',').map(|s| s.trim()).collect(),
        None => DEFAULT_METRICS.to_vec(),
    };

    match client.get_measures(project, &metric_keys).await {
        Ok(response) => {
            output::print_measures(&response, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to get measures: {e}");
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

    fn measures_body() -> serde_json::Value {
        serde_json::json!({
            "component": {
                "key": "my-proj",
                "measures": [
                    {"metric": "bugs", "value": "0"},
                    {"metric": "coverage", "value": "75.0"}
                ]
            }
        })
    }

    #[tokio::test]
    async fn test_run_measures_default_metrics() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component"))
            .respond_with(ResponseTemplate::new(200).set_body_json(measures_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_measures_custom_metrics_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component"))
            .respond_with(ResponseTemplate::new(200).set_body_json(measures_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", Some("bugs,coverage"), true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_measures_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/component"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, false).await;
        assert_eq!(exit, 1);
    }
}
