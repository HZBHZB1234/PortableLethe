import { useEffect, useMemo, useState } from 'react';
import { api } from './api';
import type {
  ConfigField, ConfigSchema, HubConfig, HubStatus, InstanceInfo, LogLine, VersionInfo,
} from './types';

type Act = (label: string, fn: () => Promise<unknown>) => Promise<unknown>;
type Page = 'Dashboard' | 'Versions' | 'Instances' | 'Config' | 'Resources' | 'ClientPackage' | 'Logs';
interface Props {
  act: Act;
  busy: boolean;
  status?: HubStatus | null;
  refresh?: () => Promise<void>;
  onNavigate?: (page: Page) => void;
  onCheckDeps?: () => Promise<void>;
}

function Section({ title, desc, action, children }: {
  title: string; desc?: string; action?: React.ReactNode; children: React.ReactNode;
}) {
  return <section className="card">
    <div className="card-head"><div><h2>{title}</h2>{desc && <p>{desc}</p>}</div>{action}</div>
    {children}
  </section>;
}

function Kpi({ label, value, foot, small }: { label: string; value: string | number; foot?: string; small?: boolean }) {
  return <section className="card kpi">
    <span className="kpi-label">{label}</span>
    <span className={small ? 'kpi-value small' : 'kpi-value'}>{value}</span>
    {foot && <span className="kpi-foot">{foot}</span>}
  </section>;
}

export function DashboardPage({ status, act, busy, onNavigate, onCheckDeps }: Props) {
  const cats = status?.resources?.categories ?? {};
  const versions = status?.version_count ?? 0;
  const instances = status?.instance_count ?? 0;
  const running = status?.running_count ?? 0;
  const categories = Object.entries(cats);
  return <div className="stack">
    <div className="grid cols-3">
      <Kpi label="运行中实例" value={running} foot={`共 ${instances} 个实例`} />
      <Kpi label="已安装版本" value={versions} foot="支持一键切换 / 回滚" />
      <Kpi label="静态数据" value={cats.staticdata?.version ?? '未同步'} foot={cats.staticdata?.updated_at ?? '—'} small />
    </div>

    <Section title="快速开始" desc="常用操作集中在此，安装与同步均校验哈希并原子切换">
      <div className="row">
        <button className="btn" disabled={busy} onClick={() => void act('安装预编译服务端', () => api.versionInstall())}>安装最新版本</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('同步全部资源', () => api.resourceSync('all'))}>同步全部资源</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('创建默认实例', () => api.instanceCreate('default'))}>创建默认实例</button>
        <button className="btn ghost" onClick={() => onNavigate?.('Instances')}>管理实例 →</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('打开 Hub 目录', () => api.hubRevealDir())}>打开 Hub 目录</button>
        <button className="btn ghost" disabled={busy} onClick={() => void onCheckDeps?.()}>检查并修复依赖</button>
      </div>
      <div className="divider" style={{ margin: '16px 0' }} />
      <p className="subtle">Hub 内部为纯 Rust；仅在安装 / 编译阶段调用外部工具链。下载自动走 gh-proxy 前缀与 SHA256 校验。</p>
    </Section>

    <Section title="资源概览" desc="最后同步时间与版本来自官方站点响应头">
      {categories.length === 0 ? <div className="empty">尚未同步任何资源</div> :
        <div className="table-wrap"><table><thead><tr><th>类别</th><th>版本</th><th>文件数</th><th>更新时间</th><th>状态</th></tr></thead><tbody>
          {categories.map(([name, st]) => <tr key={name}>
            <td><b>{name}</b></td><td>{st.version ?? '—'}</td>
            <td className="num">{Object.keys(st.files ?? {}).length}</td>
            <td className="muted">{st.updated_at ?? '—'}</td>
            <td>{st.last_error ? <span className="tag error">异常</span> : <span className="tag ok">就绪</span>}</td>
          </tr>)}
        </tbody></table></div>}
    </Section>
  </div>;
}

export function VersionsPage({ act, busy, refresh }: Props) {
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [url, setUrl] = useState('');
  const [sha, setSha] = useState('');
  const load = () => api.versionList().then(setVersions).catch(() => setVersions([]));
  useEffect(() => { void load(); }, []);
  return <div className="stack">
    <Section title="安装预编译版本" desc="留空则读取配置的 Release API，自动选择 win-x64 资产">
      <div className="form-grid">
        <label className="field">zip URL<input placeholder="留空 = Release 最新" value={url} onChange={(e) => setUrl(e.target.value)} /></label>
        <label className="field">SHA256（可选）<input placeholder="留空 = 使用配置值" value={sha} onChange={(e) => setSha(e.target.value)} /></label>
      </div>
      <div className="row" style={{ marginTop: 14 }}>
        <button className="btn" disabled={busy} onClick={() => void act('安装版本', async () => {
          const r = await api.versionInstall(url || null, null, sha || null); await load(); await refresh?.(); return r;
        })}>下载 / 校验 / 安装</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('从源码构建', async () => {
          const r = await api.versionBuildFromSource(null, null); await load(); await refresh?.(); return r;
        })}>从源码构建</button>
        <span className="muted">源码构建会按需安装 portable .NET SDK，并叠加已同步的静态数据。</span>
      </div>
    </Section>

    <Section title="已安装版本" desc="切换会停止实例并重新指向所有 server junction">
      <div className="table-wrap"><table><thead><tr><th>版本</th><th>安装时间</th><th>大小</th><th>来源</th><th style={{ textAlign: 'right' }}>操作</th></tr></thead><tbody>
        {versions.map((v) => <tr key={v.name}>
          <td><b>{v.name}</b> {v.is_current && <span className="tag Running" style={{ marginLeft: 6 }}>current</span>}</td>
          <td className="muted">{v.installed_at}</td>
          <td className="num">{(v.size_bytes / 1048576).toFixed(1)} MB</td>
          <td className="muted">{v.source}</td>
          <td style={{ textAlign: 'right' }}><button className="btn ghost sm" disabled={busy || v.is_current} onClick={() => void act('切换版本', async () => {
            const r = await api.versionSwitch(v.name); await load(); await refresh?.(); return r;
          })}>切换</button></td>
        </tr>)}
        {!versions.length && <tr><td colSpan={5}><div className="empty">暂无版本，先安装一个预编译包</div></td></tr>}
      </tbody></table></div>
      <div className="row" style={{ marginTop: 12 }}>
        <button className="btn ghost" disabled={busy} onClick={() => void act('回滚', async () => {
          const r = await api.versionRollback(); await load(); return r;
        })}>一键回滚</button>
      </div>
    </Section>
  </div>;
}

export function InstancesPage({ act, busy }: Props) {
  const [items, setItems] = useState<InstanceInfo[]>([]);
  const [name, setName] = useState('default');
  const load = () => api.instanceList().then(setItems).catch(() => setItems([]));
  useEffect(() => { void load(); }, []);
  return <div className="stack">
    <Section title="新建实例" desc="端口在配置范围内自动分配，版本默认跟随当前">
      <div className="row">
        <input placeholder="实例名" value={name} onChange={(e) => setName(e.target.value)} />
        <button className="btn" disabled={busy || !name.trim()} onClick={() => void act('创建实例', async () => {
          const r = await api.instanceCreate(name); setName(''); await load(); return r;
        })}>创建实例</button>
      </div>
    </Section>
    <Section title="实例列表" desc="手动停止不会触发自动重启；异常退出会按退避策略拉起">
      <div className="table-wrap"><table><thead><tr><th>实例</th><th>版本</th><th>端口</th><th>状态</th><th>重启次数</th><th style={{ textAlign: 'right' }}>操作</th></tr></thead><tbody>
        {items.map((i) => <tr key={i.id}>
          <td><b>{i.name}</b><div className="muted num">{i.id}</div></td>
          <td>{i.version ?? '—'}</td><td className="num">{i.port}</td>
          <td><span className={`tag ${i.state}`}>{i.state}</span>{i.health ? <span className="tag ok" style={{ marginLeft: 4 }}>health</span> : null}</td>
          <td className="num">{i.restart_count}</td>
          <td><div className="row tight" style={{ justifyContent: 'flex-end' }}>
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('打开管理面板', () => api.instanceOpenAdmin(i.id))}>面板</button>
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('启动实例', async () => { const r = await api.instanceStart(i.id); await load(); return r; })}>启动</button>
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('停止实例', async () => { const r = await api.instanceStop(i.id); await load(); return r; })}>停止</button>
            <button className="btn ghost sm" disabled={busy} onClick={() => void act('重启实例', async () => { const r = await api.instanceRestart(i.id); await load(); return r; })}>重启</button>
            <button className="btn danger sm" disabled={busy} onClick={() => void act('删除实例', async () => { const r = await api.instanceDelete(i.id); await load(); return r; })}>删除</button>
          </div></td>
        </tr>)}
        {!items.length && <tr><td colSpan={6}><div className="empty">暂无实例</div></td></tr>}
      </tbody></table></div>
    </Section>
  </div>;
}

export function ConfigPage({ act, busy }: Props) {
  const [hubSettings, setHubSettings] = useState<HubConfig | null>(null);
  const [schema, setSchema] = useState<ConfigSchema | null>(null);
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [selected, setSelected] = useState('');
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [dirty, setDirty] = useState(false);
  useEffect(() => {
    void api.hubSettingsGet().then(setHubSettings).catch(() => undefined);
    void Promise.all([api.hubConfigSchema(), api.instanceList()]).then(([s, is]) => {
      setSchema(s); setInstances(is); setSelected((old) => old || is[0]?.id || '');
    }).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!selected) return;
    void api.hubConfigGet(selected).then((v) => { setValues(v); setDirty(false); }).catch(() => setValues({}));
  }, [selected]);
  const fields = (schema?.fields ?? []).filter((f) => f.owner === 'hub');
  const change = (f: ConfigField, v: string) => {
    setValues((prev) => ({ ...prev, [f.key]: f.kind === 'number' ? Number(v) : f.kind === 'boolean' ? v === 'true' : v }));
    setDirty(true);
  };
  const proxyMode = hubSettings?.github_proxy || 'auto';
  return <div className="stack">
    <Section title="GitHub 代理" desc="下载 Release / 同步资源时使用的代理源；修改后立即生效">
      <div className="form-grid">
        <label className="field">代理源
          <select value={proxyMode} disabled={busy} onChange={(e) => void act('保存 Hub 设置', async () => {
            const r = await api.hubConfigUpdate({ github_proxy: e.target.value });
            setHubSettings(r);
            return r;
          })}>
            <option value="auto">自适应（推荐）</option>
            <option value="none">不走代理</option>
            <option value="gh-proxy.org">gh-proxy.org</option>
            <option value="gh-proxy.com">gh-proxy.com</option>
          </select>
          <span className="hint">自适应：API 请求优先直连，Release 资源优先 gh-proxy.org，失败自动切换 gh-proxy.com，最后回退直连。</span>
        </label>
        <label className="field">Release API
          <input value={hubSettings?.release_api ?? ''} readOnly />
          <span className="hint">此地址随 Hub 分发，如需更换请编辑 hub-config.json。</span>
        </label>
      </div>
    </Section>
    <Section title="目标实例" desc={`配置来源: ${schema?.source ?? '—'}`}>
      <div className="row">
        <select value={selected} onChange={(e) => setSelected(e.target.value)}>
          {instances.map((i) => <option key={i.id} value={i.id}>{i.name} ({i.id})</option>)}
        </select>
        <button className="btn ghost sm" disabled={!selected} onClick={() => void api.hubConfigGet(selected).then((v) => { setValues(v); setDirty(false); })}>重新载入</button>
      </div>
    </Section>
    <Section title="Hub 拥有的配置" desc="原子写入 server.bootstrap.json；标记需重启的字段在实例重启后生效"
      action={dirty ? <span className="tag warn">未保存</span> : <span className="tag ok">已同步</span>}>
      <div className="form-grid">
        {fields.map((f) => <label className="field" key={f.key}>
          {f.label}{f.requires_restart && <span className="muted"> · 需重启</span>}
          {f.kind === 'boolean'
            ? <select value={String(values[f.key] ?? f.default)} onChange={(e) => change(f, e.target.value)}><option value="true">true</option><option value="false">false</option></select>
            : <input type={f.kind === 'secret' ? 'password' : f.kind === 'number' ? 'number' : 'text'} value={String(values[f.key] ?? f.default ?? '')} onChange={(e) => change(f, e.target.value)} />}
          <span className="hint">{f.description}</span>
        </label>)}
      </div>
      <div className="row" style={{ marginTop: 16 }}>
        <button className="btn" disabled={busy || !selected || !dirty} onClick={() => void act('保存配置', async () => {
          const r = await api.hubConfigSet(selected, values); setDirty(false); return r;
        })}>保存配置</button>
        <span className="muted">服务端面板拥有的字段不在此处显示；密钥脱敏存入 bootstrap。</span>
      </div>
    </Section>
  </div>;
}

export function ResourcesPage({ status, act, busy }: Props) {
  const [source, setSource] = useState('');
  const categories = ['modfiles', 'staticdata', 'bundles'];
  return <div className="stack">
    <Section title="同步源" desc="留空则使用 Hub 配置中的 catalog 路径与官方源">
      <input style={{ width: 'min(100%, 560px)' }} placeholder="本地 catalog.bin / bundle 路径（可选）" value={source} onChange={(e) => setSource(e.target.value)} />
    </Section>
    <div className="grid cols-3">
      {categories.map((c) => {
        const st = status?.resources?.categories?.[c];
        return <section className="card" key={c}>
          <div className="card-head"><div><h2>{c}</h2><p>{st?.version ?? '未同步'}</p></div>
            {st?.last_error ? <span className="tag error">异常</span> : <span className="tag ok">就绪</span>}
          </div>
          <p className="muted">文件数: {st ? Object.keys(st.files ?? {}).length : 0}<br />更新: {st?.updated_at ?? '—'}</p>
          {st?.last_error && <p className="notice error" style={{ marginTop: 8 }}>{st.last_error}</p>}
          <button className="btn ghost sm" style={{ marginTop: 12 }} disabled={busy} onClick={() => void act(`同步 ${c}`, () => api.resourceSync(c, source || null))}>立即同步</button>
        </section>;
      })}
    </div>
    <Section title="全部同步" desc="modfiles 立即生效；staticdata / bundles 更新后需重启实例">
      <button className="btn" disabled={busy} onClick={() => void act('同步全部资源', () => api.resourceSync('all', source || null))}>同步全部（原子切换）</button>
    </Section>
  </div>;
}

export function ClientPackagePage({ act, busy }: Props) {
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [id, setId] = useState('');
  const [output, setOutput] = useState('');
  useEffect(() => { void api.instanceList().then((is) => { setInstances(is); setId((x) => x || is[0]?.id || ''); }).catch(() => undefined); }, []);
  return <Section title="导出客户端发布包" desc="仅包含预编译 launcher、BepInEx 基线、配置与 modfiles">
    <div className="form-grid">
      <label className="field">目标实例
        <select value={id} onChange={(e) => setId(e.target.value)}>{instances.map((i) => <option key={i.id} value={i.id}>{i.name}</option>)}</select>
      </label>
      <label className="field">输出路径（可选）
        <input value={output} onChange={(e) => setOutput(e.target.value)} placeholder="hub/clientpkg/PortableLetheClient.zip" />
      </label>
    </div>
    <div className="row" style={{ marginTop: 14 }}>
      <button className="btn" disabled={busy || !id} onClick={() => void act('导出客户端包', () => api.clientpkgExport(id, output || null))}>导出并计算 SHA256</button>
      <span className="muted">绝不打包游戏本体 / bundle / ZylarNex。</span>
    </div>
  </Section>;
}

export function LogsPage({ logs }: { logs: LogLine[] }) {
  const [level, setLevel] = useState('all');
  const [query, setQuery] = useState('');
  const [follow, setFollow] = useState(true);
  const filtered = useMemo(() => logs.filter((l) =>
    (level === 'all' || l.level === level) &&
    (!query || `${l.target} ${l.message}`.toLowerCase().includes(query.toLowerCase()))
  ), [logs, level, query]);
  return <Section title="实时日志" desc={`${filtered.length} / ${logs.length} 条`}
    action={<div className="row tight">
      <select value={level} onChange={(e) => setLevel(e.target.value)}>
        <option value="all">全部级别</option><option value="info">info</option><option value="warn">warn</option><option value="error">error</option>
      </select>
      <input style={{ width: 160 }} placeholder="筛选…" value={query} onChange={(e) => setQuery(e.target.value)} />
      <label className="checkline"><input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />跟随</label>
      <button className="btn ghost sm" onClick={() => void navigator.clipboard?.writeText(filtered.map((l) => `[${l.ts}] ${l.level} [${l.target}] ${l.message}`).join('\n'))}>复制</button>
    </div>}>
    <div className="logbox" ref={(el) => { if (el && follow) el.scrollTop = el.scrollHeight; }}>
      {filtered.map((l, i) => <div key={i}>
        <span className="ts">{l.ts}</span> <span className={l.level}>{l.level.toUpperCase()}</span> <span className="target">[{l.target}]</span> {l.message}
      </div>)}
      {!filtered.length && <span style={{ color: '#64748b' }}>暂无匹配日志</span>}
    </div>
  </Section>;
}
