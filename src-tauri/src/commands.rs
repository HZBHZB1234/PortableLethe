//! Stable Tauri command contract (doc 02 §9). Thin wrappers only.
use tauri::State;
use crate::{core, error::{Error, ErrorBody}, model::*, state::{AppState, EV_RESTART_REQUESTED}};
use tauri::Emitter;

type CResult<T> = std::result::Result<T, ErrorBody>;
fn err<E: Into<Error>>(e: E) -> ErrorBody { ErrorBody::from(e.into()) }
fn todo(feature: &str) -> ErrorBody { err(Error::msg(format!("{}: pending implementation", feature))) }

/// Open a URL or directory with the OS default handler. Arguments are passed
/// as separate values; no shell string is built from user input.
fn reveal(target: &str, is_url: bool) -> CResult<bool> {
    #[cfg(windows)]
    {
        let mut cmd = std::process::Command::new(if is_url { "rundll32" } else { "explorer" });
        if is_url { cmd.args(["url.dll,FileProtocolHandler", target]); }
        else { cmd.arg(target); }
        cmd.spawn().map_err(err)?;
    }
    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(target).spawn().map_err(err)?;
    }
    Ok(true)
}

#[tauri::command]
pub fn hub_reveal_dir(state: State<'_, AppState>) -> CResult<bool> {
    reveal(&state.hub.root.to_string_lossy(), false)
}

#[tauri::command]
pub async fn hub_status(state: State<'_, AppState>) -> CResult<HubStatus> {
    let p = state.persisted.read().await;
    let rt = state.runtime.read().await;
    Ok(HubStatus {
        app_version: env!("CARGO_PKG_VERSION").into(),
        hub_dir: state.hub.root.to_string_lossy().into_owned(),
        platform: std::env::consts::OS.into(),
        instance_count: p.instances.len(),
        version_count: p.versions.len(),
        running_count: rt.values().filter(|r| r.state == InstanceState::Running).count(),
        config: state.hub.config.clone(),
        resources: p.resources.clone(),
    })
}

#[tauri::command]
pub async fn hub_config_schema(state: State<'_, AppState>) -> CResult<ConfigSchema> {
    Ok(core::config::schema(&state.hub))
}

#[tauri::command]
pub async fn hub_config_get(state: State<'_, AppState>, instance_id: String) -> CResult<JsonMap> {
    core::config::get(&state, &instance_id).await.map_err(err)
}

#[tauri::command]
pub async fn hub_config_set(state: State<'_, AppState>, instance_id: String, values: JsonMap) -> CResult<JsonMap> {
    core::config::set(&state, &instance_id, &values).await.map_err(err)
}

#[tauri::command]
pub async fn hub_config_update(state: State<'_, AppState>, values: JsonMap) -> CResult<HubConfig> {
    let mut cfg = state.hub.config.clone();
    let merged = serde_json::to_value(&cfg).map_err(err)?;
    let mut obj = merged.as_object().cloned().unwrap_or_default();
    for (k, v) in values { obj.insert(k, v); }
    cfg = serde_json::from_value(serde_json::Value::Object(obj)).map_err(err)?;
    let mut hub = state.hub.clone();
    hub.config = cfg.clone();
    hub.save_config().map_err(err)?;
    Ok(cfg)
}

#[tauri::command]
pub async fn instance_list(state: State<'_, AppState>) -> CResult<Vec<InstanceInfo>> {
    core::process::list(&state).await.map_err(err)
}

#[tauri::command]
pub async fn instance_create(state: State<'_, AppState>, name: String, version: Option<String>, port: Option<u16>) -> CResult<InstanceInfo> {
    core::process::create(&state, &name, version.as_deref(), port).await.map_err(err)
}

#[tauri::command]
pub async fn instance_start(state: State<'_, AppState>, id: String) -> CResult<InstanceInfo> {
    core::process::start(&state, &id).await.map_err(err)
}

#[tauri::command]
pub async fn instance_stop(state: State<'_, AppState>, id: String) -> CResult<InstanceInfo> {
    core::process::stop(&state, &id).await.map_err(err)
}

#[tauri::command]
pub async fn instance_restart(state: State<'_, AppState>, id: String) -> CResult<InstanceInfo> {
    core::process::restart(&state, &id).await.map_err(err)
}

#[tauri::command]
pub async fn instance_delete(state: State<'_, AppState>, id: String) -> CResult<bool> {
    core::process::delete(&state, &id).await.map_err(err)
}

#[tauri::command]
pub async fn instance_open_admin(state: State<'_, AppState>, id: String) -> CResult<bool> {
    let items = core::process::list(&state).await.map_err(err)?;
    let inst = items.into_iter().find(|i| i.id == id)
        .ok_or_else(|| err(Error::msg("instance not found")))?;
    reveal(&format!("http://127.0.0.1:{}/admin/", inst.port), true)
}

#[tauri::command]
pub async fn version_list(state: State<'_, AppState>) -> CResult<Vec<VersionInfo>> {
    core::version::list(&state).await.map_err(err)
}

#[tauri::command]
pub async fn version_install(state: State<'_, AppState>, url: Option<String>, version: Option<String>, sha256: Option<String>) -> CResult<VersionInfo> {
    core::version::install(&state, url.as_deref(), version.as_deref(), sha256.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn version_build_from_source(state: State<'_, AppState>, repo_url: Option<String>, version: Option<String>) -> CResult<VersionInfo> {
    core::version::build_from_source(&state, repo_url.as_deref(), version.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn version_switch(state: State<'_, AppState>, version: String) -> CResult<VersionInfo> {
    core::version::switch(&state, &version).await.map_err(err)
}

#[tauri::command]
pub async fn version_rollback(state: State<'_, AppState>) -> CResult<VersionInfo> {
    core::version::rollback(&state).await.map_err(err)
}

#[tauri::command]
pub async fn resource_status(state: State<'_, AppState>) -> CResult<ResourceStatus> {
    Ok(state.persisted.read().await.resources.clone())
}

#[tauri::command]
pub async fn resource_sync(state: State<'_, AppState>, category: String, source_path: Option<String>, force: Option<bool>) -> CResult<SyncReport> {
    core::resources::sync(&state, &category, source_path.as_deref(), force.unwrap_or(false)).await.map_err(err)
}

#[tauri::command]
pub async fn clientpkg_export(state: State<'_, AppState>, instance_id: String, output: Option<String>) -> CResult<ClientPkgResult> {
    core::package::export(&state, &instance_id, output.as_deref()).await.map_err(err)
}

#[tauri::command]
pub async fn log_tail(state: State<'_, AppState>, limit: Option<usize>, instance_id: Option<String>) -> CResult<Vec<LogLine>> {
    let limit = limit.unwrap_or(200).min(1000);
    let logs = state.logs.read().await;
    let mut merged: Vec<LogLine> = logs.iter().cloned().collect();
    if let Some(g) = crate::state::GLOBAL_LOGS.get() {
        if let Ok(gl) = g.lock() { merged.extend(gl.iter().cloned()); }
    }
    merged.sort_by(|a, b| a.ts.cmp(&b.ts));
    merged.dedup_by(|a, b| a.ts == b.ts && a.message == b.message);
    let logs = merged;
    let lines: Vec<LogLine> = if let Some(id) = instance_id {
        logs.iter().filter(|l| l.target.contains(&id)).rev().take(limit).cloned().collect::<Vec<_>>().into_iter().rev().collect()
    } else { logs.iter().rev().take(limit).cloned().collect::<Vec<_>>().into_iter().rev().collect() };
    Ok(lines)
}

#[tauri::command]
pub async fn request_restart(state: State<'_, AppState>, instance_id: String, reason: String) -> CResult<bool> {
    state.hub.root.to_string_lossy().len(); // keep state reference live
    if let Ok(h) = state.handle.lock() {
        if let Some(h) = h.as_ref() {
            let _ = h.emit(EV_RESTART_REQUESTED, &RestartRequestedEvent { instance_id: instance_id.clone(), reason });
        }
    }
    core::process::restart(&state, &instance_id).await.map_err(err)?;
    Ok(true)
}

#[tauri::command]
pub async fn catalog_find(state: State<'_, AppState>, pattern: String, catalog_path: Option<String>) -> CResult<Vec<core::resources::catalog::CatalogHit>> {
    core::resources::catalog::find(&state.hub, pattern.as_str(), catalog_path.as_deref()).map_err(err)
}

#[allow(dead_code)]
fn phase1_stub_check() -> ErrorBody { todo("core") }
