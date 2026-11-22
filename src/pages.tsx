import { useEffect, useState } from 'react';
import { api } from './api';
import type {
  ConfigField, ConfigSchema, HubStatus, InstanceInfo, LogLine,
  ResourceStatus, VersionInfo,
} from './types';

type Act = (label: string, fn: () => Promise<unknown>) => Promise<void>;
interface Props { act: Act; busy: boolean; status?: HubStatus | null; refresh?: () => Promise<void> }

export function DashboardPage({ status, act, busy }: Props) {
  const cats = status?.resources?.categories ?? {};
  return <div className="stack">
    <div className="grid cols-3">
      <div className="card"><div className="muted">运行中实例</div><div className="stat">{status?.running_count ?? 0}</div></div>
      <div className="card"><div className="muted">已安装版本</div><div className="stat">{status?.version_count ?? 0}</div></div>
      <div className="card"><div className="muted">静态数据版本</div><div className="stat" style={{ fontSize: 18 }}>{cats.staticdata?.version ?? '未同步'}</div></div>
    </div>
    <div className="card">
      <h2>快速操作</h2>
      <div className="row">
        <button className="btn" disabled={busy} onClick={() => void act('下载并安装预编译服务端', () => api.versionInstall())}>安装最新版本</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('同步全部资源', () => api.resourceSync('all'))}>同步全部资源</button>
        <button className="btn ghost" disabled={busy} onClick={() => void act('创建实例', () => api.instanceCreate('default'))}>创建默认实例</button>
      </div>
      <p className="muted">Hub 内部为纯 Rust；仅在安装/编译阶段调用外部工具链。下载自动走 gh-proxy 前缀与 SHA256 校验。</p>
    </div>
    <div className="card"><h2>资源状态</h2><pre className="muted" style={{ whiteSpace: 'pre-wrap' }}>{JSON.stringify(cats, null, 2)}</pre></div>
  </div>;
}

export function VersionsPage({ act, busy, refresh }: Props) {
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [url, setUrl] = useState('');
  const [sha, setSha] = useState('');
  const load = () => api.versionList().then(setVersions).catch(() => setVersions([]));
  useEffect(() => { void load(); }, []);
  return <div className="stack">
    <div className="card">
      <h2>安装预编译版本</h2>
      <div className="row">
        <input style={{ width: 420 }} placeholder="zip URL（留空=release_api 最新）" value={url} onChange={(e) => setUrl(e.target.value)} />
        <input style={{ width: 300 }} placeholder="SHA256（可选）" value={sha} onChange={(e) => setSha(e.target.value)} />
        <button className="btn" disabled={busy} onClick={() => void act('安装版本', async () => {
          const r = await api.versionInstall(url || null, null, sha || null); await load(); await refresh?.(); return r;
        })}>下载 / 校验 / 安装</button>
      </div>
    </div>
    <div className="card"><h2>已安装版本</h2><table><thead><tr><th>版本</th><th>安装时间</th><th>大小</th><th>来源</th><th>操作</th></tr></thead><tbody>
      {versions.map((v) => <tr key={v.name}><td><b>{v.name}</b> {v.is_current && <span className="tag Running">current</span>}</td>
        <td className="muted">{v.installed_at}</td><td>{(v.size_bytes / 1048576).toFixed(1)} MB</td><td className="muted">{v.source}</td>
        <td><button className="btn ghost" disabled={busy || v.is_current} onClick={() => void act('切换版本', async () => {
          const r = await api.versionSwitch(v.name); await load(); await refresh?.(); return r;
        })}>切换</button></td></tr>)}
      {!versions.length && <tr><td colSpan={5} className="muted">暂无版本</td></tr>}
    </tbody></table>
      <div className="row" style={{ marginTop: 12 }}><button className="btn ghost" disabled={busy} onClick={() => void act('回滚到上一版本', async () => {
        const r = await api.versionRollback(); await load(); return r;
      })}>一键回滚</button></div>
    </div>
  </div>;
}

export function InstancesPage({ act, busy }: Props) {
  const [items, setItems] = useState<InstanceInfo[]>([]);
  const [name, setName] = useState('default');
  const load = () => api.instanceList().then(setItems).catch(() => setItems([]));
  useEffect(() => { void load(); }, []);
  return <div className="stack">
    <div className="card"><h2>新建实例</h2><div className="row">
      <input placeholder="实例名" value={name} onChange={(e) => setName(e.target.value)} />
      <button className="btn" disabled={busy} onClick={() => void act('创建实例', async () => {
        const r = await api.instanceCreate(name); await load(); return r;
      })}>创建（自动分配端口）</button>
    </div></div>
    <div className="card"><h2>实例列表</h2><table><thead><tr><th>ID</th><th>版本</th><th>端口</th><th>状态</th><th>重启</th><th>操作</th></tr></thead><tbody>
      {items.map((i) => <tr key={i.id}><td><b>{i.name}</b><div className="muted">{i.id}</div></td><td>{i.version ?? '—'}</td><td>{i.port}</td>
        <td><span className={`tag ${i.state}`}>{i.state}</span>{i.health ? ' ✓health' : ''}</td><td>{i.restart_count}</td>
        <td><div className="row">
          <button className="btn ghost" disabled={busy} onClick={() => void act('启动实例', async () => { const r = await api.instanceStart(i.id); await load(); return r; })}>启动</button>
          <button className="btn ghost" disabled={busy} onClick={() => void act('停止实例', async () => { const r = await api.instanceStop(i.id); await load(); return r; })}>停止</button>
          <button className="btn ghost" disabled={busy} onClick={() => void act('重启实例', async () => { const r = await api.instanceRestart(i.id); await load(); return r; })}>重启</button>
          <button className="btn danger" disabled={busy} onClick={() => void act('删除实例', async () => { const r = await api.instanceDelete(i.id); await load(); return r; })}>删除</button>
        </div></td></tr>)}
      {!items.length && <tr><td colSpan={6} className="muted">暂无实例</td></tr>}
    </tbody></table></div>
  </div>;
}

export function ConfigPage({ act, busy }: Props) {
  const [schema, setSchema] = useState<ConfigSchema | null>(null);
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [selected, setSelected] = useState('');
  const [values, setValues] = useState<Record<string, unknown>>({});
  useEffect(() => {
    void Promise.all([api.hubConfigSchema(), api.instanceList()]).then(([s, is]) => {
      setSchema(s); setInstances(is); setSelected((old) => old || is[0]?.id || '');
    }).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!selected) return;
    void api.hubConfigGet(selected).then(setValues).catch(() => setValues({}));
  }, [selected]);
  const fields = (schema?.fields ?? []).filter((f) => f.owner === 'hub');
  const change = (f: ConfigField, v: string) => setValues((prev) => ({
    ...prev, [f.key]: f.kind === 'number' ? Number(v) : f.kind === 'boolean' ? v === 'true' : v,
  }));
  return <div className="stack">
    <div className="card"><div className="row"><span className="muted">目标实例</span>
      <select value={selected} onChange={(e) => setSelected(e.target.value)}>
        {instances.map((i) => <option key={i.id} value={i.id}>{i.name} ({i.id})</option>)}
      </select><span className="muted">schema: {schema?.source ?? '—'}</span></div></div>
    <div className="card"><h2>Hub 拥有的配置</h2>
      <div className="grid cols-2">
        {fields.map((f) => <label className="field" key={f.key}>
          {f.label}{f.requires_restart && <span className="muted"> · 需重启</span>}
          {f.kind === 'boolean'
            ? <select value={String(values[f.key] ?? f.default)} onChange={(e) => change(f, e.target.value)}><option value="true">true</option><option value="false">false</option></select>
            : <input type={f.kind === 'secret' ? 'password' : 'text'} value={String(values[f.key] ?? f.default ?? '')} onChange={(e) => change(f, e.target.value)} />}
          <span className="muted">{f.description}</span>
        </label>)}
      </div>
      <div className="row" style={{ marginTop: 16 }}><button className="btn" disabled={busy || !selected} onClick={() => void act('保存配置（原子写入 bootstrap）', () => api.hubConfigSet(selected, values))}>保存</button>
        <span className="muted">服务端面板拥有的字段不在此处显示；密钥脱敏存入 bootstrap。</span></div>
    </div>
  </div>;
}

export function ResourcesPage({ status, act, busy }: Props) {
  const [source, setSource] = useState('');
  const categories = ['modfiles', 'staticdata', 'bundles'];
  return <div className="stack">
    <div className="card"><h2>同步源（可选）</h2><div className="row">
      <input style={{ width: 520 }} placeholder="本地 catalog.bin 路径（静态数据同步用，留空=HubConfig.catalog_path）" value={source} onChange={(e) => setSource(e.target.value)} />
    </div></div>
    <div className="grid cols-3">
      {categories.map((c) => {
        const st = status?.resources?.categories?.[c];
        return <div className="card" key={c}><h2>{c}</h2>
          <p className="muted">版本: {st?.version ?? '未同步'}<br />文件: {st ? Object.keys(st.files).length : 0}<br />更新: {st?.updated_at ?? '—'}</p>
          {st?.last_error && <p style={{ color: '#ff9fb2' }}>{st.last_error}</p>}
          <button className="btn" disabled={busy} onClick={() => void act(`同步 ${c}`, () => api.resourceSync(c, source || null))}>立即同步</button>
        </div>;
      })}
    </div>
    <div className="card"><h2>全部同步</h2><button className="btn" disabled={busy} onClick={() => void act('同步全部资源', () => api.resourceSync('all', source || null))}>同步全部（原子切换）</button>
      <p className="muted">modfiles 立即生效；staticdata 更新后标记需重启；任一类别失败保持上一版可用。</p></div>
  </div>;
}

export function ClientPackagePage({ act, busy }: Props) {
  const [instances, setInstances] = useState<InstanceInfo[]>([]);
  const [id, setId] = useState('');
  const [output, setOutput] = useState('');
  useEffect(() => { void api.instanceList().then((is) => { setInstances(is); setId((x) => x || is[0]?.id || ''); }).catch(() => undefined); }, []);
  return <div className="card"><h2>导出客户端发布包</h2>
    <div className="stack">
      <label className="field">目标实例<select value={id} onChange={(e) => setId(e.target.value)}>{instances.map((i) => <option key={i.id} value={i.id}>{i.name}</option>)}</select></label>
      <label className="field">输出路径（可选）<input value={output} onChange={(e) => setOutput(e.target.value)} placeholder="hub/clientpkg/PortableLetheClient-<ver>.zip" /></label>
      <button className="btn" disabled={busy || !id} onClick={() => void act('导出客户端包', () => api.clientpkgExport(id, output || null))}>导出并计算 SHA256</button>
      <p className="muted">仅包含预编译 launcher、BepInEx 基线、配置与 modfiles；绝不包含游戏本体 / bundle / ZylarNex。</p>
    </div>
  </div>;
}

export function LogsPage({ logs }: { logs: LogLine[] }) {
  return <div className="card"><h2>实时日志</h2><div className="logbox">
    {logs.map((l, i) => <div key={i}><span className="muted">{l.ts}</span> <span className={`tag ${l.level}`}>{l.level}</span> [{l.target}] {l.message}</div>)}
    {!logs.length && <span className="muted">暂无日志</span>}
  </div></div>;
}
