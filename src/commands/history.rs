use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::MeasureHistory;

pub async fn run(
    config: SonarQubeConfig,
    project: &str,
    metrics: &str,
    from: Option<&str>,
    to: Option<&str>,
    json: bool,
) -> i32 {
    let client = match SonarQubeClient::new(config) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to create client: {e}");
            return 1;
        }
    };

    // Paginate history data — the API paginates data points, not metrics.
    // We need to merge history values across pages for each metric.
    let mut all_measures: Vec<MeasureHistory> = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let result = client
            .get_measures_history(project, metrics, from, to, page, page_size)
            .await;

        match result {
            Ok(response) => {
                if all_measures.is_empty() {
                    all_measures = response.measures;
                } else {
                    // Merge history values from subsequent pages
                    for page_measure in response.measures {
                        if let Some(existing) = all_measures
                            .iter_mut()
                            .find(|m| m.metric == page_measure.metric)
                        {
                            existing.history.extend(page_measure.history);
                        } else {
                            all_measures.push(page_measure);
                        }
                    }
                }

                let total = response.paging.total;
                let fetched = page * page_size;
                if fetched >= total {
                    break;
                }
                page += 1;
                if page > 100 {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Failed to fetch measures history: {e}");
                return 1;
            }
        }
    }

    output::print_history(&all_measures, project, json);
    0
}
