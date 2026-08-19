import { t } from '../i18n';

export type QStatus = 'queued' | 'starting' | 'running' | 'done' | 'error' | 'cancelled';

export interface QItem {
  key: number;
  tid?: number;
  direction: 'download' | 'upload';
  name: string;
  status: QStatus;
  done: number;
  total: number | null;
  error?: string;
  /** Transfer-Parameter für den sequenziellen Abarbeiter. */
  conn: number;
  remote: string;
  local: string;
}

function pct(item: QItem): number {
  if (item.status === 'done') return 100;
  if (!item.total) return 0;
  return Math.min(100, Math.round((item.done / item.total) * 100));
}

export default function QueuePanel({
  items,
  onCancel,
  onClear,
}: {
  items: QItem[];
  onCancel: (item: QItem) => void;
  onClear: () => void;
}) {
  if (items.length === 0) return null;
  const finished = items.some((q) => q.status === 'done' || q.status === 'error' || q.status === 'cancelled');

  return (
    <footer className="queue">
      <div className="queue-head">
        <span className="pane-title">{t.queueTitle}</span>
        {finished && (
          <button className="small" onClick={onClear}>
            {t.clearFinished}
          </button>
        )}
      </div>
      <div className="queue-list">
        {items.map((item) => (
          <div className={`qrow ${item.status}`} key={item.key}>
            <span className="qdir" aria-hidden="true">
              {item.direction === 'upload' ? '→' : '←'}
            </span>
            <span className="qname">{item.name}</span>
            <span className="qstatus">
              {item.status === 'queued' && t.qQueued}
              {(item.status === 'running' || item.status === 'starting') && `${pct(item)} %`}
              {item.status === 'done' && t.qDone}
              {item.status === 'error' && (item.error ?? t.qError)}
              {item.status === 'cancelled' && t.qCancelled}
            </span>
            <div className="qbar">
              <div className="qfill" style={{ width: `${pct(item)}%` }} />
            </div>
            {(item.status === 'queued' || item.status === 'running' || item.status === 'starting') && (
              <button className="icon" title={t.cancel} onClick={() => onCancel(item)}>
                ✕
              </button>
            )}
          </div>
        ))}
      </div>
    </footer>
  );
}
