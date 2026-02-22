use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;

pub async fn run(
    config: SonarQubeConfig,
    search: Option<&str>,
    qualifier: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    match client.get_all_projects(search, qualifier).await {
        Ok(projects) => {
            output::print_projects(&projects, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch projects: {e}");
            1
        }
    }
}
