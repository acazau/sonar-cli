use std::time::Duration;

use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    task_id: &str,
    timeout_secs: u64,
    poll_interval_secs: u64,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    if !json {
        eprintln!("Waiting for analysis task {task_id}...");
    }

    match client
        .wait_for_analysis(
            task_id,
            Duration::from_secs(timeout_secs),
            Duration::from_secs(poll_interval_secs),
        )
        .await
    {
        Ok(task) => {
            output::print_wait_result(&task, json);
            0
        }
        Err(e) => {
            eprintln!("Analysis failed: {e}");
            1
        }
    }
}
