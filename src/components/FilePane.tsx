import { useCallback, useEffect, useRef, useState } from 'react';
import { api, Entry } from '../api';
import { t } from '../i18n';
import { joinPath, parentPath } from '../paths';

function formatSize(n: number): string {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n;
  let u = -1;
  do {
    v /= 1024;
    u++;
  } while (v >= 1024 && u < units.length - 1);
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[u]}`;
}

function formatDate(secs: number | null): string {
  if (!secs) return '—';
  return new Date(secs * 1000).toLocaleString(undefined, {
    year: '2-digit',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
}

type Prompt =
  | { kind: 'newFolder' }
  | { kind: 'rename'; entry: Entry }
  | { kind: 'delete'; entry: Entry }
  | null;

export interface FilePaneProps {
  conn: number;
  /** Kopfzeile der Pane, z. B. „Lokal" oder das Verbindungs-Label. */
  title: string;
  onError: (msg: string) => void;
  /** Aktuelle Auswahl nach außen melden (für Transfers). */
  onSelect?: (entry: Entry | null, dir: string) => void;
  /** Von außen angestoßenes Neuladen (z. B. nach einem Transfer). */
  reloadKey?: number;
  /** Zusätzliche Kopfzeilen-Aktion (z. B. „Trennen" bei der Gegenstelle). */
  headExtra?: React.ReactNode;
  /** Doppelklick auf eine Datei (Ordner navigieren immer). */
  onFileActivate?: (entry: Entry) => void;
}

export default function FilePane({
  conn,
  title,
  onError,
  onSelect,
  reloadKey,
  headExtra,
  onFileActivate,
}: FilePaneProps) {
  const [dir, setDir] = useState<string>('');
  const [pathInput, setPathInput] = useState('');
  const [entries, setEntries] = useState<Entry[]>([]);
  const [selected, setSelected] = useState<Entry | null>(null);
  const [loading, setLoading] = useState(false);
  const [prompt, setPrompt] = useState<Prompt>(null);
  const [promptValue, setPromptValue] = useState('');
  const promptInput = useRef<HTMLInputElement>(null);

  const load = useCallback(
    async (path: string) => {
      setLoading(true);
      try {
        const list = await api.list(conn, path);
        list.sort((a, b) =>
          a.kind === b.kind ? a.name.localeCompare(b.name) : a.kind === 'dir' ? -1 : 1
        );
        setEntries(list);
        setDir(path);
        setPathInput(path);
        setSelected(null);
        onSelect?.(null, path);
      } catch (err) {
        onError(String(err));
      } finally {
        setLoading(false);
      }
    },
    [conn, onError, onSelect]
  );

  useEffect(() => {
    api
      .initialDir(conn)
      .then((home) => load(home))
      .catch((err) => onError(String(err)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conn]);

  useEffect(() => {
    if (reloadKey && dir) load(dir);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reloadKey]);

  useEffect(() => {
    if (prompt && prompt.kind !== 'delete') promptInput.current?.focus();
  }, [prompt]);

  const select = (e: Entry) => {
    setSelected(e);
    onSelect?.(e, dir);
  };

  const open = (e: Entry) => {
    if (e.kind === 'dir') load(e.path);
    else onFileActivate?.(e);
  };

  const submitPrompt = async () => {
    if (!prompt) return;
    try {
      if (prompt.kind === 'newFolder') {
        const name = promptValue.trim();
        if (!name) return;
        await api.mkdir(conn, joinPath(dir, name));
      } else if (prompt.kind === 'rename') {
        const name = promptValue.trim();
        if (!name || name === prompt.entry.name) return setPrompt(null);
        await api.rename(conn, prompt.entry.path, joinPath(dir, name));
      } else if (prompt.kind === 'delete') {
        await api.remove(conn, prompt.entry.path, prompt.entry.kind === 'dir');
      }
      setPrompt(null);
      setPromptValue('');
      await load(dir);
    } catch (err) {
      setPrompt(null);
      onError(String(err));
    }
  };

  return (
    <section className="pane">
      <div className="pane-head">
        <span className="pane-title">{title}</span>
        <span className="pane-count">{t.entries(entries.length)}</span>
        {headExtra}
      </div>

      <div className="pane-toolbar">
        <button className="icon" title={t.up} onClick={() => load(parentPath(dir))}>
          ↑
        </button>
        <input
          className="pathbar"
          value={pathInput}
          onChange={(e) => setPathInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && load(pathInput.trim() || '/')}
          spellCheck={false}
        />
        <button className="icon" title={t.refresh} onClick={() => load(dir)} disabled={loading}>
          ⟳
        </button>
      </div>

      <div className="pane-list" onClick={() => { setSelected(null); onSelect?.(null, dir); }}>
        <table>
          <thead>
            <tr>
              <th>{t.colName}</th>
              <th className="num">{t.colSize}</th>
              <th className="num">{t.colModified}</th>
            </tr>
          </thead>
          <tbody>
            {entries.map((e) => (
              <tr
                key={e.path}
                className={selected?.path === e.path ? 'sel' : ''}
                onClick={(ev) => {
                  ev.stopPropagation();
                  select(e);
                }}
                onDoubleClick={() => open(e)}
              >
                <td>
                  <span className={`fico ${e.kind}`} aria-hidden="true">
                    {e.kind === 'dir' ? (
                      <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
                        <path d="M1.5 3.5A1.5 1.5 0 0 1 3 2h3.2c.4 0 .8.16 1.06.44l.9.9c.1.11.24.16.38.16H13a1.5 1.5 0 0 1 1.5 1.5v7A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12v-8.5Z" />
                      </svg>
                    ) : (
                      <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
                        <path d="M4 1.5A1.5 1.5 0 0 0 2.5 3v10A1.5 1.5 0 0 0 4 14.5h8a1.5 1.5 0 0 0 1.5-1.5V5.9c0-.4-.16-.78-.44-1.06l-2.9-2.9A1.5 1.5 0 0 0 9.1 1.5H4Zm5.5 1.2 2.8 2.8H10a.5.5 0 0 1-.5-.5V2.7Z" />
                      </svg>
                    )}
                  </span>
                  {e.name}
                </td>
                <td className="num">{e.kind === 'dir' ? '—' : formatSize(e.size)}</td>
                <td className="num">{formatDate(e.modified)}</td>
              </tr>
            ))}
            {entries.length === 0 && !loading && (
              <tr className="empty">
                <td colSpan={3}>{t.emptyDir}</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="pane-actions">
        <button
          onClick={() => {
            setPromptValue('');
            setPrompt({ kind: 'newFolder' });
          }}
        >
          {t.newFolder}
        </button>
        <button
          disabled={!selected}
          onClick={() => {
            if (!selected) return;
            setPromptValue(selected.name);
            setPrompt({ kind: 'rename', entry: selected });
          }}
        >
          {t.rename}
        </button>
        <button
          className="danger"
          disabled={!selected}
          onClick={() => selected && setPrompt({ kind: 'delete', entry: selected })}
        >
          {t.del}
        </button>
      </div>

      {prompt && (
        <div className="pane-prompt">
          {prompt.kind === 'delete' ? (
            <span className="prompt-text">
              {prompt.entry.kind === 'dir'
                ? t.deleteConfirmDir(prompt.entry.name)
                : t.deleteConfirm(prompt.entry.name)}
            </span>
          ) : (
            <input
              ref={promptInput}
              value={promptValue}
              placeholder={prompt.kind === 'newFolder' ? t.newFolderPrompt : t.renamePrompt}
              onChange={(e) => setPromptValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') submitPrompt();
                if (e.key === 'Escape') setPrompt(null);
              }}
              spellCheck={false}
            />
          )}
          <button className="primary" onClick={submitPrompt}>
            {prompt.kind === 'delete' ? t.del : t.ok}
          </button>
          <button onClick={() => setPrompt(null)}>{t.cancel}</button>
        </div>
      )}
    </section>
  );
}
