use std::time::Duration;

use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    task_id: &str,
    timeout_secs: u64,
    poll_interval_secs: u64,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    if !json {
        eprintln!("Waiting for analysis task {task_id}...");
    }

    match client
        .wait_for_analysis(
            task_id,
            Duration::from_secs(timeout_secs),
            Duration::from_secs(poll_interval_secs),
        )
        .await
    {
        Ok(task) => {
            output::print_wait_result(&task, json);
            0
        }
        Err(e) => {
            eprintln!("Analysis failed: {e}");
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

    fn task_success_body(task_id: &str) -> serde_json::Value {
        serde_json::json!({
            "task": {
                "id": task_id,
                "type": "REPORT",
                "status": "SUCCESS",
                "submittedAt": "2026-01-01T00:00:00+0000",
                "executedAt": "2026-01-01T00:00:01+0000"
            }
        })
    }

    fn task_failed_body(task_id: &str) -> serde_json::Value {
        serde_json::json!({
            "task": {
                "id": task_id,
                "type": "REPORT",
                "status": "FAILED",
                "submittedAt": "2026-01-01T00:00:00+0000",
                "errorMessage": "Analysis failed"
            }
        })
    }

    #[tokio::test]
    async fn test_run_wait_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/ce/task"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_success_body("task-123")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        // Use short timeout and poll interval for tests
        let exit = run(config, "task-123", 10, 1, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_wait_success_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/ce/task"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_success_body("task-456")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "task-456", 10, 1, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_wait_failure() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/ce/task"))
            .respond_with(ResponseTemplate::new(200).set_body_json(task_failed_body("task-789")))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "task-789", 10, 1, false).await;
        assert_eq!(exit, 1);
    }
}
