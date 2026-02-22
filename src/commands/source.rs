use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::SourceLine;

pub async fn run(
    config: SonarQubeConfig,
    component: &str,
    from: Option<usize>,
    to: Option<usize>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    // Use /api/sources/show when line range is specified, otherwise use /api/sources/raw
    let lines = if from.is_some() || to.is_some() {
        client.get_source_show(component, from, to).await
    } else {
        match client.get_source_raw(component).await {
            Ok(raw) => {
                let lines: Vec<SourceLine> = raw
                    .lines()
                    .enumerate()
                    .map(|(i, line)| SourceLine {
                        line: i + 1,
                        code: line.to_string(),
                    })
                    .collect();
                Ok(lines)
            }
            Err(e) => Err(e),
        }
    };

    match lines {
        Ok(lines) => {
            output::print_source(&lines, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch source: {e}");
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
    async fn test_run_source_raw() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/sources/raw"))
            .respond_with(ResponseTemplate::new(200).set_body_string("fn main() {}\n"))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj:src/main.rs", None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_source_raw_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/sources/raw"))
            .respond_with(ResponseTemplate::new(200).set_body_string("fn main() {}\n"))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj:src/main.rs", None, None, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_source_show_with_range() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/sources/show"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "sources": [[1, "fn main() {}"], [2, "    println!(\"hi\");"], [3, "}"]]
                })),
            )
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj:src/main.rs", Some(1), Some(3), false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_source_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/sources/raw"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj:src/main.rs", None, None, false).await;
        assert_eq!(exit, 1);
    }
}
