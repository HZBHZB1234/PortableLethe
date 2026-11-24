import { useEffect, useMemo, useState, type ReactNode } from 'react';
import { api } from './api';
import type {
  ConfigField, ConfigSchema, GameInstall, HubConfig, HubStatus, InstanceInfo,
  LogLine, ResourceSet, ResourceSource, VersionInfo,
} from './types';

/* ------------------------------------------------------------------ *
 * Shared page contracts (App owns the data store; pages own their forms)
 * ------------------------------------------------------------------ */

export type PageName = 'Instances' | 'Versions' | 'Resources' | 'Settings' | 'Logs' | 'ClientPackage';

export interface HubData {
  status: HubStatus | null;
  instances: InstanceInfo[];
  versions: VersionInfo[];
  resourceSets: ResourceSet[];
  gameInstall: GameInstall | null;
  settings: HubConfig | null;
  schema: ConfigSchema | null;
  logs: LogLine[];
}

export type Act = (label: string, fn: () => Promise<unknown>) => Promise<unknown>;

export interface PageProps {
  data: HubData;
  act: Act;
  busy: boolean;
  refresh: () => Promise<void>;
  onNavigate?: (page: PageName) => void;
  onOpenInstanceSettings?: (id: string) => void;
  initialInstanceId?: string | null;
}

/* ------------------------------------------------------------------ *
 * Small shared primitives
 * ------------------------------------------------------------------ */

export function PageHeader({ title, desc, actions }: {
  title: string; desc: string; actions?: ReactNode;
}) {
  return <header className="page-head">
    <div>
      <h1 className="page-title">{title}</h1>
      <p className="page-desc">{desc}</p>
    </div>
    {actions && <div className="page-actions">{actions}</div>}
  </header>;
}

function Section({ title, desc, action, children, className = '' }: {
  title: string; desc?: string; action?: ReactNode; children: ReactNode; className?: string;
}) {
  return <section className={`section ${className}`}>
    <div className="section-head">
      <div>
        <h2 className="section-title">{title}</h2>
        {desc && <p className="section-desc">{desc}</p>}
      </div>
      {action}
    </div>
    {children}
  </section>;
}

function MiniIcon({ name }: { name: 'server' | 'cube' | 'layers' | 'folder' | 'shield' | 'file' }) {
  const paths: Record<string, ReactNode> = {
    server: <><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01M7 16.5h.01" /></>,
    cube: <><path d="M21 8 12 3 3 8v8l9 5 9-5V8Z" /><path d="m3 8 9 5 9-5M12 13v8" /></>,
    layers: <><path d="m12 3 9 5-9 5-9-5 9-5ZM3 13l9 5 9-5M3 17l9 5 9-5" /></>,
    folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />,
    shield: <><path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Z" /><path d="m9 12 2 2 4-4" /></>,
    file: <><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Z" /><path d="M14 3v5h5" /></>,
  };
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name]}</svg>;
}

const fmtSize = (bytes: number) => {
  if (!bytes) return '0 B';
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`;
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${bytes} B`;
};

const STATE_TEXT: Record<string, string> = {
  Stopped: '已停止', Starting: '启动中', Running: '运行中',
  Unhealthy: '不健康', Updating: '更新中', Failed: '失败',
};
const stateTone = (s: string) => {
  if (s === 'Running') return 'green';
  if (s === 'Starting' || s === 'Updating') return 'amber';
  if (s === 'Failed' || s === 'Unhealthy') return 'red';
  return 'gray';
};

function Pill({ tone = 'gray', children }: { tone?: string; children: ReactNode }) {
  return <span className={`pill ${tone}`}>{children}</span>;
}

function KV({ k, v, mono = false, title }: { k: string; v: ReactNode; mono?: boolean; title?: string }) {
  return <div className="kv">
    <span className="kv-k">{k}</span>
    <span className={`kv-v${mono ? ' mono ellipsis' : ''}`} title={title}>{v}</span>
  </div>;
}

/* ------------------------------------------------------------------ *
 * 部署实例 (Instances)
 * ------------------------------------------------------------------ */

export function InstancesPage({ data, act, busy, onOpenInstanceSettings }: PageProps) {
  const [filter, setFilter] = useState<'all' | 'running' | 'stopped'>('all');
  const [query, setQuery] = useState('');
  const [showCreate, setShowCreate] = useState(false);
  const [form, setForm] = useState({ name: '', port: '', version: '', resourceSet: '' });
  const [revealed, setRevealed] = useState<string | null>(null);

  const items = data.instances.filter((i) => {
    if (filter === 'running' && i.state !== 'Running') return false;
    if (filter === 'stopped' && i.state === 'Running') return false;
    const q = query.trim().toLowerCase();
    return !q || `${i.name} ${i.id} ${i.version ?? ''}`.toLowerCase().includes(q);
  });
  const defaultVersion = data.versions.find((v) => v.is_current)?.name ?? '';
  const runningCount = data.instances.filter((i) => i.state === 'Running').length;

  const createInstance = () => {
    const name = form.name.trim();
    if (!name) return;
    void act('创建实例', async () => {
      const created = await api.instanceCreate(
        name,
        form.version || null,
        form.resourceSet || null,
        form.port.trim() ? Number(form.port) : null,
      );
      setShowCreate(false);
      setForm({ name: '', port: '', version: '', resourceSet: '' });
      return created;
    });
  };

  return <>
    <PageHeader
      title="部署实例"
      desc="管理本地服务端实例：启动 / 停止 / 重启、切换服务端版本与数据资源集。"
      actions={<button className="btn" disabled={busy} onClick={() => setShowCreate(true)}><span className="btn-plus">＋</span>新建实例</button>}
    />

    <div className="toolbar">
      <div className="chip-row">
        {([
          ['all', `全部 (${data.instances.length})`],
          ['running', `运行中 (${runningCount})`],
          ['stopped', `未运行 (${data.instances.length - runningCount})`],
        ] as const).map(([key, label]) => (
          <button key={key} className={`chip${filter === key ? ' active' : ''}`} onClick={() => setFilter(key)}>{label}</button>
        ))}
      </div>
      <div className="toolbar-spacer" />
      <label className="search-box">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
        <input placeholder="搜索实例名称 / ID / 版本" value={query} onChange={(e) => setQuery(e.target.value)} />
      </label>
    </div>

    {items.length === 0
      ? <div className="empty-state"><MiniIcon name="server" /><h3>暂无实例</h3><p>点击右上角「新建实例」创建一个本地服务端实例。</p></div>
      : <div className="instance-grid">
        {items.map((i) => <article className={`card instance-card ${i.state === 'Running' ? 'online' : ''}`} key={i.id}>
          <div className="card-top">
            <div className="card-icon"><MiniIcon name="server" /></div>
            <div className="card-title-wrap">
              <h3 className="card-title">{i.name}</h3>
              <span className="card-sub mono">{i.id}</span>
            </div>
            <Pill tone={stateTone(i.state)}>{STATE_TEXT[i.state] ?? i.state}</Pill>
          </div>

          <div className="kv-list">
            <KV k="端口" v={<span className="mono">{i.port}</span>} />
            <KV k="运行状态" v={<span className={`health ${i.health ? 'good' : 'bad'}`}><span className="health-dot" />{i.health ? '健康' : '未通过健康检查'}</span>} />
            <KV k="PID" v={<span className="mono">{i.pid ?? '—'}</span>} />
            <KV k="重启次数" v={<span className="mono">{i.restart_count}</span>} />
            <div className="kv kv-select">
              <span className="kv-k">服务端版本</span>
              <select className="inline-select" value={i.version ?? ''} disabled={busy}
                title="切换该实例使用的服务端版本"
                onChange={(e) => {
                  const value = e.target.value || defaultVersion;
                  if (!value) return;
                  void act('切换服务端版本', () => api.instanceSetVersion(i.id, value));
                }}>
                <option value="">跟随默认{defaultVersion ? ` (${defaultVersion})` : ''}</option>
                {data.versions.map((v) => <option key={v.name} value={v.name}>{v.name}{v.is_current ? ' · 默认' : ''}</option>)}
                {i.version && !data.versions.some((v) => v.name === i.version) && <option value={i.version}>{i.version} (未安装)</option>}
              </select>
            </div>
            <div className="kv kv-select">
              <span className="kv-k">数据资源集</span>
              <select className="inline-select" value={i.resource_set ?? ''} disabled={busy}
                title="切换该实例使用的数据资源集"
                onChange={(e) => void act('切换数据资源集', () => api.instanceSetResourceSet(i.id, e.target.value))}>
                <option value="">不绑定资源集</option>
                {data.resourceSets.map((r) => <option key={r.id} value={r.id}>{r.name}{r.is_default ? ' · 默认' : ''}</option>)}
                {i.resource_set && !data.resourceSets.some((r) => r.id === i.resource_set) && <option value={i.resource_set}>{i.resource_set} (缺失)</option>}
              </select>
            </div>
          </div>

          <div className="card-actions">
            {i.state === 'Running'
              ? <button className="btn sm" disabled={busy} onClick={() => void act('停止实例', () => api.instanceStop(i.id))}>停止</button>
              : <button className="btn sm" disabled={busy} onClick={() => void act('启动实例', () => api.instanceStart(i.id))}>启动</button>}
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('重启实例', () => api.instanceRestart(i.id))}>重启</button>
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('打开管理面板', () => api.instanceOpenAdmin(i.id))}>管理面板</button>
          </div>
          <div className="card-actions secondary">
            <button className="btn ghost sm" onClick={() => setRevealed(revealed === i.id ? null : i.id)}>数据目录</button>
            <button className="btn ghost sm" onClick={() => onOpenInstanceSettings?.(i.id)}>设置</button>
            <button className="btn danger-ghost sm" disabled={busy} onClick={() => {
              if (window.confirm(`确定删除实例「${i.name}」？此操作不可撤销。`)) {
                void act('删除实例', () => api.instanceDelete(i.id));
              }
            }}>删除</button>
          </div>
          {revealed === i.id && <div className="path-reveal">
            <code>{i.data_dir}</code>
            <button className="link-btn" disabled={busy} onClick={() => void act('在资源管理器中显示', () => api.instanceReveal(i.id))}>在资源管理器中显示 ↗</button>
          </div>}
        </article>)}
      </div>}

    {showCreate && <div className="modal-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) setShowCreate(false); }}>
      <div className="modal" role="dialog" aria-modal="true" aria-label="新建实例">
        <div className="modal-head">
          <div><h2>新建实例</h2><p className="muted">创建一个独立的本地服务端实例</p></div>
          <button className="icon-btn" aria-label="关闭" onClick={() => setShowCreate(false)}>×</button>
        </div>
        <div className="modal-body">
          <label className="field">实例名称
            <input autoFocus placeholder="例如：主线私服" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
          </label>
          <div className="form-grid">
            <label className="field">端口
              <input type="number" placeholder="留空自动分配" value={form.port} onChange={(e) => setForm({ ...form, port: e.target.value })} />
              <span className="hint">自动分配范围 {data.status?.config.port_range_start ?? 5180}–{data.status?.config.port_range_end ?? 5280}</span>
            </label>
            <label className="field">服务端版本
              <select value={form.version} onChange={(e) => setForm({ ...form, version: e.target.value })}>
                <option value="">默认（{defaultVersion || '当前版本'}）</option>
                {data.versions.map((v) => <option key={v.name} value={v.name}>{v.name}</option>)}
              </select>
            </label>
            <label className="field">数据资源集
              <select value={form.resourceSet} onChange={(e) => setForm({ ...form, resourceSet: e.target.value })}>
                <option value="">默认资源集</option>
                {data.resourceSets.map((r) => <option key={r.id} value={r.id}>{r.name}{r.is_default ? ' · 默认' : ''}</option>)}
              </select>
            </label>
          </div>
          <p className="hint">调用 instance_create(name, version, resource_set, port)；留空字段由 Hub 使用当前默认配置填充。</p>
        </div>
        <div className="modal-foot">
          <button className="btn ghost" onClick={() => setShowCreate(false)}>取消</button>
          <button className="btn" disabled={busy || !form.name.trim()} onClick={createInstance}>创建实例</button>
        </div>
      </div>
    </div>}
  </>;
}

/* ------------------------------------------------------------------ *
 * 服务端 (Server executables)
 * ------------------------------------------------------------------ */

export function VersionsPage({ data, act, busy }: PageProps) {
  const [url, setUrl] = useState('');
  const [version, setVersion] = useState('');
  const [sha, setSha] = useState('');
  const [repo, setRepo] = useState('');

  return <>
    <PageHeader
      title="服务端"
      desc="安装、切换与回滚服务端可执行文件（Editor  equivalents）。"
      actions={<Pill tone="indigo">已安装 {data.versions.length} 个版本</Pill>}
    />

    <div className="notice info note-banner">
      <span className="notice-icon">i</span>
      <span><b>关于「设为默认」</b>：默认版本只决定<b>之后新建实例</b>使用的服务端版本，不影响已有实例；已有实例可在「部署实例」卡片中单独切换版本。</span>
    </div>

    <Section title="安装新版本" desc="从 Release 下载并校验 SHA256，或从源码构建（按需安装 portable .NET SDK）。">
      <div className="form-grid">
        <label className="field">Release ZIP URL
          <input placeholder="留空 = 使用配置的 Release API 最新版本" value={url} onChange={(e) => setUrl(e.target.value)} />
        </label>
        <label className="field">版本号（可选）
          <input placeholder="例如 v2.5.0" value={version} onChange={(e) => setVersion(e.target.value)} />
        </label>
        <label className="field">SHA256（可选）
          <input placeholder="留空 = 使用配置的校验值" value={sha} onChange={(e) => setSha(e.target.value)} />
        </label>
        <label className="field">源码仓库 URL（源码构建）
          <input placeholder="留空 = 使用配置的 source_repo_url" value={repo} onChange={(e) => setRepo(e.target.value)} />
        </label>
      </div>
      <div className="row" style={{ marginTop: 14 }}>
        <button className="btn" disabled={busy} onClick={() => void act('安装预编译版本', () => api.versionInstall(url || null, version || null, sha || null))}>从 Release 安装</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('从源码构建', () => api.versionBuildFromSource(repo || null, version || null))}>从源码构建</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('回滚版本', () => api.versionRollback())}>回滚上一版本</button>
      </div>
    </Section>

    <Section title="已安装版本" desc="切换会更新默认版本指针；已有实例保持各自版本不变。">
      {data.versions.length === 0
        ? <div className="empty-state"><MiniIcon name="cube" /><h3>暂无已安装版本</h3><p>先在上方从 Release 安装一个预编译版本。</p></div>
        : <div className="version-grid">
          {data.versions.map((v) => <article className={`card version-card${v.is_current ? ' is-current' : ''}`} key={v.name}>
            <div className="card-top">
              <div className="card-icon"><MiniIcon name="cube" /></div>
              <div className="card-title-wrap">
                <h3 className="card-title">{v.name}</h3>
                <span className="card-sub mono ellipsis" title={v.path}>{v.path}</span>
              </div>
              {v.is_current && <Pill tone="indigo">默认</Pill>}
            </div>
            <div className="kv-list">
              <KV k="安装时间" v={v.installed_at} />
              <KV k="大小" v={fmtSize(v.size_bytes)} mono />
              <KV k="来源" v={<Pill tone={v.source === 'source' ? 'amber' : 'blue'}>{v.source}</Pill>} />
            </div>
            <div className="card-actions">
              <button className="btn sm" disabled={busy || v.is_current} onClick={() => void act('设为默认版本', () => api.versionSwitch(v.name))}>设为默认</button>
              <button className="btn ghost sm" disabled={busy} onClick={() => void act('打开版本目录', () => api.versionReveal(v.name))}>打开目录</button>
              <button className="btn ghost sm" onClick={() => void navigator.clipboard?.writeText(v.path)}>复制路径</button>
            </div>
          </article>)}
        </div>}
    </Section>
  </>;
}

/* ------------------------------------------------------------------ *
 * 数据资源 (Data resources)
 * ------------------------------------------------------------------ */

function DetectItem({ label, value }: { label: string; value: string | null }) {
  return <div className="detect-item">
    <span className="detect-k">{label}</span>
    <code className="detect-v" title={value ?? ''}>{value || '—'}</code>
  </div>;
}

export function ResourcesPage({ data, act, busy, refresh }: PageProps) {
  const [name, setName] = useState('');
  const [source, setSource] = useState<ResourceSource>('release');
  const [sourcePath, setSourcePath] = useState('');
  const [importName, setImportName] = useState('本机游戏资源');

  // resource_set_list() 在页面加载时调用（App.refresh 也会调用，这里确保直达页面时数据最新）。
  useEffect(() => { void api.resourceSetList().then(() => refresh()).catch(() => undefined); }, [refresh]);

  const g = data.gameInstall;
  const defaultCount = data.resourceSets.filter((r) => r.is_default).length;

  return <>
    <PageHeader
      title="数据资源"
      desc="管理数据资源集、检测本机游戏安装，并从游戏目录直接导入资源。"
      actions={<Pill tone="indigo">资源集 {data.resourceSets.length} · 默认 {defaultCount}</Pill>}
    />

    <section className={`card detect-card${g?.found ? ' found' : ''}`}>
      <div className="detect-head">
        <div>
          <span className="eyebrow">game_detect()</span>
          <h2>本机游戏检测</h2>
          <p className="section-desc">自动定位 Limbus Company 安装目录、Unity 缓存与 Addressables catalog。</p>
        </div>
        <Pill tone={g?.found ? 'green' : 'gray'}>{g?.found ? '已检测到游戏' : '未检测到游戏'}</Pill>
      </div>
      <div className="detect-grid">
        <DetectItem label="游戏目录" value={g?.game_dir ?? null} />
        <DetectItem label="数据目录" value={g?.data_dir ?? null} />
        <DetectItem label="Unity 缓存目录" value={g?.cache_dir ?? null} />
        <DetectItem label="语言目录" value={g?.lang_dir ?? null} />
        <DetectItem label="存档目录" value={g?.save_dir ?? null} />
        <DetectItem label="StreamingAssets" value={g?.streaming_dir ?? null} />
        <DetectItem label="Addressables catalog" value={g?.catalog ?? null} />
        <DetectItem label="settings.json" value={g?.settings ?? null} />
        <DetectItem label="游戏版本" value={g?.version ?? null} />
      </div>
      <div className="detect-foot">
        <input placeholder="导入后的资源集名称" value={importName} onChange={(e) => setImportName(e.target.value)} />
        <button className="btn" disabled={busy || !g?.found} onClick={() => void act('从本机游戏导入', () =>
          api.resourceSetCreate(importName.trim() || '本机游戏资源', 'game', null))}>从本机游戏导入</button>
        <span className="hint">调用 resource_set_create(name, 'game', null)，以检测到的安装目录建立资源集。</span>
      </div>
    </section>

    <Section title="新建资源集" desc="source 支持 release / official / lethe / game / local；选择 local 时需填写本地路径。">
      <div className="form-grid">
        <label className="field">名称
          <input placeholder="例如：官服静态数据 2026-11" value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label className="field">来源
          <select value={source} onChange={(e) => setSource(e.target.value as ResourceSource)}>
            <option value="release">release · 随版本发布</option>
            <option value="official">official · 官服 CDN</option>
            <option value="lethe">lethe · 私服 CDN</option>
            <option value="game">game · 本机游戏</option>
            <option value="local">local · 本地目录</option>
          </select>
        </label>
        <label className="field">本地路径（source = local）
          <input disabled={source !== 'local'} placeholder="选择本地目录后填写完整路径" value={sourcePath} onChange={(e) => setSourcePath(e.target.value)} />
        </label>
      </div>
      <div className="row" style={{ marginTop: 14 }}>
        <button className="btn" disabled={busy || !name.trim() || (source === 'local' && !sourcePath.trim())}
          onClick={() => void act('创建资源集', () =>
            api.resourceSetCreate(name.trim(), source, source === 'local' ? sourcePath.trim() : null))}>创建资源集</button>
        <span className="hint">resource_set_create(name, source, source_path?)</span>
      </div>
    </Section>

    <Section title={`资源集 (${data.resourceSets.length})`} desc="设为默认仅影响之后新建的实例；已有实例可在实例卡片中单独切换资源集。">
      {data.resourceSets.length === 0
        ? <div className="empty-state"><MiniIcon name="layers" /><h3>暂无资源集</h3><p>使用上方表单创建，或从本机游戏导入。</p></div>
        : <div className="resource-grid">
          {data.resourceSets.map((r) => <article className={`card resource-card${r.is_default ? ' is-default' : ''}`} key={r.id}>
            <div className="card-top">
              <div className="card-icon"><MiniIcon name="layers" /></div>
              <div className="card-title-wrap">
                <h3 className="card-title">{r.name}</h3>
                <span className="card-sub mono">{r.id}</span>
              </div>
              {r.is_default && <Pill tone="indigo">默认</Pill>}
            </div>
            <div className="badge-row">
              <Pill tone="gray">{r.kind}</Pill>
              <Pill tone="blue">{r.source}</Pill>
              <Pill tone="gray">{r.version || '—'}</Pill>
            </div>
            <div className="kv-list">
              <KV k="文件数" v={<span className="mono">{r.file_count.toLocaleString()}</span>} />
              <KV k="大小" v={<span className="mono">{fmtSize(r.size_bytes)}</span>} />
              <KV k="创建时间" v={r.created_at} />
              <KV k="静态目录" v={r.static_dir || '—'} mono title={r.static_dir} />
              <KV k="本地化目录" v={r.localize_dir || '—'} mono title={r.localize_dir} />
              <KV k="Mod 目录" v={r.modfiles_dir || '—'} mono title={r.modfiles_dir} />
            </div>
            {Object.keys(r.extra).length > 0 && <div className="extra-row">
              {Object.entries(r.extra).map(([k, v]) => <span className="extra-chip" key={k} title={`${k}: ${v}`}><b>{k}</b> {v}</span>)}
            </div>}
            <div className="card-actions">
              <button className="btn sm" disabled={busy || r.is_default} onClick={() => void act('设为默认资源集', () => api.resourceSetDefault(r.id))}>设为默认</button>
              <button className="btn ghost sm" disabled={busy} onClick={() => void act('打开资源集目录', () => api.resourceSetReveal(r.id))}>打开目录</button>
              <button className="btn danger-ghost sm" disabled={busy} onClick={() => {
                if (window.confirm(`确定删除资源集「${r.name}」？绑定该资源集的实例将自动解绑。`)) {
                  void act('删除资源集', () => api.resourceSetDelete(r.id));
                }
              }}>删除</button>
            </div>
          </article>)}
        </div>}
    </Section>
  </>;
}

/* ------------------------------------------------------------------ *
 * 设置 (Settings)
 * ------------------------------------------------------------------ */

const HUB_SETTING_KEYS = new Set([
  'github_proxy', 'static_source', 'port_range_start', 'port_range_end',
  'data_dir', 'lethe_dir', 'catalog_path', 'settings_path', 'game_dir',
  'keep_versions', 'health_timeout_ms', 'backoff_base_ms', 'proxy',
]);

export function SettingsPage({ data, act, busy, initialInstanceId }: PageProps) {
  const [draft, setDraft] = useState<HubConfig | null>(data.settings);
  const [selected, setSelected] = useState(initialInstanceId ?? '');
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [dirty, setDirty] = useState(false);

  useEffect(() => { void api.hubSettingsGet().then(setDraft).catch(() => undefined); }, []);
  useEffect(() => { if (data.settings) setDraft(data.settings); }, [data.settings]);
  useEffect(() => {
    const list = data.instances;
    const preferred = initialInstanceId && list.some((i) => i.id === initialInstanceId) ? initialInstanceId : list[0]?.id ?? '';
    setSelected((old) => (old && list.some((i) => i.id === old) ? old : preferred));
  }, [data.instances, initialInstanceId]);
  useEffect(() => {
    if (!selected) return;
    void api.hubConfigGet(selected).then((v) => { setValues(v); setDirty(false); }).catch(() => setValues({}));
  }, [selected]);

  const fields: ConfigField[] = (data.schema?.fields ?? []).filter((f) => !HUB_SETTING_KEYS.has(f.key));
  const change = (f: ConfigField, v: string) => {
    setValues((prev) => ({
      ...prev,
      [f.key]: f.kind === 'number' ? Number(v) : f.kind === 'boolean' ? v === 'true' : v,
    }));
    setDirty(true);
  };
  const setDraftField = (key: keyof HubConfig, value: string) => {
    setDraft((old) => old ? { ...old, [key]: value } : old);
  };
  const proxyMode = draft?.github_proxy || 'auto';
  const staticSource = draft?.static_source || 'release';

  return <>
    <PageHeader
      title="设置"
      desc="Hub 全局配置与实例级配置。全局配置由 hub_config_update 原子写入。"
      actions={<Pill tone={dirty ? 'amber' : 'green'}>{dirty ? '实例配置未保存' : '实例配置已同步'}</Pill>}
    />

    <Section title="下载与代理" desc="Release 下载、资源同步使用的代理源与默认静态数据来源。">
      <div className="form-grid">
        <label className="field">GitHub 代理
          <select value={proxyMode} onChange={(e) => setDraftField('github_proxy', e.target.value)}>
            <option value="auto">自适应（推荐）</option>
            <option value="none">不走代理</option>
            <option value="gh-proxy.org">gh-proxy.org</option>
            <option value="gh-proxy.com">gh-proxy.com</option>
          </select>
          <span className="hint">API 优先直连，Release 资源优先代理，失败自动切换，最后回退直连。</span>
        </label>
        <label className="field">默认静态数据来源
          <select value={staticSource} onChange={(e) => setDraftField('static_source', e.target.value)}>
            <option value="release">release 预置快照</option>
            <option value="official">官服 CDN</option>
            <option value="lethe">私服 CDN（Lethe）</option>
          </select>
          <span className="hint">仅影响之后新建的实例。</span>
        </label>
        <label className="field">Release API
          <input value={draft?.release_api ?? ''} onChange={(e) => setDraftField('release_api', e.target.value)} />
        </label>
        <label className="field">源码仓库 URL
          <input value={draft?.source_repo_url ?? ''} onChange={(e) => setDraftField('source_repo_url', e.target.value)} />
        </label>
      </div>
    </Section>

    <Section title="端口与保留策略" desc="新建实例的自动端口范围与版本清理策略。">
      <div className="form-grid">
        <label className="field">端口范围起点
          <input type="number" value={draft?.port_range_start ?? 5180} onChange={(e) => setDraftField('port_range_start', e.target.value)} />
        </label>
        <label className="field">端口范围终点
          <input type="number" value={draft?.port_range_end ?? 5280} onChange={(e) => setDraftField('port_range_end', e.target.value)} />
        </label>
        <label className="field">保留版本数
          <input type="number" value={draft?.keep_versions ?? 3} onChange={(e) => setDraftField('keep_versions', e.target.value)} />
        </label>
        <label className="field">健康检查超时 (ms)
          <input type="number" value={draft?.health_timeout_ms ?? 30000} onChange={(e) => setDraftField('health_timeout_ms', e.target.value)} />
        </label>
      </div>
      <div className="row" style={{ marginTop: 16 }}>
        <button className="btn" disabled={busy || !draft} onClick={() => void act('保存 Hub 设置', async () => {
          const saved = await api.hubConfigUpdate({
            github_proxy: draft!.github_proxy,
            static_source: draft!.static_source,
            release_api: draft!.release_api,
            source_repo_url: draft!.source_repo_url,
            port_range_start: Number(draft!.port_range_start),
            port_range_end: Number(draft!.port_range_end),
            keep_versions: Number(draft!.keep_versions),
            health_timeout_ms: Number(draft!.health_timeout_ms),
          });
          setDraft(saved);
          return saved;
        })}>保存 Hub 设置</button>
        <span className="hint">hub_settings_get / hub_config_update</span>
      </div>
    </Section>

    <Section title="目录与路径" desc="留空表示使用 Hub 默认位置；点击「打开」在资源管理器中显示 Hub 目录。">
      <div className="form-grid">
        {([
          ['data_dir', '数据根目录'],
          ['game_dir', '游戏本体目录'],
          ['catalog_path', 'Addressables catalog'],
          ['settings_path', 'Unity settings.json'],
          ['lethe_dir', '私服资源目录'],
        ] as Array<[keyof HubConfig, string]>).map(([key, label]) => <label className="field" key={key}>
          {label}
          <div className="path-input">
            <input value={draft?.[key] ?? ''} placeholder="使用默认位置" onChange={(e) => setDraftField(key, e.target.value)} />
            <button className="icon-btn" title="打开 Hub 目录" disabled={busy} onClick={() => void act('打开 Hub 目录', () => api.hubRevealDir())}>↗</button>
          </div>
        </label>)}
        <label className="field">Hub 目录（只读）
          <input value={data.status?.hub_dir ?? '—'} readOnly />
        </label>
      </div>
      <div className="row" style={{ marginTop: 16 }}>
        <button className="btn" disabled={busy || !draft} onClick={() => void act('保存路径配置', async () => {
          const saved = await api.hubConfigUpdate({
            data_dir: draft!.data_dir || null,
            game_dir: draft!.game_dir || null,
            catalog_path: draft!.catalog_path || null,
            settings_path: draft!.settings_path || null,
            lethe_dir: draft!.lethe_dir || null,
          });
          setDraft(saved);
          return saved;
        })}>保存路径配置</button>
      </div>
    </Section>

    <Section title="实例配置" desc={`配置来源：${data.schema?.source ?? '—'} · 原子写入 server.bootstrap.json`}
      action={dirty ? <Pill tone="amber">未保存</Pill> : <Pill tone="green">已同步</Pill>}>
      <div className="row">
        <select value={selected} onChange={(e) => setSelected(e.target.value)}>
          <option value="">选择实例…</option>
          {data.instances.map((i) => <option key={i.id} value={i.id}>{i.name} ({i.id})</option>)}
        </select>
        <button className="btn ghost sm" disabled={!selected} onClick={() => void api.hubConfigGet(selected).then((v) => { setValues(v); setDirty(false); })}>重新载入</button>
      </div>
      {fields.length > 0 ? <div className="form-grid" style={{ marginTop: 14 }}>
        {fields.map((f) => <label className="field" key={f.key}>
          {f.label}{f.requires_restart && <span className="muted"> · 需重启</span>}
          {f.kind === 'boolean'
            ? <select value={String(values[f.key] ?? f.default)} onChange={(e) => change(f, e.target.value)}><option value="true">true</option><option value="false">false</option></select>
            : <input type={f.kind === 'secret' ? 'password' : f.kind === 'number' ? 'number' : 'text'}
                value={String(values[f.key] ?? f.default ?? '')} onChange={(e) => change(f, e.target.value)} />}
          <span className="hint">{f.description}</span>
        </label>)}
      </div> : <div className="empty-state compact"><MiniIcon name="shield" /><p>当前 schema 没有实例级字段。</p></div>}
      <div className="row" style={{ marginTop: 16 }}>
        <button className="btn" disabled={busy || !selected || !dirty} onClick={() => void act('保存实例配置', async () => {
          const saved = await api.hubConfigSet(selected, values);
          setDirty(false);
          return saved;
        })}>保存实例配置</button>
        <span className="hint">密钥脱敏存入 bootstrap；标记需重启的字段在实例重启后生效。</span>
      </div>
    </Section>
  </>;
}

/* ------------------------------------------------------------------ *
 * 日志 (Logs) — log_tail + filters
 * ------------------------------------------------------------------ */

export function LogsPage({ data, refresh }: PageProps) {
  const [level, setLevel] = useState('all');
  const [query, setQuery] = useState('');
  const [follow, setFollow] = useState(true);
  const filtered = useMemo(() => data.logs.filter((l) =>
    (level === 'all' || l.level === level) &&
    (!query || `${l.target} ${l.message}`.toLowerCase().includes(query.toLowerCase())),
  ), [data.logs, level, query]);

  const copy = () => void navigator.clipboard?.writeText(
    filtered.map((l) => `[${l.ts}] ${l.level} [${l.target}] ${l.message}`).join('\n'),
  );

  return <>
    <PageHeader
      title="日志"
      desc="实时查看 Hub 与实例的日志流，可按级别与关键字筛选。"
      actions={<Pill tone="gray">{filtered.length} / {data.logs.length} 条</Pill>}
    />
    <Section title="实时日志" desc="水平滚动查找；开启「跟随」后自动滚动到最新。"
      action={<div className="row tight">
        <select value={level} onChange={(e) => setLevel(e.target.value)}>
          <option value="all">全部级别</option><option value="info">info</option>
          <option value="warn">warn</option><option value="error">error</option>
        </select>
        <label className="search-box compact">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
          <input placeholder="筛选…" value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        <label className="checkline"><input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />跟随</label>
        <button className="btn ghost sm" onClick={() => void api.logTail(500).then(() => refresh()).catch(() => undefined)}>重新载入</button>
        <button className="btn ghost sm" onClick={copy}>复制</button>
      </div>}>
      <div className="logbox" ref={(el) => { if (el && follow) el.scrollTop = el.scrollHeight; }}>
        {filtered.map((l, i) => <div className="log-line" key={i}>
          <span className="log-ts">{l.ts}</span>
          <span className={`log-level ${l.level}`}>{l.level.toUpperCase()}</span>
          <span className="log-target">[{l.target}]</span>
          <span className="log-message">{l.message}</span>
        </div>)}
        {filtered.length === 0 && <div className="log-empty">暂无匹配日志</div>}
      </div>
    </Section>
  </>;
}

/* ------------------------------------------------------------------ *
 * 客户端包 (Client package export — kept from the original UI)
 * ------------------------------------------------------------------ */

export function ClientPackagePage({ data, act, busy }: PageProps) {
  const [id, setId] = useState('');
  const [output, setOutput] = useState('');
  useEffect(() => {
    setId((old) => old || data.instances[0]?.id || '');
  }, [data.instances]);

  return <>
    <PageHeader title="客户端包" desc="导出可发布的轻量客户端包：仅包含 launcher、BepInEx 基线、配置与 modfiles。" />
    <Section title="导出客户端发布包" desc="绝不打包游戏本体 / bundle / ZylarNex；导出后计算 SHA256。">
      <div className="form-grid">
        <label className="field">目标实例
          <select value={id} onChange={(e) => setId(e.target.value)}>
            <option value="">选择实例…</option>
            {data.instances.map((i) => <option key={i.id} value={i.id}>{i.name}</option>)}
          </select>
        </label>
        <label className="field">输出路径（可选）
          <input value={output} onChange={(e) => setOutput(e.target.value)} placeholder="hub/clientpkg/PortableLetheClient.zip" />
        </label>
      </div>
      <div className="row" style={{ marginTop: 14 }}>
        <button className="btn" disabled={busy || !id} onClick={() => void act('导出客户端包', () => api.clientpkgExport(id, output || null))}>导出并计算 SHA256</button>
        <span className="hint">clientpkg_export(instanceId, output?)</span>
      </div>
    </Section>
  </>;
}
