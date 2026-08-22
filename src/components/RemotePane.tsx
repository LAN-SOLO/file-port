import { useEffect, useState } from 'react';
import { api, emptyProfile, Profile, Protocol } from '../api';
import { t } from '../i18n';
import FilePane, { FilePaneProps } from './FilePane';

/** Nur verschlüsselte Protokolle — Klartext-FTP gibt es bewusst nicht. */
const PROTOCOLS: { value: Protocol; label: string }[] = [
  { value: 'sftp', label: 'SFTP (SSH)' },
  { value: 'scp', label: 'SCP (SSH)' },
  { value: 'rsync', label: 'rsync über SSH' },
  { value: 'ftps', label: 'FTPS (explizit, AUTH TLS)' },
  { value: 'ftps_implicit', label: 'FTPS (implizit, Port 990)' },
  { value: 'webdav', label: 'WebDAV (HTTPS)' },
  { value: 'smb', label: 'SMB 2/3 (Windows/NAS)' },
  { value: 's3', label: 'S3 (AWS & kompatible)' },
  { value: 'azure', label: 'Azure Blob Storage' },
  { value: 'gcs', label: 'Google Cloud Storage' },
];

/** Alte „ftp"-Profile im Formular als FTPS führen — verbunden wird ohnehin nur per TLS. */
const normalizeProtocol = (p: Protocol): Protocol => (p === 'ftp' ? 'ftps' : p);

interface RemotePaneProps {
  onError: (msg: string) => void;
  onConnected: (conn: number | null) => void;
  paneProps: Omit<FilePaneProps, 'conn' | 'title'>;
}

/**
 * Rechte Seite des Zwei-Fenster-Layouts: Verbindungs-Manager solange
 * getrennt, danach die Datei-Ansicht der Gegenstelle.
 */
export default function RemotePane({ onError, onConnected, paneProps }: RemotePaneProps) {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [form, setForm] = useState<Profile | null>(null);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [session, setSession] = useState<{ conn: number; label: string } | null>(null);

  const loadProfiles = () => {
    api.profiles().then(setProfiles).catch((err) => onError(String(err)));
  };

  useEffect(loadProfiles, []);

  const doConnect = async (id: string) => {
    setBusy(id);
    try {
      const result = await api.connect(id);
      setSession(result);
      onConnected(result.conn);
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
      const saved = await api.saveProfile(form, secret || null);
      setForm(null);
      setSecret('');
      loadProfiles();
      if (connectAfter) await doConnect(saved.id);
    } catch (err) {
      onError(String(err));
    } finally {
      setBusy(null);
    }
  };

  const removeProfile = async (id: string) => {
    try {
      await api.deleteProfile(id);
      loadProfiles();
    } catch (err) {
      onError(String(err));
    }
  };

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
              }}
            >
              ✎
            </button>
            <button className="small danger" title={t.del} onClick={() => removeProfile(p.id)}>
              ✕
            </button>
          </div>
        ))}

        {!f ? (
          <button className="mt" onClick={() => setForm(emptyProfile())}>
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
    </section>
  );
}
