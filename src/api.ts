import { invoke } from '@tauri-apps/api/core';

/** Update found on GitHub Releases (latest.json), incl. the changelog notes. */
export interface UpdateInfo {
  version: string;
  notes: string | null;
  date: string | null;
}

/** Ein Verzeichniseintrag, wie ihn jedes Backend liefert (fileport-core). */
export interface Entry {
  name: string;
  path: string;
  kind: 'dir' | 'file' | 'symlink';
  size: number;
  modified: number | null;
}

/** Verbindung 0 ist immer das lokale Dateisystem. */
export const LOCAL_CONN = 0;

/** `ftp` existiert nur noch in alten Profilen — verbindet immer per AUTH TLS. */
export type Protocol =
  | 'sftp'
  | 'scp'
  | 'rsync'
  | 'ftp'
  | 'ftps'
  | 'ftps_implicit'
  | 'webdav'
  | 'smb'
  | 's3'
  | 'azure'
  | 'gcs';

/** Verbindungs-Profil — Geheimnisse liegen im OS-Schlüsselbund, nie hier. */
export interface Profile {
  id: string;
  name: string;
  protocol: Protocol;
  host: string;
  port: number;
  user: string;
  key_file: string;
  host_key: string;
  share: string;
  base_url: string;
  account: string;
  endpoint: string;
  region: string;
  bucket: string;
  access_key: string;
  path_style: boolean;
  accept_invalid_certs: boolean;
}

export function emptyProfile(protocol: Protocol = 'sftp'): Profile {
  return {
    id: '',
    name: '',
    protocol,
    host: '',
    port: 0,
    user: '',
    key_file: '',
    host_key: '',
    share: '',
    base_url: '',
    account: '',
    endpoint: '',
    region: '',
    bucket: '',
    access_key: '',
    path_style: false,
    accept_invalid_certs: false,
  };
}

/** Rückfrage zu einem SSH-Host-Key, bevor Zugangsdaten an den Server gehen. */
export interface HostKeyPrompt {
  /** SHA256-Fingerprint (wie `ssh-keygen -lf`). */
  fingerprint: string;
  /** Der Key selbst — bei Bestätigung als `trustHostKey` zurückgeben. */
  key: string;
  /** true: Der Server zeigt einen anderen Key als den gespeicherten. */
  changed: boolean;
  old_fingerprint: string | null;
}

/** Entweder offene Verbindung (`conn`) oder Host-Key-Rückfrage (`host_key`). */
export interface ConnectResult {
  conn: number | null;
  label: string;
  host_key: HostKeyPrompt | null;
}

export const api = {
  checkUpdate: () => invoke<UpdateInfo | null>('check_update'),
  installUpdate: () => invoke<void>('install_update'),

  initialDir: (conn: number) => invoke<string>('fs_initial_dir', { conn }),
  list: (conn: number, path: string) => invoke<Entry[]>('fs_list', { conn, path }),
  mkdir: (conn: number, path: string) => invoke<void>('fs_mkdir', { conn, path }),
  remove: (conn: number, path: string, isDir: boolean) =>
    invoke<void>('fs_remove', { conn, path, isDir }),
  rename: (conn: number, from: string, to: string) => invoke<void>('fs_rename', { conn, from, to }),

  profiles: () => invoke<Profile[]>('profiles_list'),
  saveProfile: (profile: Profile, secret: string | null, clearSecret = false) =>
    invoke<Profile>('profile_save', { profile, secret, clearSecret }),
  deleteProfile: (id: string) => invoke<void>('profile_delete', { id }),
  resetHostKey: (id: string) => invoke<void>('profile_reset_host_key', { id }),
  connect: (id: string, trustHostKey: string | null = null) =>
    invoke<ConnectResult>('connect', { id, trustHostKey }),
  disconnect: (conn: number) => invoke<void>('disconnect', { conn }),

  /** `id` vergibt die UI (Warteschlangen-Schlüssel) — Events tragen dieselbe ID. */
  transferStart: (
    id: number,
    direction: 'download' | 'upload',
    conn: number,
    remote: string,
    local: string
  ) => invoke<number>('transfer_start', { id, direction, conn, remote, local }),
  transferCancel: (id: number) => invoke<void>('transfer_cancel', { id }),
};

/** Event-Payloads der Transfer-Engine (Tauri-Events). */
export interface TransferProgressEvent {
  id: number;
  done: number;
  total: number | null;
}

export interface TransferDoneEvent {
  id: number;
  ok: boolean;
  error: string | null;
  bytes: number;
  blake3: string | null;
  cancelled: boolean;
}
