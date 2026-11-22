use crate::{error::Result, model::{InstanceInfo, InstanceState}, state::{AppState, RuntimeInfo}};

pub async fn list(state: &AppState) -> Result<Vec<InstanceInfo>> {
    let p = state.persisted.read().await;
    let rt = state.runtime.read().await;
    Ok(p.instances.iter().map(|r| {
        let run = rt.get(&r.id).cloned().unwrap_or_default();
        InstanceInfo {
            id: r.id.clone(), name: r.name.clone(), version: r.version.clone(), port: r.port,
            state: if run.state == InstanceState::Stopped { InstanceState::Stopped } else { run.state },
            pid: run.pid, health: run.health, created_at: r.created_at.clone(),
            data_dir: state.hub.instance_dir(&r.id).join("data").to_string_lossy().into_owned(),
            restart_count: run.restart_count,
        }
    }).collect())
}

pub async fn _runtime_of(state: &AppState, id: &str) -> RuntimeInfo {
    state.runtime.read().await.get(id).cloned().unwrap_or_default()
}

pub async fn create(_state: &AppState, _name: &str, _version: Option<&str>, _port: Option<u16>) -> Result<InstanceInfo> {
    Err(crate::error::Error::msg("instance create: phase 2"))
}
pub async fn start(_state: &AppState, _id: &str) -> Result<InstanceInfo> { Err(crate::error::Error::msg("instance start: phase 2")) }
pub async fn stop(_state: &AppState, _id: &str) -> Result<InstanceInfo> { Err(crate::error::Error::msg("instance stop: phase 2")) }
pub async fn restart(_state: &AppState, _id: &str) -> Result<InstanceInfo> { Err(crate::error::Error::msg("instance restart: phase 2")) }
pub async fn delete(_state: &AppState, _id: &str) -> Result<bool> { Err(crate::error::Error::msg("instance delete: phase 2")) }
