# fileport

Datei-Transfer-Client für macOS, Windows und Linux: spricht SFTP, SCP,
rsync über SSH, FTPS, FTP, WebDAV (HTTPS) und SMB 2/3, verbindet Clouds von
S3-kompatiblen Diensten über Azure Blob und Google Cloud Storage bis
(geplant) Dropbox, Google Drive und OneDrive — und überträgt mit
BLAKE3-Prüfsumme und Warteschlange (Direkt-Drop von Gerät zu Gerät geplant).
Klartext-FTP nur nach ausdrücklicher Bestätigung (Warnung im Profil), kein SMB1; SMB ist nur verschlüsselt, wenn der Server
SMB3-Verschlüsselung verlangt (sonst signiert). **Free** bleibt dauerhaft kostenlos und voll
nutzbar; **fileport bridged** (12 €/Jahr = 1 €/Monat) ergänzt die
Server-zu-Server-Brücke, AS2/OFTP2 (MFT) und Automatisierung.

Produktseite: https://lan-solo.com/de/tools/file-port/ · Plan:
`FILEPORT_PLAN.md` (Produktdefinition, Protokolle, Architektur, Roadmap).

## Status

**v1.0.0 — erste stabile Version (Testbuild für macOS und Windows).**
Audit, Testabdeckung, Sicherheitsbefunde und manuelle Testliste:
[`AUDIT-1.0.md`](AUDIT-1.0.md). Die 0.x-Versionen folgten dem Schema
`v0.PHASE.SCHRITT`.

Neu in 1.0: Bestätigung unbekannter/geänderter SSH-Host-Keys per
SHA256-Fingerprint *vor* der Anmeldung; rsync prüft gegen den gemerkten
Host-Key; FTPS über rustls mit TLS-Session-Wiederaufnahme (vsftpd & Co.)
und funktionierendem impliziten Modus; Rückfrage vor dem Ersetzen;
Downloads über Teildatei (das Ziel bleibt bei Fehlern erhalten);
Ordner-Löschen bei SFTP/FTPS und Ordner-Umbenennen bei Objektspeichern;
Windows-Pfadkorrekturen und Schutz vor bösartigen Server-Dateinamen.
Nach der zweiten Audit-Runde zusätzlich: Server-Dateinamen werden im
Backend plattformgerecht geprüft; Downloads, Uploads und Umbenennen
ersetzen nie still eine vorhandene Datei; Wiederholung bei kurzzeitig
gesperrten Dateien unter Windows; Fehlermeldungen in der Sprache der
Oberfläche (`src/errors.ts`); Schlüsseldateien mit BOM/CRLF; korrekte
Änderungszeiten bei SCP/rsync-Servern in anderen Zeitzonen; Rückfrage
beim Trennen mit laufenden Transfers; die Warteschlange wartet, solange
eine Ersetzen-Rückfrage offen ist.
Nach der dritten Runde: Schließen des Fensters oder Update-Neustart
mitten im Download hinterlässt keine `.fileport-part` mehr (Journal,
Aufräumen beim Beenden und beim nächsten Start); FTPS liest
Änderungszeiten per MLSD in UTC, wo der Server es kann; die
Upload-Vorprüfung bricht nicht mehr ab, wenn der Zielordner sich nicht
listen lässt; ohne geladenen Zielordner startet kein Transfer und
„Neuer Ordner“ bleibt gesperrt (legte sonst in der Server-Wurzel an).

- **Phase 1 — Protokoll-Engine (fertig):** Der Rust-Core (`core/`,
  `fileport-core`) spricht über einen gemeinsamen `Backend`-Trait das lokale
  Dateisystem, **SFTP** (russh, Passwort/Key, TOFU-Host-Key), **SCP** und
  **rsync über SSH** (gemeinsame russh-Schicht; SCP spricht das klassische
  Protokoll selbst, rsync spawnt das lokale rsync/ssh mit Schlüssel-Auth),
  **FTPS** (suppaftp über rustls, TLS 1.2 mit Session-Reuse, explizit &
  implizit; unverschlüsseltes FTP als eigenes Protokoll `ftp_plain` nur mit
  `insecure_confirmed` im Profil), **WebDAV** (eigener DAV-Layer über reqwest, nur HTTPS),
  **SMB 2/3** (Pure-Rust-Crate smb2, nie SMB1) sowie Objektspeicher über
  object_store: **S3-kompatible Dienste** (AWS, MinIO, Backblaze B2,
  Hetzner …), **Azure Blob Storage** und **Google Cloud Storage**.
  Jeder Transfer läuft mit Fortschritt, Abbruch-Token und BLAKE3-Prüfsumme.
  Alle Backends außer Azure Blob und Google Cloud Storage sind mit
  Integrationstests gegen echte Server abgedeckt (Docker: OpenSSH für
  SFTP/SCP/rsync, vsftpd mit TLS, Apache-DAV, Samba, `rclone serve s3`;
  FTPS-MLSD zusätzlich gegen pyftpdlib) — `FILEPORT_IT=1 cargo test`;
  Rezepte stehen in den jeweiligen `core/tests/*_it.rs`. Azure und GCS
  laufen über dieselbe object_store-Schicht wie S3 und sind nur per
  Unit-Test (InMemory) und Code-Review geprüft — ohne echtes Konto.
- **Phase 2 — Desktop-App (fertig):** Zwei-Fenster-UI (lokal ⇄ Gegenstelle),
  Verbindungs-Manager mit Profilen (Geheimnisse im OS-Schlüsselbund, nie in
  Dateien), sequenzielle Warteschlange mit Fortschritt und Abbruch,
  Dateioperationen (Neuer Ordner, Umbenennen, Löschen) in beiden Panes,
  Deutsch/Englisch nach Systemsprache, Dunkel-/Hell-Modus per Button in
  der Kopfzeile (gespeichert in `localStorage`).
- **Später:** OAuth-Clouds (Dropbox/Drive/OneDrive), Direkt-Drop,
  Server-zu-Server-Brücke, AS2/OFTP2 (bridged) — siehe `FILEPORT_PLAN.md`.

Das Tauri-App-Grundgerüst (`src-tauri/` + `src/`) enthält außerdem den
**In-App-Updater** (gleiches Muster wie keypile): signierte Updates von GitHub
Releases, stiller Check beim Start, „Nach Updates suchen"-Button — und vor
jeder Installation zeigt die App das Changelog, installiert wird erst nach
Bestätigung. Releases entstehen per Git-Tag `v*` (`.github/workflows/build.yml`
baut, signiert, generiert das Changelog aus den Commits und published
`latest.json`). Signatur-Key: `~/.tauri/fileport-updater.key`.

## Entwicklung

```sh
pnpm install           # Frontend-Abhängigkeiten
pnpm tauri dev         # App im Dev-Modus starten
pnpm tauri build       # Release-Build (App-Bundle/DMG)
cargo test --workspace               # Unit-Tests (Engine + App-Logik)
pnpm test                            # Frontend-Tests (Pfade, Fehlerübersetzung; Node-Testrunner)
FILEPORT_IT=1 cargo test -p fileport-core   # + Integrationstests (Docker nötig,
                                            #   Startbefehle: siehe core/tests/*.rs)
```
