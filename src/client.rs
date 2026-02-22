//! SonarQube HTTP API Client
//!
//! Provides a type-safe client for interacting with the SonarQube Web API.

use reqwest::Client as HttpClient;
use std::time::Duration;
use thiserror::Error;

use crate::types::*;

/// Errors from the SonarQube client
#[derive(Debug, Error)]
pub enum SonarQubeError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("deserialization failed: {0}")]
    Deserialize(String),

    #[error("configuration error: {0}")]
    Config(String),

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

    /// Create config from environment variables
    pub fn from_env() -> Self {
        let url = std::env::var("SONAR_HOST_URL")
            .or_else(|_| std::env::var("SONAR_URL"))
            .unwrap_or_else(|_| "http://localhost:9000".to_string());

        let token = std::env::var("SONAR_TOKEN").ok();
        let project_key = std::env::var("SONAR_PROJECT_KEY").ok();
        let branch = std::env::var("SONAR_BRANCH").ok();

        Self {
            url,
            token,
            project_key,
            branch,
            ..Default::default()
        }
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

    /// Get the base URL
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Get the project key
    pub fn project_key(&self) -> Option<&str> {
        self.config.project_key.as_deref()
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

    /// Search for issues
    pub async fn search_issues(
        &self,
        project_key: &str,
        page: usize,
        page_size: usize,
    ) -> Result<IssuesResponse, SonarQubeError> {
        let url = format!(
            "{}/api/issues/search?projectKeys={}&p={}&ps={}&statuses=OPEN,CONFIRMED,REOPENED{}",
            self.config.url,
            project_key,
            page,
            page_size,
            self.branch_param()
        );
        self.get_json(&url).await
    }

    /// Search for issues with optional severity and type filters
    pub async fn search_issues_filtered(
        &self,
        project_key: &str,
        page: usize,
        page_size: usize,
        severities: Option<&str>,
        types: Option<&str>,
    ) -> Result<IssuesResponse, SonarQubeError> {
        let mut url = format!(
            "{}/api/issues/search?projectKeys={}&p={}&ps={}&statuses=OPEN,CONFIRMED,REOPENED{}",
            self.config.url,
            project_key,
            page,
            page_size,
            self.branch_param()
        );
        if let Some(sev) = severities {
            url.push_str(&format!("&severities={}", sev));
        }
        if let Some(t) = types {
            url.push_str(&format!("&types={}", t));
        }
        self.get_json(&url).await
    }

    /// Get all issues for a project (handles pagination)
    pub async fn get_all_issues(
        &self,
        project_key: &str,
    ) -> Result<Vec<SonarIssue>, SonarQubeError> {
        let mut all_issues = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            let response = self.search_issues(project_key, page, page_size).await?;
            let issues_count = response.issues.len();
            let total = response.total;
            all_issues.extend(response.issues);

            if all_issues.len() >= total || issues_count < page_size {
                break;
            }
            page += 1;

            if page > 100 {
                break;
            }
        }

        Ok(all_issues)
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

    /// Check server health — returns true only when status is "UP"
    pub async fn health_check(&self) -> Result<bool, SonarQubeError> {
        let url = format!("{}/api/system/status", self.config.url);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| SonarQubeError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let body = response.text().await.unwrap_or_default();
        Ok(body.contains("\"UP\""))
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
    fn test_config_from_env() {
        let config = SonarQubeConfig::from_env();
        assert!(!config.url.is_empty());
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
    async fn test_health_check_success() {
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

        assert!(client.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_failure() {
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

        assert!(!client.health_check().await.unwrap_or(false));
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

        let result = client.search_issues("my-project", 1, 10).await;
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
}
