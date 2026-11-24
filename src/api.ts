import { invoke } from '@tauri-apps/api/core';
import type {
  ClientPkgResult, ConfigSchema, DepReport, GameInstall, HubConfig, HubStatus,
  InstanceInfo, LogLine, ResourceSet, ResourceSource, ResourceStatus, SyncReport,
  VersionInfo,
} from './types';

// Thin, typed wrappers around the frozen Tauri command contract.
// The UI never touches the filesystem or spawns processes itself.
//
// Browser dev (`npm run dev`) has no Tauri runtime: every call is routed to an
// in-memory mock so the whole Hub remains usable for UI development. In a real
// app `isTauri()` is true and the real backend is always used — backend errors
// are intentionally NOT swallowed so the UI's errorText() pattern still works.

export const isTauri = () =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function bridge<T>(
  command: string,
  args: Record<string, unknown>,
  fallback: () => T | Promise<T>,
): Promise<T> {
  if (isTauri()) return invoke<T>(command, args); // real errors bubble up
  return await fallback();
}

const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value)) as T;
const nowIso = () => new Date().toISOString();
const sleep = (ms = 180) => new Promise<void>((r) => window.setTimeout(r, ms));

export const exampleHubConfig: HubConfig = {
  data_dir: String.raw`C:\Users\tester\PortableLetheHub`,
  gh_proxy_prefix: 'https://gh-proxy.org/',
  github_proxy: 'auto',
  static_source: 'release',
  static_release_api: 'https://api.github.com/repos/HZBHZB1234/PortableOpenLethe/releases/latest',
  static_zip_url: null,
  lethe_dir: null,
  lethe_cdn: 'https://assets.lethelc.site/{token}/{name}',
  cdn_requested_with: 'this_is_header_value',
  release_api: 'https://api.github.com/repos/HZBHZB1234/PortableOpenLethe/releases/latest',
  zip_url: null,
  expected_sha256: null,
  keep_versions: 3,
  port_range_start: 5180,
  port_range_end: 5280,
  health_timeout_ms: 30000,
  backoff_base_ms: 1000,
  backoff_max_ms: 30000,
  launcher_source: null,
  bepinex_source: null,
  catalog_path: String.raw`C:\Users\tester\AppData\LocalLow\ProjectMoon\LimbusCompany\com.unity.addressables\catalog_S1.bin`,
  settings_path: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\StreamingAssets\aa\settings.json`,
  game_dir: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company`,
  proxy: null,
  source_repo_url: 'https://github.com/HZBHZB1234/PortableOpenLethe',
  dotnet_sdk_channel: '9.0',
};

/* ------------------------------------------------------------------ *
 * Browser preview fixtures (only used when Tauri is unavailable)
 * ------------------------------------------------------------------ */

const mockHubDir = String.raw`C:\Users\tester\PortableLetheHub`;

const mockGameInstall: GameInstall = {
  found: true,
  game_dir: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company`,
  data_dir: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data`,
  cache_dir: String.raw`D:\Unity\ProjectMoon_LimbusCompany`,
  save_dir: String.raw`E:\limbus\ProjectMoon\LimbusCompany`,
  catalog: String.raw`C:\Users\tester\AppData\LocalLow\ProjectMoon\LimbusCompany\com.unity.addressables\catalog_S1.bin`,
  settings: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\StreamingAssets\aa\settings.json`,
  lang_dir: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\lang\LLC_zh-CN`,
  streaming_dir: String.raw`C:\Program Files (x86)\Steam\steamapps\common\Limbus Company\LimbusCompany_Data\StreamingAssets\aa`,
  version: '1.12.3 (6000.3.12f1)',
};

let mockConfig: HubConfig = clone(exampleHubConfig);

let mockVersions: VersionInfo[] = [
  {
    name: 'v2.4.1-release',
    path: `${mockHubDir}\\versions\\v2.4.1-release`,
    installed_at: '2026-11-20 18:32',
    size_bytes: 193273528,
    is_current: true,
    source: 'release',
  },
  {
    name: 'v2.3.8-release',
    path: `${mockHubDir}\\versions\\v2.3.8-release`,
    installed_at: '2026-10-02 09:14',
    size_bytes: 187695104,
    is_current: false,
    source: 'release',
  },
  {
    name: 'v2.4.0-source',
    path: `${mockHubDir}\\versions\\v2.4.0-source`,
    installed_at: '2026-11-11 22:07',
    size_bytes: 404750336,
    is_current: false,
    source: 'source',
  },
];

let mockResourceSets: ResourceSet[] = [
  {
    id: 'rs-release-default',
    name: '随版本发布快照',
    kind: 'release',
    source: 'release',
    version: '2026-11-18',
    path: `${mockHubDir}\\resources\\sets\\rs-release-default`,
    static_dir: `${mockHubDir}\\resources\\sets\\rs-release-default\\static`,
    localize_dir: `${mockHubDir}\\resources\\sets\\rs-release-default\\localize`,
    modfiles_dir: `${mockHubDir}\\resources\\sets\\rs-release-default\\modfiles`,
    extra: { catalog: 'catalog_S1.bin', note: '随服务端发布，开箱即用' },
    created_at: '2026-11-20 18:35',
    size_bytes: 268435456,
    file_count: 2185,
    is_default: true,
  },
  {
    id: 'rs-local-game',
    name: '本机游戏（检测导入）',
    kind: 'game',
    source: 'game',
    version: '1.12.3',
    path: `${mockHubDir}\\resources\\sets\\rs-local-game`,
    static_dir: `${mockHubDir}\\resources\\sets\\rs-local-game\\static`,
    localize_dir: `${mockHubDir}\\resources\\sets\\rs-local-game\\localize`,
    modfiles_dir: `${mockHubDir}\\resources\\sets\\rs-local-game\\modfiles`,
    extra: { lang: 'LLC_zh-CN' },
    created_at: '2026-11-22 13:10',
    size_bytes: 3221225472,
    file_count: 55123,
    is_default: false,
  },
  {
    id: 'rs-lethe-cdn',
    name: '私服 CDN 资源',
    kind: 'lethe',
    source: 'lethe',
    version: '2026-11-21',
    path: `${mockHubDir}\\resources\\sets\\rs-lethe-cdn`,
    static_dir: `${mockHubDir}\\resources\\sets\\rs-lethe-cdn\\static`,
    localize_dir: `${mockHubDir}\\resources\\sets\\rs-lethe-cdn\\localize`,
    modfiles_dir: `${mockHubDir}\\resources\\sets\\rs-lethe-cdn\\modfiles`,
    extra: {},
    created_at: '2026-11-21 20:45',
    size_bytes: 413990912,
    file_count: 3402,
    is_default: false,
  },
];

let mockInstances: InstanceInfo[] = [
  {
    id: 'ins-a1b2c3',
    name: '主线私服',
    version: 'v2.4.1-release',
    resource_set: 'rs-release-default',
    port: 5180,
    state: 'Running',
    pid: 18432,
    health: true,
    created_at: '2026-11-20 19:02',
    data_dir: `${mockHubDir}\\instances\\ins-a1b2c3\\data`,
    restart_count: 1,
  },
  {
    id: 'ins-d4e5f6',
    name: '镜像测试',
    version: 'v2.3.8-release',
    resource_set: 'rs-lethe-cdn',
    port: 5181,
    state: 'Stopped',
    pid: null,
    health: false,
    created_at: '2026-11-21 11:26',
    data_dir: `${mockHubDir}\\instances\\ins-d4e5f6\\data`,
    restart_count: 0,
  },
  {
    id: 'ins-g7h8i9',
    name: '资源验证',
    version: 'v2.4.0-source',
    resource_set: 'rs-local-game',
    port: 5182,
    state: 'Unhealthy',
    pid: 22108,
    health: false,
    created_at: '2026-11-23 08:47',
    data_dir: `${mockHubDir}\\instances\\ins-g7h8i9\\data`,
    restart_count: 4,
  },
];

const mockInstanceValues = new Map<string, Record<string, unknown>>();

const mockResourceStatus: ResourceStatus = {
  current: `${mockHubDir}\\resources\\active`,
  categories: {
    staticdata: {
      source: 'release', version: '2026-11-18', etag: 'W/"9f3a"',
      updated_at: '2026-11-20 18:35',
      files: { 'userticket_l.json': 'a1', 'userbanner.json': 'b2', 'greeting.json': 'c3' },
      last_error: null,
    },
    modfiles: {
      source: 'local', version: '2026-11-23', etag: null,
      updated_at: '2026-11-23 09:12',
      files: { 'BepInEx.dll': 'd4', 'PortableLethe.dll': 'e5' },
      last_error: null,
    },
    bundles: {
      source: 'local', version: '1.12.3', etag: null,
      updated_at: '2026-11-22 13:15',
      files: {}, last_error: null,
    },
  },
};

const mockLogs: LogLine[] = [
  { ts: '2026-11-24 18:02:11', level: 'info', target: 'hub', message: 'Hub 启动完成，版本 v0.1.1' },
  { ts: '2026-11-24 18:02:12', level: 'info', target: 'server', message: '实例 主线私服 已就绪，监听 5180' },
  { ts: '2026-11-24 18:03:40', level: 'warn', target: 'health', message: '资源验证 健康检查超时（30000ms），准备重启' },
  { ts: '2026-11-24 18:03:41', level: 'info', target: 'server', message: '实例 资源验证 重启 #4' },
  { ts: '2026-11-24 18:05:02', level: 'error', target: 'resource', message: 'catalog_S1.bin 校验失败，已回退上一版本' },
  { ts: '2026-11-24 18:05:03', level: 'info', target: 'hub', message: '资源同步完成：staticdata +3 changed 0' },
];

const mockConfigSchema: ConfigSchema = {
  source: 'hub-config.json + server.bootstrap.json',
  fields: [
    { key: 'github_proxy', label: 'GitHub 代理', kind: 'string', owner: 'hub', requires_restart: false, secret: false, default: 'auto', description: '下载 Release 与同步资源时使用的代理源。' },
    { key: 'static_source', label: '默认静态数据来源', kind: 'string', owner: 'hub', requires_restart: false, secret: false, default: 'release', description: 'release / official / lethe，仅影响之后新建的实例。' },
    { key: 'port_range_start', label: '端口范围起点', kind: 'number', owner: 'hub', requires_restart: false, secret: false, default: 5180, description: '新建实例时自动分配端口的下界。' },
    { key: 'port_range_end', label: '端口范围终点', kind: 'number', owner: 'hub', requires_restart: false, secret: false, default: 5280, description: '新建实例时自动分配端口的上界。' },
    { key: 'data_dir', label: '数据根目录', kind: 'path', owner: 'hub', requires_restart: true, secret: false, default: null, description: 'Hub 数据根目录，修改后需重启 Hub。' },
    { key: 'lethe_dir', label: '私服资源目录', kind: 'path', owner: 'hub', requires_restart: false, secret: false, default: null, description: 'lethe 来源的本地资源目录。' },
    { key: 'catalog_path', label: 'Addressables catalog', kind: 'path', owner: 'hub', requires_restart: false, secret: false, default: null, description: 'catalog_S1.bin 路径，用于 bundles 同步。' },
    { key: 'settings_path', label: 'Unity settings.json', kind: 'path', owner: 'hub', requires_restart: false, secret: false, default: null, description: '游戏 StreamingAssets/aa/settings.json。' },
    { key: 'game_dir', label: '游戏本体目录', kind: 'path', owner: 'hub', requires_restart: false, secret: false, default: null, description: 'Limbus Company 安装目录。' },
    { key: 'keep_versions', label: '保留版本数', kind: 'number', owner: 'hub', requires_restart: false, secret: false, default: 3, description: '自动清理时保留的历史版本数量。' },
    { key: 'health_timeout_ms', label: '健康检查超时 (ms)', kind: 'number', owner: 'hub', requires_restart: false, secret: false, default: 30000, description: '判定实例不健康的等待时间。' },
    { key: 'backoff_base_ms', label: '退避基数 (ms)', kind: 'number', owner: 'hub', requires_restart: false, secret: false, default: 1000, description: '自动重启的初始退避时间。' },
    { key: 'server_port', label: '服务端口', kind: 'number', owner: 'server', requires_restart: true, secret: false, default: 5180, description: '服务端监听端口。' },
    { key: 'max_players', label: '最大玩家数', kind: 'number', owner: 'server', requires_restart: true, secret: false, default: 32, description: '服务端允许的同时在线人数。' },
  ],
};

const mockHubStatus = (): HubStatus => ({
  app_version: '0.1.1',
  hub_dir: mockHubDir,
  platform: 'win32-x64',
  instance_count: mockInstances.length,
  version_count: mockVersions.length,
  running_count: mockInstances.filter((i) => i.state === 'Running').length,
  resource_set_count: mockResourceSets.length,
  game_install: clone(mockGameInstall),
  config: clone(mockConfig),
  resources: clone(mockResourceStatus),
});

const currentVersionName = () => mockVersions.find((v) => v.is_current)?.name ?? mockVersions[0]?.name ?? 'v0.0.0';
const nextPort = () => {
  const used = new Set(mockInstances.map((i) => i.port));
  for (let p = mockConfig.port_range_start; p <= mockConfig.port_range_end; p++) {
    if (!used.has(p)) return p;
  }
  return mockConfig.port_range_start;
};

/* ------------------------------------------------------------------ *
 * Public API
 * ------------------------------------------------------------------ */

export const api = {
  // ---- Hub -----------------------------------------------------------
  hubStatus: () => bridge<HubStatus>('hub_status', {}, mockHubStatus),
  hubRevealDir: () => bridge<boolean>('hub_reveal_dir', {}, () => {
    // eslint-disable-next-line no-console
    console.info('[browser preview] 打开 Hub 目录:', mockHubDir);
    return true;
  }),
  hubEnsureDeps: (instanceId?: string | null) =>
    bridge<DepReport>('hub_ensure_deps', { instanceId: instanceId ?? null }, async () => {
      await sleep();
      return { fixed: [], missing: [] };
    }),
  hubSettingsGet: () => bridge<HubConfig>('hub_settings_get', {}, () => clone(mockConfig)),
  hubConfigSchema: () => bridge<ConfigSchema>('hub_config_schema', {}, () => clone(mockConfigSchema)),
  hubConfigGet: (instanceId: string) =>
    bridge<Record<string, unknown>>('hub_config_get', { instanceId }, () => {
      if (!mockInstanceValues.has(instanceId)) {
        mockInstanceValues.set(instanceId, {
          server_port: mockInstances.find((i) => i.id === instanceId)?.port ?? 5180,
          max_players: 32,
        });
      }
      return clone(mockInstanceValues.get(instanceId)!);
    }),
  hubConfigSet: (instanceId: string, values: Record<string, unknown>) =>
    bridge<Record<string, unknown>>('hub_config_set', { instanceId, values }, async () => {
      await sleep();
      const next = { ...(mockInstanceValues.get(instanceId) ?? {}), ...values };
      mockInstanceValues.set(instanceId, next);
      return clone(next);
    }),
  hubConfigUpdate: (values: Partial<HubConfig>) =>
    bridge<HubConfig>('hub_config_update', { values }, async () => {
      await sleep();
      mockConfig = { ...mockConfig, ...values };
      return clone(mockConfig);
    }),

  // ---- Instances -----------------------------------------------------
  instanceList: () => bridge<InstanceInfo[]>('instance_list', {}, () => clone(mockInstances)),
  /** 注意参数顺序：name, version, resource_set, port（冻结契约）。 */
  instanceCreate: (
    name: string,
    version?: string | null,
    resource_set?: string | null,
    port?: number | null,
  ) => bridge<InstanceInfo>('instance_create', {
    name,
    version: version ?? null,
    resource_set: resource_set ?? null,
    port: port ?? null,
  }, async () => {
    await sleep(220);
    const item: InstanceInfo = {
      id: `ins-${Math.random().toString(16).slice(2, 8)}`,
      name: name.trim() || '未命名实例',
      version: version || currentVersionName(),
      resource_set: resource_set || mockResourceSets.find((r) => r.is_default)?.id || null,
      port: port ?? nextPort(),
      state: 'Stopped',
      pid: null,
      health: false,
      created_at: nowIso(),
      data_dir: `${mockHubDir}\\instances\\new\\data`,
      restart_count: 0,
    };
    mockInstances.push(item);
    return clone(item);
  }),
  instanceStart: (id: string) =>
    bridge<InstanceInfo>('instance_start', { id }, async () => {
      await sleep();
      const i = mockInstances.find((x) => x.id === id);
      if (!i) throw new Error(`实例不存在: ${id}`);
      i.state = 'Running'; i.pid = 20000 + Math.floor(Math.random() * 40000); i.health = true;
      return clone(i);
    }),
  instanceStop: (id: string) =>
    bridge<InstanceInfo>('instance_stop', { id }, async () => {
      await sleep();
      const i = mockInstances.find((x) => x.id === id);
      if (!i) throw new Error(`实例不存在: ${id}`);
      i.state = 'Stopped'; i.pid = null; i.health = false;
      return clone(i);
    }),
  instanceRestart: (id: string) =>
    bridge<InstanceInfo>('instance_restart', { id }, async () => {
      await sleep();
      const i = mockInstances.find((x) => x.id === id);
      if (!i) throw new Error(`实例不存在: ${id}`);
      i.state = 'Running'; i.pid = 20000 + Math.floor(Math.random() * 40000); i.health = true;
      i.restart_count += 1;
      return clone(i);
    }),
  instanceDelete: (id: string) =>
    bridge<boolean>('instance_delete', { id }, async () => {
      await sleep();
      const n = mockInstances.length;
      mockInstances = mockInstances.filter((x) => x.id !== id);
      mockInstanceValues.delete(id);
      return n !== mockInstances.length;
    }),
  instanceReveal: (id: string) =>
    bridge<boolean>('instance_reveal', { id }, () => {
      window.open('file:///C:/Users/tester/PortableLetheHub/instances/' + id, '_blank');
      return true;
    }),
  instanceOpenAdmin: (id: string) =>
    bridge<boolean>('instance_open_admin', { id }, () => {
      // eslint-disable-next-line no-console
      console.info('[browser preview] 打开管理面板:', id);
      return true;
    }),
  instanceSetVersion: (id: string, version: string) =>
    bridge<InstanceInfo>('instance_set_version', { id, version }, async () => {
      await sleep();
      const i = mockInstances.find((x) => x.id === id);
      if (!i) throw new Error(`实例不存在: ${id}`);
      i.version = version;
      return clone(i);
    }),
  instanceSetResourceSet: (id: string, resource_set: string) =>
    bridge<InstanceInfo>('instance_set_resource_set', { id, resource_set }, async () => {
      await sleep();
      const i = mockInstances.find((x) => x.id === id);
      if (!i) throw new Error(`实例不存在: ${id}`);
      i.resource_set = resource_set || null;
      return clone(i);
    }),

  // ---- Server versions ----------------------------------------------
  versionList: () => bridge<VersionInfo[]>('version_list', {}, () => clone(mockVersions)),
  versionReveal: (version: string) =>
    bridge<boolean>('version_reveal', { version }, () => {
      window.open('file:///C:/Users/tester/PortableLetheHub/versions/' + version, '_blank');
      return true;
    }),
  versionInstall: (url?: string | null, version?: string | null, sha256?: string | null) =>
    bridge<VersionInfo>('version_install', {
      url: url ?? null, version: version ?? null, sha256: sha256 ?? null,
    }, async () => {
      await sleep(320);
      const item: VersionInfo = {
        name: version || `v2.5.0-${Date.now().toString(36)}`,
        path: `${mockHubDir}\\versions\\${version || 'v2.5.0'}`,
        installed_at: nowIso().slice(0, 16).replace('T', ' '),
        size_bytes: 199229440,
        is_current: false,
        source: 'release',
      };
      mockVersions.push(item);
      return clone(item);
    }),
  versionBuildFromSource: (repoUrl?: string | null, version?: string | null) =>
    bridge<VersionInfo>('version_build_from_source', {
      repoUrl: repoUrl ?? null, version: version ?? null,
    }, async () => {
      await sleep(400);
      const item: VersionInfo = {
        name: version || `v2.5.0-source-${Date.now().toString(36)}`,
        path: `${mockHubDir}\\versions\\source`,
        installed_at: nowIso().slice(0, 16).replace('T', ' '),
        size_bytes: 421108121,
        is_current: false,
        source: 'source',
      };
      mockVersions.push(item);
      return clone(item);
    }),
  versionSwitch: (version: string) =>
    bridge<VersionInfo>('version_switch', { version }, async () => {
      await sleep();
      const v = mockVersions.find((x) => x.name === version);
      if (!v) throw new Error(`版本不存在: ${version}`);
      mockVersions.forEach((x) => { x.is_current = x.name === version; });
      return clone(v);
    }),
  versionRollback: () =>
    bridge<VersionInfo>('version_rollback', {}, async () => {
      await sleep();
      const prev = mockVersions.find((x) => !x.is_current);
      if (!prev) throw new Error('没有可回滚的历史版本');
      mockVersions.forEach((x) => { x.is_current = x.name === prev.name; });
      return clone(prev);
    }),

  // ---- Game detection & resource sets --------------------------------
  gameDetect: () =>
    bridge<GameInstall>('game_detect', {}, async () => {
      await sleep(120);
      return clone(mockGameInstall);
    }),
  resourceSetList: () => bridge<ResourceSet[]>('resource_set_list', {}, () => clone(mockResourceSets)),
  resourceSetCreate: (name: string, source: ResourceSource | string, source_path?: string | null) =>
    bridge<ResourceSet>('resource_set_create', {
      name, source, source_path: source_path ?? null,
    }, async () => {
      await sleep(320);
      const id = `rs-${Math.random().toString(16).slice(2, 8)}`;
      const item: ResourceSet = {
        id,
        name: name.trim() || `资源集 ${mockResourceSets.length + 1}`,
        kind: source,
        source,
        version: source === 'game' ? mockGameInstall.version : 'local',
        path: `${mockHubDir}\\resources\\sets\\${id}`,
        static_dir: `${mockHubDir}\\resources\\sets\\${id}\\static`,
        localize_dir: `${mockHubDir}\\resources\\sets\\${id}\\localize`,
        modfiles_dir: `${mockHubDir}\\resources\\sets\\${id}\\modfiles`,
        extra: source_path ? { source_path } : {},
        created_at: nowIso().slice(0, 16).replace('T', ' '),
        size_bytes: source === 'game' ? 3221225472 : 0,
        file_count: source === 'game' ? 55123 : 0,
        is_default: false,
      };
      mockResourceSets.push(item);
      return clone(item);
    }),
  resourceSetDelete: (id: string) =>
    bridge<boolean>('resource_set_delete', { id }, async () => {
      await sleep();
      const n = mockResourceSets.length;
      mockResourceSets = mockResourceSets.filter((x) => x.id !== id);
      if (mockInstances.some((i) => i.resource_set === id)) {
        mockInstances.forEach((i) => { if (i.resource_set === id) i.resource_set = null; });
      }
      return n !== mockResourceSets.length;
    }),
  resourceSetDefault: (id: string) =>
    bridge<boolean>('resource_set_default', { id }, async () => {
      await sleep();
      if (!mockResourceSets.some((x) => x.id === id)) return false;
      mockResourceSets.forEach((x) => { x.is_default = x.id === id; });
      return true;
    }),
  resourceSetReveal: (id: string) =>
    bridge<boolean>('resource_set_reveal', { id }, () => {
      // eslint-disable-next-line no-console
      console.info('[browser preview] 打开资源集目录:', id);
      return true;
    }),

  // ---- Resources / export / logs ------------------------------------
  resourceStatus: () => bridge<ResourceStatus>('resource_status', {}, () => clone(mockResourceStatus)),
  resourceSync: (category: string, source?: string | null, sourcePath?: string | null, force = false) =>
    bridge<SyncReport>('resource_sync', {
      category, source: source ?? null, sourcePath: sourcePath ?? null, force,
    }, async () => {
      await sleep(280);
      return {
        category, source: source ?? null, version: '2026-11-24',
        added: 12, changed: 3, removed: 0, skipped: 2185,
        needs_restart: category !== 'modfiles', errors: [],
      };
    }),
  clientpkgExport: (instanceId: string, output?: string | null) =>
    bridge<ClientPkgResult>('clientpkg_export', {
      instanceId, output: output ?? null,
    }, async () => {
      await sleep(380);
      return {
        path: output || `${mockHubDir}\\clientpkg\\PortableLetheClient.zip`,
        sha256: '9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08',
        size_bytes: 83886080,
        file_count: 126,
      };
    }),
  logTail: (limit = 200, instanceId?: string | null) =>
    bridge<LogLine[]>('log_tail', { limit, instanceId: instanceId ?? null }, () =>
      clone(mockLogs.slice(-Math.max(0, limit)))),
};

export const onEvent = <T>(event: string, cb: (payload: T) => void): Promise<() => void> => {
  if (!isTauri()) return Promise.resolve(() => { /* no events in browser preview */ });
  return import('@tauri-apps/api/event').then(({ listen }) =>
    listen<T>(event, (e) => cb(e.payload)).then((un) => () => { void un(); })
  );
};
