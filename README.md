# file/port

Datei-Transfer-Client für macOS, Windows und Linux: spricht SFTP, FTP(S),
SCP/rsync, WebDAV(S), HTTP(S) und TFTP, verbindet Clouds von S3-kompatiblen
Diensten über Azure und Google Cloud bis Dropbox, Google Drive, OneDrive und
Nextcloud — und überträgt abbruchsicher mit Prüfsummen, Warteschlange und
Direkt-Drop von Gerät zu Gerät. **Free** bleibt dauerhaft kostenlos und voll
nutzbar; **file/port bridged** (12 €/Jahr = 1 €/Monat) ergänzt die
Server-zu-Server-Brücke, AS2/OFTP2 (MFT) und Automatisierung.

Produktseite: https://lan-solo.com/de/tools/file-port/ · Plan:
`FILEPORT_PLAN.md` (Produktdefinition, Protokolle, Architektur, Roadmap).

## Status

**Phase 0 — Core & CLI (geplant).** Der Rust-Core (`core/`) bekommt eine
gemeinsame Backend-Abstraktion für alle Protokolle; eine CLI (`cli/`) treibt
ihn für Skripte und Tests. Die Desktop-App (Tauri, Zwei-Fenster-UI) folgt in
Phase 2 — gleiche Toolchain wie bei keypile und packed.

## Entwicklung

```sh
cargo build            # Workspace bauen (sobald Phase 0 steht)
cargo test -p core     # Core-Tests
```
