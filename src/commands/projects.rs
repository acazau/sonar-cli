use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    search: Option<&str>,
    qualifier: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_all_projects(search, qualifier).await {
        Ok(projects) => {
            output::print_projects(&projects, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch projects: {e}");
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

    fn projects_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "components": [
                {
                    "key": "sonar-cli",
                    "name": "sonar-cli",
                    "qualifier": "TRK"
                }
            ]
        })
    }

    fn empty_projects_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 0},
            "components": []
        })
    }

    #[tokio::test]
    async fn test_run_projects_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/components/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(projects_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_projects_with_search_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/components/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_projects_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, Some("sonar"), Some("TRK"), true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_projects_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/components/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, None, None, false).await;
        assert_eq!(exit, 1);
    }
}
