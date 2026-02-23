use crate::client::{IssueSearchParams, SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::severity;

/// Build a comma-separated severity filter from a minimum severity level.
///
/// Returns all severities at or above `min_severity`, or `None` if unset.
pub fn build_severity_filter(min_severity: Option<&str>) -> Option<String> {
    min_severity.map(|sev| {
        let min_ord = severity::ordinal(&sev.to_uppercase());
        severity::ALL
            .iter()
            .filter(|s| severity::ordinal(s) >= min_ord)
            .copied()
            .collect::<Vec<_>>()
            .join(",")
    })
}

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    search_params: &IssueSearchParams<'_>,
    limit: Option<usize>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let mut all_issues = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let response = match client
            .search_issues_with_params(project, page, page_size, search_params)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to fetch issues: {e}");
                return 1;
            }
        };

        let count = response.issues.len();
        let total = response.total;
        all_issues.extend(response.issues);

        if let Some(lim) = limit {
            if all_issues.len() >= lim {
                all_issues.truncate(lim);
                break;
            }
        }

        if all_issues.len() >= total || count < page_size || page >= 100 {
            break;
        }
        page += 1;
    }

    output::print_issues(&all_issues, project, json);
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

    fn issues_body(count: usize) -> serde_json::Value {
        let issues: Vec<serde_json::Value> = (0..count)
            .map(|i| {
                serde_json::json!({
                    "key": format!("issue-{i}"),
                    "rule": "rust:S3776",
                    "severity": "CRITICAL",
                    "component": "my-proj:src/main.rs",
                    "project": "my-proj",
                    "line": i + 1,
                    "message": "Cognitive complexity too high",
                    "type": "CODE_SMELL",
                    "status": "OPEN",
                    "tags": []
                })
            })
            .collect();
        serde_json::json!({"total": count, "issues": issues})
    }

    #[tokio::test]
    async fn test_run_issues_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_body(2)))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams::default();
        let exit = run(config, "my-proj", &params, None, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_issues_with_severity_and_type() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_body(1)))
            .mount(&mock_server)
            .await;

        let severities = build_severity_filter(Some("CRITICAL"));
        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams {
            severities: severities.as_deref(),
            types: Some("CODE_SMELL"),
            ..IssueSearchParams::default()
        };
        let exit = run(config, "my-proj", &params, None, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_issues_with_limit() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_body(5)))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams::default();
        let exit = run(config, "my-proj", &params, Some(2), false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_issues_api_error() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams::default();
        let exit = run(config, "my-proj", &params, None, false).await;
        assert_eq!(exit, 1);
    }

    #[tokio::test]
    async fn test_run_issues_empty() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_body(0)))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams::default();
        let exit = run(config, "my-proj", &params, None, true).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_issues_pagination() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        // Page 1: 100 issues with total=101 to trigger page 2
        use wiremock::matchers::query_param;
        let page1_issues: Vec<serde_json::Value> = (0..100)
            .map(|i| {
                serde_json::json!({
                    "key": format!("issue-{i}"),
                    "rule": "rust:S3776",
                    "severity": "MAJOR",
                    "component": "my-proj:src/main.rs",
                    "project": "my-proj",
                    "line": i + 1,
                    "message": "Issue",
                    "type": "CODE_SMELL",
                    "status": "OPEN",
                    "tags": []
                })
            })
            .collect();

        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .and(query_param("p", "1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({"total": 101, "issues": page1_issues}),
            ))
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .and(query_param("p", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({
                    "total": 101,
                    "issues": [{
                        "key": "issue-100",
                        "rule": "rust:S3776",
                        "severity": "MAJOR",
                        "component": "my-proj:src/main.rs",
                        "project": "my-proj",
                        "line": 101,
                        "message": "Issue",
                        "type": "CODE_SMELL",
                        "status": "OPEN",
                        "tags": []
                    }]
                }),
            ))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssueSearchParams::default();
        let exit = run(config, "my-proj", &params, None, false).await;
        assert_eq!(exit, 0);
    }

    #[test]
    fn test_build_severity_filter_none() {
        assert_eq!(build_severity_filter(None), None);
    }

    #[test]
    fn test_build_severity_filter_critical() {
        let result = build_severity_filter(Some("CRITICAL"));
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("CRITICAL"));
        assert!(s.contains("BLOCKER"));
        assert!(!s.contains("MAJOR"));
    }

    #[test]
    fn test_build_severity_filter_info() {
        let result = build_severity_filter(Some("INFO"));
        assert!(result.is_some());
        let s = result.unwrap();
        assert!(s.contains("INFO"));
        assert!(s.contains("BLOCKER"));
    }
}
