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
        'fileport ist ein Transfer-Client mit zwei Fenstern: links Ihr Rechner, rechts der Server oder Cloud-Speicher.',
        'Unterstützt werden SFTP, SCP, rsync über SSH, FTPS (explizit & implizit), WebDAV, SMB 2/3, S3, Azure Blob Storage und Google Cloud Storage — jede Verbindung sieht gleich aus. Unverschlüsseltes FTP gibt es nur nach ausdrücklicher Bestätigung (siehe Handbuch › Verbindungen & Profile).',
        'Dieses Tutorial dauert eine Minute. Sie finden es jederzeit wieder über den ?-Knopf unten rechts.',
      ],
    },
    {
      title: 'Verbindung anlegen',
      body: [
        'Rechts auf „Neue Verbindung“ klicken, Protokoll wählen und die Zugangsdaten eintragen.',
        '• SFTP/SCP: Host, Port, Benutzer, Passwort — oder SSH-Schlüsseldatei (+ Passphrase); rsync: nur mit Schlüsseldatei',
        '• FTPS: Host, Port, Benutzer, Passwort',
        '• FTP (unverschlüsselt): wie FTPS, zusätzlich Risiko bestätigen',
        '• WebDAV: die Basis-URL des Servers (https://…) — SMB: Host, Freigabe, Benutzer, Passwort',
        '• S3: Endpoint, Region, Bucket, Access- und Secret-Key',
        '• Azure: Storage-Konto, Container, Access Key — GCS: Bucket und Service-Account-Datei',
        '„Speichern & verbinden“ legt das Profil an — beim nächsten Mal genügt ein Klick in der Profilliste.',
        'Bei SSH-Servern zeigt fileport beim ersten Verbinden den Fingerprint des Servers an. Erst nach Ihrer Bestätigung gehen Passwort oder Schlüssel an den Server.',
      ],
    },
    {
      title: 'Dateien übertragen',
      body: [
        'Datei links oder rechts auswählen, dann die Pfeile in der Mitte:',
        '• → lädt die lokale Datei zum Server hoch',
        '• ← holt die Server-Datei auf Ihren Rechner',
        'Noch schneller: Doppelklick auf eine Datei überträgt sie direkt in die jeweils andere Seite.',
        'Gibt es die Datei im Zielordner schon, fragt fileport vor dem Ersetzen nach.',
      ],
    },
    {
      title: 'Die Warteschlange',
      body: [
        'Alle Übertragungen sammeln sich unten in der Warteschlange und laufen nacheinander ab — mit Fortschritt pro Datei.',
        'Laufende Übertragungen lassen sich abbrechen; „Erledigte entfernen“ leert die Liste.',
      ],
    },
    {
      title: 'Dateien verwalten',
      body: [
        'Beide Fenster können mehr als anzeigen: neuer Ordner, umbenennen, löschen, aktualisieren — lokal wie auf dem Server.',
        'Navigation: Doppelklick öffnet Ordner, der ↑-Knopf neben dem Pfadfeld geht eine Ebene nach oben.',
      ],
    },
    {
      title: 'Updates',
      body: [
        'fileport prüft beim Start automatisch auf neue Versionen — der Changelog-Dialog öffnet sich, installiert wird erst nach Ihrem Klick.',
        'Updates kommen signiert von GitHub; Ihre Verbindungsprofile bleiben dabei erhalten.',
      ],
    },
  ],
  sections: [
    {
      id: 'ui',
      title: 'Oberfläche',
      body: [
        'fileport ist ein klassischer Zwei-Fenster-Client:',
        '• Links — Ihr lokaler Rechner: Ordner durchsuchen, Dateien auswählen',
        '• Mitte — die Transfer-Pfeile: → hochladen, ← herunterladen',
        '• Rechts — der Server: Profilliste, solange nichts verbunden ist; danach das Server-Dateifenster',
        '• Unten — die Warteschlange mit allen Übertragungen',
        'Beide Dateifenster zeigen Name, Größe und Änderungsdatum; Doppelklick öffnet Ordner (auch Symlinks auf Ordner), der ↑-Knopf führt eine Ebene nach oben, ⟳ lädt neu.',
        'Das Pfadfeld über der Liste ist direkt editierbar: Pfad eintippen, Enter — das Fenster springt dorthin. Unter Windows wechseln Sie so auch das Laufwerk (z. B. D:\\).',
        'Dunkel oder Hell: Der Sonne-/Mond-Button rechts in der Kopfzeile schaltet den Modus um; die Wahl bleibt beim nächsten Start erhalten.',
      ],
    },
    {
      id: 'connections',
      title: 'Verbindungen & Profile',
      body: [
        '„Neue Verbindung“ öffnet das Profil-Formular. Die Felder hängen vom Protokoll ab:',
        '• SFTP — Host, Port (22), Benutzer, Passwort oder SSH-Schlüsseldatei (+ Passphrase). „~/.ssh/id_ed25519“ wird auf Ihr Home-Verzeichnis aufgelöst',
        '• SCP — wie SFTP, aber für SSH-Server ohne SFTP-Subsystem: Transfers laufen über das klassische SCP-Protokoll, die Dateiverwaltung über Shell-Kommandos (ls, mkdir, rm, mv)',
        '• rsync über SSH — Verwaltung wie bei SCP; die Transfers übernimmt das lokale rsync/ssh-Programm. Deshalb: SSH-Schlüsseldatei ohne Passphrase-Abfrage erforderlich (Passwort-Anmeldung gibt es hier nicht), und rsync muss lokal installiert sein (macOS/Linux: ab Werk; Windows: rsync und ssh müssen im PATH liegen, z. B. über MSYS2 oder cwRsync)',
        '• FTPS explizit — Host, Port (21), Benutzer, Passwort; TLS per AUTH TLS auf dem Standardport',
        '• FTPS implizit — wie explizit, aber TLS ab dem ersten Byte auf Port 990 (ältere Server)',
        '• FTP (unverschlüsselt) — Host, Port (21), Benutzer, Passwort; nur für Server ohne TLS. Das Formular zeigt eine Warnung, gespeichert und verbunden wird erst, wenn Sie das Risiko bestätigen',
        '• WebDAV — Basis-URL (nur https://…), Benutzer, Passwort',
        '• SMB 2/3 — Host, Port (445), Freigabe, Benutzer, Passwort; Windows-Freigaben und NAS. Domänen-Konten als DOMÄNE\\benutzer, SMB1/CIFS wird nie gesprochen. Der Datenverkehr ist nur dann verschlüsselt, wenn der Server SMB3-Verschlüsselung für die Freigabe verlangt (Windows: Set-SmbShare -EncryptData $true, auf NAS „SMB-Verschlüsselung erzwingen“) — sonst nur signiert. SMB daher nur im eigenen Netz oder per VPN nutzen',
        '• S3 — Endpoint (nur https://…), Region, Bucket, Access Key, Secret Key; Path-Style für MinIO & Co.',
        '• Azure Blob Storage — Storage-Konto, Container, Access Key',
        '• Google Cloud Storage — Bucket und Service-Account-Schlüsseldatei (JSON)',
        'fileport baut verschlüsselte Verbindungen auf. Einzige Ausnahme ist „FTP (unverschlüsselt)“: Benutzername, Passwort und Dateien gehen dabei im Klartext übers Netz, deshalb verlangt fileport vorher Ihre Bestätigung und markiert das Profil rot. Unverschlüsseltes WebDAV gibt es nicht; alte FTP-Profile aus früheren Versionen verbinden weiter per FTPS. WebDAV folgt Weiterleitungen nur auf https://-Adressen.',
        'FTPS nutzt TLS 1.2 mit Session-Wiederaufnahme auf dem Datenkanal — das verlangen vsftpd, ProFTPD und FileZilla Server standardmäßig. Zertifikate prüft der Zertifikatsspeicher Ihres Systems.',
        'Für Server mit selbstsignierten Zertifikaten (FTPS, WebDAV, S3) gibt es die Option, ungültige Zertifikate zu akzeptieren — nur einschalten, wenn Sie dem Server vertrauen.',
        'Gespeicherte Profile erscheinen in der Liste rechts: verbinden per Klick, bearbeiten über das Stift-Symbol, löschen über ✕ (mit Rückfrage; das gespeicherte Geheimnis wird mit entfernt).',
        'Geheimnisse (Passwörter, Secret Keys, Passphrasen) liegen im Schlüsselbund des Systems, nie in einer Datei. Beim Bearbeiten lässt ein leeres Feld das gespeicherte Geheimnis unverändert; „Gespeichertes Geheimnis löschen“ entfernt es (z. B. beim Wechsel auf eine Schlüsseldatei ohne Passphrase).',
      ],
    },
    {
      id: 'hostkeys',
      title: 'SSH-Host-Keys',
      body: [
        'SFTP, SCP und rsync prüfen die Identität des Servers über seinen Host-Key:',
        '• Erste Verbindung — fileport holt nur den Schlüssel (ohne Anmeldung) und zeigt seinen SHA256-Fingerprint. Vergleichen Sie ihn mit dem Server (ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub). Erst nach „Vertrauen & verbinden“ gehen Zugangsdaten an den Server; der Schlüssel wird im Profil gemerkt',
        '• Jede weitere Verbindung — der Server muss genau diesen Schlüssel zeigen',
        '• Geänderter Schlüssel — fileport verbindet nicht, sondern warnt mit altem und neuem Fingerprint. Das kann eine Neuinstallation sein oder ein Angriff (Man-in-the-Middle); dem neuen Schlüssel nur vertrauen, wenn Sie die Änderung bestätigen können',
        'Ändern Sie Host oder Port eines Profils, verwirft fileport den gemerkten Schlüssel automatisch. Im Bearbeiten-Formular steht der gemerkte Fingerprint; „Host-Key vergessen“ setzt ihn zurück.',
        'rsync: Das gestartete ssh-Programm prüft den Server gegen genau diesen gemerkten Schlüssel (eigene, temporäre known_hosts-Datei) — Ihre ~/.ssh/known_hosts wird weder gelesen noch verändert.',
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
        'Ziel ist immer der Ordner, der auf der anderen Seite gerade offen ist. Liegt dort schon eine Datei gleichen Namens, fragt fileport vor dem Ersetzen nach — Groß-/Kleinschreibung zählt dabei nicht („Report.pdf“ und „report.pdf“ gelten als derselbe Name, wie unter macOS und Windows). Unmittelbar vor dem Start prüft fileport das Ziel noch einmal; ist die Datei inzwischen entstanden (etwa durch einen früheren Eintrag der Warteschlange), kommt die Rückfrage dann. Solange eine solche Rückfrage offen ist, wartet die übrige Warteschlange.',
        'Übertragen werden einzelne Dateien; ganze Ordner lassen sich (noch) nicht am Stück übertragen. Ordner, die sich nicht auflisten lassen (nur Schreibrecht, z. B. ein Upload-Briefkasten), kann fileport nicht öffnen und daher nicht als Ziel nutzen.',
        'Die Übertragung läuft sequenziell: genau ein Transfer zur Zeit, der Rest wartet. Das hält Verbindungen stabil und Server freundlich gestimmt.',
        'Während jeder Übertragung rechnet fileport eine BLAKE3-Prüfsumme über die Daten mit. Downloads schreibt fileport zuerst in eine versteckte Teildatei (.name.fileport-part) und ersetzt das Ziel erst nach vollständigem Erfolg — bricht ein Download ab oder schlägt fehl, verschwindet die Teildatei und eine vorhandene Datei bleibt unverändert. Schließen Sie das Fenster mitten im Download (oder startet fileport für ein Update neu), bricht fileport ab und entfernt die Teildatei; nach einem Absturz räumt sie der nächste Start weg.',
        'Dateinamen, die auf Ihrem System nicht zulässig sind (unter Windows z. B. mit \\ : * ? " < > | oder Gerätenamen wie CON), lädt fileport nicht herunter, sondern meldet einen Fehler — so kann kein Server Dateien außerhalb des Zielordners ablegen.',
        'Nach einer erfolgreichen Übertragung aktualisiert sich die Zielseite automatisch.',
        'Hält unter Windows ein Virenscanner, OneDrive oder Office die frisch geladene Datei kurz fest, versucht fileport das abschließende Ersetzen bis zu drei Sekunden lang erneut.',
        '„Trennen“, während über die Verbindung noch Übertragungen laufen oder warten: fileport fragt nach und bricht diese Übertragungen erst nach Ihrer Bestätigung ab.',
      ],
    },
    {
      id: 'queue',
      title: 'Warteschlange',
      body: [
        'Jede Übertragung erscheint unten mit Richtung, Namen und Fortschrittsbalken.',
        '• Status: wartet, läuft (Prozent), fertig, Fehler, abgebrochen',
        '• ✕ — laufende und wartende Übertragungen lassen sich stoppen',
        '• „Erledigte entfernen“ — entfernt fertige, fehlgeschlagene und abgebrochene Einträge aus der Liste',
        'Bei einem Fehler steht die Ursache direkt am Eintrag (vollständig im Tooltip).',
      ],
    },
    {
      id: 'files',
      title: 'Dateiverwaltung',
      body: [
        'Beide Fenster bieten die Grundwerkzeuge — lokal wie auf dem Server:',
        '• Neuer Ordner — legt ein Verzeichnis im aktuellen Pfad an',
        '• Umbenennen — Datei oder Ordner auswählen, neuen Namen eingeben (Enter bestätigt, Esc bricht ab)',
        '• Löschen — mit Sicherheitsabfrage; Ordner werden samt Inhalt entfernt (bei allen Protokollen). Symlinks werden als Link gelöscht, ihr Ziel bleibt erhalten',
        '• Aktualisieren (⟳) — lädt die Ansicht neu',
        'Ließ sich der Ordner einer Seite nicht laden (etwa ein Start-Ordner ohne Leserecht), bleiben „Neuer Ordner“ und die Transfer-Pfeile zu dieser Seite gesperrt, bis Sie über das Pfadfeld oder ↑ einen lesbaren Ordner öffnen.',
        'Namen dürfen weder „/“ noch „\\“ enthalten und nicht „.“ oder „..“ sein; lokal unter Windows zusätzlich keine : * ? " < > | und keine Gerätenamen wie CON oder NUL.',
        'Umbenennen ersetzt nie still eine vorhandene Datei: Existiert der neue Name im Ordner schon (ohne Rücksicht auf Groß-/Kleinschreibung), meldet fileport einen Fehler. Nur die Groß-/Kleinschreibung desselben Eintrags zu ändern, geht — außer auf Servern, die Groß-/Kleinschreibung unterscheiden und bereits einen zweiten Eintrag mit genau diesem Namen haben.',
        'Änderungszeiten zeigt fileport in der Ortszeit Ihres Rechners. Bei SCP und rsync liest fileport sie als UTC vom Server. Bei FTPS liest fileport die Zeiten zusätzlich per MLSD (in UTC), wenn der Server es anbietet (z. B. ProFTPD, FileZilla Server, Pure-FTPd); sonst bleibt nur LIST ohne Zeitzone — dann nimmt fileport UTC an, wie es vsftpd standardmäßig sendet. Server, die per LIST Ortszeit senden, zeigen dann um den Versatz verschobene Zeiten.',
        'Objektspeicher (S3, Azure, GCS) kennen keine echten Ordner: Ein neuer Ordner ist ein unsichtbares Marker-Objekt, und das Umbenennen eines Ordners verschiebt jedes Objekt darin einzeln (nicht atomar — bei sehr vielen Objekten dauert das).',
      ],
    },
    {
      id: 'updates',
      title: 'Updates',
      body: [
        'fileport prüft bei jedem Start automatisch auf neue Versionen. Liegt eine bereit, öffnet sich der Update-Dialog mit dem Changelog — installiert wird erst nach Ihrem Klick.',
        'Manuell prüfen: „Nach Updates suchen“ oben rechts.',
        'Updates kommen signiert von GitHub (LAN-SOLO/file-port): Die App prüft die Signatur, bevor irgendetwas installiert wird. Profile und Einstellungen bleiben erhalten.',
      ],
    },
    {
      id: 'whatsnew',
      title: 'Neu in 0.3 (Beta)',
      body: [
        'Version 0.3 ist die Beta vor 1.0 — die stabile Ausgabe folgt nach Abschluss der Tests. Gegenüber 0.2:',
        '• Unverschlüsseltes FTP (Port 21) für Server ohne TLS — nur nach Warnung und ausdrücklicher Bestätigung im Profil',
        '• SSH-Host-Keys werden vor der ersten Anmeldung mit Fingerprint bestätigt; geänderte Schlüssel lösen eine Warnung aus, gemerkte Schlüssel lassen sich vergessen',
        '• rsync prüft den Server gegen den gemerkten Host-Key statt ihn blind zu übernehmen',
        '• FTPS funktioniert mit Servern, die TLS-Session-Wiederaufnahme verlangen (vsftpd & Co.); implizites FTPS überträgt jetzt Daten zuverlässig',
        '• Rückfrage vor dem Ersetzen vorhandener Dateien; Downloads ersetzen das Ziel erst nach vollständigem Erfolg',
        '• Ordner löschen bei SFTP und FTPS samt Inhalt, Ordner umbenennen bei S3/Azure/GCS',
        '• Windows: „Ebene hoch“ bleibt auf der Laufwerkswurzel, unzulässige Dateinamen vom Server werden abgelehnt, rsync mit Laufwerkspfaden',
        '• WebDAV: Dateinamen mit „&“ und Sonderzeichen werden korrekt angezeigt, die Wurzel zeigt keinen leeren Eintrag mehr',
        '• Fehlermeldungen erscheinen in der Sprache der Oberfläche; Umbenennen und Uploads ersetzen nie still eine vorhandene Datei',
        '• Windows: Wiederholung, wenn Virenscanner/OneDrive eine Datei kurz sperren; Schlüsseldateien und profiles.json aus dem Windows-Editor (BOM, CRLF) werden gelesen',
        '• SCP/rsync: Änderungszeiten stimmen auch bei Servern in anderen Zeitzonen; FTPS liest sie per MLSD in UTC, wo der Server das kann',
        '• Keine liegengebliebenen Teildateien mehr, wenn das Fenster während eines Downloads geschlossen wird',
      ],
    },
    {
      id: 'roadmap',
      title: 'Was noch kommt',
      body: [
        'fileport wächst per Update weiter. Auf der Liste stehen unter anderem:',
        '• Ordner am Stück übertragen, Mehrfachauswahl und Drag & Drop',
        '• Fortsetzen abgebrochener Übertragungen und sichtbare Prüfsummen-Verifikation (berechnet wird die BLAKE3-Prüfsumme schon heute bei jedem Transfer)',
        '• Cloud-Speicher mit OAuth-Anmeldung (Dropbox, Google Drive, OneDrive)',
        '• Direkt-Drop: Dateien Ende-zu-Ende-verschlüsselt von Gerät zu Gerät',
        'Später mit fileport bridged: Server-zu-Server-Brücke, AS2/OFTP2, Zeitpläne, Watch-Folder und Audit-Log.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privatsphäre',
      body: [
        'fileport läuft lokal: kein Konto, keine Telemetrie, keine Zwischenserver — Übertragungen gehen direkt von Ihnen zum Zielserver.',
        'Zugangsdaten bleiben auf Ihrem Rechner (Schlüsselbund des Systems) und gehen nur an den Server, zu dem sie gehören. Die einzige weitere Netzwerkverbindung ist der Update-Check gegen GitHub.',
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
        'It speaks SFTP, SCP, rsync over SSH, FTPS (explicit & implicit), WebDAV, SMB 2/3, S3, Azure Blob Storage and Google Cloud Storage — every connection looks the same. Unencrypted FTP is available only after explicit confirmation (see Manual › Connections & profiles).',
        'This tutorial takes a minute. Reopen it anytime via the ? button in the bottom right.',
      ],
    },
    {
      title: 'Creating a connection',
      body: [
        'Click “New connection” on the right, pick a protocol and enter your credentials.',
        '• SFTP/SCP: host, port, user, password — or an SSH key file (+ passphrase); rsync: key file only',
        '• FTPS: host, port, user, password',
        '• FTP (unencrypted): like FTPS, plus confirming the risk',
        '• WebDAV: the server’s base URL (https://…) — SMB: host, share, user, password',
        '• S3: endpoint, region, bucket, access and secret key',
        '• Azure: storage account, container, access key — GCS: bucket and service account file',
        '“Save & connect” stores the profile — next time a single click in the profile list is enough.',
        'For SSH servers fileport shows the server’s fingerprint on the first connection. Your password or key is only sent after you confirm it.',
      ],
    },
    {
      title: 'Transferring files',
      body: [
        'Select a file on either side, then use the arrows in the middle:',
        '• → uploads the local file to the server',
        '• ← fetches the server file to your machine',
        'Even faster: double-click a file to transfer it straight to the other side.',
        'If the file already exists in the target folder, fileport asks before replacing it.',
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
        'Navigation: double-click opens folders, the ↑ button next to the path field goes one level up.',
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
        'Both panes show name, size and modified date; double-click opens folders (including symlinks to folders), the ↑ button goes up one level, ⟳ reloads.',
        'The path field above the list is directly editable: type a path, hit Enter — the pane jumps there. On Windows this is also how you switch drives (e.g. D:\\).',
        'Dark or light: the sun/moon button on the right of the header switches the mode; your choice is kept for the next start.',
      ],
    },
    {
      id: 'connections',
      title: 'Connections & profiles',
      body: [
        '“New connection” opens the profile form. Fields depend on the protocol:',
        '• SFTP — host, port (22), user, password or SSH key file (+ passphrase). “~/.ssh/id_ed25519” resolves to your home directory',
        '• SCP — like SFTP, but for SSH servers without an SFTP subsystem: transfers use the classic SCP protocol, file management runs over shell commands (ls, mkdir, rm, mv)',
        '• rsync over SSH — management like SCP; transfers are handled by your local rsync/ssh programs. Hence: an SSH key file without a passphrase prompt is required (no password login here), and rsync must be installed locally (macOS/Linux: out of the box; Windows: rsync and ssh must be on the PATH, e.g. via MSYS2 or cwRsync)',
        '• FTPS explicit — host, port (21), user, password; TLS via AUTH TLS on the standard port',
        '• FTPS implicit — like explicit, but TLS from the first byte on port 990 (older servers)',
        '• FTP (unencrypted) — host, port (21), user, password; only for servers without TLS. The form shows a warning and only saves and connects once you confirm the risk',
        '• WebDAV — base URL (https:// only), user, password',
        '• SMB 2/3 — host, port (445), share, user, password; Windows shares and NAS boxes. Domain accounts as DOMAIN\\user, SMB1/CIFS is never spoken. Traffic is only encrypted when the server requires SMB3 encryption for the share (Windows: Set-SmbShare -EncryptData $true, on a NAS “force SMB encryption”) — otherwise it is signed only. Use SMB on your own network or over a VPN',
        '• S3 — endpoint (https:// only), region, bucket, access key, secret key; path-style for MinIO & co.',
        '• Azure Blob Storage — storage account, container, access key',
        '• Google Cloud Storage — bucket and service account key file (JSON)',
        'fileport establishes encrypted connections. The one exception is “FTP (unencrypted)”: user name, password and files cross the network in plain text, so fileport asks for your confirmation first and marks the profile in red. There is no unencrypted WebDAV; old FTP profiles from earlier versions keep connecting via FTPS. WebDAV only follows redirects to https:// addresses.',
        'FTPS uses TLS 1.2 with session resumption on the data channel — which vsftpd, ProFTPD and FileZilla Server require by default. Certificates are checked against your system’s certificate store.',
        'For servers with self-signed certificates (FTPS, WebDAV, S3) there is an option to accept invalid certificates — only enable it if you trust the server.',
        'Saved profiles appear in the list on the right: connect with a click, edit via the pencil icon, delete via ✕ (with confirmation; the stored secret is removed too).',
        'Secrets (passwords, secret keys, passphrases) live in the system keychain, never in a file. When editing, an empty field keeps the stored secret; “Delete stored secret” removes it (e.g. when switching to a key file without passphrase).',
      ],
    },
    {
      id: 'hostkeys',
      title: 'SSH host keys',
      body: [
        'SFTP, SCP and rsync verify the server’s identity via its host key:',
        '• First connection — fileport only fetches the key (without logging in) and shows its SHA256 fingerprint. Compare it with the server (ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub). Credentials are only sent after “Trust & connect”; the key is remembered in the profile',
        '• Every later connection — the server must present exactly this key',
        '• Changed key — fileport does not connect but warns with the old and new fingerprint. This may be a reinstall or an attack (man-in-the-middle); only trust the new key if you can confirm the change',
        'If you change a profile’s host or port, fileport discards the remembered key automatically. The edit form shows the remembered fingerprint; “Forget host key” resets it.',
        'rsync: the spawned ssh program verifies the server against exactly this remembered key (its own temporary known_hosts file) — your ~/.ssh/known_hosts is neither read nor modified.',
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
        'The target is always the folder currently open on the other side. If a file with the same name already exists there, fileport asks before replacing it — case does not matter (“Report.pdf” and “report.pdf” count as the same name, as on macOS and Windows). Right before starting, fileport checks the target again; if the file has appeared in the meantime (e.g. from an earlier queue entry), you are asked then. While such a question is open, the rest of the queue waits.',
        'Transfers work on single files; whole folders cannot be transferred in one go (yet). Folders that cannot be listed (write-only, e.g. an upload drop box) cannot be opened in fileport and therefore not used as a target.',
        'Transfers run sequentially: exactly one at a time, the rest waits. That keeps connections stable and servers happy.',
        'During every transfer fileport computes a BLAKE3 checksum over the data. Downloads are first written to a hidden partial file (.name.fileport-part) and only replace the target after full success — if a download aborts or fails, the partial file is removed and an existing file stays untouched. If you close the window mid-download (or fileport restarts for an update), fileport cancels and removes the partial file; after a crash, the next start cleans it up.',
        'File names that are not allowed on your system (on Windows e.g. containing \\ : * ? " < > | or device names like CON) are not downloaded; fileport reports an error instead — so no server can place files outside the target folder.',
        'After a successful transfer the target pane refreshes automatically.',
        'If a virus scanner, OneDrive or Office briefly holds the freshly downloaded file on Windows, fileport retries the final replace for up to three seconds.',
        '“Disconnect” while transfers are still running or queued on that connection: fileport asks first and only cancels those transfers after you confirm.',
      ],
    },
    {
      id: 'queue',
      title: 'Queue',
      body: [
        'Every transfer appears at the bottom with direction, name and a progress bar.',
        '• States: queued, running (percent), done, error, cancelled',
        '• ✕ — running and queued transfers can be stopped',
        '• “Clear finished” — removes done, failed and cancelled entries from the list',
        'On errors, the cause is shown right on the entry (in full in the tooltip).',
      ],
    },
    {
      id: 'files',
      title: 'File management',
      body: [
        'Both panes offer the basics — locally and on the server:',
        '• New folder — creates a directory in the current path',
        '• Rename — select a file or folder, enter the new name (Enter confirms, Esc cancels)',
        '• Delete — with confirmation; folders are removed with their contents (on every protocol). Symlinks are deleted as links, their target stays',
        '• Refresh (⟳) — reloads the view',
        'If a pane’s folder could not be loaded (e.g. a start folder without read permission), “New folder” and the transfer arrows towards that pane stay disabled until you open a readable folder via the path field or ↑.',
        'Names must not contain “/” or “\\” and must not be “.” or “..”; locally on Windows also no : * ? " < > | and no device names like CON or NUL.',
        'Renaming never silently replaces an existing file: if the new name already exists in the folder (regardless of case), fileport reports an error. Changing only the case of the same entry works — except on servers that distinguish case and already hold a second entry with exactly that name.',
        'Modification times are shown in your computer’s local time. For SCP and rsync fileport reads them from the server as UTC. For FTPS fileport additionally reads the times via MLSD (in UTC) when the server offers it (e.g. ProFTPD, FileZilla Server, Pure-FTPd); otherwise only LIST without a time zone remains — fileport then assumes UTC, which vsftpd sends by default. Servers that send local time via LIST then show times shifted by the offset.',
        'Object stores (S3, Azure, GCS) have no real folders: a new folder is an invisible marker object, and renaming a folder moves every object inside it one by one (not atomic — with many objects this takes a while).',
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
      id: 'whatsnew',
      title: 'New in 0.3 (beta)',
      body: [
        'Version 0.3 is the beta ahead of 1.0 — the stable release follows once testing is complete. Compared to 0.2:',
        '• Unencrypted FTP (port 21) for servers without TLS — only after a warning and explicit confirmation in the profile',
        '• SSH host keys are confirmed by fingerprint before the first login; changed keys trigger a warning, remembered keys can be forgotten',
        '• rsync verifies the server against the remembered host key instead of accepting it blindly',
        '• FTPS works with servers that require TLS session resumption (vsftpd & co.); implicit FTPS now transfers data reliably',
        '• Confirmation before replacing existing files; downloads only replace the target after full success',
        '• Deleting folders with contents over SFTP and FTPS, renaming folders on S3/Azure/GCS',
        '• Windows: “Go up” stays on the drive root, server file names that are invalid on Windows are rejected, rsync with drive paths',
        '• WebDAV: file names with “&” and special characters display correctly, the root no longer shows an empty entry',
        '• Error messages appear in the interface language; renaming and uploads never silently replace an existing file',
        '• Windows: retries when a virus scanner/OneDrive briefly locks a file; key files and profiles.json saved by Windows Notepad (BOM, CRLF) are read',
        '• SCP/rsync: modification times are correct even for servers in other time zones; FTPS reads them in UTC via MLSD where the server supports it',
        '• No more leftover partial files when the window is closed during a download',
      ],
    },
    {
      id: 'roadmap',
      title: 'What’s coming',
      body: [
        'fileport keeps growing via updates. On the list, among other things:',
        '• Transferring whole folders, multi-select and drag & drop',
        '• Resuming interrupted transfers and visible checksum verification (the BLAKE3 checksum is already computed on every transfer today)',
        '• Cloud storage with OAuth sign-in (Dropbox, Google Drive, OneDrive)',
        '• Direct drop: end-to-end-encrypted device-to-device transfers',
        'Later with fileport bridged: server-to-server bridge, AS2/OFTP2, schedules, watch folders and an audit log.',
      ],
    },
    {
      id: 'privacy',
      title: 'Privacy',
      body: [
        'fileport runs locally: no account, no telemetry, no relay servers — transfers go straight from you to the target server.',
        'Credentials stay on your machine (system keychain) and only go to the server they belong to. The only other network connection is the update check against GitHub.',
      ],
    },
  ],
};

const SEEN_KEY = 'fileport.tutorialSeen';

export default function Help() {
  // Sprache der Hilfe: folgt der Systemsprache, lässt sich aber im Kopf der
  // Hilfe jederzeit zwischen DE und EN umschalten
  const [helpLang, setHelpLang] = useState<'de' | 'en'>(() =>
    navigator.language.toLowerCase().startsWith('de') ? 'de' : 'en'
  );
  const c = helpLang === 'de' ? de : en;
  const [mode, setMode] = useState<'closed' | 'tutorial' | 'manual'>(() => {
    try {
      return localStorage.getItem(SEEN_KEY) ? 'closed' : 'tutorial';
    } catch {
      return 'tutorial';
    }
  });
  const [step, setStep] = useState(0);
  const [sel, setSel] = useState(c.sections[0].id);
  const [q, setQ] = useState('');

  const close = () => {
    try {
      localStorage.setItem(SEEN_KEY, '1');
    } catch {
      // ohne localStorage erscheint das Tutorial beim nächsten Start erneut
    }
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
              <span className="hlp-lang" role="group" aria-label="Sprache / Language">
                <button className={`hlp-lang-btn ${helpLang === 'de' ? 'active' : ''}`} onClick={() => setHelpLang('de')}>
                  DE
                </button>
                <button className={`hlp-lang-btn ${helpLang === 'en' ? 'active' : ''}`} onClick={() => setHelpLang('en')}>
                  EN
                </button>
              </span>
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
