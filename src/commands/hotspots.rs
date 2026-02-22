use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    status: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_security_hotspots(project, status).await {
        Ok(hotspots) => {
            output::print_hotspots(&hotspots, project, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to get hotspots: {e}");
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

    fn empty_hotspots_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 0},
            "hotspots": []
        })
    }

    fn hotspots_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "hotspots": [
                {
                    "key": "h1",
                    "component": "my-proj:src/main.rs",
                    "project": "my-proj",
                    "securityCategory": "sql-injection",
                    "vulnerabilityProbability": "HIGH",
                    "status": "TO_REVIEW",
                    "line": 42,
                    "message": "Make sure that...",
                    "ruleKey": "rust:S2077"
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_hotspots_empty() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/hotspots/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_hotspots_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_hotspots_with_results_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/hotspots/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(hotspots_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", Some("TO_REVIEW"), true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_hotspots_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/hotspots/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", None, false).await;
        assert_eq!(exit, 1);
    }
}
