use crate::client::{RuleSearchParams, SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    search: Option<&str>,
    language: Option<&str>,
    severity: Option<&str>,
    rule_type: Option<&str>,
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

    let params = RuleSearchParams {
        search,
        language,
        severity,
        rule_type,
        status,
    };

    match client.get_all_rules(&params).await {
        Ok(rules) => {
            output::print_rules(&rules, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch rules: {e}");
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

    fn rules_body() -> serde_json::Value {
        serde_json::json!({
            "total": 1,
            "rules": [
                {
                    "key": "rust:S3776",
                    "name": "Cognitive Complexity should not be too high",
                    "severity": "CRITICAL",
                    "type": "CODE_SMELL",
                    "lang": "rust",
                    "status": "READY",
                    "langName": "Rust"
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_rules_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/rules/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(rules_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, None, None, None, None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_rules_with_filters_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/rules/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(rules_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(
            config,
            Some("cognitive"),
            Some("rust"),
            Some("CRITICAL"),
            Some("CODE_SMELL"),
            Some("READY"),
            true,
        )
        .await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_rules_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/rules/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, None, None, None, None, None, false).await;
        assert_eq!(exit, 1);
    }
}
