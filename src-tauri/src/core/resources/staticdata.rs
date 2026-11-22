//! unityfs TextAsset extraction to StaticData/ + Localize/ (doc 04 §3/§5).
use std::path::Path;
use crate::{error::Result, state::AppState};

pub async fn sync(_state: &AppState, _catalog_path: Option<&str>) -> Result<crate::model::SyncReport> {
    Err(crate::error::Error::msg("staticdata sync: phase 3"))
}

pub fn extract_bundle(bundle: &Path, out_dir: &Path) -> Result<usize> {
    let _ = (bundle, out_dir);
    Err(crate::error::Error::msg("staticdata extract: phase 3"))
}
