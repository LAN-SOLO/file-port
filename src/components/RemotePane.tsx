import { useEffect, useState } from 'react';
import { api, emptyProfile, HostKeyPrompt, Profile, Protocol } from '../api';
import { t } from '../i18n';
import FilePane, { FilePaneProps } from './FilePane';

/** Nur verschlüsselte Protokolle — Klartext-FTP gibt es bewusst nicht. */
const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: 'sftp', label: t.pSftp },
  { value: 'scp', label: t.pScp },
  { value: 'rsync', label: t.pRsync },
  { value: 'ftps', label: t.pFtps },
  { value: 'ftps_implicit', label: t.pFtpsImplicit },
  { value: 'webdav', label: t.pWebdav },
  { value: 'smb', label: t.pSmb },
  { value: 's3', label: t.pS3 },
  { value: 'azure', label: t.pAzure },
  { value: 'gcs', label: t.pGcs },
];

/** SHA256-Fingerprint eines OpenSSH-Public-Keys (wie `ssh-keygen -lf`). */
async function fingerprintOf(openssh: string): Promise<string> {
  try {
    const b64 = openssh.trim().split(/\s+/)[1] ?? '';
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
    let bin = '';
    digest.forEach((b) => (bin += String.fromCharCode(b)));
    return 'SHA256:' + btoa(bin).replace(/=+$/, '');
  } catch {
    return openssh.trim().split(/\s+/)[0] ?? '';
  }
}

/** Alte „ftp"-Profile im Formular als FTPS führen — verbunden wird ohnehin nur per TLS. */
const normalizeProtocol = (p: Protocol): Protocol => (p === 'ftp' ? 'ftps' : p);

interface RemotePaneProps {
  onError: (msg: string) => void;
  onInfo: (msg: string) => void;
  onConnected: (conn: number | null) => void;
  paneProps: Omit<FilePaneProps, 'conn' | 'title'>;
}

/**
 * Rechte Seite des Zwei-Fenster-Layouts: Verbindungs-Manager solange
 * getrennt, danach die Datei-Ansicht der Gegenstelle.
 */
export default function RemotePane({ onError, onInfo, onConnected, paneProps }: RemotePaneProps) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [form, setForm] = useState<Profile | null>(null);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [session, setSession] = useState<{ conn: number; label: string } | null>(null);
  const [clearSecret, setClearSecret] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<Profile | null>(null);
  const [hostKeyAsk, setHostKeyAsk] = useState<{ id: string; host: string; prompt: HostKeyPrompt } | null>(null);
  const [storedFp, setStoredFp] = useState('');

  const loadProfiles = () => {
    api.profiles().then(setProfiles).catch((err) => onError(String(err)));
  };

  useEffect(loadProfiles, []);

  const doConnect = async (id: string, trustHostKey: string | null = null, hostHint = '') => {
    setBusy(id);
    try {
      const result = await api.connect(id, trustHostKey);
      if (result.host_key) {
        // Unbekannter oder geänderter SSH-Host-Key: erst bestätigen lassen
        const host = profiles.find((p) => p.id === id)?.host || hostHint;
        setHostKeyAsk({ id, host, prompt: result.host_key });
      } else if (result.conn !== null) {
        setSession({ conn: result.conn, label: result.label });
        onConnected(result.conn);
      }
      loadProfiles();
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  };

  const doDisconnect = async () => {
    if (session) {
      await api.disconnect(session.conn).catch(() => {});
      setSession(null);
      onConnected(null);
    }
  };

  const submitForm = async (connectAfter: boolean) => {
    if (!form) return;
    if (!form.name.trim()) return onError(t.fName + '?');
    setBusy('form');
    try {
      const saved = await api.saveProfile(form, secret || null, clearSecret && !secret);
      setForm(null);
      setSecret('');
      setClearSecret(false);
      loadProfiles();
      if (connectAfter) await doConnect(saved.id, null, saved.host);
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  };

  const removeProfile = async (id: string) => {
    try {
      await api.deleteProfile(id);
      if (form?.id === id) setForm(null);
      loadProfiles();
    } catch (err) {
      onError(String(err));
    }
  };

  const forgetHostKey = async () => {
    if (!form?.id) return;
    try {
      await api.resetHostKey(form.id);
      setForm({ ...form, host_key: '' });
      setStoredFp('');
      loadProfiles();
      onInfo(t.hostKeyForgotten);
    } catch (err) {
      onError(String(err));
    }
  };

  // Fingerprint des gemerkten Host-Keys im Bearbeiten-Formular
  useEffect(() => {
    let alive = true;
    if (form?.host_key) fingerprintOf(form.host_key).then((fp) => alive && setStoredFp(fp));
    else setStoredFp('');
    return () => {
      alive = false;
    };
  }, [form?.host_key]);

  if (session) {
    return (
      <FilePane
        conn={session.conn}
        title={session.label}
        headExtra={
          <button className="small" onClick={doDisconnect}>
            {t.disconnect}
          </button>
        }
        {...paneProps}
      />
    );
  }

  const f = form;
  const set = (patch: Partial<Profile>) => f && setForm({ ...f, ...patch });
  const isS3 = f?.protocol === 's3';
  const isDav = f?.protocol === 'webdav';
  const isSsh = f?.protocol === 'sftp' || f?.protocol === 'scp' || f?.protocol === 'rsync';
  const isRsync = f?.protocol === 'rsync';
  const isSmb = f?.protocol === 'smb';
  const isFtps = f?.protocol === 'ftps' || f?.protocol === 'ftps_implicit';
  const isAzure = f?.protocol === 'azure';
  const isGcs = f?.protocol === 'gcs';
  const secretLabel = isS3
    ? t.fSecretKey
    : isAzure
      ? t.fAzureKey
      : isSsh && f?.key_file
        ? t.fPassphrase
        : t.fPassword;

  return (
    <section className="pane">
      <div className="pane-head">
        <span className="pane-title">{t.remoteTitle}</span>
        <span className="pane-count">{t.notConnected}</span>
      </div>

      <div className="connect-view">
        <div className="fieldlabel">{t.savedConnections}</div>
        {profiles.length === 0 && <p className="faint">{t.noProfiles}</p>}
        {profiles.map((p) => (
          <div className="profile-row" key={p.id}>
            <span className="proto-badge">{normalizeProtocol(p.protocol)}</span>
            <span className="profile-name">{p.name}</span>
            <span className="profile-target">
              {p.protocol === 'webdav'
                ? p.base_url
                : p.protocol === 's3' || p.protocol === 'gcs'
                  ? p.bucket
                  : p.protocol === 'azure'
                    ? `${p.account}/${p.bucket}`
                    : p.protocol === 'smb'
                      ? `${p.host}/${p.share}`
                      : p.host}
            </span>
            <button
              className="primary small"
              disabled={busy !== null}
              onClick={() => doConnect(p.id)}
            >
              {busy === p.id ? t.connecting : t.connectBtn}
            </button>
            <button
              className="small"
              title={t.edit}
              onClick={() => {
                setForm({ ...p, protocol: normalizeProtocol(p.protocol) });
                setSecret('');
                setClearSecret(false);
              }}
            >
              ✎
            </button>
            <button className="small danger" title={t.del} onClick={() => setConfirmDelete(p)}>
              ✕
            </button>
          </div>
        ))}

        {confirmDelete && (
          <div className="pane-prompt">
            <span className="prompt-text">{t.deleteProfileConfirm(confirmDelete.name)}</span>
            <button
              className="primary"
              onClick={() => {
                const id = confirmDelete.id;
                setConfirmDelete(null);
                removeProfile(id);
              }}
            >
              {t.del}
            </button>
            <button onClick={() => setConfirmDelete(null)}>{t.cancel}</button>
          </div>
        )}

        {!f ? (
          <button
            className="mt"
            onClick={() => {
              setForm(emptyProfile());
              setSecret('');
              setClearSecret(false);
            }}
          >
            {t.newConnection}
          </button>
        ) : (
          <div className="conn-form">
            <div className="fieldlabel">{f.id ? t.editConnection : t.newConnection}</div>

            <div className="frow">
              <label>
                {t.fName}
                <input value={f.name} onChange={(e) => set({ name: e.target.value })} spellCheck={false} />
              </label>
              <label>
                {t.fProtocol}
                <select
                  value={f.protocol}
                  onChange={(e) => set({ protocol: e.target.value as Protocol })}
                >
                  {PROTOCOLS.map((p) => (
                    <option key={p.value} value={p.value}>
                      {p.label}
                    </option>
                  ))}
                </select>
              </label>
            </div>

            {isDav && (
              <label>
                {t.fBaseUrl}
                <input value={f.base_url} onChange={(e) => set({ base_url: e.target.value })} spellCheck={false} />
              </label>
            )}

            {isS3 && (
              <>
                <label>
                  {t.fEndpoint}
                  <input value={f.endpoint} onChange={(e) => set({ endpoint: e.target.value })} spellCheck={false} />
                </label>
                <div className="frow">
                  <label>
                    {t.fRegion}
                    <input value={f.region} placeholder="us-east-1" onChange={(e) => set({ region: e.target.value })} spellCheck={false} />
                  </label>
                  <label>
                    {t.fBucket}
                    <input value={f.bucket} onChange={(e) => set({ bucket: e.target.value })} spellCheck={false} />
                  </label>
                </div>
                <label>
                  {t.fAccessKey}
                  <input value={f.access_key} onChange={(e) => set({ access_key: e.target.value })} spellCheck={false} />
                </label>
              </>
            )}

            {isAzure && (
              <div className="frow">
                <label>
                  {t.fAccount}
                  <input value={f.account} onChange={(e) => set({ account: e.target.value })} spellCheck={false} />
                </label>
                <label>
                  {t.fContainer}
                  <input value={f.bucket} onChange={(e) => set({ bucket: e.target.value })} spellCheck={false} />
                </label>
              </div>
            )}

            {isGcs && (
              <>
                <label>
                  {t.fBucket}
                  <input value={f.bucket} onChange={(e) => set({ bucket: e.target.value })} spellCheck={false} />
                </label>
                <label>
                  {t.fGcsKeyFile}
                  <input value={f.key_file} onChange={(e) => set({ key_file: e.target.value })} spellCheck={false} />
                </label>
              </>
            )}

            {!isDav && !isS3 && !isAzure && !isGcs && (
              <div className="frow">
                <label>
                  {t.fHost}
                  <input value={f.host} onChange={(e) => set({ host: e.target.value })} spellCheck={false} />
                </label>
                <label className="short">
                  {t.fPort}
                  <input
                    value={f.port || ''}
                    placeholder={
                      isSsh ? '22' : isSmb ? '445' : f.protocol === 'ftps_implicit' ? '990' : '21'
                    }
                    onChange={(e) => set({ port: parseInt(e.target.value, 10) || 0 })}
                    spellCheck={false}
                  />
                </label>
              </div>
            )}

            {isSmb && (
              <label>
                {t.fShare}
                <input value={f.share} onChange={(e) => set({ share: e.target.value })} spellCheck={false} />
              </label>
            )}

            {!isS3 && !isAzure && !isGcs && (
              <label>
                {t.fUser}
                <input value={f.user} onChange={(e) => set({ user: e.target.value })} spellCheck={false} autoCapitalize="off" />
              </label>
            )}

            {isSsh && (
              <label>
                {isRsync ? t.fKeyFileRequired : t.fKeyFile}
                <input value={f.key_file} placeholder="~/.ssh/id_ed25519" onChange={(e) => set({ key_file: e.target.value })} spellCheck={false} />
              </label>
            )}
            {isRsync && <div className="faint">{t.rsyncKeyNote}</div>}

            {isSsh && f.id && (
              <div className="hostkey-row">
                <span className="fieldlabel">{t.hostKeyStored}</span>
                {f.host_key ? (
                  <>
                    <code className="fp">{storedFp}</code>
                    <button className="small" onClick={forgetHostKey}>
                      {t.hostKeyForget}
                    </button>
                  </>
                ) : (
                  <span className="faint">{t.hostKeyNone}</span>
                )}
              </div>
            )}

            {!isGcs && (
              <label>
                {secretLabel}
                <input
                  type="password"
                  value={secret}
                  placeholder={f.id ? t.secretKeptNote : ''}
                  onChange={(e) => setSecret(e.target.value)}
                />
              </label>
            )}

            {!isGcs && f.id && !secret && (
              <label className="check">
                <input type="checkbox" checked={clearSecret} onChange={(e) => setClearSecret(e.target.checked)} />
                {t.clearSecret}
              </label>
            )}

            {isS3 && (
              <label className="check">
                <input type="checkbox" checked={f.path_style} onChange={(e) => set({ path_style: e.target.checked })} />
                {t.fPathStyle}
              </label>
            )}
            {(isDav || isFtps || isS3) && (
              <label className="check">
                <input
                  type="checkbox"
                  checked={f.accept_invalid_certs}
                  onChange={(e) => set({ accept_invalid_certs: e.target.checked })}
                />
                {t.fInvalidCerts}
              </label>
            )}

            <div className="faint">{t.secretStoreNote}</div>

            <div className="form-actions">
              <button className="primary" disabled={busy !== null} onClick={() => submitForm(true)}>
                {busy === 'form' ? t.connecting : t.saveAndConnect}
              </button>
              <button disabled={busy !== null} onClick={() => submitForm(false)}>
                {t.save}
              </button>
              <button onClick={() => setForm(null)}>{t.cancel}</button>
            </div>
          </div>
        )}
      </div>

      {hostKeyAsk && (
        <div className="overlay" onClick={() => setHostKeyAsk(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <div className="mhead">
              <h3 className={hostKeyAsk.prompt.changed ? 'warn-text' : ''}>
                {hostKeyAsk.prompt.changed ? t.hostKeyChangedTitle : t.hostKeyNewTitle}
              </h3>
              <button className="ghost" onClick={() => setHostKeyAsk(null)}>
                ×
              </button>
            </div>
            <div className="mbody">
              <p>
                {hostKeyAsk.prompt.changed
                  ? t.hostKeyChangedBody(hostKeyAsk.host)
                  : t.hostKeyNewBody(hostKeyAsk.host)}
              </p>
              <div className="fieldlabel">{t.hostKeyFingerprint}</div>
              <code className="fp block">{hostKeyAsk.prompt.fingerprint}</code>
              {hostKeyAsk.prompt.old_fingerprint && (
                <>
                  <div className="fieldlabel">{t.hostKeyOld}</div>
                  <code className="fp block">{hostKeyAsk.prompt.old_fingerprint}</code>
                </>
              )}
            </div>
            <div className="mfoot">
              <button autoFocus onClick={() => setHostKeyAsk(null)}>
                {t.cancel}
              </button>
              <button
                className={hostKeyAsk.prompt.changed ? 'danger' : 'primary'}
                disabled={busy !== null}
                onClick={() => {
                  const ask = hostKeyAsk;
                  setHostKeyAsk(null);
                  doConnect(ask.id, ask.prompt.key, ask.host);
                }}
              >
                {hostKeyAsk.prompt.changed ? t.hostKeyTrustChanged : t.hostKeyTrust}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
