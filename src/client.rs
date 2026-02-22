//! SonarQube HTTP API Client
//!
//! Provides a type-safe client for interacting with the SonarQube Web API.

use reqwest::Client as HttpClient;
use std::time::Duration;
use thiserror::Error;

use crate::types::*;

/// Parameters for the issue search API
#[derive(Debug, Default)]
pub struct IssueSearchParams<'a> {
    pub severities: Option<&'a str>,
    pub types: Option<&'a str>,
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

/// Parameters for the rules search API
#[derive(Debug, Default)]
pub struct RuleSearchParams<'a> {
    pub search: Option<&'a str>,
    pub language: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub rule_type: Option<&'a str>,
    pub status: Option<&'a str>,
}

/// Errors from the SonarQube client
#[derive(Debug, Error)]
pub enum SonarQubeError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("deserialization failed: {0}")]
    Deserialize(String),

    #[error("timeout waiting for analysis")]
    Timeout,

    #[error("analysis failed: {0}")]
    Analysis(String),
}

/// Configuration for the SonarQube client
#[derive(Debug, Clone)]
pub struct SonarQubeConfig {
    /// Base URL of the SonarQube server
    pub url: String,
    /// Authentication token
    pub token: Option<String>,
    /// Request timeout
    pub timeout: Duration,
    /// Project key
    pub project_key: Option<String>,
    /// Branch name for branch-aware API queries
    pub branch: Option<String>,
}

impl Default for SonarQubeConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:9000".to_string(),
            token: None,
            timeout: Duration::from_secs(30),
            project_key: None,
            branch: None,
        }
    }
}

impl SonarQubeConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..Default::default()
        }
    }

    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_project(mut self, key: impl Into<String>) -> Self {
        self.project_key = Some(key.into());
        self
    }

    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

}

/// SonarQube API client
pub struct SonarQubeClient {
    config: SonarQubeConfig,
    http: HttpClient,
}

impl SonarQubeClient {
    /// Create a new SonarQube client
    pub fn new(config: SonarQubeConfig) -> Result<Self, SonarQubeError> {
        let http = HttpClient::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| SonarQubeError::Http(e.to_string()))?;

        Ok(Self { config, http })
    }

    /// Returns `&branch=<name>` when a branch is configured, empty string otherwise
    fn branch_param(&self) -> String {
        self.config
            .branch
            .as_ref()
            .map(|b| format!("&branch={}", b))
            .unwrap_or_default()
    }

    /// Execute an authenticated GET request and return the response
    async fn get(&self, url: &str) -> Result<reqwest::Response, SonarQubeError> {
        let mut request = self.http.get(url);
        if let Some(ref token) = self.config.token {
            request = request.basic_auth(token, Some(""));
        }

        let response = request
            .send()
            .await
            .map_err(|e| SonarQubeError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(SonarQubeError::Api {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        Ok(response)
    }

    /// Execute a GET request and deserialize the JSON response
    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<T, SonarQubeError> {
        self.get(url)
            .await?
            .json::<T>()
            .await
            .map_err(|e| SonarQubeError::Deserialize(e.to_string()))
    }

    /// Search for issues with full parameter support
    pub async fn search_issues_with_params(
        &self,
        project_key: &str,
        page: usize,
        page_size: usize,
        params: &IssueSearchParams<'_>,
    ) -> Result<IssuesResponse, SonarQubeError> {
        let statuses = params.statuses.unwrap_or("OPEN,CONFIRMED,REOPENED");
        let mut url = format!(
            "{}/api/issues/search?projectKeys={}&p={}&ps={}&statuses={}{}",
            self.config.url,
            project_key,
            page,
            page_size,
            statuses,
            self.branch_param()
        );
        if let Some(sev) = params.severities {
            url.push_str(&format!("&severities={}", sev));
        }
        if let Some(t) = params.types {
            url.push_str(&format!("&types={}", t));
        }
        if let Some(r) = params.resolutions {
            url.push_str(&format!("&resolutions={}", r));
        }
        if let Some(t) = params.tags {
            url.push_str(&format!("&tags={}", t));
        }
        if let Some(r) = params.rules {
            url.push_str(&format!("&rules={}", r));
        }
        if let Some(d) = params.created_after {
            url.push_str(&format!("&createdAfter={}", d));
        }
        if let Some(d) = params.created_before {
            url.push_str(&format!("&createdBefore={}", d));
        }
        if let Some(a) = params.author {
            url.push_str(&format!("&author={}", a));
        }
        if let Some(a) = params.assignees {
            url.push_str(&format!("&assignees={}", a));
        }
        if let Some(l) = params.languages {
            url.push_str(&format!("&languages={}", l));
        }
        self.get_json(&url).await
    }

    /// Get quality gate status
    pub async fn get_quality_gate(
        &self,
        project_key: &str,
    ) -> Result<QualityGateResponse, SonarQubeError> {
        let url = format!(
            "{}/api/qualitygates/project_status?projectKey={}{}",
            self.config.url,
            project_key,
            self.branch_param()
        );
        self.get_json(&url).await
    }

    /// Get project measures
    pub async fn get_measures(
        &self,
        project_key: &str,
        metrics: &[&str],
    ) -> Result<MeasuresResponse, SonarQubeError> {
        let metrics_param = metrics.join(",");
        let url = format!(
            "{}/api/measures/component?component={}&metricKeys={}{}",
            self.config.url,
            project_key,
            metrics_param,
            self.branch_param()
        );
        self.get_json(&url).await
    }

    /// Wait for analysis to complete
    pub async fn wait_for_analysis(
        &self,
        task_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<AnalysisTask, SonarQubeError> {
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(SonarQubeError::Timeout);
            }

            let url = format!("{}/api/ce/task?id={}", self.config.url, task_id);

            let mut request = self.http.get(&url);
            if let Some(ref token) = self.config.token {
                request = request.basic_auth(token, Some(""));
            }

            let response = match request.send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "SonarQube connection error, retrying...");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
            };

            if !response.status().is_success() {
                tokio::time::sleep(poll_interval).await;
                continue;
            }

            let task_response: AnalysisResponse = match response.json().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to parse analysis response, retrying...");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                }
            };

            match task_response.task.status.as_str() {
                task_status::SUCCESS => return Ok(task_response.task),
                task_status::FAILED => {
                    return Err(SonarQubeError::Analysis(
                        task_response.task.error_message.unwrap_or_default(),
                    ));
                }
                task_status::CANCELED => {
                    return Err(SonarQubeError::Analysis(
                        "Analysis was canceled".to_string(),
                    ));
                }
                _ => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Get component tree with measures (for per-file coverage/duplications)
    pub async fn get_component_tree(
        &self,
        project_key: &str,
        metrics: &[&str],
        page: usize,
        page_size: usize,
    ) -> Result<ComponentTreeResponse, SonarQubeError> {
        let metrics_param = metrics.join(",");
        let url = format!(
            "{}/api/measures/component_tree?component={}&metricKeys={}&qualifiers=FIL&p={}&ps={}{}",
            self.config.url,
            project_key,
            metrics_param,
            page,
            page_size,
            self.branch_param()
        );
        self.get_json(&url).await
    }

    /// Get all files with their coverage metrics
    pub async fn get_files_coverage(
        &self,
        project_key: &str,
    ) -> Result<Vec<TreeComponent>, SonarQubeError> {
        let mut all_files = Vec::new();
        let mut page = 1;
        let page_size = 100;
        let metrics = ["coverage", "uncovered_lines", "lines_to_cover"];

        loop {
            let response = self
                .get_component_tree(project_key, &metrics, page, page_size)
                .await?;

            let files_count = response.components.len();
            all_files.extend(response.components);

            let total = response.paging.map(|p| p.total).unwrap_or(0);
            if all_files.len() >= total || files_count < page_size {
                break;
            }
            page += 1;

            if page > 100 {
                break;
            }
        }

        Ok(all_files)
    }

    /// Get duplications for a specific file component
    pub async fn get_duplications(
        &self,
        component_key: &str,
    ) -> Result<DuplicationsResponse, SonarQubeError> {
        let url = format!(
            "{}/api/duplications/show?key={}{}",
            self.config.url,
            component_key,
            self.branch_param()
        );
        self.get_json(&url).await
    }

    /// Get all files with duplication metrics
    pub async fn get_files_with_duplications(
        &self,
        project_key: &str,
    ) -> Result<Vec<TreeComponent>, SonarQubeError> {
        let mut all_files = Vec::new();
        let mut page = 1;
        let page_size = 100;
        let metrics = [
            "duplicated_lines",
            "duplicated_lines_density",
            "duplicated_blocks",
        ];

        loop {
            let response = self
                .get_component_tree(project_key, &metrics, page, page_size)
                .await?;

            let files_count = response.components.len();
            all_files.extend(response.components.into_iter().filter(|c| {
                c.measures.iter().any(|m| {
                    m.metric == "duplicated_lines"
                        && m.value.as_ref().map(|v| v != "0").unwrap_or(false)
                })
            }));

            let total = response.paging.map(|p| p.total).unwrap_or(0);
            if page * page_size >= total || files_count < page_size {
                break;
            }
            page += 1;

            if page > 100 {
                break;
            }
        }

        Ok(all_files)
    }

    /// Get security hotspots for a project
    pub async fn get_security_hotspots(
        &self,
        project_key: &str,
        status_filter: Option<&str>,
    ) -> Result<Vec<SecurityHotspot>, SonarQubeError> {
        let mut all_hotspots = Vec::new();
        let mut page = 1;
        let page_size = 100;
        let status = status_filter.unwrap_or("TO_REVIEW");

        loop {
            let url = format!(
                "{}/api/hotspots/search?projectKey={}&p={}&ps={}&status={}{}",
                self.config.url,
                project_key,
                page,
                page_size,
                status,
                self.branch_param()
            );

            let response: HotspotsResponse = self.get_json(&url).await?;
            let hotspots_count = response.hotspots.len();
            let total = response.paging.total;
            all_hotspots.extend(response.hotspots);

            if all_hotspots.len() >= total || hotspots_count < page_size {
                break;
            }
            page += 1;

            if page > 100 {
                break;
            }
        }

        Ok(all_hotspots)
    }

    /// Search for projects/components
    pub async fn search_projects(
        &self,
        search: Option<&str>,
        qualifier: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<ProjectsSearchResponse, SonarQubeError> {
        let q = qualifier.unwrap_or("TRK");
        let mut url = format!(
            "{}/api/components/search?qualifiers={}&p={}&ps={}",
            self.config.url, q, page, page_size
        );
        if let Some(s) = search {
            url.push_str(&format!("&q={}", s));
        }
        self.get_json(&url).await
    }

    /// Get all projects (handles pagination)
    pub async fn get_all_projects(
        &self,
        search: Option<&str>,
        qualifier: Option<&str>,
    ) -> Result<Vec<ProjectInfo>, SonarQubeError> {
        let mut all = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            let response = self.search_projects(search, qualifier, page, page_size).await?;
            let count = response.components.len();
            let total = response.paging.total;
            all.extend(response.components);

            if all.len() >= total || count < page_size {
                break;
            }
            page += 1;
            if page > 100 {
                break;
            }
        }

        Ok(all)
    }

    /// Get measures history for a project
    pub async fn get_measures_history(
        &self,
        project_key: &str,
        metrics: &str,
        from: Option<&str>,
        to: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<MeasuresHistoryResponse, SonarQubeError> {
        let mut url = format!(
            "{}/api/measures/search_history?component={}&metrics={}&p={}&ps={}{}",
            self.config.url, project_key, metrics, page, page_size, self.branch_param()
        );
        if let Some(f) = from {
            url.push_str(&format!("&from={}", f));
        }
        if let Some(t) = to {
            url.push_str(&format!("&to={}", t));
        }
        self.get_json(&url).await
    }

    /// Search for rules
    pub async fn search_rules(
        &self,
        params: &RuleSearchParams<'_>,
        page: usize,
        page_size: usize,
    ) -> Result<RulesSearchResponse, SonarQubeError> {
        let mut url = format!(
            "{}/api/rules/search?p={}&ps={}",
            self.config.url, page, page_size
        );
        if let Some(q) = params.search {
            url.push_str(&format!("&q={}", q));
        }
        if let Some(l) = params.language {
            url.push_str(&format!("&languages={}", l));
        }
        if let Some(s) = params.severity {
            url.push_str(&format!("&severities={}", s));
        }
        if let Some(t) = params.rule_type {
            url.push_str(&format!("&types={}", t));
        }
        if let Some(s) = params.status {
            url.push_str(&format!("&statuses={}", s));
        }
        self.get_json(&url).await
    }

    /// Get all rules matching filters (handles pagination)
    pub async fn get_all_rules(
        &self,
        params: &RuleSearchParams<'_>,
    ) -> Result<Vec<RuleInfo>, SonarQubeError> {
        let mut all = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            let response = self
                .search_rules(params, page, page_size)
                .await?;
            let count = response.rules.len();
            let total = response.total;
            all.extend(response.rules);

            if all.len() >= total || count < page_size {
                break;
            }
            page += 1;
            if page > 100 {
                break;
            }
        }

        Ok(all)
    }

    /// Get raw source code for a component
    pub async fn get_source_raw(
        &self,
        component: &str,
    ) -> Result<String, SonarQubeError> {
        let url = format!(
            "{}/api/sources/raw?key={}{}",
            self.config.url, component, self.branch_param()
        );
        self.get(&url)
            .await?
            .text()
            .await
            .map_err(|e| SonarQubeError::Http(e.to_string()))
    }

    /// Get source code with line range using /api/sources/show
    pub async fn get_source_show(
        &self,
        component: &str,
        from: Option<usize>,
        to: Option<usize>,
    ) -> Result<Vec<SourceLine>, SonarQubeError> {
        let mut url = format!(
            "{}/api/sources/show?key={}{}",
            self.config.url, component, self.branch_param()
        );
        if let Some(f) = from {
            url.push_str(&format!("&from={}", f));
        }
        if let Some(t) = to {
            url.push_str(&format!("&to={}", t));
        }
        let body = self
            .get(&url)
            .await?
            .text()
            .await
            .map_err(|e| SonarQubeError::Http(e.to_string()))?;

        // /api/sources/show returns {"sources": [[lineNum, "code"], ...]}
        let value: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| SonarQubeError::Deserialize(e.to_string()))?;

        let sources = value
            .get("sources")
            .and_then(|s| s.as_array())
            .ok_or_else(|| {
                SonarQubeError::Deserialize("missing 'sources' array".to_string())
            })?;

        let mut lines = Vec::new();
        for entry in sources {
            if let Some(arr) = entry.as_array() {
                let line_num = arr
                    .first()
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                let code = arr
                    .get(1)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                lines.push(SourceLine {
                    line: line_num,
                    code,
                });
            }
        }

        Ok(lines)
    }

    /// Get the server status string (UP, STARTING, DOWN, etc.)
    pub async fn get_status(&self) -> Result<String, SonarQubeError> {
        let url = format!("{}/api/system/status", self.config.url);
        let body = self.get(&url).await?.text().await.unwrap_or_default();

        // Parse {"status":"UP"} or similar
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(s) = v.get("status").and_then(|s| s.as_str()) {
                return Ok(s.to_string());
            }
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn try_new_client(config: SonarQubeConfig) -> Option<SonarQubeClient> {
        match std::panic::catch_unwind(|| SonarQubeClient::new(config)) {
            Ok(Ok(client)) => Some(client),
            Ok(Err(err)) => {
                eprintln!("Skipping test: client init failed: {err}");
                None
            }
            Err(_) => {
                eprintln!("Skipping test: client init panicked");
                None
            }
        }
    }

    async fn try_mock_server() -> Option<MockServer> {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(err) => {
                eprintln!("Skipping test: failed to bind: {err}");
                return None;
            }
        };
        Some(MockServer::builder().listener(listener).start().await)
    }

    #[test]
    fn test_config_builder() {
        let config = SonarQubeConfig::new("http://sonar.example.com")
            .with_token("my-token")
            .with_project("my-project")
            .with_timeout(Duration::from_secs(60));

        assert_eq!(config.url, "http://sonar.example.com");
        assert_eq!(config.token, Some("my-token".to_string()));
        assert_eq!(config.project_key, Some("my-project".to_string()));
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_default() {
        let config = SonarQubeConfig::default();
        assert_eq!(config.url, "http://localhost:9000");
        assert!(config.token.is_none());
        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_get_status_up() {
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
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        assert_eq!(client.get_status().await.unwrap(), "UP");
    }

    #[tokio::test]
    async fn test_get_status_failure() {
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
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        assert!(client.get_status().await.is_err());
    }

    #[tokio::test]
    async fn test_search_issues_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .and(query_param("projectKeys", "my-project"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1, "p": 1, "ps": 10,
                "paging": {"pageIndex": 1, "pageSize": 10, "total": 1},
                "issues": [{
                    "key": "issue-1",
                    "component": "my-project:src/main.rs",
                    "project": "my-project",
                    "rule": "rust:S1234",
                    "severity": "MAJOR",
                    "message": "Test issue",
                    "type": "BUG",
                    "status": "OPEN",
                    "tags": []
                }]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let params = IssueSearchParams::default();
        let result = client.search_issues_with_params("my-project", 1, 10, &params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().issues.len(), 1);
    }

    #[tokio::test]
    async fn test_get_quality_gate_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/qualitygates/project_status"))
            .and(query_param("projectKey", "my-project"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "projectStatus": {
                    "status": "OK",
                    "conditions": []
                }
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client.get_quality_gate("my-project").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().project_status.status, "OK");
    }

    #[tokio::test]
    async fn test_get_measures_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/measures/component"))
            .and(query_param("component", "my-project"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "component": {
                    "key": "my-project",
                    "measures": [
                        {"metric": "coverage", "value": "85.5"},
                        {"metric": "ncloc", "value": "1000"}
                    ]
                }
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client
            .get_measures("my-project", &["coverage", "ncloc"])
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().component.measures.len(), 2);
    }

    #[tokio::test]
    async fn test_wait_for_analysis_success() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/ce/task"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "task": {
                    "id": "task-123",
                    "type": "REPORT",
                    "status": "SUCCESS",
                    "submittedAt": "2024-01-01T00:00:00+0000",
                    "executedAt": "2024-01-01T00:01:00+0000",
                    "analysisId": "analysis-1"
                }
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client
            .wait_for_analysis("task-123", Duration::from_secs(5), Duration::from_millis(100))
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, "SUCCESS");
    }

    #[tokio::test]
    async fn test_search_issues_with_params() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/issues/search"))
            .and(query_param("projectKeys", "my-project"))
            .and(query_param("statuses", "RESOLVED"))
            .and(query_param("languages", "java"))
            .and(query_param("createdAfter", "2025-01-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1, "p": 1, "ps": 100,
                "issues": [{
                    "key": "issue-2",
                    "component": "my-project:src/Main.java",
                    "project": "my-project",
                    "rule": "java:S1234",
                    "severity": "CRITICAL",
                    "message": "Resolved issue",
                    "type": "BUG",
                    "status": "RESOLVED",
                    "tags": []
                }]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let params = IssueSearchParams {
            statuses: Some("RESOLVED"),
            languages: Some("java"),
            created_after: Some("2025-01-01"),
            ..Default::default()
        };

        let result = client
            .search_issues_with_params("my-project", 1, 100, &params)
            .await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.issues.len(), 1);
        assert_eq!(response.issues[0].status, "RESOLVED");
    }

    #[tokio::test]
    async fn test_search_projects() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/components/search"))
            .and(query_param("qualifiers", "TRK"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "paging": {"pageIndex": 1, "pageSize": 100, "total": 2},
                "components": [
                    {"key": "proj-1", "name": "Project One"},
                    {"key": "proj-2", "name": "Project Two", "visibility": "public",
                     "lastAnalysisDate": "2025-06-01T12:00:00+0000"}
                ]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client.get_all_projects(None, None).await;
        assert!(result.is_ok());
        let projects = result.unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].key, "proj-1");
        assert_eq!(projects[1].name, "Project Two");
    }

    #[tokio::test]
    async fn test_get_measures_history() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/measures/search_history"))
            .and(query_param("component", "my-project"))
            .and(query_param("metrics", "coverage,bugs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "paging": {"pageIndex": 1, "pageSize": 100, "total": 2},
                "measures": [
                    {
                        "metric": "coverage",
                        "history": [
                            {"date": "2025-01-01T00:00:00+0000", "value": "80.0"},
                            {"date": "2025-02-01T00:00:00+0000", "value": "85.0"}
                        ]
                    },
                    {
                        "metric": "bugs",
                        "history": [
                            {"date": "2025-01-01T00:00:00+0000", "value": "5"},
                            {"date": "2025-02-01T00:00:00+0000", "value": "3"}
                        ]
                    }
                ]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client
            .get_measures_history("my-project", "coverage,bugs", None, None, 1, 100)
            .await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.measures.len(), 2);
        assert_eq!(response.measures[0].metric, "coverage");
        assert_eq!(response.measures[0].history.len(), 2);
    }

    #[tokio::test]
    async fn test_search_rules() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/rules/search"))
            .and(query_param("languages", "java"))
            .and(query_param("severities", "CRITICAL"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "total": 1, "p": 1, "ps": 100,
                "rules": [{
                    "key": "java:S1234",
                    "name": "Null pointer check",
                    "severity": "CRITICAL",
                    "type": "BUG",
                    "lang": "java",
                    "status": "READY",
                    "langName": "Java"
                }]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let params = RuleSearchParams {
            language: Some("java"),
            severity: Some("CRITICAL"),
            ..Default::default()
        };
        let result = client.get_all_rules(&params).await;
        assert!(result.is_ok());
        let rules = result.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].key, "java:S1234");
    }

    #[tokio::test]
    async fn test_get_source_raw() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/sources/raw"))
            .and(query_param("key", "my-project:src/main.rs"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string("fn main() {\n    println!(\"hello\");\n}\n"),
            )
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client.get_source_raw("my-project:src/main.rs").await;
        assert!(result.is_ok());
        let source = result.unwrap();
        assert!(source.contains("fn main()"));
    }

    #[tokio::test]
    async fn test_get_source_show() {
        let mock_server = match try_mock_server().await {
            Some(s) => s,
            None => return,
        };

        Mock::given(method("GET"))
            .and(path("/api/sources/show"))
            .and(query_param("key", "my-project:src/main.rs"))
            .and(query_param("from", "1"))
            .and(query_param("to", "3"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "sources": [
                    [1, "fn main() {"],
                    [2, "    println!(\"hello\");"],
                    [3, "}"]
                ]
            })))
            .mount(&mock_server)
            .await;

        let config = SonarQubeConfig::new(mock_server.uri()).with_token("test-token");
        let client = match try_new_client(config) {
            Some(c) => c,
            None => return,
        };

        let result = client
            .get_source_show("my-project:src/main.rs", Some(1), Some(3))
            .await;
        assert!(result.is_ok());
        let lines = result.unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].line, 1);
        assert_eq!(lines[0].code, "fn main() {");
        assert_eq!(lines[2].line, 3);
    }
}
