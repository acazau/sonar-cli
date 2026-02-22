use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(config: SonarQubeConfig, json: bool) -> i32 {
    let client = match SonarQubeClient::new(config.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let status_result = client.get_status().await;
    match status_result {
        Ok(status) => {
            let url = config.url.clone();
            output::print_health(&status, &url, json);
            if status == "UP" { 0 } else { 1 }
        }
        Err(e) => {
            if json {
                let url = config.url.clone();
                output::print_health("UNREACHABLE", &url, json);
            } else {
                eprintln!("Failed to reach SonarQube at {}: {e}", config.url);
            }
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

    #[tokio::test]
    async fn test_run_health_up() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/system/status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status": "UP"})),
            )
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_health_down() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/system/status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status": "DOWN"})),
            )
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, false).await;
        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn test_run_health_unreachable_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/system/status"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, true).await;
        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn test_run_health_up_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/system/status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"status": "UP"})),
            )
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, true).await;
        assert_eq!(exit, 0);
    }
}
