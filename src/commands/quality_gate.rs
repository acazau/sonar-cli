use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(config: SonarQubeConfig, project: &str, fail_on_error: bool, json: bool) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_quality_gate(project).await {
        Ok(response) => {
            output::print_quality_gate(&response, project, json);
            let status = &response.project_status.status;
            if fail_on_error && status != "OK" {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("Failed to get quality gate: {e}");
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

    fn quality_gate_ok_body() -> serde_json::Value {
        serde_json::json!({
            "projectStatus": {
                "status": "OK",
                "conditions": [
                    {
                        "status": "OK",
                        "metricKey": "new_bugs",
                        "comparator": "GT",
                        "errorThreshold": "0",
                        "actualValue": "0"
                    }
                ]
            }
        })
    }

    fn quality_gate_error_body() -> serde_json::Value {
        serde_json::json!({
            "projectStatus": {
                "status": "ERROR",
                "conditions": [
                    {
                        "status": "ERROR",
                        "metricKey": "coverage",
                        "comparator": "LT",
                        "errorThreshold": "80",
                        "actualValue": "50"
                    }
                ]
            }
        })
    }

    #[tokio::test]
    async fn test_run_quality_gate_ok() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(quality_gate_ok_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", false, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_quality_gate_ok_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(quality_gate_ok_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", false, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_quality_gate_error_fail_on_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(quality_gate_error_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        // fail_on_error=true should return exit code 1
        let exit = run(config, "my-proj", true, false).await;
        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn test_run_quality_gate_error_no_fail() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .respond_with(ResponseTemplate::new(200).set_body_json(quality_gate_error_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        // fail_on_error=false should still return 0
        let exit = run(config, "my-proj", false, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_quality_gate_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", false, false).await;
        assert_eq!(exit, 1);
    }
}
