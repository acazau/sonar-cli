use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::severity;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    min_severity: Option<&str>,
    issue_type: Option<&str>,
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

    // Build severity filter: include this severity and all above it
    let severities = min_severity.map(|sev| {
        let min_ord = severity::ordinal(&sev.to_uppercase());
        severity::ALL
            .iter()
            .filter(|s| severity::ordinal(s) >= min_ord)
            .copied()
            .collect::<Vec<_>>()
            .join(",")
    });

    let types = issue_type.map(|t| t.to_uppercase());

    // Fetch issues with filters
    let mut all_issues = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let result = client
            .search_issues_filtered(
                project,
                page,
                page_size,
                severities.as_deref(),
                types.as_deref(),
            )
            .await;

        match result {
            Ok(response) => {
                let count = response.issues.len();
                let total = response.total;
                all_issues.extend(response.issues);

                if let Some(lim) = limit {
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
