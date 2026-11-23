// Mirror of the Rust serde DTOs in src-tauri/src/model.rs.
// This file is the UI<->core contract; keep it in sync when changing commands.

export type InstanceState = 'Stopped' | 'Starting' | 'Running' | 'Unhealthy' | 'Updating' | 'Failed';

export interface HubConfig {
  data_dir: string | null;
  gh_proxy_prefix: string;
  github_proxy: 'auto' | 'none' | 'gh-proxy.org' | 'gh-proxy.com' | string;
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

export interface HubStatus {
  app_version: string;
  hub_dir: string;
  platform: string;
  instance_count: number;
  version_count: number;
  running_count: number;
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
