use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(config: SonarQubeConfig, project: &str, fail_on_error: bool, json: bool) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_quality_gate(project).await {
        Ok(response) => {
            output::print_quality_gate(&response, project, json);
            let status = &response.project_status.status;
            if fail_on_error && status != "OK" {
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("Failed to get quality gate: {e}");
            1
        }
    }
}
