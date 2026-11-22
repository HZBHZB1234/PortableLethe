pub mod catalog;
pub mod staticdata;
pub mod modfiles;

use crate::{error::{Error, Result}, model::SyncReport, state::AppState};

pub async fn sync(state: &AppState, category: &str, source_path: Option<&str>, force: bool) -> Result<SyncReport> {
    match category {
        "all" => {
            let mut mods = modfiles::sync(state, force).await?;
            let stat = staticdata::sync(state, source_path).await?;
            mods.needs_restart |= stat.needs_restart;
            mods.version = stat.version;
            Ok(mods)
        },
        "modfiles" => modfiles::sync(state, force).await,
        "staticdata" => staticdata::sync(state, source_path).await,
        "bundles" => Err(Error::msg("bundle diff sync is planned for a later phase")),
        other => Err(Error::msg(format!("unknown resource category: {}", other))),
    }
}
