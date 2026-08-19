import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import {
  api,
  Entry,
  LOCAL_CONN,
  TransferDoneEvent,
  TransferProgressEvent,
  UpdateInfo,
} from './api';
import { t } from './i18n';
import { joinPath } from './paths';
import FilePane from './components/FilePane';
import QueuePanel, { QItem } from './components/QueuePanel';
import RemotePane from './components/RemotePane';
import UpdateModal from './components/UpdateModal';

let nextKey = 1;

export default function App() {
  const [version, setVersion] = useState('');
  const [update, setUpdate] = useState<UpdateInfo | null | 'unchecked'>('unchecked');
  const [checking, setChecking] = useState(false);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const [toastMsg, setToastMsg] = useState<{ msg: string; err: boolean } | null>(null);
  const toastTimer = useRef<number>(0);

  const [localSel, setLocalSel] = useState<{ entry: Entry | null; dir: string }>({ entry: null, dir: '' });
  const [remoteSel, setRemoteSel] = useState<{ entry: Entry | null; dir: string }>({ entry: null, dir: '' });
  const [remoteConn, setRemoteConn] = useState<number | null>(null);
  const [queue, setQueue] = useState<QItem[]>([]);
  const [reloadLocal, setReloadLocal] = useState(0);
  const [reloadRemote, setReloadRemote] = useState(0);

  const toast = useCallback((msg: string, isError = false) => {
    setToastMsg({ msg, err: isError });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToastMsg(null), 5000);
  }, []);

  const onPaneError = useCallback((msg: string) => toast(msg, true), [toast]);
  const onLocalSelect = useCallback(
    (entry: Entry | null, dir: string) => setLocalSel({ entry, dir }),
    []
  );
  const onRemoteSelect = useCallback(
    (entry: Entry | null, dir: string) => setRemoteSel({ entry, dir }),
    []
  );

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
    // silent update check on app start; when an update exists the changelog
    // dialog opens first — installing always needs an explicit confirmation
    api
      .checkUpdate()
      .then((u) => {
        setUpdate(u);
        if (u) setShowUpdateModal(true);
      })
      .catch(() => {});
  }, []);

  // Transfer-Events der Engine → Warteschlangen-Status
  useEffect(() => {
    const unProgress = listen<TransferProgressEvent>('transfer_progress', ({ payload }) => {
      setQueue((qs) =>
        qs.map((q) =>
          q.tid === payload.id ? { ...q, done: payload.done, total: payload.total ?? q.total } : q
        )
      );
    });
    const unDone = listen<TransferDoneEvent>('transfer_done', ({ payload }) => {
      setQueue((qs) =>
        qs.map((q) => {
          if (q.tid !== payload.id) return q;
          if (payload.ok) {
            setTimeout(() => (q.direction === 'download' ? setReloadLocal((n) => n + 1) : setReloadRemote((n) => n + 1)), 0);
            return { ...q, status: 'done', done: payload.bytes, total: payload.bytes };
          }
          return {
            ...q,
            status: payload.cancelled ? 'cancelled' : 'error',
            error: payload.error ?? undefined,
          };
        })
      );
    });
    return () => {
      unProgress.then((f) => f());
      unDone.then((f) => f());
    };
  }, []);

  // Sequenzieller Abarbeiter: immer genau ein Transfer läuft.
  useEffect(() => {
    if (queue.some((q) => q.status === 'running' || q.status === 'starting')) return;
    const next = queue.find((q) => q.status === 'queued');
    if (!next) return;
    setQueue((qs) => qs.map((q) => (q.key === next.key ? { ...q, status: 'starting' } : q)));
    api
      .transferStart(next.direction, next.conn, next.remote, next.local)
      .then((tid) =>
        setQueue((qs) => qs.map((q) => (q.key === next.key ? { ...q, tid, status: 'running' } : q)))
      )
      .catch((err) =>
        setQueue((qs) =>
          qs.map((q) => (q.key === next.key ? { ...q, status: 'error', error: String(err) } : q))
        )
      );
  }, [queue]);

  const enqueue = (direction: 'download' | 'upload') => {
    if (remoteConn === null) return;
    const src = direction === 'upload' ? localSel.entry : remoteSel.entry;
    if (!src || src.kind === 'dir') return;
    const item: QItem = {
      key: nextKey++,
      direction,
      name: src.name,
      status: 'queued',
      done: 0,
      total: src.size || null,
      conn: remoteConn,
      remote: direction === 'upload' ? joinPath(remoteSel.dir, src.name) : src.path,
      local: direction === 'upload' ? src.path : joinPath(localSel.dir, src.name),
    };
    setQueue((qs) => [...qs, item]);
  };

  const cancelItem = (item: QItem) => {
    if ((item.status === 'running' || item.status === 'starting') && item.tid) {
      api.transferCancel(item.tid).catch(() => {});
    } else if (item.status === 'queued') {
      setQueue((qs) =>
        qs.map((q) => (q.key === item.key ? { ...q, status: 'cancelled' } : q))
      );
    }
  };

  const clearFinished = () =>
    setQueue((qs) => qs.filter((q) => q.status !== 'done' && q.status !== 'error' && q.status !== 'cancelled'));

  const doCheckUpdate = async () => {
    setChecking(true);
    try {
      const u = await api.checkUpdate();
      setUpdate(u);
      if (u) setShowUpdateModal(true);
      else toast(t.upToDate);
    } catch (err) {
      toast(String(err), true);
    } finally {
      setChecking(false);
    }
  };

  const canUpload = remoteConn !== null && !!localSel.entry && localSel.entry.kind !== 'dir';
  const canDownload = remoteConn !== null && !!remoteSel.entry && remoteSel.entry.kind !== 'dir';

  return (
    <div className="app">
      <header className="topbar">
        <h1>
          <span className="brand">file/port</span>
          <span className="dot">.</span>
        </h1>
        <span className="subtitle">{t.subtitle}</span>
        <div className="topbar-right">
          {update !== 'unchecked' && update !== null ? (
            <button className="primary small" onClick={() => setShowUpdateModal(true)}>
              {t.updateAvailable(update.version)}
            </button>
          ) : (
            <button className="small" onClick={doCheckUpdate} disabled={checking}>
              {checking ? t.updateChecking : t.checkForUpdates}
            </button>
          )}
          {version && <span className="version">v{version}</span>}
        </div>
      </header>

      <main className="panes with-middle">
        <FilePane
          conn={LOCAL_CONN}
          title={t.localPane}
          onError={onPaneError}
          onSelect={onLocalSelect}
          reloadKey={reloadLocal}
        />
        <div className="transfer-buttons">
          <button
            className="icon big"
            title={t.uploadTitle}
            disabled={!canUpload}
            onClick={() => enqueue('upload')}
          >
            →
          </button>
          <button
            className="icon big"
            title={t.downloadTitle}
            disabled={!canDownload}
            onClick={() => enqueue('download')}
          >
            ←
          </button>
        </div>
        <RemotePane
          onError={onPaneError}
          onConnected={(conn) => {
            setRemoteConn(conn);
            if (conn === null) setRemoteSel({ entry: null, dir: '' });
          }}
          paneProps={{ onError: onPaneError, onSelect: onRemoteSelect, reloadKey: reloadRemote }}
        />
      </main>

      <QueuePanel items={queue} onCancel={cancelItem} onClear={clearFinished} />

      {showUpdateModal && update !== 'unchecked' && update !== null && (
        <UpdateModal info={update} onToast={toast} onClose={() => setShowUpdateModal(false)} />
      )}

      {toastMsg && <div className={`toast${toastMsg.err ? ' error' : ''}`}>{toastMsg.msg}</div>}
    </div>
  );
}
