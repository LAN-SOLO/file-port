import { useState } from 'react';
import { api, UpdateInfo } from '../api';
import { t } from '../i18n';

interface Props {
  info: UpdateInfo;
  onToast: (msg: string, isError?: boolean) => void;
  onClose: () => void;
}

/** Shown before installing an update: version, date and the changelog from
 *  the release notes. Install only starts after explicit confirmation. */
export default function UpdateModal({ info, onToast, onClose }: Props) {
  const [busy, setBusy] = useState(false);

  const install = async () => {
    setBusy(true);
    onToast(t.updateInstalling);
    try {
      await api.installUpdate();
      // on success the app restarts — this line is never reached
    } catch (err) {
      onToast(`${t.updateFailed}: ${String(err)}`, true);
      setBusy(false);
    }
  };

  const date = info.date ? new Date(info.date) : null;

  return (
    <div className="overlay" onClick={busy ? undefined : onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="mhead">
          <h3>{t.updateModalTitle(info.version)}</h3>
          <button className="ghost" onClick={onClose} disabled={busy}>
            ×
          </button>
        </div>
        <div className="mbody">
          {date && !Number.isNaN(date.getTime()) && (
            <div className="faint">{t.publishedOn(date.toLocaleDateString())}</div>
          )}
          <div className="fieldlabel">{t.changelogTitle}</div>
          <div className="changelog-box">
            {info.notes?.trim() ? info.notes.trim() : t.noChangelog}
          </div>
          <div className="faint safenote">{t.updateSafeNote}</div>
        </div>
        <div className="mfoot">
          <button onClick={onClose} disabled={busy}>
            {t.updateLater}
          </button>
          <button className="primary" onClick={install} disabled={busy}>
            {busy ? '…' : t.updateNow}
          </button>
        </div>
      </div>
    </div>
  );
}
