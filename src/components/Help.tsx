import { useState } from 'react';

// Selbstständiges Hilfe-System: schwebender ?-Button, First-Run-Tutorial
// und durchsuchbares Handbuch. Sprache folgt wie i18n.ts der Systemsprache.

interface Step {
  title: string;
  body: string[];
}

interface Section {
  id: string;
  title: string;
  body: string[];
}

interface Content {
  labels: {
    fab: string;
    tutorial: string;
    manual: string;
    search: string;
    next: string;
    back: string;
    skip: string;
    done: string;
    stepOf: (n: number, total: number) => string;
    noResults: string;
  };
  tutorial: Step[];
  sections: Section[];
}

const de: Content = {
  labels: {
    fab: 'Hilfe & Handbuch',
    tutorial: 'Tutorial',
    manual: 'Handbuch',
    search: 'Handbuch durchsuchen …',
    next: 'Weiter',
    back: 'Zurück',
    skip: 'Überspringen',
    done: 'Los geht’s',
    stepOf: (n, total) => `Schritt ${n} von ${total}`,
    noResults: 'Keine Treffer',
  },
  tutorial: [
    {
      title: 'Willkommen bei fileport.',
      body: [
        'fileport ist ein Transfer-Client mit zwei Fenstern: links dein Rechner, rechts der Server oder Cloud-Speicher.',
        'Unterstützt werden SFTP, FTP, FTPS, WebDAV und S3 — jede Verbindung sieht gleich aus.',
        'Dieses Tutorial dauert eine Minute. Du findest es jederzeit wieder über den ?-Knopf unten rechts.',
      ],
    },
    {
      title: 'Verbindung anlegen',
      body: [
        'Rechts auf „Neue Verbindung“ klicken, Protokoll wählen und die Zugangsdaten eintragen.',
        '• SFTP/FTP(S): Host, Port, Benutzer, Passwort — oder SSH-Schlüsseldatei mit Passphrase',
        '• WebDAV: die Basis-URL des Servers',
        '• S3: Endpoint, Region, Bucket, Access- und Secret-Key',
        '„Speichern & Verbinden“ legt das Profil an — beim nächsten Mal genügt ein Klick in der Profilliste.',
      ],
    },
    {
      title: 'Dateien übertragen',
      body: [
        'Datei links oder rechts auswählen, dann die Pfeile in der Mitte:',
        '• → lädt die lokale Datei zum Server hoch',
        '• ← holt die Server-Datei auf deinen Rechner',
        'Noch schneller: Doppelklick auf eine Datei überträgt sie direkt in die jeweils andere Seite.',
      ],
    },
    {
      title: 'Die Warteschlange',
      body: [
        'Alle Übertragungen sammeln sich unten in der Warteschlange und laufen nacheinander ab — mit Fortschritt pro Datei.',
        'Laufende Übertragungen lassen sich abbrechen; „Fertige aufräumen“ leert die Liste.',
      ],
    },
    {
      title: 'Dateien verwalten',
      body: [
        'Beide Fenster können mehr als anzeigen: neuer Ordner, umbenennen, löschen, aktualisieren — lokal wie auf dem Server.',
        'Navigation: Doppelklick auf Ordner öffnet sie, „..“ geht eine Ebene nach oben.',
      ],
    },
    {
      title: 'Updates',
      body: [
        'fileport prüft beim Start automatisch auf neue Versionen — der Changelog-Dialog öffnet sich, installiert wird erst nach deinem Klick.',
        'Updates kommen signiert von GitHub; deine Verbindungsprofile bleiben dabei erhalten.',
      ],
    },
  ],
  sections: [
    {
      id: 'ui',
      title: 'Oberfläche',
      body: [
        'fileport ist ein klassischer Zwei-Fenster-Client:',
        '• Links — dein lokaler Rechner: Ordner durchsuchen, Dateien auswählen',
        '• Mitte — die Transfer-Pfeile: → hochladen, ← herunterladen',
        '• Rechts — der Server: Profilliste, solange nichts verbunden ist; danach das Server-Dateifenster',
        '• Unten — die Warteschlange mit allen Übertragungen',
        'Beide Dateifenster zeigen Name, Größe und Änderungsdatum; Doppelklick öffnet Ordner, „..“ führt nach oben.',
      ],
    },
    {
      id: 'connections',
      title: 'Verbindungen & Profile',
      body: [
        '„Neue Verbindung“ öffnet das Profil-Formular. Die Felder hängen vom Protokoll ab:',
        '• SFTP — Host, Port (22), Benutzer, Passwort oder SSH-Schlüsseldatei (+ Passphrase)',
        '• FTP / FTPS — Host, Port (21), Benutzer, Passwort; FTPS verschlüsselt die Verbindung',
        '• WebDAV — Basis-URL (https://…), Benutzer, Passwort',
        '• S3 — Endpoint, Region, Bucket, Access Key, Secret Key; Path-Style für MinIO & Co.',
        'Für Server mit selbstsignierten Zertifikaten gibt es die Option, ungültige Zertifikate zu akzeptieren — nur einschalten, wenn du dem Server vertraust.',
        'Gespeicherte Profile erscheinen in der Liste rechts: verbinden per Klick, bearbeiten über das Stift-Symbol. Passwörter und Schlüssel bleiben lokal auf deinem Rechner und werden nie übertragen — außer an den Server, zu dem sie gehören.',
      ],
    },
    {
      id: 'transfers',
      title: 'Übertragungen',
      body: [
        'Drei Wege, eine Datei zu übertragen:',
        '• Datei auswählen und den Pfeil in der Mitte klicken (→ hochladen, ← herunterladen)',
        '• Doppelklick auf eine Datei — sie wandert direkt in die andere Seite',
        '• Mehrere Dateien nacheinander anstoßen — die Warteschlange arbeitet sie der Reihe nach ab',
        'Die Übertragung läuft sequenziell: genau ein Transfer zur Zeit, der Rest wartet. Das hält Verbindungen stabil und Server freundlich gestimmt.',
        'Nach einer erfolgreichen Übertragung aktualisiert sich die Zielseite automatisch.',
      ],
    },
    {
      id: 'queue',
      title: 'Warteschlange',
      body: [
        'Jede Übertragung erscheint unten mit Richtung, Namen und Fortschrittsbalken.',
        '• Status: wartend, läuft, fertig, Fehler, abgebrochen',
        '• Abbrechen — laufende und wartende Übertragungen lassen sich stoppen',
        '• „Fertige aufräumen“ — entfernt erledigte, fehlgeschlagene und abgebrochene Einträge aus der Liste',
        'Bei einem Fehler steht die Ursache direkt am Eintrag.',
      ],
    },
    {
      id: 'files',
      title: 'Dateiverwaltung',
      body: [
        'Beide Fenster bieten die Grundwerkzeuge — lokal wie auf dem Server:',
        '• Neuer Ordner — legt ein Verzeichnis im aktuellen Pfad an',
        '• Umbenennen — Datei oder Ordner auswählen, neuen Namen eingeben',
        '• Löschen — mit Sicherheitsabfrage; Ordner werden samt Inhalt entfernt',
        '• Aktualisieren — lädt die Ansicht neu',
      ],
    },
    {
      id: 'updates',
      title: 'Updates',
      body: [
        'fileport prüft bei jedem Start automatisch auf neue Versionen. Liegt eine bereit, öffnet sich der Update-Dialog mit dem Changelog — installiert wird erst nach deinem Klick.',
        'Manuell prüfen: „Nach Updates suchen“ oben rechts.',
        'Updates kommen signiert von GitHub (LAN-SOLO/file-port): Die App prüft die Signatur, bevor irgendetwas installiert wird. Profile und Einstellungen bleiben erhalten.',
      ],
    },
    {
      id: 'roadmap',
      title: 'Was noch kommt',
      body: [
        'fileport wächst per Update weiter. Auf der Liste stehen unter anderem:',
        '• Fortsetzen abgebrochener Übertragungen und Prüfsummen-Verifikation',
        '• Weitere Protokolle (SCP, rsync über SSH) und Cloud-Dienste',
        '• Direkt-Drop: Dateien Ende-zu-Ende-verschlüsselt von Gerät zu Gerät',
        'Später mit fileport bridged: Server-zu-Server-Brücke, AS2/OFTP2, Zeitpläne, Watch-Folder und Audit-Log.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privatsphäre',
      body: [
        'fileport läuft lokal: kein Konto, keine Telemetrie, keine Zwischenserver — Übertragungen gehen direkt von dir zum Zielserver.',
        'Zugangsdaten bleiben auf deinem Rechner. Die einzige weitere Netzwerkverbindung ist der Update-Check gegen GitHub.',
      ],
    },
  ],
};

const en: Content = {
  labels: {
    fab: 'Help & manual',
    tutorial: 'Tutorial',
    manual: 'Manual',
    search: 'Search the manual …',
    next: 'Next',
    back: 'Back',
    skip: 'Skip',
    done: 'Let’s go',
    stepOf: (n, total) => `Step ${n} of ${total}`,
    noResults: 'No matches',
  },
  tutorial: [
    {
      title: 'Welcome to fileport.',
      body: [
        'fileport is a transfer client with two panes: your machine on the left, the server or cloud storage on the right.',
        'It speaks SFTP, FTP, FTPS, WebDAV and S3 — every connection looks the same.',
        'This tutorial takes a minute. Reopen it anytime via the ? button in the bottom right.',
      ],
    },
    {
      title: 'Creating a connection',
      body: [
        'Click “New connection” on the right, pick a protocol and enter your credentials.',
        '• SFTP/FTP(S): host, port, user, password — or an SSH key file with passphrase',
        '• WebDAV: the server’s base URL',
        '• S3: endpoint, region, bucket, access and secret key',
        '“Save & connect” stores the profile — next time a single click in the profile list is enough.',
      ],
    },
    {
      title: 'Transferring files',
      body: [
        'Select a file on either side, then use the arrows in the middle:',
        '• → uploads the local file to the server',
        '• ← fetches the server file to your machine',
        'Even faster: double-click a file to transfer it straight to the other side.',
      ],
    },
    {
      title: 'The queue',
      body: [
        'All transfers collect in the queue at the bottom and run one after another — with per-file progress.',
        'Running transfers can be cancelled; “Clear finished” empties the list.',
      ],
    },
    {
      title: 'Managing files',
      body: [
        'Both panes can do more than browse: new folder, rename, delete, refresh — locally and on the server.',
        'Navigation: double-click opens folders, “..” goes one level up.',
      ],
    },
    {
      title: 'Updates',
      body: [
        'fileport checks for new versions on launch — the changelog dialog opens, installing needs your click.',
        'Updates come signed from GitHub; your connection profiles survive every update.',
      ],
    },
  ],
  sections: [
    {
      id: 'ui',
      title: 'Interface',
      body: [
        'fileport is a classic two-pane client:',
        '• Left — your local machine: browse folders, select files',
        '• Middle — the transfer arrows: → upload, ← download',
        '• Right — the server: the profile list while disconnected, the server file pane once connected',
        '• Bottom — the queue with all transfers',
        'Both panes show name, size and modified date; double-click opens folders, “..” goes up.',
      ],
    },
    {
      id: 'connections',
      title: 'Connections & profiles',
      body: [
        '“New connection” opens the profile form. Fields depend on the protocol:',
        '• SFTP — host, port (22), user, password or SSH key file (+ passphrase)',
        '• FTP / FTPS — host, port (21), user, password; FTPS encrypts the connection',
        '• WebDAV — base URL (https://…), user, password',
        '• S3 — endpoint, region, bucket, access key, secret key; path-style for MinIO & co.',
        'For servers with self-signed certificates there is an option to accept invalid certificates — only enable it if you trust the server.',
        'Saved profiles appear in the list on the right: connect with a click, edit via the pencil icon. Passwords and keys stay local on your machine and are never sent anywhere — except to the server they belong to.',
      ],
    },
    {
      id: 'transfers',
      title: 'Transfers',
      body: [
        'Three ways to transfer a file:',
        '• Select a file and click the middle arrow (→ upload, ← download)',
        '• Double-click a file — it goes straight to the other side',
        '• Kick off several files — the queue works through them in order',
        'Transfers run sequentially: exactly one at a time, the rest waits. That keeps connections stable and servers happy.',
        'After a successful transfer the target pane refreshes automatically.',
      ],
    },
    {
      id: 'queue',
      title: 'Queue',
      body: [
        'Every transfer appears at the bottom with direction, name and a progress bar.',
        '• States: queued, running, done, error, cancelled',
        '• Cancel — running and queued transfers can be stopped',
        '• “Clear finished” — removes done, failed and cancelled entries from the list',
        'On errors, the cause is shown right on the entry.',
      ],
    },
    {
      id: 'files',
      title: 'File management',
      body: [
        'Both panes offer the basics — locally and on the server:',
        '• New folder — creates a directory in the current path',
        '• Rename — select a file or folder, enter the new name',
        '• Delete — with confirmation; folders are removed with their contents',
        '• Refresh — reloads the view',
      ],
    },
    {
      id: 'updates',
      title: 'Updates',
      body: [
        'fileport checks for new versions automatically on every launch. When one is available, the update dialog opens with the changelog — installing needs your click.',
        'Check manually: “Check for updates” in the top right.',
        'Updates come signed from GitHub (LAN-SOLO/file-port): the app verifies the signature before installing anything. Profiles and settings are preserved.',
      ],
    },
    {
      id: 'roadmap',
      title: 'What’s coming',
      body: [
        'fileport keeps growing via updates. On the list, among other things:',
        '• Resuming interrupted transfers and checksum verification',
        '• More protocols (SCP, rsync over SSH) and cloud services',
        '• Direct drop: end-to-end-encrypted device-to-device transfers',
        'Later with fileport bridged: server-to-server bridge, AS2/OFTP2, schedules, watch folders and an audit log.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privacy',
      body: [
        'fileport runs locally: no account, no telemetry, no relay servers — transfers go straight from you to the target server.',
        'Credentials stay on your machine. The only other network connection is the update check against GitHub.',
      ],
    },
  ],
};

const SEEN_KEY = 'fileport.tutorialSeen';

export default function Help() {
  const c = navigator.language.toLowerCase().startsWith('de') ? de : en;
  const [mode, setMode] = useState<'closed' | 'tutorial' | 'manual'>(() =>
    localStorage.getItem(SEEN_KEY) ? 'closed' : 'tutorial'
  );
  const [step, setStep] = useState(0);
  const [sel, setSel] = useState(c.sections[0].id);
  const [q, setQ] = useState('');

  const close = () => {
    localStorage.setItem(SEEN_KEY, '1');
    setMode('closed');
    setStep(0);
  };

  const query = q.trim().toLowerCase();
  const filtered = query
    ? c.sections.filter(
        (s) =>
          s.title.toLowerCase().includes(query) ||
          s.body.some((p) => p.toLowerCase().includes(query))
      )
    : c.sections;
  const current = filtered.find((s) => s.id === sel) ?? filtered[0] ?? null;

  return (
    <>
      <button className="hlp-fab" title={c.labels.fab} onClick={() => setMode('manual')}>
        ?
      </button>
      {mode !== 'closed' && (
        <div className="hlp-overlay" onClick={close}>
          <div className="hlp-modal" onClick={(e) => e.stopPropagation()}>
            <div className="hlp-head">
              <span className="hlp-brand">
                <span className="hlp-name">fileport</span>
                <span className="hlp-dot">.</span>
              </span>
              <button
                className={`hlp-tab ${mode === 'tutorial' ? 'active' : ''}`}
                onClick={() => {
                  setMode('tutorial');
                  setStep(0);
                }}
              >
                {c.labels.tutorial}
              </button>
              <button
                className={`hlp-tab ${mode === 'manual' ? 'active' : ''}`}
                onClick={() => setMode('manual')}
              >
                {c.labels.manual}
              </button>
              <span className="hlp-spacer" />
              <button className="hlp-close" onClick={close}>
                ✕
              </button>
            </div>

            {mode === 'tutorial' && (
              <div className="hlp-tut">
                <div className="hlp-step-count">
                  {c.labels.stepOf(step + 1, c.tutorial.length)}
                </div>
                <h2>{c.tutorial[step].title}</h2>
                {c.tutorial[step].body.map((p, i) =>
                  p.startsWith('• ') ? (
                    <div key={i} className="hlp-li">
                      {p.slice(2)}
                    </div>
                  ) : (
                    <p key={i}>{p}</p>
                  )
                )}
                <div className="hlp-tut-nav">
                  <button className="hlp-ghost" onClick={close}>
                    {c.labels.skip}
                  </button>
                  <span className="hlp-dots">
                    {c.tutorial.map((_, i) => (
                      <span key={i} className={i === step ? 'on' : ''} />
                    ))}
                  </span>
                  {step > 0 && (
                    <button onClick={() => setStep(step - 1)}>{c.labels.back}</button>
                  )}
                  {step < c.tutorial.length - 1 ? (
                    <button className="hlp-primary" onClick={() => setStep(step + 1)}>
                      {c.labels.next}
                    </button>
                  ) : (
                    <button className="hlp-primary" onClick={close}>
                      {c.labels.done}
                    </button>
                  )}
                </div>
              </div>
            )}

            {mode === 'manual' && (
              <div className="hlp-body">
                <div className="hlp-toc">
                  <input
                    type="text"
                    placeholder={c.labels.search}
                    value={q}
                    onChange={(e) => setQ(e.target.value)}
                  />
                  {filtered.length === 0 && (
                    <div className="hlp-empty">{c.labels.noResults}</div>
                  )}
                  {filtered.map((s) => (
                    <button
                      key={s.id}
                      className={`hlp-toc-item ${current?.id === s.id ? 'active' : ''}`}
                      onClick={() => setSel(s.id)}
                    >
                      {s.title}
                    </button>
                  ))}
                </div>
                <div className="hlp-content">
                  {current && (
                    <>
                      <h2>{current.title}</h2>
                      {current.body.map((p, i) =>
                        p.startsWith('• ') ? (
                          <div key={i} className="hlp-li">
                            {p.slice(2)}
                          </div>
                        ) : (
                          <p key={i}>{p}</p>
                        )
                      )}
                    </>
                  )}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </>
  );
}
