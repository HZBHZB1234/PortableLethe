pub mod catalog;
pub mod staticdata;
pub mod modfiles;

use crate::{error::Result, model::SyncReport, state::AppState};

pub async fn sync(_state: &AppState, category: &str, _source_path: Option<&str>, _force: bool) -> Result<SyncReport> {
    Err(crate::error::Error::msg(format!("resource sync ({}) : phase 3", category)))
}
