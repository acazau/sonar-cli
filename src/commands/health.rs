use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(config: SonarQubeConfig, json: bool) -> i32 {
    let client = match SonarQubeClient::new(config.clone()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_status().await {
        Ok(status) => {
            output::print_health(&status, &config.url, json);
            if status == "UP" { 0 } else { 1 }
        }
        Err(e) => {
            if json {
                output::print_health("UNREACHABLE", &config.url, json);
            } else {
                eprintln!("Failed to reach SonarQube at {}: {e}", config.url);
            }
            1
        }
    }
}
