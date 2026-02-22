use crate::client::{RuleSearchParams, SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    search: Option<&str>,
    language: Option<&str>,
    severity: Option<&str>,
    rule_type: Option<&str>,
    status: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let params = RuleSearchParams {
        search,
        language,
        severity,
        rule_type,
        status,
    };

    match client.get_all_rules(&params).await {
        Ok(rules) => {
            output::print_rules(&rules, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch rules: {e}");
            1
        }
    }
}
