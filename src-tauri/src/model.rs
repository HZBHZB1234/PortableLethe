//! serde DTOs forming the stable UI <-> Rust core contract (doc 02 §9).
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};

pub type JsonMap = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HubConfig {
    pub data_dir: Option<String>,
    /// Legacy explicit proxy prefix. Kept for hand-edited configs; the UI now
    /// uses `github_proxy` (auto | none | gh-proxy.org | gh-proxy.com).
    pub gh_proxy_prefix: String,
    pub github_proxy: String,
    /// Static resource source: release | official | lethe.
    pub static_source: String,
    /// Release API used to discover standalone static zip assets.
    pub static_release_api: String,
    /// Explicit static zip URL; overrides Release API discovery.
    pub static_zip_url: Option<String>,
    /// Lethe installation dir used for its catalog/settings extraction.
    pub lethe_dir: Option<String>,
    /// Bundle CDN template; `{token}` and `{name}` are expanded.
    pub lethe_cdn: String,
    /// X-Requested-With header required by both bundle CDNs.
    pub cdn_requested_with: String,
    pub release_api: String,
    pub zip_url: Option<String>,
    pub expected_sha256: Option<String>,
    pub keep_versions: usize,
    pub port_range_start: u16,
    pub port_range_end: u16,
    pub health_timeout_ms: u64,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    pub launcher_source: Option<String>,
    pub bepinex_source: Option<String>,
    pub catalog_path: Option<String>,
    pub settings_path: Option<String>,
    pub game_dir: Option<String>,
    pub proxy: Option<String>,
    /// Optional source-build channel settings (doc 02 §3).
    pub source_repo_url: Option<String>,
    pub dotnet_sdk_channel: String,
}
impl Default for HubConfig {
    fn default() -> Self {
        Self {
            data_dir: None,
            gh_proxy_prefix: String::new(),
            github_proxy: "auto".into(),
            static_source: "release".into(),
            static_release_api: "https://api.github.com/repos/HZBHZB1234/PortableOpenLethe/releases/latest".into(),
            static_zip_url: None,
            lethe_dir: None,
            lethe_cdn: "https://assets.lethelc.site/{token}/{name}".into(),
            cdn_requested_with: "this_is_header_value".into(),
            release_api: "https://api.github.com/repos/HZBHZB1234/PortableOpenLethe/releases/latest".into(),
            zip_url: None, expected_sha256: None, keep_versions: 3,
            port_range_start: 5180, port_range_end: 5280,
            health_timeout_ms: 30_000, backoff_base_ms: 1_000, backoff_max_ms: 30_000,
            launcher_source: None, bepinex_source: None, catalog_path: None,
            settings_path: None, game_dir: None, proxy: None,
            source_repo_url: None, dotnet_sdk_channel: "9.0".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CategoryState {
    pub source: Option<String>,
    pub version: Option<String>,
    pub etag: Option<String>,
    pub updated_at: Option<String>,
    pub files: BTreeMap<String, String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceStatus {
    pub current: String,
    pub categories: BTreeMap<String, CategoryState>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HubStatus {
    pub app_version: String,
    pub hub_dir: String,
    pub platform: String,
    pub instance_count: usize,
    pub version_count: usize,
    pub running_count: usize,
    pub config: HubConfig,
    pub resources: ResourceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub owner: String,
    pub requires_restart: bool,
    pub secret: bool,
    pub default: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigSchema {
    pub source: String,
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InstanceState { #[default] Stopped, Starting, Running, Unhealthy, Updating, Failed }
impl InstanceState { pub fn as_str(&self) -> &'static str { match self {
    Self::Stopped => "Stopped", Self::Starting => "Starting", Self::Running => "Running",
    Self::Unhealthy => "Unhealthy", Self::Updating => "Updating", Self::Failed => "Failed" } } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub port: u16,
    pub created_at: String,
    #[serde(default)]
    pub auto_restart: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub port: u16,
    pub state: InstanceState,
    pub pid: Option<u32>,
    pub health: bool,
    pub created_at: String,
    pub data_dir: String,
    pub restart_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub name: String,
    pub path: String,
    pub installed_at: String,
    pub size_bytes: u64,
    pub is_current: bool,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SyncReport {
    pub category: String,
    pub source: Option<String>,
    pub version: String,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub skipped: usize,
    pub needs_restart: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientPkgResult {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceStateEvent {
    pub id: String,
    pub state: InstanceState,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestartRequestedEvent {
    pub instance_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub ts: String,
    pub level: String,
    pub target: String,
    pub message: String,
}
