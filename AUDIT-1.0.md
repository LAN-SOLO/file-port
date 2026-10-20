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
- FTPS-Änderungszeiten: Seit Runde 3 per MLSD (UTC), wenn der Server MLST/MLSD kann (ProFTPD,
  FileZilla Server, Pure-FTPd). Ohne MLSD (vsftpd) bleibt LIST ohne Zeitzone, fileport nimmt UTC an —
  Server, die per LIST Ortszeit senden *und* kein MLSD können, zeigen verschobene Zeiten.
- ~~Schließen des Fensters während eines Downloads hinterlässt `.name.fileport-part`~~ — seit Runde 3
  behoben (Journal + Aufräumen beim Beenden und beim nächsten Start). Rest: läuft ein **zweites**
  fileport-Fenster/-Prozess gleichzeitig, kann dessen Start die Teildatei eines laufenden Downloads
  der ersten Instanz entfernen — der Download scheitert dann mit Fehler, Ziel bleibt unversehrt.
- Ordner ohne List-Recht (nur Schreibrecht: SFTP-Briefkasten, S3/Azure/GCS mit reiner Put-Policy)
  lassen sich nicht öffnen und daher nicht als Upload-Ziel wählen; Objektspeicher verlangen schon beim
  Verbinden List-Recht auf die Bucket-Wurzel (sofortige Prüfung der Zugangsdaten).

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

## 9. Runde 3 (2026-10-20, final)

Basis `7369410`. Ziel: jede Aussage aus Handbuch (DE/EN), README und UI-Texten gegen den Code prüfen,
Nutzerreisen inkl. Fehlerfällen durchspielen, Reste aus §8 beheben. Version bleibt 1.0.0.

**Behoben (Code)**

1. *Liegengebliebene Teildatei beim Schließen/Neustart (Rest §4/§8):* Downloads tragen ihre
   `.name.fileport-part` in ein Journal ein (`pending-parts.txt` im Konfigordner neben
   `profiles.json`). `RunEvent::Exit` (letztes Fenster zu, Cmd+Q) und der Update-Neustart brechen
   laufende Transfers ab und löschen deren Teildateien sofort; nach Absturz/Kill räumt der nächste
   Start anhand des Journals auf. Gelöscht wird nur, was wie eine fileport-Teildatei aussieht
   (absoluter Pfad, `.`-Präfix, Endung `.fileport-part`, reguläre Datei) — Unit-Tests
   `journal_cleans_only_leftover_part_files`, `shutdown_removes_running_part_files`.
2. *Upload-Vorprüfung ohne List-Recht (Regression aus Runde 2):* `remote_existing` wertet
   „Zugriff verweigert“, „Zugriff abgelehnt“ (Objektspeicher 401/403) und „Nicht gefunden“ (FTP 550)
   beim Listen des Zielordners als „nicht prüfbar“ und lässt den Upload starten; andere Fehler
   (Verbindung weg) brechen weiter ab. Unit-Test mit Ordner `0o300` (nur Schreib-/Suchrecht).
   Hinweis: In der Oberfläche ist ein solcher Ordner nicht öffenbar (siehe §4) — der Fix greift für
   Ordner, deren Leserecht sich zwischen Anzeige und Upload ändert, und für FTP-Server, die auf LIST
   eines leeren Ordners 550 antworten.
3. *FTPS-Zeiten (Rest §4):* Einmal pro Verbindung `MLST` auf der Steuerverbindung; antwortet der
   Server, holt fileport zusätzlich zu LIST ein `MLSD` und übernimmt daraus nur die UTC-Zeiten.
   Art und Größe bleiben aus LIST (manche Server melden Ordner-Symlinks in MLSD als `type=dir` —
   rekursives Löschen liefe sonst ins Link-Ziel). MLSD wird nie „auf Verdacht“ geschickt: suppaftp
   baut vor der Antwort den TLS-Datenkanal auf, ein abgelehntes MLSD hinge 120 s. `FEAT` scheidet aus
   (suppaftp 10 wartet bei einzeiliger `211`-Antwort auf weitere Zeilen). Eigener MLSD-Parser
   (Sekundenbruchteile, `;` im Namen, `OS.unix=slink`). Unit-Tests + IT (siehe unten).
4. *Upload in die Server-Wurzel (neu gefunden):* Scheiterte das erste Listing der Gegenstelle (z. B.
   Home ohne Leserecht), war der Zielordner `''` — `joinPath('', name)` ergab `/name`, ein Upload
   per Pfeil oder Doppelklick landete in der Server-Wurzel. Jetzt sind Pfeile und Doppelklick
   gesperrt, solange auf der Zielseite kein Ordner geladen ist (UI-Lauf J10).

**Falsche/irreführende Aussagen korrigiert (Text)**

- Tutorial Schritt 1 (DE/EN) verwies auf einen Handbuch-Abschnitt „Protokolle“, den es nicht gibt →
  „Handbuch › Verbindungen & Profile“.
- Tutorial Schritt 2 (DE/EN): „SFTP/SCP/rsync/FTPS: … oder SSH-Schlüsseldatei mit Passphrase“ —
  FTPS hat keine Schlüsseldatei, rsync keine Passwort-Anmeldung. Aufgeteilt in SFTP/SCP, rsync, FTPS.
- Handbuch „Dateiverwaltung“ (DE/EN): „ProFTPD sendet per LIST UTC“ war als Standard für alle
  FTPS-Server formuliert; jetzt MLSD/LIST-Verhalten beschrieben.
- Handbuch „Übertragungen“ (DE/EN): Verhalten beim Schließen/Update-Neustart ergänzt; nicht
  auflistbare Ordner als Grenze genannt.
- README: „Alle Backends sind mit Integrationstests gegen echte Server abgedeckt“ — falsch für
  Azure Blob und GCS (nur Unit-Test/Code-Review). Korrigiert.

**Aussagen geprüft, zutreffend** (Code gelesen; „UI“ = im Mock-Harness gefahren): Protokollliste und
Standardports (22/21/990/445); nur `https://` für WebDAV/S3 (`profiles.rs`); rsync ohne Passwort
(`RsyncBackend::connect` lehnt ab); SCP-Verwaltung per `ls`/`mkdir`/`rm -r --`/`mv --`;
SMB `DOMÄNE\benutzer`; Option „selbstsignierte Zertifikate“ nur bei FTPS/WebDAV/S3 (UI);
Host-Key-Dialog vor Anmeldung, Abbrechen verbindet nicht (UI J2); „Host-Key vergessen“ (UI J9);
Profil löschen mit Rückfrage (UI J9); Ersetzen-Rückfrage ohne Groß-/Kleinschreibung, Klick neben den
Dialog bricht ab ohne Transfer (UI J4); sequenzielle Warteschlange, ✕ für laufende *und* wartende
Einträge, „Erledigte entfernen“ entfernt fertig/Fehler/abgebrochen (UI J6); Fehlerursache am
Eintrag + Tooltip (UI J3); Zielseite aktualisiert sich nach Erfolg (UI J5); Ordner löschen rekursiv
bei allen Protokollen (SFTP/FTPS/SMB rekursiv, SCP/rsync `rm -r`, WebDAV `DELETE` auf Collection,
Objektspeicher Präfix); lokale Symlinks werden als Link gelöscht (`remove_dir_all` folgt keinen
Links, Windows-Junctions per `remove_dir`); Namensregeln (UI J7); Enter/Esc beim Umbenennen (UI J7);
Pfadfeld + Enter, falscher Pfad lässt Ansicht stehen (UI J2); Wiederholung beim Ersetzen unter
Windows bis ~3 s (`rename_with_retry`); Hilfe-Knopf unten rechts, Theme-Knopf in der Kopfzeile,
Theme und Tutorial-Flag überleben Neustart (UI J1); Update-Check beim Start, Installation erst nach
Klick (UI aus Runde 2). Handbuch DE und EN haben dieselben Abschnitte und Aussagen (Absatz für Absatz
abgeglichen).

**Nutzerreisen (Headless-Chrome gegen gemocktes IPC, DE und EN, 800 × 540, 83/83 grün)**

Mock wie §7/§8, erweitert: Profile überleben einen Neustart (wie `profiles.json`), Fehlerfälle als
`transfer_done`-Events mit den echten deutschen Engine-Texten, Ordner ohne Leserecht. Keine
`window.confirm`-Aufrufe (alle Rückfragen sind eigene Dialoge, siehe §7 L1).

- J1 Erster Start: Tutorial (6 Schritte) → schließen → Theme umschalten → Neustart: kein Tutorial,
  Theme bleibt; Handbuchsuche („Teildatei“/„partial file“, „MLSD“, ohne Treffer).
- J2 Profil anlegen: Pflichtfeld Name, Host-Key-Dialog abbrechen (nicht verbunden), erneut und
  vertrauen; Doppelklick in Ordner, Ebene hoch, Pfadfeld, falscher Pfad.
- J3 Fehlerfälle Download: Datenträger schreibgeschützt (OS 30), voll (OS 28), Datei gesperrt
  (Windows OS 32) → Fehler am Eintrag in der UI-Sprache, Warteschlange läuft weiter.
- J4 Ersetzen-Rückfrage per Klick neben den Dialog abbrechen → kein `transfer_start`.
- J5 Upload per Doppelklick, Server-Liste aktualisiert.
- J6 Laufenden Download per ✕ abbrechen, „Erledigte entfernen“.
- J7 Lokal: ungültiger Name, neuer Ordner, Löschen abbrechen/bestätigen, Umbenennen per Esc.
- J8 Neustart mitten im Download: leere Warteschlange, Profilliste, App bedienbar (die Teildatei
  selbst: Unit-Tests oben).
- J9 Profil bearbeiten, Host-Key vergessen, Profil löschen mit Rückfrage.
- J10 Gegenstelle ohne Leserecht im Home: übersetzter Fehler, Upload-Pfeil gesperrt, Doppelklick
  startet nichts (ohne Fix: Upload nach `/foto.jpg`).
- Regression: UI-Lauf aus §8 (106 Prüfpunkte, DE/EN × hell/dunkel) unverändert grün.

**Läufe**

- `cargo test --workspace`: 33 Unit-Tests grün (17 core, 16 app). `cargo clippy --workspace
  --all-targets`: 0 Warnungen. Windows-Typecheck und -Clippy (`x86_64-pc-windows-msvc`, Rezept §7): grün.
- ITs FTPS: vsftpd explizit + implizit (kein MLSD → LIST-Pfad, neue mtime-Prüfung ±10 min) grün;
  pyftpdlib 2.2 mit TLS (Wegwerf-venv, kein Projekt-Abhängigkeit) mit **LIST in Asia/Tokyo-Ortszeit
  und MLSD in UTC**: Test grün, d. h. die Zeiten kommen aus MLSD (LIST allein läge 9 h daneben,
  per `ftplib` gegengeprüft); Serverlog zeigt `MLST` einmal, danach `LIST`+`MLSD` je Ordner.
- `pnpm test` 10/10, `pnpm build` ok.
- Keine neuen Abhängigkeiten (Rust und JS); `async-trait` nicht in die App übernommen, Test nutzt
  `LocalBackend`.

**Nur auf echter Hardware prüfbar (zusätzlich zu §7/§8)**

32. Großen Download starten, Fenster schließen → keine `.name.fileport-part` im Zielordner
    (macOS: Cmd+Q und rotes ✕; Windows: ✕ und Alt+F4). Danach App per Taskmanager/`kill -9`
    während eines Downloads beenden, neu starten → Teildatei verschwindet beim Start.
33. Update installieren, während ein Download läuft → nach dem Neustart keine Teildatei.
34. FTPS gegen ProFTPD (Standard, `TimesGMT off`) und FileZilla Server: Änderungszeiten wie auf dem
    Server in UTC umgerechnet; Ordner-Symlink löschen → nur der Link verschwindet.
35. SFTP-Konto mit Home ohne Leserecht: Fehler beim Verbinden, Pfeile gesperrt.

## 10. Runde 3 – Verifikation (2026-10-20)

Unabhängige Gegenprüfung von `7369410..52db370` mit dem Ziel, die Angaben aus §9 zu widerlegen.

**Bestätigt (selbst ausgeführt):** `cargo test --workspace` grün (33 Unit-Tests: 17 core, 16 app),
`cargo clippy --workspace --all-targets` 0 Warnungen; Windows-Typecheck und -Clippy
(`x86_64-pc-windows-msvc`, Rezept §7) grün — weiterhin nur Typprüfung. `pnpm test` 10/10,
`pnpm build` ok. `cargo audit`: unverändert `rsa` + 3× `zip` plus „unmaintained“-Hinweise (Linux).
FTPS-IT `ftps_roundtrip_against_real_server` neu gegen vsftpd (delfer/alpine-ftp-server, MLST → 500,
LIST-Pfad) und gegen pyftpdlib mit TLS, LIST in Asia/Tokyo-Ortszeit und MLSD in UTC grün; Serverlog
zeigt `MLST` einmal, danach `LIST`+`MLSD` je Ordner. Der implizite FTPS-Test lief in dieser Runde
nicht (braucht den zweiten Container auf Port 9990). UI-Lauf §9: jetzt 87/87, §8: 106/106, keine
Konsolenfehler; ein einzelner Lauf blieb einmal in J7 an einem CDP-Aufruf hängen (Harness, nicht App),
drei Wiederholungen danach grün.

Code-Review des Diffs ohne Befund: MLST läuft über die Steuerverbindung (suppaftp liest die
mehrzeilige 250-Antwort vollständig), 500/501/502/504 schalten dauerhaft auf LIST, andere Fehler
prüfen beim nächsten Listing erneut; Journal/`shutdown()` sind idempotent (Update-Neustart ruft es,
danach `RunEvent::Exit` noch einmal); Windows: Rust öffnet Dateien mit `FILE_SHARE_DELETE`, das
Löschen der Teildatei bei noch offenem Handle ist also möglich bzw. greift beim Prozessende.

**Help-Stichproben (nicht in §9 genannt), alle im Code bestätigt:** Host-Key verworfen bei Änderung
von Host/Port (`profile_save`); leeres Geheimnis-Feld lässt das gespeicherte unverändert; Profil
löschen entfernt das Geheimnis; `~/.ssh/…` wird aufs Home aufgelöst; WebDAV folgt Weiterleitungen
nur auf https; WebDAV/S3 nur `https://`; FTPS nur TLS 1.2; rsync-ssh mit eigener `UserKnownHostsFile`
und `StrictHostKeyChecking=yes`; Ersetzen-Wiederholung unter Windows bis ~3 s
(`rename_with_retry`); Updater-Endpoint `LAN-SOLO/file-port` mit Signatur-Pubkey.

**Gefunden und behoben**

1. *„Neuer Ordner“ ohne geladenen Ordner legte in der Server-Wurzel an* (gleiche Ursache wie J10
   aus §9): Scheitert das erste Listing (Start-Ordner ohne Leserecht), ist der Ordner der Seite leer,
   `joinPath('', name)` ergibt `/name` — der Ordner landete in `/` statt im gemeinten Verzeichnis.
   Im UI-Lauf reproduziert (Mock protokolliert `fs_mkdir` mit `/X`). Jetzt ist „Neuer Ordner“ gesperrt,
   bis die Seite einen Ordner geladen hat (zusätzlich Absicherung in `submitPrompt`); nach ↑ oder
   Pfadfeld in einen lesbaren Ordner sind Knopf und Pfeile wieder frei (neue Prüfpunkte J10).
   Datei: `src/components/FilePane.tsx`; Handbuch „Dateiverwaltung“ DE/EN und README ergänzt.

**Nur auf echter Hardware prüfbar:** unverändert §5/§7/§8 und §9 Punkte 32–35 (Fenster schließen/
Cmd+Q/Alt+F4/Update-Neustart während eines Downloads, `kill -9` + Neustart, ProFTPD/FileZilla
Server mit MLSD, Home ohne Leserecht), dazu: SFTP-Konto mit unlesbarem Home → „Neuer Ordner“ gesperrt.

## 11. Runde 4 – Regressionen (platform) (2026-10-20)

Prüfung von `7369410..095dfba` auf Nebenläufigkeit, Plattform und Lebenszyklus (Exit-/Updater-Hooks,
Sperren, Windows-Zweige, Pfade, Handbuch DE/EN).

**Gefunden und behoben**

1. *Zweite Instanz löschte die Teildatei eines laufenden Downloads* (Regression aus §9 Punkt 1): Das
   Journal `pending-parts.txt` war eine Datei für alle Instanzen, und jeder Start räumte es
   bedingungslos ab. Läuft in Instanz A ein Download und startet Instanz B (unter Windows/Linux
   startet jeder Doppelklick eine neue Instanz; ein Single-Instance-Plugin gibt es nicht), löschte B
   A's `.name.fileport-part` — A's Download scheiterte beim abschließenden Umbenennen; zudem
   überschrieben sich die Journale gegenseitig. Jetzt führt jede Instanz `pending-parts-<pid>.txt`
   und hält eine Dateisperre (`File::try_lock`, std) auf `pending-parts-<pid>.lock`; der Start räumt
   nur Journale ab, deren Sperre frei ist (Prozess beendet/abgestürzt). Das alte gemeinsame Journal
   früherer Testbuilds wird weiter abgeräumt. Unit-Test `second_instance_keeps_running_part_files`
   (vorher rot: Teildatei weg), deckt auch den Absturzfall ab.

**Geprüft ohne Befund:** `shutdown()` blockiert den Main-Thread nicht (`try_lock` auf die
tokio-Mutex, Journal-Mutex nie über `await` gehalten), läuft beim Update-Neustart und danach bei
`RunEvent::Exit` idempotent; `is_part_file` schützt Zieldateien beim Wettlauf Umbenennen ↔ Beenden;
MLST/MLSD-Zustand pro Verbindung (`AtomicU8`) ohne Sperre über langsame IO hinaus; neue Pfeil-/
„Neuer Ordner“-Sperre trifft keine Gegenstelle im Normalfall (alle `initial_dir` liefern einen
nicht leeren Pfad, Objektspeicher/WebDAV/SMB `/`); Handbuch-Texte DE/EN deckungsgleich und mit
dem Fix weiter zutreffend.

**Läufe:** `cargo test --workspace` grün (17 app-Unit-Tests, 17 core), `cargo clippy --workspace
--all-targets` 0 Warnungen, Windows-Clippy (`x86_64-pc-windows-msvc`, Rezept §7) grün — nur
Typprüfung. `pnpm test` grün, `pnpm build` ok.

**Nur auf echter Hardware prüfbar:** Windows — Download in Instanz A, zweite Instanz starten →
Download in A läuft fertig; A per Taskmanager beenden, neu starten → Teildatei weg.
