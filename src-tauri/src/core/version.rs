use crate::{error::Result, model::VersionInfo, state::AppState};

pub async fn list(_state: &AppState) -> Result<Vec<VersionInfo>> { Ok(Vec::new()) }
pub async fn install(_state: &AppState, _url: Option<&str>, _version: Option<&str>, _sha256: Option<&str>) -> Result<VersionInfo> {
    Err(crate::error::Error::msg("version install: phase 2"))
}
pub async fn switch(_state: &AppState, _version: &str) -> Result<VersionInfo> {
    Err(crate::error::Error::msg("version switch: phase 2"))
}
pub async fn rollback(_state: &AppState) -> Result<VersionInfo> {
    Err(crate::error::Error::msg("version rollback: phase 2"))
}
