import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { api, isTauri, onEvent } from './api';
import type {
  InstanceStateEvent, LogLine, ProgressEvent, RestartRequestedEvent,
} from './types';
import {
  ClientPackagePage, InstancesPage, LogsPage, ResourcesPage,
  SettingsPage, VersionsPage, type HubData, type PageName,
} from './pages';
import './styles.css';

const NAV: Array<{ key: PageName; label: string; icon: string; eyebrow: string; desc: string }> = [
  { key: 'Instances', label: '部署实例', icon: 'server', eyebrow: '工作区', desc: '启动 / 停止实例，切换服务端版本与数据资源集' },
  { key: 'Versions', label: '服务端', icon: 'cube', eyebrow: '工作区', desc: '已安装版本、默认版本、安装与回滚' },
  { key: 'Resources', label: '数据资源', icon: 'layers', eyebrow: '工作区', desc: '资源集、本机游戏检测与静态数据' },
  { key: 'Settings', label: '设置', icon: 'settings', eyebrow: '系统', desc: 'Hub 全局配置与实例级配置' },
  { key: 'Logs', label: '日志', icon: 'terminal', eyebrow: '系统', desc: '实时日志流与级别 / 关键字筛选' },
  { key: 'ClientPackage', label: '客户端包', icon: 'download', eyebrow: '工具', desc: '导出可发布的轻量客户端包' },
];

function Icon({ name }: { name: string }) {
  const paths: Record<string, ReactNode> = {
    server: <><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01M7 16.5h.01" /></>,
    cube: <><path d="M21 8 12 3 3 8v8l9 5 9-5V8Z" /><path d="m3 8 9 5 9-5M12 13v8" /></>,
    layers: <><path d="m12 3 9 5-9 5-9-5 9-5ZM3 13l9 5 9-5M3 17l9 5 9-5" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21h-4v-.1A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.9.3l-.1.1-2.8-2.8.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H3v-4h.1A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.3-1.9l-.1-.1 2.8-2.8.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.6V3h4v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1 2.8 2.8-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.6 1H21v4h-.1a1.7 1.7 0 0 0-1.5 1Z" /></>,
    terminal: <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 9 3 3-3 3M13 15h4" /></>,
    download: <><path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v3h16v-3" /></>,
    refresh: <><path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" /></>,
    shield: <><path d="M12 3 4 6v6c0 5 3.5 8 8 9 4.5-1 8-4 8-9V6l-8-3Z" /><path d="m9 12 2 2 4-4" /></>,
    folder: <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />,
  };
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name] ?? paths.server}</svg>;
}

type Toast = { id: number; text: string; tone: 'info' | 'error' | 'success' };

/// Tauri rejects with strings, Error objects, or plain {message, code} objects.
/// String(e) turns the latter into "[object Object]" - always extract a real message.
function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === 'object') {
    const o = e as Record<string, unknown>;
    const message = o.message ?? o.error ?? o.reason;
    if (typeof message === 'string' && message) {
      const code = typeof o.code === 'string' || typeof o.code === 'number' ? ` (${o.code})` : '';
      return message + code;
    }
    try { return JSON.stringify(e); } catch { /* fall through */ }
  }
  return String(e);
}

const emptyData: HubData = {
  status: null, instances: [], versions: [], resourceSets: [],
  gameInstall: null, settings: null, schema: null, logs: [],
};

function sectionStats(page: PageName, data: HubData): Array<{ label: string; value: string }> {
  switch (page) {
    case 'Instances':
      return [
        { label: '实例总数', value: String(data.instances.length) },
        { label: '运行中', value: String(data.instances.filter((i) => i.state === 'Running').length) },
        { label: '占用端口', value: data.instances.map((i) => String(i.port)).slice(0, 3).join(' / ') || '—' },
      ];
    case 'Versions':
      return [
        { label: '已安装', value: String(data.versions.length) },
        { label: '默认版本', value: data.versions.find((v) => v.is_current)?.name ?? '—' },
      ];
    case 'Resources':
      return [
        { label: '资源集', value: String(data.resourceSets.length) },
        { label: '默认资源集', value: data.resourceSets.find((r) => r.is_default)?.name ?? '—' },
        { label: '本机游戏', value: data.gameInstall?.found ? '已检测到' : '未检测到' },
      ];
    case 'Settings':
      return [
        { label: 'Hub 版本', value: data.status?.app_version ?? '—' },
        { label: '平台', value: data.status?.platform ?? '—' },
      ];
    case 'Logs':
      return [{ label: '日志条数', value: String(data.logs.length) }];
    case 'ClientPackage':
      return [{ label: '可选实例', value: String(data.instances.length) }];
  }
}

export default function App() {
  const [page, setPage] = useState<PageName>('Instances');
  const [data, setData] = useState<HubData>(emptyData);
  const [settingsInstanceId, setSettingsInstanceId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [busy, setBusy] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const toastId = useRef(0);
  const firstLoad = useRef(true);

  const toast = useCallback((text: string, tone: Toast['tone'] = 'info') => {
    const id = ++toastId.current;
    setToasts((prev) => [...prev.slice(-3), { id, text, tone }]);
    window.setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), tone === 'error' ? 10000 : 5000);
  }, []);

  const refresh = useCallback(async () => {
    const results = await Promise.allSettled([
      api.hubStatus(), api.instanceList(), api.versionList(), api.resourceSetList(),
      api.gameDetect(), api.hubSettingsGet(), api.hubConfigSchema(), api.logTail(400),
    ]);
    const [st, ins, ver, sets, game, settings, schema, logLines] = results.map((r) =>
      r.status === 'fulfilled' ? r.value : null,
    ) as unknown as [
      HubData['status'], HubData['instances'], HubData['versions'], HubData['resourceSets'],
      HubData['gameInstall'], HubData['settings'], HubData['schema'], LogLine[],
    ];
    if (results.some((r) => r.status === 'rejected')) {
      if (firstLoad.current) toast('部分后端接口不可用，已切换到离线预览数据', 'info');
    }
    firstLoad.current = false;
    setData({
      status: st,
      instances: ins ?? [],
      versions: ver ?? [],
      resourceSets: sets ?? [],
      gameInstall: game ?? null,
      settings: settings,
      schema: schema,
      logs: logLines ?? [],
    });
  }, [toast]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => { if (autoRefresh) void refresh(); }, 5000);
    let unsubs: Array<() => void> = [];
    if (isTauri()) {
      void Promise.all([
        onEvent<LogLine>('log-line', (l) => setData((prev) => ({ ...prev, logs: [...prev.logs.slice(-999), l] }))),
        onEvent<ProgressEvent>('sync-progress', (p) => setProgress(p)),
        onEvent<ProgressEvent>('download-progress', (p) => setProgress({ ...p, phase: p.phase ? `download ${p.phase}` : 'download' })),
        onEvent<InstanceStateEvent>('instance-state', () => void refresh()),
        onEvent<RestartRequestedEvent>('restart-requested', (r) => toast(`${r.instance_id} 请求重启: ${r.reason}`, 'info')),
      ]).then((fns) => { unsubs = fns; });
    }
    return () => { window.clearInterval(timer); unsubs.forEach((fn) => fn()); };
  }, [refresh, autoRefresh, toast]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'r') { e.preventDefault(); void refresh(); }
      if (e.key === 'Escape') setToasts([]);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [refresh]);

  const act = useCallback(async (label: string, fn: () => Promise<unknown>) => {
    setBusy(true);
    toast(`${label} 开始…`, 'info');
    try {
      const result = await fn();
      toast(`${label} 完成`, 'success');
      await refresh();
      return result;
    } catch (e) {
      toast(`${label} 失败: ${errorText(e)}`, 'error');
      return undefined;
    } finally { setBusy(false); setProgress(null); }
  }, [refresh, toast]);

  const checkDeps = useCallback(async () => {
    setBusy(true);
    try {
      const r = await api.hubEnsureDeps(null);
      const detail = r.missing.length ? `；缺失: ${r.missing.join('; ')}` : '';
      toast(`依赖检查完成：修复 ${r.fixed.length} 项${detail}`, r.missing.length ? 'error' : 'success');
      await refresh();
    } catch (e) { toast(`依赖检查失败: ${errorText(e)}`, 'error'); }
    finally { setBusy(false); }
  }, [refresh, toast]);

  const openInstanceSettings = (id: string) => {
    setSettingsInstanceId(id);
    setPage('Settings');
  };

  const current = NAV.find((n) => n.key === page)!;
  const stats = sectionStats(page, data);
  const pct = progress?.total ? Math.min(100, Math.round((progress.current / progress.total) * 100)) : null;
  const running = data.status?.running_count ?? 0;
  const statusPillText = data.status ? `v${data.status.app_version}` : '未连接';

  const shared = { data, act, busy, refresh, onNavigate: setPage };

  return (
    <div className="app-shell">
      {/* 1) Slim icon-only rail */}
      <nav className="icon-rail" aria-label="主导航">
        <div className="rail-brand" title="Portable Lethe Hub"><span className="rail-mark">PL</span></div>
        <div className="rail-nav">
          {NAV.map((n) => <button
            key={n.key}
            className={`rail-item${page === n.key ? ' active' : ''}`}
            title={n.label}
            aria-label={n.label}
            aria-current={page === n.key ? 'page' : undefined}
            onClick={() => setPage(n.key)}
          >
            <Icon name={n.icon} />
            {n.key === 'Instances' && data.instances.length > 0 && <span className="rail-badge">{data.instances.length}</span>}
          </button>)}
        </div>
        <div className="rail-foot">
          <span className={`rail-dot${running > 0 ? ' on' : ''}`} title={running > 0 ? `${running} 个实例运行中` : '无运行实例'} />
          <button className="rail-item" title="检查依赖" aria-label="检查依赖" onClick={() => void checkDeps()}><Icon name="shield" /></button>
        </div>
      </nav>

      {/* 2) Secondary contextual sidebar */}
      <aside className="context-sidebar" aria-label="上下文导航">
        <div className="ctx-head">
          <span className="ctx-eyebrow">{current.eyebrow} / {current.label}</span>
          <h2 className="ctx-title">{current.label}</h2>
          <p className="ctx-desc">{current.desc}</p>
        </div>
        <div className="ctx-divider" />
        <div className="ctx-stats">
          {stats.map((s) => <div className="ctx-stat" key={s.label}>
            <span className="ctx-stat-label">{s.label}</span>
            <span className="ctx-stat-value" title={s.value}>{s.value}</span>
          </div>)}
        </div>
        <div className="ctx-divider" />
        <div className="ctx-label">快捷操作</div>
        <div className="ctx-actions">
          <button className="ctx-action" disabled={busy} onClick={() => void checkDeps()}><Icon name="shield" />检查并修复依赖</button>
          <button className="ctx-action" disabled={busy} onClick={() => void act('打开 Hub 目录', () => api.hubRevealDir())}><Icon name="folder" />打开 Hub 目录</button>
          <button className="ctx-action" disabled={busy} onClick={() => void act('同步全部资源', () => api.resourceSync('all'))}><Icon name="refresh" />同步全部资源</button>
        </div>
        <div className="ctx-foot">
          <div className="ctx-foot-line"><span className={`dot${running > 0 ? ' green' : ''}`} />{running} 运行中 / {data.instances.length} 实例</div>
          <code title={data.status?.hub_dir ?? ''}>{data.status?.hub_dir ?? '连接中…'}</code>
        </div>
      </aside>

      {/* 3) Light main canvas */}
      <main className="workspace">
        <header className="topbar">
          <div className="crumbs"><span>工作区</span><span className="crumb-sep">/</span><span className="crumb-current">{current.label}</span></div>
          <div className="topbar-spacer" />
          <label className="checkline"><input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} />自动刷新</label>
          <span className={`pill ${data.status ? 'indigo' : 'gray'}`}><span className="status-dot" />{statusPillText}</span>
          <button className="icon-btn" title="刷新 (Ctrl+R)" disabled={busy} onClick={() => void refresh()}><Icon name="refresh" /></button>
        </header>

        <div className="notice-layer">
          {progress && <div className="progress-panel">
            <div className="progress-head">
              <span>{({
                download: '下载中', extracting: '解压中', indexing: '建立索引中',
              } as Record<string, string>)[progress.phase] ?? progress.phase ?? '处理中'}{progress.message ? ` · ${progress.message}` : ''}</span>
              {progress.total > 0 && <span className="mono">{pct}% · {progress.current}/{progress.total}</span>}
            </div>
            <div className="progress-track">
              <div className={`progress-fill${progress.total > 0 ? '' : ' indeterminate'}`}
                style={progress.total > 0 ? { width: `${pct}%` } : undefined} />
            </div>
          </div>}
          {toasts.length > 0 && <div className="toast-stack">
            {toasts.map((t) => <div key={t.id} className={`toast ${t.tone}`}>{t.text}</div>)}
          </div>}
        </div>

        <div className="canvas">
          {page === 'Instances' && <InstancesPage {...shared} onOpenInstanceSettings={openInstanceSettings} />}
          {page === 'Versions' && <VersionsPage {...shared} />}
          {page === 'Resources' && <ResourcesPage {...shared} />}
          {page === 'Settings' && <SettingsPage {...shared} initialInstanceId={settingsInstanceId} />}
          {page === 'Logs' && <LogsPage {...shared} />}
          {page === 'ClientPackage' && <ClientPackagePage {...shared} />}
        </div>
      </main>
    </div>
  );
}
