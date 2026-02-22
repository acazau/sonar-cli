use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::SourceLine;

pub async fn run(
    config: SonarQubeConfig,
    component: &str,
    from: Option<usize>,
    to: Option<usize>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    // Use /api/sources/show when line range is specified, otherwise use /api/sources/raw
    let lines = if from.is_some() || to.is_some() {
        client.get_source_show(component, from, to).await
    } else {
        match client.get_source_raw(component).await {
            Ok(raw) => {
                let lines: Vec<SourceLine> = raw
                    .lines()
                    .enumerate()
                    .map(|(i, line)| SourceLine {
                        line: i + 1,
                        code: line.to_string(),
                    })
                    .collect();
                Ok(lines)
            }
            Err(e) => Err(e),
        }
    };

    match lines {
        Ok(lines) => {
            output::print_source(&lines, json);
            0
        }
        Err(e) => {
            eprintln!("Failed to fetch source: {e}");
            1
        }
    }
}
