import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react';
import { api, isTauri, onEvent } from './api';
import type {
  HubStatus, InstanceStateEvent, LogLine, ProgressEvent, RestartRequestedEvent,
} from './types';
import {
  ClientPackagePage, ConfigPage, DashboardPage, InstancesPage, LogsPage,
  ResourcesPage, VersionsPage,
} from './pages';
import './styles.css';

type PageName = 'Dashboard' | 'Versions' | 'Instances' | 'Config' | 'Resources' | 'ClientPackage' | 'Logs';

const NAV: Array<{ key: PageName; label: string; icon: string; group: string }> = [
  { key: 'Dashboard', label: '总览', icon: 'grid', group: '工作区' },
  { key: 'Instances', label: '实例', icon: 'server', group: '工作区' },
  { key: 'Versions', label: '版本', icon: 'package', group: '工作区' },
  { key: 'Resources', label: '资源', icon: 'layers', group: '工作区' },
  { key: 'ClientPackage', label: '客户端包', icon: 'download', group: '工作区' },
  { key: 'Config', label: '配置', icon: 'settings', group: '系统' },
  { key: 'Logs', label: '日志', icon: 'terminal', group: '系统' },
];

function Icon({ name }: { name: string }) {
  const paths: Record<string, ReactNode> = {
    grid: <><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></>,
    server: <><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path d="M7 7.5h.01M7 16.5h.01" /></>,
    package: <><path d="M21 8 12 3 3 8v8l9 5 9-5V8Z" /><path d="m3 8 9 5 9-5M12 13v8" /></>,
    layers: <><path d="m12 3 9 5-9 5-9-5 9-5ZM3 13l9 5 9-5M3 17l9 5 9-5" /></>,
    download: <><path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v3h16v-3" /></>,
    settings: <><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21h-4v-.1A1.7 1.7 0 0 0 9 19.4a1.7 1.7 0 0 0-1.9.3l-.1.1-2.8-2.8.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H3v-4h.1A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.3-1.9l-.1-.1 2.8-2.8.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.6V3h4v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1 2.8 2.8-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.6 1H21v4h-.1a1.7 1.7 0 0 0-1.5 1Z" /></>,
    terminal: <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="m7 9 3 3-3 3M13 15h4" /></>,
    refresh: <><path d="M20 11a8 8 0 1 0-2.3 5.7M20 5v6h-6" /></>,
    external: <><path d="M14 4h6v6M20 4l-9 9M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" /></>,
  };
  return <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{paths[name] ?? paths.grid}</svg>;
}

type Toast = { id: number; text: string; tone: 'info' | 'error' | 'success' };

export default function App() {
  const [page, setPage] = useState<PageName>('Dashboard');
  const [status, setStatus] = useState<HubStatus | null>(null);
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [busy, setBusy] = useState(false);
  const [autoRefresh, setAutoRefresh] = useState(true);
  const toastId = useRef(0);

  const toast = useCallback((text: string, tone: Toast['tone'] = 'info') => {
    const id = ++toastId.current;
    setToasts((prev) => [...prev.slice(-3), { id, text, tone }]);
    window.setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 5000);
  }, []);

  const refresh = useCallback(async () => {
    try { setStatus(await api.hubStatus()); }
    catch (e) { toast(String(e), 'error'); }
    try { setLogs(await api.logTail(300)); }
    catch { /* offline preview */ }
  }, [toast]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => { if (autoRefresh) void refresh(); }, 5000);
    let unsubs: Array<() => void> = [];
    if (isTauri()) {
      void Promise.all([
        onEvent<LogLine>('log-line', (l) => setLogs((prev) => [...prev.slice(-999), l])),
        onEvent<ProgressEvent>('sync-progress', (p) => setProgress(p)),
        onEvent<ProgressEvent>('download-progress', (p) => setProgress({ ...p, phase: `download ${p.phase}` })),
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

  const act = async (label: string, fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      const r = await fn();
      toast(`${label} 完成`, 'success');
      await refresh();
      return r;
    } catch (e) {
      toast(`${label} 失败: ${String(e)}`, 'error');
      return undefined;
    } finally { setBusy(false); }
  };

  const current = NAV.find((n) => n.key === page)!;
  const groups = [...new Set(NAV.map((n) => n.group))];
  const pct = progress?.total ? Math.min(100, Math.round((progress.current / progress.total) * 100)) : null;

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">PL</span>
          <div><div className="brand-name">Portable Lethe</div><div className="brand-sub">Hub · v{status?.app_version ?? '…'}</div></div>
        </div>
        {groups.map((group) => <div key={group}>
          <div className="nav-section">{group}</div>
          <nav className="nav">
            {NAV.filter((n) => n.group === group).map((n) => (
              <button key={n.key} className={page === n.key ? 'nav-item active' : 'nav-item'} onClick={() => setPage(n.key)}>
                <Icon name={n.icon} /><span>{n.label}</span>
                {n.key === 'Instances' && status && <span className="badge">{status.instance_count}</span>}
              </button>
            ))}
          </nav>
        </div>)}
        <div className="side-foot">
          <span className={status && status.running_count > 0 ? 'dot green' : 'dot'} />
          {status ? `${status.running_count} 运行中 / ${status.instance_count} 实例` : '连接中…'}
          <div style={{ marginTop: 6 }}>{status?.hub_dir ?? '—'}</div>
        </div>
      </aside>

      <main className="main">
        <header className="topbar">
          <div><h1>{current.label}</h1><div className="topbar-sub">本地服务控制中心</div></div>
          <div className="topbar-spacer" />
          <label className="checkline"><input type="checkbox" checked={autoRefresh} onChange={(e) => setAutoRefresh(e.target.checked)} /> 自动刷新</label>
          <span className={status && status.running_count > 0 ? 'pill live' : 'pill'}><span className="dot" />{status ? `v${status.app_version}` : '未连接'}</span>
          <button className="btn ghost btn-icon" title="刷新 (Ctrl+R)" onClick={() => void refresh()}><Icon name="refresh" /></button>
        </header>

        {progress && <div className="banner"><div className="banner-inner">
          <div className="card">
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <span className="subtle">{[progress.phase, progress.message].filter(Boolean).join(' · ')}</span>
              <span className="muted num">{progress.current}/{progress.total}</span>
            </div>
            {pct !== null && <div className="progress-wrap" style={{ marginTop: 8 }}><div className="progress-bar" style={{ width: `${pct}%` }} /></div>}
          </div>
        </div></div>}
        {toasts.length > 0 && <div className="banner"><div className="banner-inner stack">
          {toasts.map((t) => <div key={t.id} className={`notice ${t.tone === 'info' ? '' : t.tone}`}>{t.text}</div>)}
        </div></div>}

        <div className="content"><div className="content-inner">
          {page === 'Dashboard' && <DashboardPage status={status} act={act} busy={busy} onNavigate={setPage} />}
          {page === 'Versions' && <VersionsPage status={status} act={act} busy={busy} refresh={refresh} />}
          {page === 'Instances' && <InstancesPage status={status} act={act} busy={busy} />}
          {page === 'Config' && <ConfigPage act={act} busy={busy} />}
          {page === 'Resources' && <ResourcesPage status={status} act={act} busy={busy} />}
          {page === 'ClientPackage' && <ClientPackagePage act={act} busy={busy} />}
          {page === 'Logs' && <LogsPage logs={logs} />}
        </div></div>
      </main>
    </div>
  );
}
