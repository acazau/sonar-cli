use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::scanner;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    details: bool,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match scanner::fetch_extended_data(&client, project).await {
        Ok(data) => {
            output::print_duplications(&data.duplications, project, json, details);
            0
        }
        Err(e) => {
            eprintln!("Failed to get duplications: {e}");
            1
        }
    }
}
