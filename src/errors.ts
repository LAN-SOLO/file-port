/**
 * Fehlermeldungen der Engine (fileport-core + Tauri-Commands) kommen
 * deutsch. Für die englische Oberfläche werden die festen Textbausteine
 * hier übersetzt — als Teilstring-Regeln, weil sich Meldungen
 * zusammensetzen (z. B. „Anmeldung fehlgeschlagen: Schlüsseldatei: Datei
 * oder Ordner nicht gefunden (OS-Fehler 2)“). Variable Teile (Pfade, Hosts,
 * Namen, Bibliotheksmeldungen) bleiben unverändert.
 *
 * Reine Funktion ohne DOM — läuft mit dem Node-Testrunner (tests/errors.test.ts).
 * Neue Fehlertexte im Rust-Code hier mitpflegen.
 */

type Rule = [RegExp, string];

/** Reihenfolge zählt: spezifische Bausteine vor allgemeinen. */
const EN_RULES: Rule[] = [
  // FpError-Varianten (core/src/error.rs)
  [/Verbindung fehlgeschlagen: /g, 'Connection failed: '],
  [/Anmeldung fehlgeschlagen: /g, 'Authentication failed: '],
  [
    /Host-Key von (.+?) stimmt nicht mit dem gespeicherten überein — möglicher Angriff \(Man-in-the-Middle\) oder neu aufgesetzter Server/g,
    'Host key of $1 does not match the stored one — possible attack (man-in-the-middle) or reinstalled server',
  ],
  [/Nicht gefunden: /g, 'Not found: '],
  [/Zugriff verweigert: /g, 'Access denied: '],
  [/Übertragung abgebrochen/g, 'Transfer cancelled'],
  [/E\/A-Fehler: /g, 'I/O error: '],

  // Betriebssystem-Fehler (io_text)
  [/Die Datei wird von einem anderen Programm verwendet/g, 'The file is in use by another program'],
  [/Datei oder Ordner nicht gefunden/g, 'File or folder not found'],
  [/Keine Berechtigung/g, 'Permission denied'],
  [/Existiert bereits/g, 'Already exists'],
  [/Kein Speicherplatz mehr frei/g, 'No space left on the drive'],
  [/Ordner ist nicht leer/g, 'Folder is not empty'],
  [/Ungültiger oder zu langer Dateiname/g, 'Invalid or too long file name'],
  [/Datenträger ist schreibgeschützt/g, 'The drive is read-only'],
  [/Zeitüberschreitung beim Verbinden mit /g, 'Timed out connecting to '],
  [/Zeitüberschreitung/g, 'Timed out'],
  [/Ziel ist kein Ordner/g, 'Target is not a folder'],
  [/Ziel ist ein Ordner/g, 'Target is a folder'],
  [/Verschieben über Laufwerksgrenzen nicht möglich/g, 'Cannot move across drives'],
  [/\(OS-Fehler (-?\d+)\)/g, '(OS error $1)'],

  // SSH / SCP / rsync
  [/Schlüsseldatei: /g, 'Key file: '],
  [/SCP: überlange Protokollzeile/g, 'SCP: protocol line too long'],
  [/SCP: unlesbarer Header „([^“]*)“/g, 'SCP: unreadable header “$1”'],
  [/SCP: unerwarteter Header „([^“]*)“/g, 'SCP: unexpected header “$1”'],
  [
    /SCP: Server hat die Sitzung beendet — ist scp auf dem Server installiert\?/g,
    'SCP: the server closed the session — is scp installed on the server?',
  ],
  [
    /Server erlaubt keine Shell-Kommandos \(für (SCP|rsync) nötig\)/g,
    'The server does not allow shell commands (required for $1)',
  ],
  [
    /rsync-Transfers laufen über das System-SSH und brauchen eine SSH-Schlüsseldatei — Passwort-Anmeldung wird hier nicht unterstützt/g,
    'rsync transfers use the system SSH and need an SSH key file — password login is not supported here',
  ],
  [
    /Kein rsync-Programm gefunden — bitte rsync lokal installieren/g,
    'No rsync program found — please install rsync locally',
  ],
  [/rsync ließ sich nicht starten: /g, 'rsync could not be started: '],
  [
    / — funktioniert die Schlüsseldatei ohne Passphrase-Abfrage\?/g,
    ' — does the key file work without a passphrase prompt?',
  ],
  [/Server hat keinen Host-Key gezeigt/g, 'The server presented no host key'],
  [/Server hat die Anmeldung als (.+?) abgelehnt/g, 'The server rejected the login as $1'],
  [/SFTP-Subsystem: /g, 'SFTP subsystem: '],
  [/SFTP-Handshake: /g, 'SFTP handshake: '],
  [/: unlesbare Dateigröße/g, ': unreadable file size'],
  [/: kein Dateiname/g, ': no file name'],

  // FTPS / WebDAV / SMB / Objektspeicher
  [/: keine Adresse gefunden/g, ': no address found'],
  [/FTP-Verbindung ist geschlossen/g, 'FTP connection is closed'],
  [/Zugangsdaten abgelehnt \(401\)/g, 'Credentials rejected (401)'],
  [/Ungültige URL: /g, 'Invalid URL: '],
  [/PROPFIND-Antwort: /g, 'PROPFIND response: '],
  [/: Zugriff abgelehnt/g, ': access denied'],
  [/Freigabe (\S*): /g, 'Share $1: '],

  // Profile & Schlüsselbund (src-tauri/src/profiles.rs)
  [/Host fehlt/g, 'Host is missing'],
  [/Benutzer fehlt/g, 'User is missing'],
  [/Name fehlt/g, 'Name is missing'],
  [/Ungültiger Host: /g, 'Invalid host: '],
  [/Ungültiger Benutzername: /g, 'Invalid user name: '],
  [/Schlüsselbund: /g, 'Keychain: '],
  [/Profil nicht gefunden/g, 'Profile not found'],
  [
    /Nur https:\/\/-URLs — unverschlüsseltes WebDAV wird nicht unterstützt/g,
    'Only https:// URLs — unencrypted WebDAV is not supported',
  ],
  [
    /Nur https:\/\/-Endpoints — unverschlüsselte Verbindungen sind deaktiviert/g,
    'Only https:// endpoints — unencrypted connections are disabled',
  ],

  // Updater (src-tauri/src/main.rs)
  [/Update-Prüfung fehlgeschlagen: /g, 'Update check failed: '],
  [/Kein Update verfügbar/g, 'No update available'],
  [/Update fehlgeschlagen: /g, 'Update failed: '],

  // Dateioperationen & Transfers (engine.rs, transfers.rs)
  [
    /Ungültiger lokaler Zielpfad \(enthält „\.\.“\): /g,
    'Invalid local target path (contains “..”): ',
  ],
  [/Ungültiger lokaler Zielpfad: /g, 'Invalid local target path: '],
  [
    /Der Dateiname „(.*?)“ ist auf diesem System nicht zulässig — bitte auf dem Server umbenennen/g,
    'The file name “$1” is not allowed on this system — please rename it on the server',
  ],
  [
    /Der Dateiname „(.*?)“ ist auf diesem System nicht zulässig/g,
    'The file name “$1” is not allowed on this system',
  ],
  [/„(.*?)“ existiert im Zielordner bereits/g, '“$1” already exists in the target folder'],
  [/Ungültiger Pfad auf der Gegenstelle/g, 'Invalid remote path'],
  [/Transfer (\S+) läuft bereits/g, 'Transfer $1 is already running'],
  [/Verbindung ist nicht mehr offen/g, 'The connection is no longer open'],
  // WebDAV-Abbruch als E/A-Fehler („E/A-Fehler: abgebrochen“)
  [/\babgebrochen\b/g, 'cancelled'],
];

/** Fehler (beliebiger Typ aus invoke/catch) als Text in der UI-Sprache. */
export function translateError(err: unknown, lang: 'de' | 'en'): string {
  const raw = err instanceof Error ? err.message : String(err);
  if (lang === 'de') return raw;
  return EN_RULES.reduce((s, [re, rep]) => s.replace(re, rep), raw);
}

/** Ziel existiert schon — die UI zeigt dann die Ersetzen-Rückfrage. */
export function isExistsError(err: unknown): boolean {
  return /existiert im Zielordner bereits/.test(String(err));
}
