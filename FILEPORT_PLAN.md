# file/port — Produkt- und Implementierungsplan

Ein Datei-Transfer-Client für macOS, Windows und Linux, der praktisch jedes
Transferprotokoll spricht (SFTP, FTP(S), SCP/rsync, WebDAV(S), HTTP(S), TFTP,
später AS2/OFTP2), die großen Cloud-Speicher anbindet und abbruchsicher mit
Prüfsummen überträgt. **Free** bleibt dauerhaft kostenlos und voll nutzbar;
**file/port bridged** (12 €/Jahr = 1 €/Monat) ergänzt die
Server-zu-Server-Brücke, Managed File Transfer (AS2/OFTP2 mit MDN) und
Automatisierung (Zeitpläne, Watch-Folder, Regeln, Audit-Log).

Produktseite: https://lan-solo.com/de/tools/file-port/

## 1. Produktdefinition

### Pläne & Preise (fixiert, siehe Landing Page)

| | Free | bridged |
|---|---|---|
| Preis | 0 € (dauerhaft) | 12 €/Jahr (1 €/Monat) |
| Protokolle | SFTP, FTP(S), SCP/rsync, WebDAV(S), HTTP(S), TFTP | gleich |
| Clouds | S3-kompatibel, Azure Blob, GCS, Dropbox, Drive, OneDrive, Nextcloud | gleich |
| UI | Zwei-Fenster, Warteschlange, Drag & Drop | gleich |
| Robustheit | Resume, parallele Verbindungen, Prüfsummen | gleich |
| Direkt-Drop (Gerät ⇄ Gerät, E2E-verschlüsselt) | ✓ | ✓ |
| Server-zu-Server-Brücke (protokollübergreifend) | — | ✓ |
| AS2 & OFTP2 inkl. Empfangsquittungen (MDN) | — | ✓ |
| Zeitpläne, Watch-Folder, Wiederholungs-Regeln | — | ✓ |
| Audit-Log & Übertragungsberichte | — | ✓ |

### Alleinstellungsmerkmale (die die Konkurrenz nicht hat)

1. **Direkt-Drop:** Dateien per Einmal-Code Ende-zu-Ende-verschlüsselt direkt
   an ein anderes file/port schicken — ohne Server, ohne Konto, im LAN und
   übers Internet. Transmit, FileZilla & Co. können das nicht; die
   Wormhole-Idee existiert nur als CLI-Nische.
2. **Server-zu-Server-Brücke:** S3 → SFTP, WebDAV → Azure — direkt zwischen
   den Gegenstellen gestreamt, ohne lokalen Umweg. Klassisches FXP kann das
   nur FTP↔FTP; rclone kann es headless, aber niemand im Zwei-Fenster-GUI.
3. **MFT zum Indie-Preis:** AS2 mit MDN, Zeitpläne, Watch-Folder und
   Audit-Log stecken sonst in vier- bis fünfstelligen Enterprise-Suiten
   (Cleo, GoAnywhere, Sterling).
4. **keypile-Integration:** Zugangsdaten und SSH-Schlüssel optional im
   keypile-Tresor statt in einer eigenen Config — ein Secrets-Speicher für
   das ganze LAN-SOLO-Ökosystem.

### Harte Randbedingungen (ehrlich einplanen!)

- **Kein Tempo-Wunder versprechen.** Schneller als Leitung und Gegenstelle
  geht nicht; file/port optimiert alles dazwischen (parallele Streams,
  Wiederaufnahme, keine unnötigen Roundtrips). So steht es auch auf der Seite.
- **AS2/OFTP2 sind Zertifikats- und Partnerkonfigurations-Monster.** Wird
  erst ausgeliefert, wenn Interop gegen reale Gegenstellen (z. B. mendelson
  AS2, Drummond-zertifizierte Server) getestet ist — nicht früher.
- **rsync** startet als Wrapper um das System-rsync über SSH (überall außer
  Windows vorhanden); eine native Delta-Implementierung ist Kür, nicht Pflicht.
- **Brücke:** Nicht jede Kombination kann serverseitig direkt (echtes FXP nur
  FTP↔FTP). Die Brücke streamt sonst durch den Client — aber ohne
  Zwischenspeichern auf Platte. Genau so kommunizieren.
- **Keine Telemetrie, kein Konto-Zwang.** Zugangsdaten lokal (Keychain /
  Credential Manager / Secret Service oder keypile) — gleiche Disziplin wie
  bei [[secrets]], [[keypile]], [[all-backed]] und [[packed]].

## 2. Protokolle & Bibliotheken (Rust-Core)

| Protokoll/Dienst | Ansatz/Crate | Phase |
|---|---|---|
| Lokales Dateisystem | `std`/`tokio::fs` (Referenz-Backend) | 0 |
| SFTP & SCP | `russh` + `russh-sftp` | 0 |
| FTP & FTPS | `suppaftp` (+ `rustls`) | 0 |
| HTTP(S)/REST | `reqwest` (Range-Resume) | 1 |
| WebDAV(S) | `reqwest` + eigener dünner DAV-Layer | 1 |
| S3-kompatibel, Azure Blob, GCS | `object_store` (eine API für alle drei) | 1 |
| Dropbox, Google Drive, OneDrive | REST + OAuth 2.0 PKCE (Loopback) | 3 |
| Nextcloud/ownCloud | WebDAV-Backend + App-Passwörter | 3 |
| Direkt-Drop | `iroh` (QUIC hole punching) oder `magic-wormhole.rs` | 3 |
| rsync über SSH | Wrapper um System-`rsync` | 3 |
| TFTP | `async-tftp` | 5 |
| AS2 (MDN) | HTTP + S/MIME (`openssl`/`cms`) — Interop-Tests Pflicht | 5 |
| OFTP2 | Eigenimplementierung (RFC 5024) — nur bei echter Nachfrage | 5+ |

Gemeinsamer Kern: ein `Backend`-Trait (list/stat/get/put/mkdir/remove/rename
als Streams mit Offset), darüber eine Transfer-Engine mit Warteschlange,
Checkpoints (Resume), parallelen Verbindungen und Prüfsummen (BLAKE3 lokal,
serverseitige Checksummen wo das Protokoll sie hergibt). Die Brücke ist
schlicht `Backend::get → Backend::put` ohne Datei dazwischen.

## 3. Architektur

```
file-port/
├── core/       # Rust: Backend-Trait, Protokolle, Transfer-Engine, Checksummen
├── cli/        # fileport-CLI (ls/get/put/sync/drop) — treibt den Core, testbar
└── src-tauri/  # Desktop-App (Tauri 2, wie keypile/packed): Zwei-Fenster-UI
```

- **UI (Phase 2):** Zwei-Fenster-Layout (lokal ⇄ remote bzw. remote ⇄ remote),
  Verbindungs-Manager mit Favoriten, Warteschlange mit Pause/Resume je
  Transfer, Transfer-Log im Terminal-Stil (Ton der Marke), Dark/Light.
- **Secrets:** OS-Keystore per `keyring`-Crate; optional keypile-Tresor.
- **Updates:** signierter In-App-Updater wie bei keypile (Tauri Updater).

## 4. Roadmap

- **Phase 0 — Core & CLI:** Backend-Trait, lokales FS, SFTP, FTP(S);
  Resume + BLAKE3-Prüfsummen; CLI `ls/get/put`; Integrationstests gegen
  Docker-Container (openssh, vsftpd).
- **Phase 1 — Web & Objekt-Speicher:** HTTP(S), WebDAV, `object_store`
  (S3/Azure/GCS inkl. MinIO/B2/Hetzner via S3-API).
- **Phase 2 — Desktop-App (Alpha):** Tauri-App mit Zwei-Fenster-UI,
  Warteschlange, Verbindungs-Manager, Keystore. → erste Downloads auf der
  Produktseite (Alpha, wie bei keypile).
- **Phase 3 — Clouds & Drop:** OAuth-Clouds (Dropbox/Drive/OneDrive),
  Nextcloud, Direkt-Drop, rsync-Wrapper.
- **Phase 4 — bridged I:** Server-zu-Server-Brücke, Zeitpläne, Watch-Folder,
  Wiederholungs-Regeln, Audit-Log. → Beta + Verkaufsstart bridged.
- **Phase 5 — bridged II (MFT):** AS2 mit MDN und Interop-Tests, TFTP;
  OFTP2 nur bei nachgewiesener Nachfrage.

## 5. Website

Die Produktseite (beide Sprachen) liegt im Website-Repo unter
`app/[lang]/tools/file-port/` + `components/fileport/FilePortPage.tsx`;
Texte in `i18n/DE.ts`/`EN.ts` (`filePort`), Icon `public/brand/fileport.svg`.
Status dort: „In Entwicklung" — Download-Buttons kommen erst mit Phase 2
(echte Builds), wie bei keypile.
