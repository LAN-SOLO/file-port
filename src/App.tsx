import { useCallback, useEffect, useRef, useState } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { api, UpdateInfo } from './api';
import { t } from './i18n';
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
    toastTimer.current = window.setTimeout(() => setToastMsg(null), 4000);
  }, []);

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
    } catch (err) {
      toast(String(err), true);
    } finally {
      setChecking(false);
    }
  };

  return (
    <div className="shell">
      <header>
        <h1>
          <span className="brand">file/port</span>
          <span className="dot">.</span>
        </h1>
        <div className="subtitle">{t.subtitle}</div>
        {version && <div className="version">v{version}</div>}
      </header>

      <section className="card">
        <h2>{t.statusTitle}</h2>
        <p>{t.statusText}</p>
        <div className="updrow">
          {update !== 'unchecked' && update !== null ? (
            <button className="primary" onClick={() => setShowUpdateModal(true)}>
              {t.updateAvailable(update.version)} — {t.updateNow}
            </button>
          ) : (
            <button onClick={doCheckUpdate} disabled={checking}>
              {checking ? t.updateChecking : t.checkForUpdates}
            </button>
          )}
          {update === null && <span className="ok-text">{t.upToDate}</span>}
        </div>
        <div className="faint safenote">{t.updateSafeNote}</div>
      </section>

      {showUpdateModal && update !== 'unchecked' && update !== null && (
        <UpdateModal info={update} onToast={toast} onClose={() => setShowUpdateModal(false)} />
      )}

      {toastMsg && <div className={`toast${toastMsg.err ? ' error' : ''}`}>{toastMsg.msg}</div>}
    </div>
  );
}
