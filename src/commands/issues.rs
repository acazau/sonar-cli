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
