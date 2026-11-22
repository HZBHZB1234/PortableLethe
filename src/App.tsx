import { useCallback, useEffect, useState } from 'react';
import { api, isTauri, onEvent } from './api';
import type {
  HubStatus, InstanceStateEvent, LogLine, ProgressEvent, RestartRequestedEvent,
} from './types';
import {
  ClientPackagePage, ConfigPage, DashboardPage, InstancesPage, LogsPage,
  ResourcesPage, VersionsPage,
} from './pages';
import './styles.css';

const PAGES = [
  ['Dashboard', '仪表盘'],
  ['Versions', '版本'],
  ['Instances', '实例'],
  ['Config', '配置'],
  ['Resources', '资源'],
  ['ClientPackage', '客户端包'],
  ['Logs', '日志'],
] as const;

type PageName = (typeof PAGES)[number][0];

export default function App() {
  const [page, setPage] = useState<PageName>('Dashboard');
  const [status, setStatus] = useState<HubStatus | null>(null);
  const [logs, setLogs] = useState<LogLine[]>([]);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [notice, setNotice] = useState<string>('');
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.hubStatus());
    } catch (e) {
      setNotice(String(e));
    }
    try {
      setLogs(await api.logTail(200));
    } catch { /* offline preview */ }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5000);
    let unsubs: Array<() => void> = [];
    if (isTauri()) {
      void Promise.all([
        onEvent<LogLine>('log-line', (l) => setLogs((prev) => [...prev.slice(-499), l])),
        onEvent<ProgressEvent>('sync-progress', (p) => setProgress(p)),
        onEvent<ProgressEvent>('download-progress', (p) => setProgress({ ...p, phase: `download ${p.phase}` })),
        onEvent<InstanceStateEvent>('instance-state', () => void refresh()),
        onEvent<RestartRequestedEvent>('restart-requested', (r) =>
          setNotice(`${r.instance_id} 请求重启: ${r.reason}`)),
      ]).then((fns) => { unsubs = fns; });
    }
    return () => {
      window.clearInterval(timer);
      unsubs.forEach((fn) => fn());
    };
  }, [refresh]);

  const act = async (label: string, fn: () => Promise<unknown>) => {
    setBusy(true);
    setNotice(label + '…');
    try {
      const r = await fn();
      setNotice(`${label} 完成` + (r && typeof r === 'object' && 'path' in r ? `: ${String((r as { path: unknown }).path)}` : ''));
      await refresh();
    } catch (e) {
      setNotice(`${label} 失败: ${String(e)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand"><span className="brand-mark">PL</span><div><b>Portable Lethe</b><small>Hub · v{status?.app_version ?? '…'}</small></div></div>
        <nav>{PAGES.map(([key, label]) => (
          <button key={key} className={page === key ? 'nav active' : 'nav'} onClick={() => setPage(key)}>{label}</button>
        ))}</nav>
        <div className="side-foot">
          <div className={status && status.running_count > 0 ? 'dot green' : 'dot'} />
          {status ? `${status.running_count} running / ${status.instance_count} instances` : 'connecting…'}
        </div>
      </aside>
      <main className="main">
        <header className="topbar">
          <div><h1>{PAGES.find(([k]) => k === page)?.[1]}</h1><p className="subtitle">hub dir: {status?.hub_dir ?? '—'}</p></div>
          <button className="btn ghost" onClick={() => void refresh()}>刷新</button>
        </header>
        {progress && <div className="progress">{[progress.phase, progress.message, `${progress.current}/${progress.total}`].filter(Boolean).join(' · ')}</div>}
        {notice && <div className="notice">{notice}</div>}
        <div className="content">
          {page === 'Dashboard' && <DashboardPage status={status} act={act} busy={busy} />}
          {page === 'Versions' && <VersionsPage status={status} act={act} busy={busy} refresh={refresh} />}
          {page === 'Instances' && <InstancesPage status={status} act={act} busy={busy} />}
          {page === 'Config' && <ConfigPage act={act} busy={busy} />}
          {page === 'Resources' && <ResourcesPage status={status} act={act} busy={busy} />}
          {page === 'ClientPackage' && <ClientPackagePage act={act} busy={busy} />}
          {page === 'Logs' && <LogsPage logs={logs} />}
        </div>
      </main>
    </div>
  );
}
