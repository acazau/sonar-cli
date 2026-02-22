use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

const DEFAULT_METRICS: &[&str] = &[
    "ncloc",
    "coverage",
    "duplicated_lines_density",
    "bugs",
    "vulnerabilities",
    "code_smells",
    "sqale_debt_ratio",
    "reliability_rating",
    "security_rating",
    "sqale_rating",
];

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    metrics: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    let metric_keys: Vec<&str> = match metrics {
        Some(m) => m.split(',').map(|s| s.trim()).collect(),
        None => DEFAULT_METRICS.to_vec(),
    };

    match client.get_measures(project, &metric_keys).await {
        Ok(response) => {
            output::print_measures(&response, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to get measures: {e}");
            1
        }
    }
}
