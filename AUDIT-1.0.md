# fileport 1.0.0 — Audit (Funktion & Sicherheit)

Stand: 2026-10-19 · Basis: `c24eb7d` (v0.2.9) · Ziel: 1.0.0-Testbuild für macOS und Windows
(CI per `workflow_dispatch` aus `main`).

Legende Teststatus:

- **IT** — Integrationstest gegen echten Server in Docker (`FILEPORT_IT=1`, Rezepte in `core/tests/*_it.rs`)
- **UT** — Unit-Test (`cargo test --workspace` bzw. `pnpm test`)
- **UI** — im Browser gegen gemocktes Backend durchgeklickt (Headless-Chrome + `@tauri-apps/api/mocks`,
  Skript nicht im Repo; DE und EN, macOS- und Windows-Pfade)
- **statisch** — Code-Review, nicht ausgeführt (braucht echte App / echtes Windows / echten Cloud-Account)

## 1. Testläufe

| Suite | Befehl | Ergebnis |
|---|---|---|
| Rust-Unit-Tests (Core + App) | `cargo test --workspace` | 20 Tests grün (13 Core, 7 App); ITs ohne `FILEPORT_IT` übersprungen |
| Integrationstests | `FILEPORT_IT=1 FILEPORT_IT_FTPS_IMPLICIT=1 FILEPORT_IT_SSH_KEY=… cargo test -p fileport-core --tests` | 9/9 grün (SFTP, SCP, rsync, FTPS explizit, FTPS implizit, WebDAV, SMB, S3) |
| Frontend-Tests | `pnpm test` | 4/4 grün (Pfad-Helfer inkl. Windows-Laufwerke) |
| Typecheck + Build | `pnpm build` (tsc + vite) | grün |
| Lint | `cargo clippy --workspace --all-targets` | 0 Warnungen |
| UI-Durchlauf (Mock) | Headless-Chrome | 45 Prüfpunkte grün (DE/macOS 37, EN/Windows-Pfade 8) |
| Abhängigkeiten | `pnpm audit` | 0 (vorher 1× high: source-map-js, Dev-Tooling → 1.2.2) |
| Rust-Advisories | `cargo audit` | Runde 2: gelaufen, Patch-Updates eingespielt — Rest siehe §7 |

Docker-Testserver (arm64-tauglich): `atmoz/sftp`, `lscr.io/linuxserver/openssh-server` (SCP, rsync),
`delfer/alpine-ftp-server` (vsftpd mit `require_ssl_reuse=YES`), `bytemark/webdav`, `dperson/samba`,
`rclone/rclone serve s3` (MinIO-Images sind nicht mehr frei verfügbar; `pure-ftpd` stürzt unter Rosetta ab).

## 2. Funktionsinventar

### Verbindungen

| Funktion | Status | Test |
|---|---|---|
| SFTP: Passwort, Schlüsseldatei (+ Passphrase), `~` im Pfad | ok | IT (Passwort), UT (`~`-Auflösung); Schlüssel-Auth über rsync-IT |
| SCP (Shell-Kommandos + scp -f/-t) | ok | IT |
| rsync über SSH (Schlüsseldatei Pflicht) | ok | IT (inkl. Namen mit `' $( ) & ;`, leeres HOME) |
| FTPS explizit (AUTH TLS) | **behoben** — vorher Abbruch bei Servern mit Session-Reuse-Pflicht | IT gegen vsftpd-Standardkonfiguration |
| FTPS implizit (Port 990) | **behoben** — Datenkanal hing (kein PROT P) | IT |
| Alte „ftp"-Profile → FTPS | ok | UT (Profil-Parsing), statisch |
| WebDAV (nur https://) | ok | IT (http nur im Test), UT (PROPFIND) |
| SMB 2/3 (DOMÄNE\\benutzer) | ok | IT (ohne Domäne) |
| S3 (Endpoint/Region/Path-Style, nur https://) | ok | IT (rclone), UT (InMemory) |
| Azure Blob | ok | **statisch** (kein Account) — gleiche `ObjectBackend`-Logik wie S3 (UT) |
| Google Cloud Storage | ok | **statisch** (kein Account) — dito |
| „Selbstsignierte Zertifikate akzeptieren" (FTPS/WebDAV/S3) | ok | IT (FTPS: an = verbindet, aus = abgelehnt) |
| SSH-Host-Key: Bestätigung vor der ersten Anmeldung | **neu** | IT (Probe ohne Login, Fingerprint), UI (Dialog) |
| SSH-Host-Key geändert → Warnung, bewusst übernehmen | **neu** | IT (Mismatch liefert gesehenen Key), UI |
| Host-Key vergessen / Reset bei Host-/Port-Änderung | **neu** | UI, statisch (Backend) |
| Profile anlegen/bearbeiten/löschen (mit Rückfrage) | ok | UI |
| Geheimnis im Schlüsselbund, „leer = behalten", „Geheimnis löschen" | ok / **neu** | UI (Mock), Schlüsselbund selbst **statisch** |

### Dateien & Transfers

| Funktion | Status | Test |
|---|---|---|
| Listing lokal/remote, Sortierung Ordner zuerst | ok | IT, UI |
| Pfadfeld (Enter), ↑ Ebene hoch, ⟳ | ok; Windows-Wurzel **behoben** | UT, UI |
| Ordner-Symlinks öffnen (Doppelklick) | **neu** | UI, IT (SCP-Listing mit `/`) |
| Neuer Ordner / Umbenennen / Löschen (Rückfrage) | ok | IT je Protokoll, UI |
| Ordner samt Inhalt löschen | **behoben** für SFTP + FTPS (vorher nur leere) | IT |
| Ordner umbenennen auf S3/Azure/GCS | **behoben** (vorher NotFound) | UT, IT |
| Namensprüfung (`/`, `\`, `.`, `..`) | **neu** | UT, UI |
| Upload/Download per Pfeil und Doppelklick | ok | IT, UI |
| Rückfrage vor dem Ersetzen | **neu** | UI |
| Download über `.name.fileport-part` + Umbenennen | **neu** (vorher: Fehler löschte vorhandene Zieldatei) | UT (Pfadlogik), statisch (Task) |
| Warteschlange: sequenziell, Fortschritt, Abbruch, „Erledigte entfernen" | ok; Race **behoben** | UI (done-Event vor Command-Antwort) |
| BLAKE3 je Transfer | ok | IT (Up/Down-Hash identisch) |
| Abbruch | ok | UT (lokal, Objektspeicher) |

### App

| Funktion | Status | Test |
|---|---|---|
| Updater (Check beim Start, Button, Changelog-Dialog, signiert) | ok | **statisch** (Pubkey/HTTPS-Endpoint geprüft) |
| Dunkel/Hell-Modus (localStorage) | ok | UI |
| Sprache DE/EN nach System, Protokoll-Labels | **behoben** (Labels waren hart DE) | UI |
| Hilfe: Tutorial, Handbuch, Suche, DE/EN-Umschalter | ok, Inhalte aktualisiert, **Sie-Form** | UI |

## 3. Sicherheitsbefunde

| # | Schwere | Befund | Status |
|---|---|---|---|
| S1 | hoch | **Pfad-Traversal beim Download (Windows):** Dateiname kommt vom Server; `..\..\…\Startup\x.exe` landete per `joinPath` außerhalb des Zielordners. | behoben: `validate_local_target` (keine `..`-Komponenten, absolute Pfade, unter Windows keine `\ : * ? " < > |`, Gerätenamen, Punkt/Leerzeichen am Ende) |
| S2 | mittel | **TOFU ohne Rückfrage:** Beim ersten Verbinden ging das Passwort an einen ungeprüften Server (MITM fängt Zugangsdaten). | behoben: Probe holt nur den Host-Key, Anmeldung erst nach Bestätigung des Fingerprints |
| S3 | mittel | **rsync ignorierte den gemerkten Host-Key:** gespawntes ssh mit `StrictHostKeyChecking=accept-new` gegen die known_hosts der Nutzerin. | behoben: temporäre known_hosts mit dem verifizierten Key, `StrictHostKeyChecking=yes`, `HostKeyAlias`, `IdentitiesOnly` |
| S4 | mittel | **Datenverlust:** fehlgeschlagener/abgebrochener Download löschte eine bereits vorhandene Zieldatei (auch wenn der Server-Download gar nicht begann). | behoben: Teildatei + atomares Umbenennen |
| S5 | mittel | **UI konnte Host-Key setzen/überschreiben** (profile_save übernahm `host_key` aus dem Formular; Host-Wechsel behielt alten Key). | behoben: Key gehört dem Backend, Reset bei Host-/Port-Wechsel |
| S6 | niedrig | Optionsinjektion: Host/Benutzer mit führendem `-` an ssh/rsync; Shell-Kommandos ohne `--`. | behoben: Validierung + `--` |
| S7 | niedrig | FTP-PASV-Antwort konnte Datenkanal auf fremden Host lenken. | behoben: NAT-Workaround (immer Steuer-Host) |
| S8 | niedrig | WebDAV folgte Redirects auf `http://`. | behoben: Redirect-Policy nur HTTPS |
| S9 | niedrig | Keine Timeouts: hängender Server blockierte die Warteschlange dauerhaft (FTP) / Connect hing bis OS-Timeout. | behoben: FTP Connect 20 s / E/A 120 s, SSH Connect 20 s + Keepalive, WebDAV Connect 20 s |
| S10 | niedrig | CSP ohne `object-src`/`base-uri`/`form-action`. | ergänzt |
| S11 | high (Dev) | `source-map-js` < 1.2.2 (nur Build-Tooling). | auf 1.2.2 aktualisiert |
| S12 | info | profiles.json nicht-atomar geschrieben. | behoben: Temp-Datei + rename |

Geprüft ohne Befund: Capabilities (`core:default`, kein fs/shell/http-Plugin), kein
`dangerouslySetInnerHTML`/HTML-Rendering (Changelog als Text), Secrets nur im OS-Schlüsselbund
(nie in Dateien/localStorage/Logs), TLS-Prüfung nur auf ausdrücklichen Profil-Wunsch aus
(dann bleiben Handshake-Signaturen geprüft), Updater mit Minisign-Pubkey und HTTPS-Endpoint,
suppaftp lehnt CR/LF in Befehlen ab (keine FTP-Befehlsinjektion), localStorage enthält nur
Theme und Tutorial-Flag.

**Neue Abhängigkeiten:** keine neuen Crates. FTPS nutzt jetzt das suppaftp-Feature
`rustls-aws-lc-rs` statt `native-tls`; `rustls` und `rustls-platform-verifier` (direkt in
`core/Cargo.toml`) waren über reqwest bereits im Baum. Dadurch fallen `native-tls`, `openssl`,
`openssl-sys` u. a. ganz aus dem Lockfile.

## 4. Bekannte Grenzen

- Nur einzelne Dateien übertragbar — keine Ordner-Transfers, keine Mehrfachauswahl, kein Drag & Drop.
- Kein Fortsetzen abgebrochener Transfers; abgebrochene **Uploads** können eine unvollständige
  Datei auf dem Server hinterlassen (bei S3 wird der Multipart-Upload abgebrochen).
- FTPS spricht TLS 1.2 (nur ECDHE+AEAD). Server, die ausschließlich TLS 1.3 erlauben, gehen nicht.
  Hintergrund: TLS-1.3-Tickets sind einmalig und reichen bei vielen Upload-Datenkanälen nicht für
  die Session-Reuse-Pflicht von vsftpd & Co.
- FTP-Steuerverbindung ohne Keepalive: nach Server-Idle-Timeout (oft 5 min) schlägt die nächste
  Aktion fehl → „Trennen" und neu verbinden.
- rsync unter Windows braucht rsync **und** ssh im PATH (MSYS2/cwRsync); ungetestet.
- Objektspeicher-Ordner sind Marker-Objekte; Ordner-Umbenennen ist Kopie+Löschen je Objekt (nicht atomar).
- Azure Blob und GCS nur statisch geprüft (kein Test-Account).
- SMB-Domänenanmeldung, Kerberos und DFS nicht getestet.
- Der Download-Zielordner ist immer der links geöffnete Ordner. Die Überschreib-Rückfrage beruht
  auf der zuletzt geladenen Ansicht; seit Runde 2 prüft das Backend unmittelbar vor dem Start erneut
  und meldet ein inzwischen entstandenes Ziel (die UI fragt dann nach) — Restfenster: Datei entsteht
  zwischen Prüfung und Abschluss des Transfers.
- FTPS-Änderungszeiten: LIST liefert keine Zeitzone, fileport nimmt UTC an (vsftpd/ProFTPD-Standard).
  Server mit `use_localtime=YES` o. Ä. zeigen um den Zeitzonen-Versatz verschobene Zeiten.
- Schließen des Fensters während eines Downloads beendet den Transfer hart; eine versteckte
  `.name.fileport-part` kann liegen bleiben (wird beim nächsten Download derselben Datei überschrieben).

## 5. Manuelle Testliste (echte App)

Gemeinsam für **macOS** (arm64 + Intel) und **Windows** (MSI und NSIS):

1. Installation; Start zeigt Tutorial (Sie-Form); `?` öffnet Handbuch; DE/EN-Umschalter.
2. Kopfzeile zeigt `v1.0.0`; „Nach Updates suchen" meldet keinen Fehler.
3. Hell/Dunkel umschalten, Neustart → Modus bleibt.
4. SFTP-Profil mit Passwort: erstes Verbinden zeigt Fingerprint-Dialog; mit
   `ssh-keygen -lf` auf dem Server vergleichen; „Vertrauen & verbinden".
5. Erneut verbinden → kein Dialog. Server-Host-Key tauschen (oder Profil auf anderen Server
   mit gleichem Host-Eintrag zeigen) → rote Warnung mit altem/neuem Fingerprint.
6. Profil bearbeiten → Fingerprint sichtbar, „Host-Key vergessen" → nächstes Verbinden fragt neu.
7. SFTP mit Schlüsseldatei `~/.ssh/id_ed25519` (Windows: `~\.ssh\id_ed25519`).
8. Download einer Datei, die lokal existiert → Ersetzen-Rückfrage; Abbrechen/Ersetzen.
9. Großen Download (> 500 MB) mitten drin abbrechen → keine `.fileport-part`-Datei, alte Datei intakt.
10. Upload, Ordner anlegen, umbenennen, nicht-leeren Ordner löschen — lokal und remote.
11. FTPS explizit gegen vsftpd/ProFTPD/FileZilla Server (Standardkonfiguration); mehrere Uploads
    hintereinander. FTPS implizit (Port 990).
12. WebDAV (z. B. Nextcloud) mit Datei `A & B.txt`; Wurzel ohne leeren Eintrag.
13. SMB-Freigabe (Windows-Freigabe/NAS), auch `DOMÄNE\benutzer`.
14. S3 (AWS oder Hetzner/B2): Ordner anlegen, umbenennen, löschen.
15. Azure Blob und GCS mit echtem Konto: Verbinden, Up/Download.
16. rsync-Profil (macOS: ab Werk; Windows: rsync+ssh installiert) — Up/Download, kein
    Eintrag in `~/.ssh/known_hosts`.
17. Profil löschen → Rückfrage; Schlüsselbund-Eintrag `com.lan-solo.fileport` ist weg.

Nur **Windows**:

18. Lokale Pane startet in `C:\Users\<name>`; ↑ bis `C:\` und dort bleiben; Pfadfeld `D:\` wechselt das Laufwerk.
19. Download einer Server-Datei namens `a:b.txt`, `CON.txt`, `a\b.txt` oder `x\..\y.txt` (z. B. auf
    Linux-SFTP anlegen) → klarer Fehler in der Warteschlange (Prüfung im Backend), keine Datei
    außerhalb des Zielordners.
20. Verzeichnis-Symlink/Junction lokal löschen → nur der Link verschwindet.
21. rsync-Transfer: kein aufblitzendes Konsolenfenster.
22. Datei im Ziel geöffnet (z. B. in Excel) und Download mit Ersetzen → verständlicher Fehler,
    keine Teildatei bleibt liegen.
23. Fensterverhalten/Skalierung bei 125 %/150 %; Schließen-Knopf beendet die App.

Nur **macOS**:

24. DMG notarisiert (`spctl -a -vv`); erster Start ohne Gatekeeper-Warnung.
25. Schlüsselbund-Abfrage beim ersten Verbinden („Immer erlauben").

## 6. Verifikation (unabhängige Zweitprüfung, 2026-10-19)

Skeptische Nachprüfung des Audits auf Stand `46e28b9`; Befunde unten sind lokal committet.

**Selbst ausgeführt**

- `cargo test --workspace`: 20/20 Unit-Tests grün (13 core, 7 app; ITs ohne `FILEPORT_IT` übersprungen),
  nach den Fixes weiterhin grün (Part-Datei-Test erweitert).
- `cargo clippy --workspace --all-targets`: 0 Warnungen. `pnpm test`: 4/4. `pnpm build`: ok. `pnpm audit`: 0.
- Docker-ITs neu gefahren: SFTP (atmoz/sftp) und FTPS explizit (vsftpd, `require_ssl_reuse`) grün;
  FTPS implizit ohne `FILEPORT_IT_FTPS_IMPLICIT` übersprungen.
- Zusatztest SFTP (Wegwerf-Test, nicht im Repo): Ordner mit Verzeichnis-Symlink rekursiv gelöscht →
  Symlink-Ziel bleibt unangetastet; 20-MB-Datei `Größe & Leer.bin` hoch/runter, BLAKE3 identisch;
  `probe_host_key`-Fingerprint == `ssh-keygen -lf` des Servers; derselbe Wert auch aus dem
  UI-Algorithmus `fingerprintOf` (Node, WebCrypto).
- Versionen: package.json, tauri.conf.json, src-tauri/Cargo.toml, Cargo.lock (`fileport-app`) = 1.0.0;
  `fileport-core` bleibt als interne Bibliothek 0.1.7. build.yml: `pnpm test` nutzt Node-22-Glob
  (`--test tests/*.test.ts` funktioniert auch unter cmd.exe auf Windows); aws-lc-sys war schon vor 1.0
  (über reqwest) im Windows-Build — kein neues Build-Risiko. Signatur/Notarisierung unverändert.

**Gefunden und behoben**

1. *Datenverlust ohne Rückfrage auf macOS/Windows:* Die Ersetzen-Rückfrage verglich Namen
   case-sensitiv. Auf APFS/NTFS hätte ein Download `Report.pdf` eine vorhandene `report.pdf`
   stillschweigend ersetzt. Jetzt Vergleich ohne Groß-/Kleinschreibung (Up- und Download; eine
   zusätzliche Rückfrage auf case-sensitiven Systemen ist harmlos).
2. *Server-Namen mit `\` oder `/`* (auf Linux-Servern legal) wurden unter Windows in einen lokalen
   Pfad mit Unterordnern eingebaut. Der Backend-Check fing `..` ab, nicht aber das Schreiben in einen
   vorhandenen Unterordner. Die UI lehnt solche Downloads jetzt mit Hinweis ab.
3. *Abbrechen-Klick ging verloren*, wenn er eintraf, bevor `transfer_start` den Transfer registriert
   hatte (beide Commands laufen nebenläufig). Abbrüche werden jetzt vorgemerkt.
4. *Sehr lange Dateinamen* (> 200 Byte): `.<name>.fileport-part` überschritt die 255-Byte-Grenze,
   der Download scheiterte. Dann wird eine kurze Hash-Teildatei verwendet (Unit-Test).
5. *Irreführende Sicherheitsaussage:* Handbuch („jede Verbindung … immer verschlüsselt“), README und
   Paketbeschreibung (nannte noch FTP(S)/HTTP(S)) behaupteten durchgängige Verschlüsselung. SMB über
   die `smb2`-Crate verschlüsselt nur, wenn der Server SMB3-Verschlüsselung verlangt, sonst ist es
   nur signiert. Handbuch DE/EN, README und `longDescription` sagen das jetzt ehrlich.

**Weiter offen (nicht behebbar ohne Hardware/Konten)**

- Kein nativer Windows-Lauf: rsync-`-e`-Quoting mit Windows-Pfaden (temporäre known_hosts,
  `-i C:\…`) für cwRsync/MSYS-ssh, Junction-Löschen, `CREATE_NO_WINDOW`: nur statisch geprüft.
- Azure/GCS: weiterhin nur statisch geprüft und über InMemory-Unit-Tests.
- Backend-Fehlermeldungen (core/app) sind deutsch und erscheinen so auch in der EN-Oberfläche.
- `cargo audit` lokal nicht verfügbar.
- Meine UI-Änderungen (Case-insensitiv-Check, Namen mit Trennern) sind nur typgeprüft/gebaut,
  nicht im Mock-Harness gefahren.

## 7. Runde 2 (2026-10-20)

Basis `5929792`. Ziel: Reste aus §6 im Code beheben und die App gegen die Fehlermuster L1–L13 aus den
anderen Apps prüfen. Version bleibt 1.0.0.

**Behoben**

1. *Server-Dateinamen im Backend geprüft (L5):* `transfer_start` bekommt für Downloads Zielordner und
   Namen getrennt; `download_target` prüft den Namen plattformgerecht (Windows: `\ / : * ? " < > |`,
   Steuerzeichen, Punkt/Leerzeichen am Ende, Gerätenamen inkl. `COM0`/`LPT0`, `CONIN$`, `COM¹–³`),
   hängt genau eine Komponente an und prüft das Ergebnis gegen. Die UI-Sperre aus §6.2 entfällt.
2. *Kein stilles Ersetzen (L5/L9/L11):* Downloads und Uploads ohne bestätigtes `overwrite` scheitern,
   wenn das Ziel inzwischen existiert (Upload: Prüfung auf der Gegenstelle, ohne Groß-/Kleinschreibung);
   die UI zeigt dann die Ersetzen-Rückfrage statt eines Fehlers. Umbenennen (lokal und remote) ersetzt
   nie ein vorhandenes Ziel; nur Groß-/Kleinschreibung desselben Eintrags ändern bleibt erlaubt.
   Neue lokale Ordner/Umbenennungen folgen denselben Windows-Namensregeln.
3. *Windows-Sperren (L5):* Das abschließende Umbenennen der Teildatei und das Speichern von
   `profiles.json` wiederholen bei Sharing-/Lock-Violation (32/33) und „Zugriff verweigert“ (5) bis ~3 s.
   Nacktes `C:` wird als `C:\` gelistet.
4. *Fehlermeldungen in UI-Sprache (L2):* `src/errors.ts` übersetzt alle festen deutschen Bausteine aus
   core und App für die englische Oberfläche (Toasts, Warteschlange, Update-Dialog). Ein Wächter-Test
   (`tests/errors.test.ts`) liest alle Fehler-Literale aus dem Rust-Code und schlägt fehl, wenn einer
   ohne Übersetzung bleibt. Häufige OS-Fehler (`io_text`) haben feste Formulierungen mit OS-Code.
5. *BOM/CRLF aus Windows-Editoren (L4):* SSH-Schlüsseldateien, GCS-JSON und `profiles.json` werden mit
   UTF-8-BOM und CRLF gelesen (Unit-Test mit Wegwerf-ed25519-Schlüssel).
6. *Zeitzonen (L10):* SCP/rsync listen per `TZ=UTC0 ls -la` — vorher waren Änderungszeiten um den
   Zeitzonen-Versatz des Servers verschoben (IT gegen Container mit `TZ=Asia/Tokyo`: ohne Fix 9 h
   daneben, mit Fix grün). Datumsformat folgt der UI-Sprache. Das Update-Datum kommt als Unix-Zeit
   (vorher `OffsetDateTime::to_string()`, das `new Date()` nicht lesen konnte — das Datum fehlte).
7. *Trennen bei laufenden Transfers (L9/L11):* Rückfrage „Abbrechen und trennen?“; erst nach
   Bestätigung werden laufende und wartende Transfers dieser Verbindung abgebrochen. Fehler beim Trennen
   werden gemeldet statt verschluckt; eine offene Ersetzen-Rückfrage schließt sich.
8. *Veraltete Listen (L9):* Antworten früherer `fs_list`-Aufrufe überschreiben die aktuelle Ansicht nicht mehr.
9. *Profile (L9/L11):* Lese-Ändern-Schreiben auf `profiles.json` serialisiert (ein laufendes `connect`
   schrieb ein gelöschtes Profil zurück); Schlüsselbund-Fehler beim Löschen werden gemeldet.
10. *Update-Fehler* zeigte „Update fehlgeschlagen: Update fehlgeschlagen: …“ — Präfix nur noch einmal.
11. *Layout bei 800 × 540 (Mindestgröße):* Protokoll-Auswahl und Port liefen seitlich aus der Pane,
    lange Verbindungs-Labels überlappten die Eintragszahl, Rückfragezeilen quetschten die Dateiliste.
    Behoben (CSS), volle Texte als Tooltip. „Name?“ ersetzt durch einen ganzen Satz (DE/EN).

**Sweep L1–L13 ohne Befund:** L1 (kein `window.confirm/alert/prompt`, alle Rückfragen sind eigene
Dialoge), L3 (Sprache aus `navigator.language`), L6 (keine Tastenkürzel-Labels), L7 (kein Remote-HTML;
Changelog als Text), L8 (Transfers als Tasks, Schlüsselbund in `spawn_blocking`, FTP in
`spawn_blocking`), L13 (Updater-Endpoint/Pubkey unverändert, Vergleich durch tauri-plugin-updater,
Fehler übersetzt).

**Läufe**

- `cargo test --workspace`: 27 Unit-Tests grün (15 core, 12 app). `cargo clippy --workspace --all-targets`: 0.
- ITs neu: SCP (linuxserver/openssh-server, `TZ=Asia/Tokyo`, neue mtime-Prüfung) und rsync grün.
- `pnpm test`: 10/10 (6 neu für Fehlerübersetzung). `pnpm build`: ok. `pnpm audit`: 0.
- **Windows-Typecheck** `cargo check --target x86_64-pc-windows-msvc --workspace --tests` vom Mac: grün.
  Dafür wird das Build-Skript von aws-lc-sys per `--config target.x86_64-pc-windows-msvc.aws_lc_0_44_0.*`
  übersprungen (Bindings `universal_prefixed`) und blake3 mit `CARGO_FEATURE_PURE=1` ohne C/ASM gebaut —
  nur Typprüfung, kein Linken.
- `cargo audit`: Patch-Updates `rustls` 0.23.43→0.23.46 (4 Advisories), `quinn-proto` 0.11.16→0.11.19
  (5 Advisories), dazu `chacha20`/`wnaf` (yanked) und `zvariant`/`zbus_names` (Linux). Offen: `rsa`
  (RUSTSEC-2023-0071, Marvin, kein Fix verfügbar; russh nutzt es für RSA-Client-Schlüssel — ed25519
  empfohlen) und `zip` 4.6.1 (3 Advisories zum Entpacken, über tauri-plugin-updater; das Update auf
  2.12 brächte neue Crates. Tauri-v2-Updater-Artefakte sind signiert und unter macOS tar.gz, unter
  Windows Installer — `zip::extract` läuft für fileport nicht).
- UI im Headless-Chrome gegen gemocktes IPC (Mock bildet Backend-Regeln und deutsche Fehlertexte nach),
  800 × 540, DE/EN × hell/dunkel: 98 Prüfpunkte grün — Update-Dialog/-Fehler, SMB-Anmeldefehler,
  Formular, Host-Key-Dialog, Ersetzen-Rückfrage (Download Groß/klein, Upload, zweiter Upload derselben
  Datei), Namensfehler `a:b.txt` in der Warteschlange, Umbenennen-Kollision, Trennen mit laufendem
  Transfer, Handbuch; keine Überläufe, keine Konsolenfehler, keine deutschen Wörter in EN.

**Nur auf echter Hardware prüfbar**

- Windows nativ: Wiederholung bei Virenscanner-/OneDrive-Sperre (Liste §5.22), Gerätenamen und
  `C:`-Wurzel, Credential Manager, rsync mit cwRsync/MSYS, Junctions, MSI/NSIS.
- macOS nativ: Schlüsselbund-Abfrage, notarisiertes DMG, echter Tauri-Updater gegen GitHub
  (Datum im Dialog, Fehlertexte).
- Azure/GCS-Konten, SMB mit Domäne/Verschlüsselung, FTPS gegen ProFTPD/FileZilla (Zeitzone der
  LIST-Ausgabe).

Zusatz zur manuellen Liste: 26. Download/Upload starten, während das Ziel extern angelegt wird →
Ersetzen-Rückfrage statt stillem Ersetzen. 27. „Trennen“ bei laufendem Download → Rückfrage; nach
Bestätigung „abgebrochen“, keine Teildatei. 28. SCP/rsync gegen Server in anderer Zeitzone →
Änderungszeiten wie lokal per `ls -l` auf dem Server (umgerechnet). 29. EN-Systemsprache: ein
Verbindungsfehler (falsches Passwort) erscheint englisch.

## 8. Runde 2 – Verifikation (2026-10-20)

Unabhängige Gegenprüfung von `5929792..eece5c4` mit dem Ziel, die Angaben aus §7 zu widerlegen.

**Bestätigt (selbst ausgeführt):** `cargo test --workspace` grün (jetzt 28 Unit-Tests), `cargo clippy
--workspace --all-targets` 0 Warnungen auf macOS; Windows-Typecheck und -Clippy
(`x86_64-pc-windows-msvc`, gleiches Rezept wie §7: aws-lc-sys-Buildskript übersprungen, blake3 pure)
grün — weiterhin nur Typprüfung, die `cfg!(windows)`-Zweige der Tests liefen nie. `pnpm test` 10/10,
`pnpm build` ok. `cargo audit`: nur noch die in §7 genannten offenen Punkte (`rsa`, `zip`) plus
„unmaintained“-Hinweise für Linux-only-Crates (zbus/zvariant, glib, unic-*, proc-macro-error).
UI-Lauf im Headless-Chrome (Mock aus §7, um einen Fall erweitert): 106/106 Prüfpunkte, DE/EN ×
hell/dunkel, 800 × 540. Stichproben L1/L3/L6/L7/L10/L13 ohne neuen Befund (kein `window.confirm`,
Capability nur `core:default`, Updater-Endpoint/Pubkey unverändert, Jahreskorrektur jahresloser
`ls`/`LIST`-Zeilen übernimmt suppaftp).

**Gefunden und behoben**

1. *Ersetzen-Rückfrage konnte Einträge verlieren (L9/L11):* Der Abarbeiter lief weiter, während die
   Rückfrage offen war. Meldete ein zweiter Eintrag ebenfalls „existiert bereits“, verdrängte er die
   offene Rückfrage — der erste Eintrag verschwand kommentarlos aus Warteschlange und Dialog.
   Reproduziert im UI-Lauf (dreimal dieselbe Datei hochladen: ohne Fix fehlt die zweite Rückfrage).
   Jetzt hält die Warteschlange an, solange eine Rückfrage offen ist.
2. *Umbenennen nur der Groß-/Kleinschreibung auf Servern mit Unterscheidung (L5/L11):* Die Kollisions-
   prüfung entfiel bei reinem Umschreiben komplett. Auf einem Linux-Server mit „a.txt“ und „A.txt“
   ersetzte „a.txt → A.txt“ per `mv` (SCP/rsync) still die vorhandene „A.txt“. Jetzt wird immer
   gelistet; nur der Quell-Eintrag selbst zählt nicht als Konflikt (Unit-Test `rename_conflict`).

**Weiter offen / nur auf echter Hardware prüfbar:** unverändert wie §7 (Windows-Sperren, Gerätenamen,
`C:`-Wurzel, Credential Manager, cwRsync/MSYS, MSI/NSIS; macOS-Schlüsselbund, notarisiertes DMG,
echter Updater; Azure/GCS/SMB-Domäne/ProFTPD). Restfenster zwischen Zielprüfung und Ende eines
Transfers sowie die versteckte Teildatei beim Schließen des Fensters während eines Downloads bleiben.
Upload-Prüfung listet den Zielordner: Bei Objektspeichern mit Schreib-, aber ohne List-Recht scheitert
ein Upload jetzt mit dem List-Fehler.

Zusatz zur manuellen Liste: 30. Zwei Dateien gleichen Namens nacheinander einreihen, deren Ziel
während der Warteschlange entsteht → Rückfrage für jede, keine geht verloren. 31. SFTP/SCP gegen
Linux-Server mit „a.txt“ und „A.txt“: „a.txt“ in „A.txt“ umbenennen → Fehlermeldung, beide bleiben.
