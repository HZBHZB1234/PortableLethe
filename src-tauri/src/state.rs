//! Shared application state + event bus. Commands stay thin; all real work is in `core`.
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use crate::{core::{Hub, PersistedState}, error::Result, model::*};

/// Process output (captured on OS threads) lands here; commands read both rings.
pub static GLOBAL_LOGS: std::sync::OnceLock<std::sync::Mutex<VecDeque<LogLine>>> = std::sync::OnceLock::new();

pub fn global_log(line: LogLine) {
    if let Some(g) = GLOBAL_LOGS.get() {
        if let Ok(mut v) = g.lock() {
            v.push_back(line);
            while v.len() > 500 { v.pop_front(); }
        }
    }
}

pub const EV_INSTANCE_STATE: &str = "instance-state";
pub const EV_SYNC_PROGRESS: &str = "sync-progress";
pub const EV_DOWNLOAD_PROGRESS: &str = "download-progress";
pub const EV_LOG_LINE: &str = "log-line";
pub const EV_RESTART_REQUESTED: &str = "restart-requested";

#[derive(Debug, Clone, Default)]
pub struct RuntimeInfo {
    pub state: InstanceState,
    pub pid: Option<u32>,
    pub health: bool,
    pub restart_count: u32,
}

pub struct AppState {
    pub hub: Hub,
    pub persisted: Arc<RwLock<PersistedState>>,
    pub runtime: Arc<RwLock<BTreeMap<String, RuntimeInfo>>>,
    pub logs: Arc<RwLock<VecDeque<LogLine>>>,
    pub handle: Arc<std::sync::Mutex<Option<AppHandle>>>,
    pub monitors: Arc<std::sync::Mutex<HashMap<String, tokio::sync::watch::Sender<bool>>>>,
    pub children: Arc<std::sync::Mutex<HashMap<String, std::sync::Arc<std::sync::Mutex<Option<std::process::Child>>>>>>,
}

impl AppState {
    pub fn new(hub: Hub) -> Result<Self> {
        let _ = GLOBAL_LOGS.set(std::sync::Mutex::new(VecDeque::with_capacity(512)));
        let persisted = hub.load_state();
        let mut runtime = BTreeMap::new();
        for r in &persisted.instances {
            runtime.insert(r.id.clone(), RuntimeInfo { state: InstanceState::Stopped, pid: None, health: false, restart_count: 0 });
        }
        Ok(Self {
            hub, persisted: Arc::new(RwLock::new(persisted)), runtime: Arc::new(RwLock::new(runtime)),
            logs: Arc::new(RwLock::new(VecDeque::with_capacity(512))),
            handle: Arc::new(std::sync::Mutex::new(None)), monitors: Arc::new(std::sync::Mutex::new(HashMap::new())),
            children: Arc::new(std::sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn log(&self, level: &str, target: &str, message: &str) {
        let line = LogLine { ts: crate::core::util::now_iso(), level: level.into(), target: target.into(), message: message.into() };
        if let Ok(mut logs) = self.logs.try_write() {
            logs.push_back(line.clone());
            while logs.len() > 500 { logs.pop_front(); }
        }
        global_log(line.clone());
        let path = self.hub.logs_dir().join("hub.log");
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            use std::io::Write;
            let _ = writeln!(f, "{} [{}] [{}] {}", line.ts, line.level, line.target, line.message);
        }
        if let Ok(h) = self.handle.lock() {
            if let Some(h) = h.as_ref() { let _ = h.emit(EV_LOG_LINE, &line); }
        }
    }

    pub async fn set_runtime(&self, id: &str, patch: RuntimeInfoPatch) {
        let mut rt = self.runtime.write().await;
        let e = rt.entry(id.to_string()).or_default();
        if let Some(s) = patch.state { e.state = s; }
        if let Some(p) = patch.pid { e.pid = Some(p); }
        if patch.pid == Some(0) { e.pid = None; }
        if let Some(h) = patch.health { e.health = h; }
        if let Some(r) = patch.restart_count { e.restart_count = r; }
        let snapshot = e.clone();
        drop(rt);
        if let Ok(h) = self.handle.lock() {
            if let Some(h) = h.as_ref() {
                let _ = h.emit(EV_INSTANCE_STATE, &InstanceStateEvent {
                    id: id.to_string(), state: snapshot.state, pid: snapshot.pid,
                });
            }
        }
    }

    pub async fn persist(&self) -> Result<()> {
        let p = self.persisted.read().await;
        self.hub.save_state(&p)
    }
}

#[derive(Default)]
pub struct RuntimeInfoPatch {
    pub state: Option<InstanceState>,
    pub pid: Option<u32>,
    pub health: Option<bool>,
    pub restart_count: Option<u32>,
}
