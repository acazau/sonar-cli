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
