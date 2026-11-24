// Mirror of the Rust serde DTOs in src-tauri/src/model.rs / commands.rs.
// This file is the UI <-> core contract; keep it in sync with the frozen Tauri API.

export type InstanceState = 'Stopped' | 'Starting' | 'Running' | 'Unhealthy' | 'Updating' | 'Failed';

/** 资源集来源（resource_set_create 的 source 参数取值）。 */
export type ResourceSource = 'release' | 'official' | 'lethe' | 'game' | 'local';

export interface HubConfig {
  data_dir: string | null;
  gh_proxy_prefix: string;
  github_proxy: 'auto' | 'none' | 'gh-proxy.org' | 'gh-proxy.com' | string;
  static_source: 'release' | 'official' | 'lethe' | string;
  static_release_api: string;
  static_zip_url: string | null;
  lethe_dir: string | null;
  lethe_cdn: string;
  cdn_requested_with: string;
  release_api: string;
  zip_url: string | null;
  expected_sha256: string | null;
  keep_versions: number;
  port_range_start: number;
  port_range_end: number;
  health_timeout_ms: number;
  backoff_base_ms: number;
  backoff_max_ms: number;
  launcher_source: string | null;
  bepinex_source: string | null;
  catalog_path: string | null;
  settings_path: string | null;
  game_dir: string | null;
  proxy: string | null;
  source_repo_url: string | null;
  dotnet_sdk_channel: string;
}

export interface DepReport {
  fixed: string[];
  missing: string[];
}

export interface CategoryState {
  source: string | null;
  version: string | null;
  etag: string | null;
  updated_at: string | null;
  files: Record<string, string>;
  last_error: string | null;
}

export interface ResourceStatus {
  current: string;
  categories: Record<string, CategoryState>;
}

/** game_detect() 的返回值：本机 Limbus Company 安装探测结果。 */
export interface GameInstall {
  found: boolean;
  game_dir: string | null;
  data_dir: string | null;
  cache_dir: string | null;
  save_dir: string | null;
  catalog: string | null;
  settings: string | null;
  lang_dir: string | null;
  streaming_dir: string | null;
  version: string | null;
}

/** resource_set_list() / resource_set_create() 的返回值。 */
export interface ResourceSet {
  id: string;
  name: string;
  kind: string;
  source: string;
  version: string | null;
  path: string;
  static_dir: string;
  localize_dir: string;
  modfiles_dir: string;
  extra: Record<string, string>;
  created_at: string;
  size_bytes: number;
  file_count: number;
  is_default: boolean;
}

export interface HubStatus {
  app_version: string;
  hub_dir: string;
  platform: string;
  instance_count: number;
  version_count: number;
  running_count: number;
  resource_set_count: number;
  game_install: GameInstall | null;
  config: HubConfig;
  resources: ResourceStatus;
}

export interface ConfigField {
  key: string;
  label: string;
  kind: 'string' | 'number' | 'boolean' | 'secret' | 'path';
  owner: 'hub' | 'server';
  requires_restart: boolean;
  secret: boolean;
  default: unknown;
  description: string;
}

export interface ConfigSchema {
  source: string;
  fields: ConfigField[];
}

export interface InstanceInfo {
  id: string;
  name: string;
  version: string | null;
  resource_set: string | null;
  port: number;
  state: InstanceState;
  pid: number | null;
  health: boolean;
  created_at: string;
  data_dir: string;
  restart_count: number;
}

export interface VersionInfo {
  name: string;
  path: string;
  installed_at: string;
  size_bytes: number;
  is_current: boolean;
  source: string;
}

export interface SyncReport {
  category: string;
  source: string | null;
  version: string;
  added: number;
  changed: number;
  removed: number;
  skipped: number;
  needs_restart: boolean;
  errors: string[];
}

export interface ClientPkgResult {
  path: string;
  sha256: string;
  size_bytes: number;
  file_count: number;
}

export interface LogLine {
  ts: string;
  level: string;
  target: string;
  message: string;
}

export interface ProgressEvent {
  phase: string;
  current: number;
  total: number;
  message: string;
}

export interface DownloadProgressEvent {
  url: string;
  downloaded: number;
  total: number;
}

export interface InstanceStateEvent {
  id: string;
  state: InstanceState;
  pid: number | null;
}

export interface RestartRequestedEvent {
  instance_id: string;
  reason: string;
}
