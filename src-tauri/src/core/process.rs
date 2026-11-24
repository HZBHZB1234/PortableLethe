//! Server process lifecycle: spawn, health probe, exponential backoff restart,
//! per-instance state machine and log capture (doc 02 §5).
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use crate::{core::{config, storage, util}, error::{Error, Result}, model::*, state::{AppState, RuntimeInfo, RuntimeInfoPatch}};

fn version_path(state: &AppState, rec: &InstanceRecord) -> Option<PathBuf> {
    if let Some(v) = rec.version.as_deref() {
        return Some(state.hub.current_version_path(v));
    }
    state.persisted.try_read().ok()
        .and_then(|p| p.current_version.clone())
        .map(|v| state.hub.current_version_path(&v))
}

fn resource_root(state: &AppState, rec: &InstanceRecord) -> Option<PathBuf> {
    crate::core::resources::sets::root_for(state, rec.resource_set.as_deref()).ok()
        .or_else(|| {
            let p = state.hub.resources_current();
            if p.is_dir() { Some(p) } else { None }
        })
}

/// Repoint all junctions an instance depends on. Safe to call repeatedly; this
/// is what makes switching server version or data set a one-command operation.
async fn wire_instance(state: &AppState, rec: &InstanceRecord, mut rep: Option<&mut DepReport>) -> Result<()> {
    let dir = state.hub.instance_dir(&rec.id);
    for sub in ["data", "logs", "modfiles", "plugins"] { std::fs::create_dir_all(dir.join(sub))?; }
    let vp = match version_path(state, rec) {
        Some(v) => v,
        None => {
            if let Some(r) = rep.as_deref_mut() { r.missing.push(format!("{}:server 未选择服务端版本", rec.id)); }
            return Ok(());
        }
    };
    if !vp.join("OpenLethe.Server.exe").is_file() && !vp.join("OpenLethe.Server.dll").is_file() {
        if let Some(r) = rep.as_deref_mut() { r.missing.push(format!("{}:server 版本 {} 未安装", rec.id, rec.version.as_deref().unwrap_or("?"))); }
        return Ok(());
    }
    util::create_junction(&vp, &dir.join("server"))?;
    if vp.join("wwwroot").is_dir() {
        util::create_junction(&vp.join("wwwroot"), &dir.join("wwwroot"))?;
    }
    if vp.join("plugins").is_dir() {
        util::create_junction(&vp.join("plugins"), &dir.join("plugins-src"))?;
    }
    let rr = resource_root(state, rec);
    match rr {
        Some(root) if root.join("StaticData").is_dir() => {
            util::create_junction(&root, &dir.join("static"))?;
            for extra in ["modfiles", "Localize"] {
                let src = root.join(extra);
                if src.is_dir() { util::create_junction(&src, &dir.join(format!("{}-src", extra.to_lowercase())))?; }
            }
        }
        _ => {
            if let Some(r) = rep.as_deref_mut() { r.missing.push(format!("{}:static 数据资源集未就绪", rec.id)); }
        }
    }
    let boot = dir.join("data").join("server.bootstrap.json");
    if !boot.is_file() {
        let defaults = config::bootstrap_defaults(state, &rec.id, rec.port);
        config::set(state, &rec.id, &defaults).await?;
        if let Some(r) = rep.as_deref_mut() { r.fixed.push(format!("{}:bootstrap", rec.id)); }
    } else {
        // Rewrite bootstrap so STATIC_DATA_DIR follows a switched resource set.
        config::set(state, &rec.id, &config::raw_values(state, &rec.id)?).await?;
    }
    Ok(())
}

fn find_exe(root: &Path) -> Option<PathBuf> {
    let mut all: Vec<PathBuf> = walkdir::WalkDir::new(root).follow_links(true).max_depth(4)
        .into_iter().flatten()
        .filter(|e| e.file_type().is_file() && e.path().extension().map(|x| x.eq_ignore_ascii_case("exe")).unwrap_or(false))
        .map(|e| e.path().to_path_buf()).collect();
    all.sort_by_key(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let rank = if n.contains("portableopenlethe") { 0 } else if n.contains("openlethe") { 1 }
                   else if n.contains("lethe") { 2 } else { 3 };
        (rank, p.components().count())
    });
    all.into_iter().next()
}

fn pipe_logs<R: std::io::Read + Send + 'static>(stream: R, level: &'static str, logs_dir: &Path, id: &str) {
    use std::io::{BufRead, BufReader, Write};
    let logs_dir = logs_dir.to_path_buf();
    let id = id.to_string();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(std::result::Result::ok) {
            let line = LogLine {
                ts: util::now_iso(), level: level.into(), target: format!("instance:{}", id), message: line,
            };
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true)
                .open(logs_dir.join(format!("{}.log", id))) {
                let _ = writeln!(f, "{} [{}] {}", line.ts, line.level, line.message);
            }
            crate::state::global_log(line);
        }
    });
}

fn spawn_child(state: &AppState, rec: &InstanceRecord) -> Result<Child> {
    let dir = state.hub.instance_dir(&rec.id);
    let server = dir.join("server");
    let exe = find_exe(&server).ok_or_else(|| Error::msg(format!(
        "no server executable under {} (install a version first)", server.display())))?;
    let mut cmd = Command::new(&exe);
    cmd.current_dir(&dir).stdout(Stdio::piped()).stderr(Stdio::piped());
    // Bootstrap keys become environment variables; keys with ':' stay as-is.
    if let Ok(vals) = config::raw_values(state, &rec.id) {
        for (k, v) in vals {
            let s = match v { serde_json::Value::String(s) => s, other => other.to_string() };
            cmd.env(k, s);
        }
    }
    cmd.env("PORTABLE_LETHE_INSTANCE_ID", &rec.id);
    let mut child = cmd.spawn()?;
    // Capture stdout/stderr into the Hub log.
    if let Some(stream) = child.stdout.take() {
        pipe_logs(stream, "info", &state.hub.logs_dir(), &rec.id);
    }
    if let Some(stream) = child.stderr.take() {
        pipe_logs(stream, "warn", &state.hub.logs_dir(), &rec.id);
    }
    Ok(child)
}

async fn wait_health(port: u16, timeout_ms: u64, proxy: Option<&str>) -> bool {
    let client = match util::http_client(proxy) { Ok(c) => c, Err(_) => return false };
    let url = format!("http://127.0.0.1:{}/health", port);
    let deadline = tokio::time::Instant::now() + Duration::from_millis(timeout_ms);
    while tokio::time::Instant::now() < deadline {
        if let Ok(r) = client.get(&url).send().await {
            if r.status().is_success() { return true; }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

async fn monitor_loop(state: AppState, id: String, rec: InstanceRecord,
                      child_arc: Arc<Mutex<Option<Child>>>,
                      mut stop_rx: tokio::sync::watch::Receiver<bool>) {
    let cfg = state.hub.config.clone();
    let mut backoff = cfg.backoff_base_ms.max(250);
    loop {
        let healthy = wait_health(rec.port, cfg.health_timeout_ms, state.hub.proxy_url().as_deref()).await;
        state.set_runtime(&id, RuntimeInfoPatch { health: Some(healthy),
            state: Some(if healthy { InstanceState::Running } else { InstanceState::Unhealthy }),
            pid: None, restart_count: None }).await;
        state.log("info", "process", &format!("instance {} health={}", id, healthy));
        // Wait for either a manual stop or an unexpected process exit.
        let mut exited = false;
        while !*stop_rx.borrow() {
            if let Ok(mut guard) = child_arc.try_lock() {
                if let Some(c) = guard.as_mut() {
                    match c.try_wait() {
                        Ok(Some(status)) => { exited = true; state.log("warn", "process", &format!("instance {} exited: {}", id, status)); break; }
                        Ok(None) => {},
                        Err(e) => { state.log("warn", "process", &format!("wait error: {}", e)); exited = true; break; }
                    }
                }
            }
            if !exited { tokio::time::sleep(Duration::from_millis(350)).await; }
            if stop_rx.changed().await.is_err() { break; }
        }
        if *stop_rx.borrow() { break; }
        if !exited { break; }
        let count = state.runtime.read().await.get(&id).map(|r| r.restart_count + 1).unwrap_or(1);
        state.set_runtime(&id, RuntimeInfoPatch { state: Some(InstanceState::Starting), pid: Some(0), health: Some(false), restart_count: Some(count) }).await;
        state.log("warn", "process", &format!("restarting {} in {}ms (attempt {})", id, backoff, count));
        tokio::time::sleep(Duration::from_millis(backoff)).await;
        backoff = (backoff * 2).min(cfg.backoff_max_ms.max(backoff));
        match spawn_child(&state, &rec) {
            Ok(c) => { *child_arc.lock().unwrap() = Some(c); },
            Err(e) => { state.log("error", "process", &format!("restart failed: {}", e)); break; }
        }
    }
    state.set_runtime(&id, RuntimeInfoPatch { state: Some(InstanceState::Stopped), pid: Some(0), health: Some(false), restart_count: None }).await;
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DepReport {
    pub fixed: Vec<String>,
    pub missing: Vec<String>,
}

/// Self-heal everything a running instance needs: version/server/wwwroot/static
/// links, directories and its bootstrap config. Called on Hub startup, instance
/// start, and from the UI's "检查依赖" button.
pub async fn ensure_deps(state: &AppState, id: Option<&str>) -> Result<DepReport> {
    let records: Vec<InstanceRecord> = {
        let p = state.persisted.read().await;
        p.instances.iter()
            .filter(|i| id.map_or(true, |x| x == i.id))
            .cloned().collect()
    };
    let mut rep = DepReport::default();
    for rec in &records {
        wire_instance(state, rec, Some(&mut rep)).await?;
    }
    Ok(rep)
}

pub async fn info(state: &AppState, id: &str) -> Result<InstanceInfo> {
    list(state).await?.into_iter().find(|i| i.id == id)
        .ok_or_else(|| Error::msg(format!("unknown instance {}", id)))
}

pub async fn list(state: &AppState) -> Result<Vec<InstanceInfo>> {
    let p = state.persisted.read().await;
    let rt = state.runtime.read().await;
    Ok(p.instances.iter().map(|r| {
        let run = rt.get(&r.id).cloned().unwrap_or_default();
        InstanceInfo {
            id: r.id.clone(), name: r.name.clone(), version: r.version.clone(),
            resource_set: r.resource_set.clone(), port: r.port,
            state: run.state, pid: run.pid, health: run.health, created_at: r.created_at.clone(),
            data_dir: state.hub.instance_dir(&r.id).join("data").to_string_lossy().into_owned(),
            restart_count: run.restart_count,
        }
    }).collect())
}

pub async fn create(state: &AppState, name: &str, version: Option<&str>,
                    resource_set: Option<&str>, port: Option<u16>) -> Result<InstanceInfo> {
    let base = util::sanitize_name(name);
    let id = if state.persisted.read().await.instances.iter().any(|i| i.id == base) {
        format!("{}-{}", base, uuid::Uuid::new_v4().simple().to_string()[..6].to_string())
    } else { base };
    let port = port.unwrap_or_else(|| storage::allocate_port(state));
    let current = state.persisted.read().await.current_version.clone();
    let ver = version.map(str::to_string).or(current)
        .ok_or_else(|| Error::msg("install a server version before creating an instance"))?;
    let rs = resource_set.map(str::to_string).or_else(|| {
        state.persisted.try_read().ok().and_then(|p| p.default_resource_set.clone())
    });
    let rec = InstanceRecord { id: id.clone(), name: name.to_string(), version: Some(ver),
        resource_set: rs.clone(), port, created_at: util::now_iso(), auto_restart: true };
    {
        let mut p = state.persisted.write().await;
        p.instances.push(rec.clone());
    }
    state.runtime.write().await.insert(id.clone(), RuntimeInfo { state: InstanceState::Stopped, ..Default::default() });
    wire_instance(state, &rec, None).await?;
    state.persist().await?;
    state.log("info", "instance", &format!("created {} on port {} (server={}, resources={})",
        id, port, rec.version.as_deref().unwrap_or("?"), rs.as_deref().unwrap_or("default")));
    info(state, &id).await
}

/// Switch the server executable an instance uses. Stops the process, repoints
/// the junction, and restarts it if it was running.
pub async fn set_version(state: &AppState, id: &str, version: &str) -> Result<InstanceInfo> {
    let vp = state.hub.current_version_path(version);
    if !vp.join("OpenLethe.Server.exe").is_file() && !vp.join("OpenLethe.Server.dll").is_file() {
        return Err(Error::msg(format!("version {} is not installed", version)));
    }
    let (mut rec, was_running) = {
        let p = state.persisted.read().await;
        let rec = p.instances.iter().find(|i| i.id == id).cloned()
            .ok_or_else(|| Error::msg("unknown instance"))?;
        let running = match state.runtime.try_read() {
            Ok(guard) => guard.get(id).map(|r| r.state == InstanceState::Running).unwrap_or(false),
            Err(_) => false,
        };
        (rec, running)
    };
    if was_running { stop(state, id).await?; }
    rec.version = Some(version.to_string());
    { let mut p = state.persisted.write().await;
      if let Some(r) = p.instances.iter_mut().find(|i| i.id == id) { *r = rec.clone(); } }
    wire_instance(state, &rec, None).await?;
    state.persist().await?;
    if was_running { start(state, id).await?; }
    state.log("info", "instance", &format!("{} switched to server version {}", id, version));
    info(state, id).await
}

/// Switch the data-resource set an instance uses (junctions only, no copy).
pub async fn set_resource_set(state: &AppState, id: &str, resource_set: &str) -> Result<InstanceInfo> {
    let root = crate::core::resources::sets::root_for(state, Some(resource_set))?;
    if !root.join("StaticData").is_dir() {
        return Err(Error::msg("resource set has no StaticData directory"));
    }
    let (mut rec, was_running) = {
        let p = state.persisted.read().await;
        let rec = p.instances.iter().find(|i| i.id == id).cloned()
            .ok_or_else(|| Error::msg("unknown instance"))?;
        let running = match state.runtime.try_read() {
            Ok(guard) => guard.get(id).map(|r| r.state == InstanceState::Running).unwrap_or(false),
            Err(_) => false,
        };
        (rec, running)
    };
    if was_running { stop(state, id).await?; }
    rec.resource_set = Some(resource_set.to_string());
    { let mut p = state.persisted.write().await;
      if let Some(r) = p.instances.iter_mut().find(|i| i.id == id) { *r = rec.clone(); } }
    wire_instance(state, &rec, None).await?;
    state.persist().await?;
    if was_running { start(state, id).await?; }
    state.log("info", "instance", &format!("{} switched to resource set {}", id, resource_set));
    info(state, id).await
}

pub async fn start(state: &AppState, id: &str) -> Result<InstanceInfo> {
    let rec = {
        let p = state.persisted.read().await;
        p.instances.iter().find(|i| i.id == id).cloned().ok_or_else(|| Error::msg("unknown instance"))?
    };
    stop_monitor(state, id).await;
    // Self-heal links/config for instances created by older Hub builds.
    let deps = ensure_deps(state, Some(id)).await?;
    if !deps.fixed.is_empty() {
        state.log("info", "process", &format!("auto-configured {}: {}", id, deps.fixed.join(", ")));
    }
    let child = spawn_child(state, &rec)?;
    let pid = child.id();
    let arc = Arc::new(Mutex::new(Some(child)));
    state.children.lock().unwrap().insert(id.to_string(), arc.clone());
    let (tx, rx) = tokio::sync::watch::channel(false);
    state.monitors.lock().unwrap().insert(id.to_string(), tx);
    state.set_runtime(id, RuntimeInfoPatch { state: Some(InstanceState::Starting), pid: Some(pid), health: Some(false), restart_count: Some(0) }).await;
    let st = AppState { hub: state.hub.clone(), persisted: state.persisted.clone(), runtime: state.runtime.clone(),
        logs: state.logs.clone(), handle: state.handle.clone(), monitors: state.monitors.clone(), children: state.children.clone() };
    tokio::spawn(monitor_loop(st, id.to_string(), rec, arc, rx));
    state.log("info", "process", &format!("started instance {} (pid {})", id, pid));
    info(state, id).await
}

async fn stop_monitor(state: &AppState, id: &str) {
    if let Some(tx) = state.monitors.lock().unwrap().remove(id) { let _ = tx.send(true); }
}

pub async fn stop(state: &AppState, id: &str) -> Result<InstanceInfo> {
    stop_monitor(state, id).await;
    if let Some(arc) = state.children.lock().unwrap().remove(id) {
        if let Ok(mut guard) = arc.lock() {
            if let Some(c) = guard.as_mut() { let _ = c.kill(); let _ = c.wait(); }
            *guard = None;
        }
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    state.set_runtime(id, RuntimeInfoPatch { state: Some(InstanceState::Stopped), pid: Some(0), health: Some(false), restart_count: None }).await;
    state.log("info", "process", &format!("stopped instance {}", id));
    info(state, id).await
}

pub async fn restart(state: &AppState, id: &str) -> Result<InstanceInfo> {
    stop(state, id).await?;
    start(state, id).await
}

pub async fn stop_all(state: &AppState) {
    let ids: Vec<String> = state.persisted.read().await.instances.iter().map(|i| i.id.clone()).collect();
    for id in ids { let _ = stop(state, &id).await; }
}

pub async fn delete(state: &AppState, id: &str) -> Result<bool> {
    stop(state, id).await?;
    storage::remove_instance_layout(state, id)?;
    let mut p = state.persisted.write().await;
    p.instances.retain(|i| i.id != id);
    drop(p);
    state.runtime.write().await.remove(id);
    state.persist().await?;
    state.log("info", "instance", &format!("deleted {}", id));
    Ok(true)
}
