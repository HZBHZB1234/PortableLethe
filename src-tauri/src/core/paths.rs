//! Hub directory layout and persisted state (doc 02 §7).
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use crate::{error::{Error, Result}, model::*};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersistedState {
    pub instances: Vec<InstanceRecord>,
    pub versions: Vec<VersionInfo>,
    pub current_version: Option<String>,
    pub previous_version: Option<String>,
    pub resources: ResourceStatus,
}

#[derive(Debug, Clone)]
pub struct Hub {
    pub root: PathBuf,
    pub config: HubConfig,
}

impl Hub {
    /// Hub root: env override > %LOCALAPPDATA%/PortableLethe/hub > ./hub
    pub fn new() -> Result<Self> {
        let root = if let Ok(p) = std::env::var("PORTABLE_LETHE_HUB_DIR") {
            PathBuf::from(p)
        } else {
            dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."))
                .join("PortableLethe").join("hub")
        };
        std::fs::create_dir_all(&root)?;
        let cfg_path = root.join("hub-config.json");
        let config = if cfg_path.exists() {
            let raw: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&cfg_path)?)?;
            let defaults = serde_json::to_value(HubConfig::default())?;
            let mut merged = defaults.as_object().cloned().unwrap_or_default();
            if let Some(obj) = raw.as_object() {
                for (k, v) in obj { merged.insert(k.clone(), v.clone()); }
                // Migrate pre-settings configs that hardcoded the broken prefix.
                if !obj.contains_key("github_proxy")
                    && obj.get("gh_proxy_prefix").and_then(|v| v.as_str())
                        .map(|s| s.contains("gh-proxy.com")).unwrap_or(false) {
                    merged.insert("github_proxy".into(), serde_json::json!("auto"));
                    merged.insert("gh_proxy_prefix".into(), serde_json::json!(""));
                }
            }
            serde_json::from_value(serde_json::Value::Object(merged))?
        } else { HubConfig::default() };
        let hub = Self { root, config };
        hub.ensure_layout()?;
        Ok(hub)
    }

    pub fn ensure_layout(&self) -> Result<()> {
        for p in [self.blobs_dir(), self.versions_dir(), self.resources_dir(),
                  self.instances_dir(), self.clientpkg_dir(), self.logs_dir(),
                  self.staging_dir()] {
            std::fs::create_dir_all(p)?;
        }
        Ok(())
    }

    pub fn config_path(&self) -> PathBuf { self.root.join("hub-config.json") }
    pub fn state_path(&self) -> PathBuf { self.root.join("state.json") }
    pub fn blobs_dir(&self) -> PathBuf { self.root.join("blobs").join("sha256") }
    pub fn versions_dir(&self) -> PathBuf { self.root.join("versions") }
    pub fn staging_dir(&self) -> PathBuf { self.versions_dir().join(".staging") }
    pub fn resources_dir(&self) -> PathBuf { self.root.join("resources") }
    pub fn resources_current(&self) -> PathBuf { self.resources_dir().join("current") }
    /// Stable real directory whose children are per-category junctions.
    pub fn resources_active(&self) -> PathBuf { self.resources_dir().join("active") }
    pub fn resources_objects(&self) -> PathBuf { self.resources_dir().join("objects") }
    pub fn instances_dir(&self) -> PathBuf { self.root.join("instances") }
    pub fn clientpkg_dir(&self) -> PathBuf { self.root.join("clientpkg") }
    pub fn logs_dir(&self) -> PathBuf { self.root.join("logs") }
    pub fn tools_dir(&self) -> PathBuf { self.root.join("tools") }

    pub fn save_config(&self) -> Result<()> {
        crate::core::util::atomic_write_json(&self.config_path(), &self.config, false)
    }

    pub fn load_state(&self) -> PersistedState {
        let p = self.state_path();
        std::fs::read_to_string(&p).ok()
            .and_then(|s| serde_json::from_str::<PersistedState>(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_state(&self, state: &PersistedState) -> Result<()> {
        crate::core::util::atomic_write_json(&self.state_path(), state, true)
    }

    /// `current` is a junction pointing at versions/<name> (or resources/official/<v>).
    pub fn current_version_path(&self, name: &str) -> PathBuf {
        self.versions_dir().join(crate::core::util::sanitize_name(name))
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir().join(crate::core::util::sanitize_name(id))
    }

    pub fn proxy_url(&self) -> Option<String> {
        let raw = self.config.proxy.as_deref().unwrap_or("");
        if raw.is_empty() { None } else { Some(raw.to_string()) }
    }

    /// Read the default static source from disk so UI changes apply immediately.
    pub fn default_static_source(&self) -> String {
        if let Ok(text) = std::fs::read_to_string(self.config_path()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(s) = v.get("static_source").and_then(|x| x.as_str()) {
                    return s.to_string();
                }
            }
        }
        self.config.static_source.clone()
    }

    /// Read the GitHub proxy choice from disk on every request so changing it
    /// in the settings UI takes effect immediately (no Hub restart).
    pub fn github_proxy_mode(&self) -> String {
        if let Ok(text) = std::fs::read_to_string(self.config_path()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(m) = v.get("github_proxy").and_then(|x| x.as_str()) {
                    return m.to_string();
                }
                if v.get("gh_proxy_prefix").and_then(|x| x.as_str())
                    .map(|s| s.contains("gh-proxy.com")).unwrap_or(false) {
                    return "auto".into();
                }
            }
        }
        if self.config.github_proxy.trim().is_empty() {
            self.config.gh_proxy_prefix.clone()
        } else {
            self.config.github_proxy.clone()
        }
    }
}

/// Normalise any user-supplied path (env vars, ~, relative) before touching disk.
pub fn expand_path(s: &str, base: &Path) -> PathBuf {
    let mut out = s.trim().to_string();
    if out.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            out = out.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    for (k, v) in [("%LOCALAPPDATA%", std::env::var("LOCALAPPDATA")),
                   ("%APPDATA%", std::env::var("APPDATA")),
                   ("%USERPROFILE%", std::env::var("USERPROFILE"))] {
        if let Ok(v) = v { out = out.replace(k, &v); }
    }
    let p = PathBuf::from(out);
    if p.is_absolute() { p } else { base.join(p) }
}

#[allow(dead_code)]
fn _assert_error(_: Error) {}
