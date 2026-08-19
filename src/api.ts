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

export const api = {
  checkUpdate: () => invoke<UpdateInfo | null>('check_update'),
  installUpdate: () => invoke<void>('install_update'),

  initialDir: (conn: number) => invoke<string>('fs_initial_dir', { conn }),
  list: (conn: number, path: string) => invoke<Entry[]>('fs_list', { conn, path }),
  mkdir: (conn: number, path: string) => invoke<void>('fs_mkdir', { conn, path }),
  remove: (conn: number, path: string, isDir: boolean) =>
    invoke<void>('fs_remove', { conn, path, isDir }),
  rename: (conn: number, from: string, to: string) => invoke<void>('fs_rename', { conn, from, to }),
};
