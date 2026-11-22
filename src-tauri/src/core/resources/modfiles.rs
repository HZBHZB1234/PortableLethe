//! Official-site modfiles mirror (api/files.lethelc.site), ETag-aware.
use crate::{error::Result, model::SyncReport, state::AppState};

pub async fn sync(_state: &AppState, _force: bool) -> Result<SyncReport> {
    Err(crate::error::Error::msg("modfiles sync: phase 3"))
}
