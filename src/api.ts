import { invoke } from '@tauri-apps/api/core';
import type {
  ClientPkgResult, ConfigSchema, DepReport, HubConfig, HubStatus, InstanceInfo,
  LogLine, ResourceStatus, SyncReport, VersionInfo,
} from './types';

// Thin, typed wrappers around the stable Tauri command contract.
// The UI never touches the filesystem or spawns processes itself.
export const api = {
  hubStatus: () => invoke<HubStatus>('hub_status'),
  hubRevealDir: () => invoke<boolean>('hub_reveal_dir'),
  hubEnsureDeps: (instanceId?: string | null) => invoke<DepReport>('hub_ensure_deps', { instanceId: instanceId ?? null }),
  hubSettingsGet: () => invoke<HubConfig>('hub_settings_get'),
  hubConfigSchema: () => invoke<ConfigSchema>('hub_config_schema'),
  hubConfigGet: (instanceId: string) => invoke<Record<string, unknown>>('hub_config_get', { instanceId }),
  hubConfigSet: (instanceId: string, values: Record<string, unknown>) =>
    invoke<Record<string, unknown>>('hub_config_set', { instanceId, values }),
  hubConfigUpdate: (values: Partial<HubConfig>) => invoke<HubConfig>('hub_config_update', { values }),

  instanceList: () => invoke<InstanceInfo[]>('instance_list'),
  instanceCreate: (name: string, version?: string | null, port?: number | null) =>
    invoke<InstanceInfo>('instance_create', { name, version: version ?? null, port: port ?? null }),
  instanceStart: (id: string) => invoke<InstanceInfo>('instance_start', { id }),
  instanceStop: (id: string) => invoke<InstanceInfo>('instance_stop', { id }),
  instanceRestart: (id: string) => invoke<InstanceInfo>('instance_restart', { id }),
  instanceDelete: (id: string) => invoke<boolean>('instance_delete', { id }),
  instanceOpenAdmin: (id: string) => invoke<boolean>('instance_open_admin', { id }),

  versionList: () => invoke<VersionInfo[]>('version_list'),
  versionInstall: (url?: string | null, version?: string | null, sha256?: string | null) =>
    invoke<VersionInfo>('version_install', { url: url ?? null, version: version ?? null, sha256: sha256 ?? null }),
  versionBuildFromSource: (repoUrl?: string | null, version?: string | null) =>
    invoke<VersionInfo>('version_build_from_source', { repoUrl: repoUrl ?? null, version: version ?? null }),
  versionSwitch: (version: string) => invoke<VersionInfo>('version_switch', { version }),
  versionRollback: () => invoke<VersionInfo>('version_rollback'),

  resourceStatus: () => invoke<ResourceStatus>('resource_status'),
  resourceSync: (category: string, sourcePath?: string | null, force = false) =>
    invoke<SyncReport>('resource_sync', { category, sourcePath: sourcePath ?? null, force }),

  clientpkgExport: (instanceId: string, output?: string | null) =>
    invoke<ClientPkgResult>('clientpkg_export', { instanceId, output: output ?? null }),
  logTail: (limit = 200, instanceId?: string | null) =>
    invoke<LogLine[]>('log_tail', { limit, instanceId: instanceId ?? null }),
};

export const onEvent = <T>(event: string, cb: (payload: T) => void) =>
  import('@tauri-apps/api/event').then(({ listen }) =>
    listen<T>(event, (e) => cb(e.payload)).then((un) => () => { void un(); })
  );

export const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export const exampleHubConfig: HubConfig = {
  data_dir: null, gh_proxy_prefix: '', github_proxy: 'auto',
  release_api: 'https://api.github.com/repos/HZBHZB1234/PortableOpenLethe/releases/latest',
  zip_url: null, expected_sha256: null, keep_versions: 3,
  port_range_start: 5180, port_range_end: 5280, health_timeout_ms: 30000,
  backoff_base_ms: 1000, backoff_max_ms: 30000,
  launcher_source: null, bepinex_source: null, catalog_path: null,
  settings_path: null, game_dir: null, proxy: null,
  source_repo_url: null, dotnet_sdk_channel: '9.0',
};
