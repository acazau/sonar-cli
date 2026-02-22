use crate::client::{SonarQubeClient, SonarQubeConfig};
use crate::output;
use crate::types::{MeasureHistory, MeasuresHistoryResponse};

/// Merge page measures into the accumulated list.
///
/// On the first page the list is empty, so we take ownership directly.
/// On subsequent pages we extend existing metric histories or push new ones.
fn merge_page_measures(all: &mut Vec<MeasureHistory>, page: Vec<MeasureHistory>) {
    for page_measure in page {
        if let Some(existing) = all.iter_mut().find(|m| m.metric == page_measure.metric) {
            existing.history.extend(page_measure.history);
        } else {
            all.push(page_measure);
        }
    }
}

/// Returns true when all pages have been fetched.
fn pagination_done(response_total: usize, page: usize, page_size: usize) -> bool {
    page * page_size >= response_total || page >= 100
}

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

    let mut all_measures: Vec<MeasureHistory> = Vec::new();
    let mut page = 1;
    let page_size = 100;

    loop {
        let response: MeasuresHistoryResponse = match client
            .get_measures_history(project, metrics, from, to, page, page_size)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to fetch measures history: {e}");
                return 1;
            }
        };

        let total = response.paging.total;
        merge_page_measures(&mut all_measures, response.measures);

        if pagination_done(total, page, page_size) {
            break;
        }
        page += 1;
    }

    output::print_history(&all_measures, project, json);
    0
}
