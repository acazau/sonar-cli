use crate::client::{IssueSearchParams, SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::severity;

/// Parameters for the issues command
pub struct IssuesCommandParams<'a> {
    pub min_severity: Option<&'a str>,
    pub issue_type: Option<&'a str>,
    pub limit: Option<usize>,
    pub statuses: Option<&'a str>,
    pub resolutions: Option<&'a str>,
    pub tags: Option<&'a str>,
    pub rules: Option<&'a str>,
    pub created_after: Option<&'a str>,
    pub created_before: Option<&'a str>,
    pub author: Option<&'a str>,
    pub assignees: Option<&'a str>,
    pub languages: Option<&'a str>,
}

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    params: &IssuesCommandParams<'_>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    // Build severity filter: include this severity and all above it
    let severities = params.min_severity.map(|sev| {
        let min_ord = severity::ordinal(&sev.to_uppercase());
        severity::ALL
            .iter()
            .filter(|s| severity::ordinal(s) >= min_ord)
            .copied()
            .collect::<Vec<_>>()
            .join(",")
    });

    let types = params.issue_type.map(|t| t.to_uppercase());

    let search_params = IssueSearchParams {
        severities: severities.as_deref(),
        types: types.as_deref(),
        statuses: params.statuses,
        resolutions: params.resolutions,
        tags: params.tags,
        rules: params.rules,
        created_after: params.created_after,
        created_before: params.created_before,
        author: params.author,
        assignees: params.assignees,
        languages: params.languages,
    };

    // Fetch issues with filters
    let mut all_issues = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let result = client
            .search_issues_with_params(project, page, page_size, &search_params)
            .await;

        match result {
            Ok(response) => {
                let count = response.issues.len();
                let total = response.total;
                all_issues.extend(response.issues);

                if let Some(lim) = params.limit {
                    if all_issues.len() >= lim {
                        all_issues.truncate(lim);
                        break;
                    }
                }

                if all_issues.len() >= total || count < page_size {
                    break;
                }
                page += 1;
                if page > 100 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch issues: {e}");
                return 1;
            }
        }
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

    fn default_params() -> IssuesCommandParams<'static> {
        IssuesCommandParams {
            min_severity: None,
            issue_type: None,
            limit: None,
            statuses: None,
            resolutions: None,
            tags: None,
            rules: None,
            created_after: None,
            created_before: None,
            author: None,
            assignees: None,
            languages: None,
        }
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
        let params = default_params();
        let exit = run(config, "my-proj", &params, false).await;
        assert_eq!(exit, 0);
    }

    #[tokio::test]
    async fn test_run_issues_with_severity_filter() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };
        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(issues_body(1)))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri());
        let params = IssuesCommandParams {
            min_severity: Some("CRITICAL"),
            issue_type: Some("CODE_SMELL"),
            ..default_params()
        };
        let exit = run(config, "my-proj", &params, true).await;
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
        let params = IssuesCommandParams {
            limit: Some(2),
            ..default_params()
        };
        let exit = run(config, "my-proj", &params, false).await;
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
        let params = default_params();
        let exit = run(config, "my-proj", &params, false).await;
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
        let params = default_params();
        let exit = run(config, "my-proj", &params, true).await;
        assert_eq!(exit, 0);
    }
}
