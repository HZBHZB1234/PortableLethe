use crate::{error::Result, model::ClientPkgResult, state::AppState};

pub async fn export(_state: &AppState, _instance_id: &str, _output: Option<&str>) -> Result<ClientPkgResult> {
    Err(crate::error::Error::msg("client package export: phase 2"))
}
