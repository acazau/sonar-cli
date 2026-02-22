use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
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

    match client.get_security_hotspots(project, status).await {
        Ok(hotspots) => {
            output::print_hotspots(&hotspots, project, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to get hotspots: {e}");
            1
        }
    }
}
