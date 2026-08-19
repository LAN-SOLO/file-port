import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { api, LOCAL_CONN, UpdateInfo } from './api';
import { t } from './i18n';
import FilePane from './components/FilePane';
import RemotePane from './components/RemotePane';
import UpdateModal from './components/UpdateModal';

export default function App() {
  const [version, setVersion] = useState('');
  const [update, setUpdate] = useState<UpdateInfo | null | 'unchecked'>('unchecked');
  const [checking, setChecking] = useState(false);
  const [showUpdateModal, setShowUpdateModal] = useState(false);
  const [toastMsg, setToastMsg] = useState<{ msg: string; err: boolean } | null>(null);
  const toastTimer = useRef<number>(0);

  const toast = useCallback((msg: string, isError = false) => {
    setToastMsg({ msg, err: isError });
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToastMsg(null), 5000);
  }, []);

  const onPaneError = useCallback((msg: string) => toast(msg, true), [toast]);

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

      <main className="panes">
        <FilePane conn={LOCAL_CONN} title={t.localPane} onError={onPaneError} />
        <RemotePane onError={onPaneError} onConnected={() => {}} paneProps={{ onError: onPaneError }} />
      </main>

      {showUpdateModal && update !== 'unchecked' && update !== null && (
        <UpdateModal info={update} onToast={toast} onClose={() => setShowUpdateModal(false)} />
      )}

      {toastMsg && <div className={`toast${toastMsg.err ? ' error' : ''}`}>{toastMsg.msg}</div>}
    </div>
  );
}
