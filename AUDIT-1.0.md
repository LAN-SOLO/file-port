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
| Rust-Advisories | `cargo audit` | **nicht gelaufen** — cargo-audit ist lokal nicht installiert |

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
- Der Download-Zielordner ist immer der links geöffnete Ordner; Überschreib-Rückfrage basiert auf
  der zuletzt geladenen Ansicht (⟳ bei extern geänderten Ordnern).

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
19. Download einer Server-Datei namens `a:b.txt`, `CON.txt` oder `x\..\y.txt` (z. B. auf Linux-SFTP
    anlegen) → klarer Fehler in der Warteschlange, keine Datei außerhalb des Zielordners.
20. Verzeichnis-Symlink/Junction lokal löschen → nur der Link verschwindet.
21. rsync-Transfer: kein aufblitzendes Konsolenfenster.
22. Datei im Ziel geöffnet (z. B. in Excel) und Download mit Ersetzen → verständlicher Fehler,
    keine Teildatei bleibt liegen.
23. Fensterverhalten/Skalierung bei 125 %/150 %; Schließen-Knopf beendet die App.

Nur **macOS**:

24. DMG notarisiert (`spctl -a -vv`); erster Start ohne Gatekeeper-Warnung.
25. Schlüsselbund-Abfrage beim ersten Verbinden („Immer erlauben").
