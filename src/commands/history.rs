use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::MeasureHistory;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    metrics: &str,
    from: Option<&str>,
    to: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    // Paginate history data — the API paginates data points, not metrics.
    // We need to merge history values across pages for each metric.
    let mut all_measures: Vec<MeasureHistory> = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let result = client
            .get_measures_history(project, metrics, from, to, page, page_size)
            .await;

        match result {
            Ok(response) => {
                if all_measures.is_empty() {
                    all_measures = response.measures;
                } else {
                    // Merge history values from subsequent pages
                    for page_measure in response.measures {
                        if let Some(existing) = all_measures
                            .iter_mut()
                            .find(|m| m.metric == page_measure.metric)
                        {
                            existing.history.extend(page_measure.history);
                        } else {
                            all_measures.push(page_measure);
                        }
                    }
                }

                let total = response.paging.total;
                let fetched = page * page_size;
                if fetched >= total {
                    break;
                }
                page += 1;
                if page > 100 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch measures history: {e}");
                return 1;
            }
        }
    }

    output::print_history(&all_measures, project, json);
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

    fn history_body() -> serde_json::Value {
        serde_json::json!({
            "paging": {"total": 1},
            "measures": [
                {
                    "metric": "coverage",
                    "history": [
                        {"date": "2026-01-01T00:00:00+0000", "value": "75.0"}
                    ]
                }
            ]
        })
    }

    #[tokio::test]
    async fn test_run_history_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/search_history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(history_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", "coverage", None, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_history_json() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/search_history"))
            .respond_with(ResponseTemplate::new(200).set_body_json(history_body()))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(
            config,
            "my-proj",
            "coverage,bugs",
            Some("2026-01-01"),
            Some("2026-02-01"),
            true,
        )
        .await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_history_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/measures/search_history"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let exit = run(config, "my-proj", "coverage", None, None, false).await;
        assert_eq!(exit, 1);
    }
}
