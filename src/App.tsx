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
import { errText, t } from './i18n';
import { isExistsError } from './errors';
import { joinPath } from './paths';
import { applyTheme, readTheme, Theme } from './theme';
import FilePane from './components/FilePane';
import Logo from './components/Logo';
import QueuePanel, { QItem } from './components/QueuePanel';
import RemotePane from './components/RemotePane';
import UpdateModal from './components/UpdateModal';
import Help from './components/Help';

let nextKey = 1;

export default function App() {
  const [version, setVersion] = useState('');
  const [update, setUpdate] = useState<UpdateInfo | null | 'unchecked'>('unchecked');
  const [checking, setChecking] = useState(false);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const [toastMsg, setToastMsg] = useState<{ msg: string; err: boolean } | null>(null);
  const toastTimer = useRef<number>(0);
  const [theme, setTheme] = useState<Theme>(readTheme);

  const [localSel, setLocalSel] = useState<{ entry: Entry | null; dir: string }>({ entry: null, dir: '' });
  const [remoteSel, setRemoteSel] = useState<{ entry: Entry | null; dir: string }>({ entry: null, dir: '' });
  const [remoteConn, setRemoteConn] = useState<number | null>(null);
  const [queue, setQueue] = useState<QItem[]>([]);
  const [reloadLocal, setReloadLocal] = useState(0);
  const [reloadRemote, setReloadRemote] = useState(0);
  // Namen im aktuell angezeigten Ordner beider Seiten — für die Rückfrage,
  // bevor ein Transfer eine bestehende Datei ersetzt.
  const localNames = useRef<Set<string>>(new Set());
  const remoteNames = useRef<Set<string>>(new Set());
  const [pendingOverwrite, setPendingOverwrite] = useState<QItem | null>(null);

  const toast = useCallback((msg: string, isError = false) => {
    // Fehlertexte der Engine sind deutsch — in der UI-Sprache anzeigen.
    setToastMsg({ msg: isError ? errText(msg) : msg, err: isError });
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
  const onLocalEntries = useCallback((list: Entry[]) => {
    // Klein geschrieben: macOS (APFS) und Windows unterscheiden Groß-/Kleinschreibung
    // nicht — „Report.pdf“ würde „report.pdf“ sonst ohne Rückfrage ersetzen.
    localNames.current = new Set(list.map((e) => e.name.toLowerCase()));
  }, []);
  const onRemoteEntries = useCallback((list: Entry[]) => {
    remoteNames.current = new Set(list.map((e) => e.name.toLowerCase()));
  }, []);

  useEffect(() => applyTheme(theme), [theme]);

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
          q.key === payload.id && q.status !== 'done' && q.status !== 'error' && q.status !== 'cancelled'
            ? { ...q, status: 'running', done: payload.done, total: payload.total ?? q.total }
            : q
        )
      );
    });
    const unDone = listen<TransferDoneEvent>('transfer_done', ({ payload }) => {
      setQueue((qs) =>
        qs.map((q) => {
          if (q.key !== payload.id) return q;
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
    // Offene Ersetzen-Rückfrage: Warteschlange anhalten — sonst könnte ein
    // weiterer Eintrag mit „existiert bereits“ die Rückfrage verdrängen und
    // ginge kommentarlos verloren.
    if (pendingOverwrite) return;
    const next = queue.find((q) => q.status === 'queued');
    if (!next) return;
    setQueue((qs) => qs.map((q) => (q.key === next.key ? { ...q, status: 'starting' } : q)));
    // Die Transfer-ID ist der Warteschlangen-Schlüssel: Events, die vor der
    // Antwort des Commands eintreffen, finden ihren Eintrag trotzdem.
    api
      .transferStart(next.key, next.direction, next.conn, next.remote, next.local, next.name, !!next.overwrite)
      .then(() =>
        setQueue((qs) =>
          qs.map((q) => (q.key === next.key && q.status === 'starting' ? { ...q, status: 'running' } : q))
        )
      )
      .catch((err) => {
        // Ziel ist inzwischen entstanden (z. B. durch einen früheren Eintrag
        // der Warteschlange) — jetzt doch rückfragen statt still zu ersetzen.
        if (!next.overwrite && isExistsError(err)) {
          setQueue((qs) => qs.filter((q) => q.key !== next.key));
          setPendingOverwrite({ ...next, status: 'queued' });
          return;
        }
        setQueue((qs) =>
          qs.map((q) => (q.key === next.key ? { ...q, status: 'error', error: String(err) } : q))
        );
      });
  }, [queue, pendingOverwrite]);

  const enqueue = (direction: 'download' | 'upload', explicit?: Entry) => {
    if (remoteConn === null) return;
    const src = explicit ?? (direction === 'upload' ? localSel.entry : remoteSel.entry);
    if (!src || src.kind === 'dir') return;
    // Server-Namen (z. B. „a\\b“, „CON.txt“, „a:b“) prüft das Backend
    // plattformgerecht — unzulässige Namen enden als Fehler in der Warteschlange.
    const targetName = src.name.toLowerCase();
    const exists =
      direction === 'upload'
        ? remoteNames.current.has(targetName)
        : localNames.current.has(targetName);
    const item: QItem = {
      key: nextKey++,
      direction,
      name: src.name,
      status: 'queued',
      done: 0,
      total: src.size || null,
      conn: remoteConn,
      remote: direction === 'upload' ? joinPath(remoteSel.dir, src.name) : src.path,
      // Download: Zielordner — den Dateinamen hängt das Backend nach Prüfung an.
      local: direction === 'upload' ? src.path : localSel.dir,
    };
    if (exists) setPendingOverwrite(item);
    else setQueue((qs) => [...qs, item]);
  };

  const cancelItem = (item: QItem) => {
    if (item.status === 'running' || item.status === 'starting') {
      api.transferCancel(item.key).catch(() => {});
    } else if (item.status === 'queued') {
      setQueue((qs) =>
        qs.map((q) => (q.key === item.key ? { ...q, status: 'cancelled' } : q))
      );
    }
  };

  /** Offene Transfers (laufend/wartend) einer Verbindung. */
  const openTransfers = (conn: number) =>
    queue.filter(
      (q) => q.conn === conn && (q.status === 'queued' || q.status === 'starting' || q.status === 'running')
    );

  /** Vor dem Trennen: alle offenen Transfers dieser Verbindung abbrechen. */
  const cancelTransfersOf = (conn: number) => openTransfers(conn).forEach(cancelItem);

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
      toast(errText(err), true);
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
          <Logo />
          <span className="brand">fileport</span>
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
          <button
            className="small theme-toggle"
            title={theme === 'dark' ? t.themeToggleLight : t.themeToggleDark}
            aria-label={theme === 'dark' ? t.themeToggleLight : t.themeToggleDark}
            onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
          >
            {theme === 'dark' ? (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="4" />
                <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
              </svg>
            ) : (
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <path d="M21 12.79A9 9 0 1 1 11.21 3a7 7 0 0 0 9.79 9.79z" />
              </svg>
            )}
          </button>
          {version && <span className="version">v{version}</span>}
        </div>
      </header>

      <main className="panes with-middle">
        <FilePane
          conn={LOCAL_CONN}
          title={t.localPane}
          onError={onPaneError}
          onSelect={onLocalSelect}
          onEntries={onLocalEntries}
          reloadKey={reloadLocal}
          onFileActivate={(e) => enqueue('upload', e)}
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
          onInfo={toast}
          openTransfers={(conn) => openTransfers(conn).length}
          onCancelTransfers={cancelTransfersOf}
          onConnected={(conn) => {
            setRemoteConn(conn);
            if (conn === null) {
              setPendingOverwrite(null);
              setRemoteSel({ entry: null, dir: '' });
              remoteNames.current = new Set();
            }
          }}
          paneProps={{
            onError: onPaneError,
            onSelect: onRemoteSelect,
            onEntries: onRemoteEntries,
            reloadKey: reloadRemote,
            onFileActivate: (e) => enqueue('download', e),
          }}
        />
      </main>

      <QueuePanel items={queue} onCancel={cancelItem} onClear={clearFinished} />

      {pendingOverwrite && (
        <div className="overlay" onClick={() => setPendingOverwrite(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="mhead">
              <h3>{t.overwriteTitle}</h3>
              <button className="ghost" onClick={() => setPendingOverwrite(null)}>
                ×
              </button>
            </div>
            <div className="mbody">
              <p>{t.overwriteBody(pendingOverwrite.name)}</p>
            </div>
            <div className="mfoot">
              <button onClick={() => setPendingOverwrite(null)}>{t.cancel}</button>
              <button
                className="primary"
                autoFocus
                onClick={() => {
                  const item = pendingOverwrite;
                  setPendingOverwrite(null);
                  setQueue((qs) => [...qs, { ...item, overwrite: true }]);
                }}
              >
                {t.overwriteBtn}
              </button>
            </div>
          </div>
        </div>
      )}

      {showUpdateModal && update !== 'unchecked' && update !== null && (
        <UpdateModal info={update} onToast={toast} onClose={() => setShowUpdateModal(false)} />
      )}

      <Help />
      {toastMsg && <div className={`toast${toastMsg.err ? ' error' : ''}`}>{toastMsg.msg}</div>}
    </div>
  );
}
