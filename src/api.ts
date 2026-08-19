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

export type Protocol = 'sftp' | 'ftp' | 'ftps' | 'webdav' | 's3';

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
  base_url: string;
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
    base_url: '',
    endpoint: '',
    region: '',
    bucket: '',
    access_key: '',
    path_style: false,
    accept_invalid_certs: false,
  };
}

export interface ConnectResult {
  conn: number;
  label: string;
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
  saveProfile: (profile: Profile, secret: string | null) =>
    invoke<Profile>('profile_save', { profile, secret }),
  deleteProfile: (id: string) => invoke<void>('profile_delete', { id }),
  connect: (id: string) => invoke<ConnectResult>('connect', { id }),
  disconnect: (conn: number) => invoke<void>('disconnect', { conn }),
};
