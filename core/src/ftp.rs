use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use rustls_platform_verifier::BuilderVerifierExt;
use suppaftp::types::FileType;
use suppaftp::{Mode, RustlsConnector, RustlsFtpStream};

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::transfer::{TransferCtl, TransferResult};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FtpSecurity {
    /// Explizites FTPS (AUTH TLS auf dem Standardport 21).
    ExplicitTls,
    /// Implizites FTPS (TLS ab dem ersten Byte, Standardport 990).
    ImplicitTls,
}

pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub security: FtpSecurity,
    /// Selbstsignierte Zertifikate akzeptieren (bei FTP-Servern verbreitet) —
    /// bewusste Entscheidung der Nutzerin im Verbindungs-Profil.
    pub accept_invalid_certs: bool,
}

/// FTPS über suppaftp (blocking) — immer TLS-verschlüsselt, unverschlüsseltes
/// FTP wird nicht unterstützt. Jede Operation nimmt die Verbindung
/// kurz aus dem Mutex und arbeitet in `spawn_blocking`, damit der
/// Tokio-Reaktor frei bleibt.
///
/// TLS läuft über rustls mit einer gemeinsamen `ClientConfig` für Steuer-
/// und Datenkanal: Die Datenverbindungen nehmen die TLS-Session der
/// Steuerverbindung wieder auf, wie es vsftpd (`require_ssl_reuse`),
/// ProFTPD und FileZilla Server standardmäßig verlangen. Zertifikate prüft
/// der Zertifikatsspeicher des Betriebssystems.
pub struct FtpBackend {
    conn: tokio::sync::Mutex<Option<RustlsFtpStream>>,
    label: String,
    /// Listing-Art: unbekannt / MLSD (RFC 3659, Zeiten in UTC) / nur LIST.
    listing: Arc<AtomicU8>,
}

const LISTING_UNKNOWN: u8 = 0;
const LISTING_MLSD: u8 = 1;
const LISTING_LIST: u8 = 2;

/// Antwortcodes für „Befehl unbekannt/nicht implementiert".
fn is_unsupported(e: &suppaftp::FtpError) -> bool {
    matches!(e, suppaftp::FtpError::UnexpectedResponse(r) if matches!(r.status as u32, 500 | 501 | 502 | 504))
}

/// Eine MLSD-Zeile (RFC 3659): `fakt=wert;fakt=wert; name`. Zeiten
/// (`modify`) sind dort per Definition UTC — anders als bei LIST, das
/// gar keine Zeitzone kennt. Eigener Parser statt suppaftps: der verwirft
/// Zeilen mit Sekundenbruchteilen (`modify=20260101120000.123`) oder
/// unbekanntem Typ (`OS.unix=slink:…`) und kürzt Namen mit `;`.
fn parse_mlsd_line(dir: &str, line: &str) -> Option<Entry> {
    let line = line.trim_end_matches(['\r', '\n']);
    let (facts, name) = line.split_once(' ')?;
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return None;
    }
    let mut kind = EntryKind::File;
    let mut size = 0u64;
    let mut modified = None;
    for fact in facts.split(';').filter(|f| !f.is_empty()) {
        let Some((k, v)) = fact.split_once('=') else { continue };
        match k.to_ascii_lowercase().as_str() {
            "type" => {
                let v = v.to_ascii_lowercase();
                kind = match v.as_str() {
                    // aktueller/übergeordneter Ordner: nicht anzeigen
                    "cdir" | "pdir" => return None,
                    "dir" => EntryKind::Dir,
                    "file" => EntryKind::File,
                    _ if v.contains("link") => EntryKind::Symlink,
                    _ => EntryKind::File,
                };
            }
            "size" => size = v.parse().unwrap_or(0),
            "modify" => {
                let digits = v.split('.').next().unwrap_or("");
                modified = chrono_free_utc(digits);
            }
            _ => {}
        }
    }
    Some(Entry {
        path: rpath::join(dir, name),
        name: name.to_string(),
        kind,
        size,
        modified,
    })
}

/// `YYYYMMDDHHMMSS` (UTC) → Unix-Sekunden, ohne Datums-Crate.
fn chrono_free_utc(s: &str) -> Option<i64> {
    if s.len() != 14 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| s[a..b].parse::<i64>().ok();
    let (y, mo, d) = (n(0, 4)?, n(4, 6)?, n(6, 8)?);
    let (h, mi, se) = (n(8, 10)?, n(10, 12)?, n(12, 14)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    // Tage seit 1970-01-01 (Algorithmus nach Howard Hinnant, proleptisch gregorianisch)
    let (y, m) = if mo <= 2 { (y - 1, mo + 9) } else { (y, mo - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + h * 3600 + mi * 60 + se)
}

/// Bewusst unsicherer Verifier für „selbstsignierte Zertifikate
/// akzeptieren": Zertifikat und Hostname werden nicht geprüft, die
/// Handshake-Signaturen aber schon (der Server muss den Schlüssel besitzen).
#[derive(Debug)]
struct AcceptAnyCert(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// TLS-Konfiguration für FTPS: aws-lc-rs als Kryptografie, OS-Zertifikats-
/// prüfung (oder bewusst keine), Session-Wiederaufnahme an (rustls-Default).
/// Steuer- und Datenkanäle teilen sich diese eine Konfiguration (und damit
/// den Session-Cache).
fn tls_config(accept_invalid_certs: bool) -> Result<Arc<ClientConfig>, FpError> {
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    // TLS 1.2 (nur ECDHE + AEAD-Suites, wie rustls sie ausschließlich anbietet):
    // TLS-1.3-Tickets sind Einmal-Tickets und kommen nur an, wenn der Client
    // nach dem Handshake liest — nach wenigen Upload-Datenkanälen wären sie
    // aufgebraucht, und Server mit Session-Reuse-Pflicht lehnen ab (522).
    // TLS-1.2-Sessions lassen sich beliebig oft wiederaufnehmen.
    let builder = ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(&[&rustls::version::TLS12])
        .map_err(|e| FpError::Connect(format!("TLS: {e}")))?;
    let config = if accept_invalid_certs {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AcceptAnyCert(provider)))
            .with_no_client_auth()
    } else {
        builder
            .with_platform_verifier()
            .map_err(|e| FpError::Connect(format!("TLS: {e}")))?
            .with_no_client_auth()
    };
    Ok(Arc::new(config))
}

fn map_ftp(path: &str, e: suppaftp::FtpError) -> FpError {
    use suppaftp::FtpError;
    match &e {
        FtpError::UnexpectedResponse(resp) => {
            let code = resp.status as u32;
            match code {
                550 => FpError::NotFound(path.to_string()),
                530 => FpError::Auth(resp.to_string()),
                532 | 533 => FpError::Denied(path.to_string()),
                _ => FpError::Protocol(format!("{path}: {resp}")),
            }
        }
        _ => FpError::Protocol(e.to_string()),
    }
}

/// Zeitlimits: Verbindungsaufbau und „kein Byte mehr" auf Steuer-/Datenkanal —
/// ein hängender Server blockiert so nie dauerhaft die Warteschlange.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const IO_TIMEOUT: Duration = Duration::from_secs(120);

fn resolve(host: &str, port: u16) -> Result<SocketAddr, FpError> {
    (host, port)
        .to_socket_addrs()
        .map_err(|e| FpError::Connect(format!("{host}: {e}")))?
        .next()
        .ok_or_else(|| FpError::Connect(format!("{host}: keine Adresse gefunden")))
}

fn data_stream(addr: SocketAddr) -> suppaftp::FtpResult<TcpStream> {
    let stream = TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT)
        .map_err(suppaftp::FtpError::ConnectionError)?;
    stream
        .set_read_timeout(Some(IO_TIMEOUT))
        .and_then(|_| stream.set_write_timeout(Some(IO_TIMEOUT)))
        .map_err(suppaftp::FtpError::ConnectionError)?;
    Ok(stream)
}

impl FtpBackend {
    pub async fn connect(cfg: FtpConfig) -> Result<Self, FpError> {
        let label = format!("ftps://{}@{}", cfg.user, cfg.host);
        let stream = tokio::task::spawn_blocking(move || -> Result<RustlsFtpStream, FpError> {
            let addr = resolve(&cfg.host, cfg.port)?;
            let tls = tls_config(cfg.accept_invalid_certs)?;
            let stream = match cfg.security {
                FtpSecurity::ExplicitTls => RustlsFtpStream::connect_timeout(addr, CONNECT_TIMEOUT)
                    .map_err(|e| FpError::Connect(e.to_string()))?
                    .into_secure(RustlsConnector::from(tls), &cfg.host)
                    .map_err(|e| FpError::Connect(format!("TLS: {e}")))?,
                FtpSecurity::ImplicitTls => {
                    let mut s = RustlsFtpStream::connect_secure_implicit(
                        &addr,
                        RustlsConnector::from(tls),
                        &cfg.host,
                    )
                    .map_err(|e| FpError::Connect(format!("TLS: {e}")))?;
                    // suppaftp schaltet bei implizitem TLS den Datenkanal-Schutz
                    // nicht ein — ohne PBSZ/PROT P erwartet der Server Klartext-
                    // Datenkanäle, fileport spricht aber TLS (→ Hänger oder 522).
                    for cmd in ["PBSZ 0", "PROT P"] {
                        s.custom_command(cmd, &[suppaftp::Status::CommandOk])
                            .map_err(|e| FpError::Connect(format!("{cmd}: {e}")))?;
                    }
                    s
                }
            };
            let _ = stream.get_ref().set_read_timeout(Some(IO_TIMEOUT));
            let _ = stream.get_ref().set_write_timeout(Some(IO_TIMEOUT));
            let mut stream = stream.passive_stream_builder(data_stream);
            stream
                .login(&cfg.user, &cfg.password)
                .map_err(|e| FpError::Auth(e.to_string()))?;
            stream.set_mode(Mode::Passive);
            // PASV-Antworten mit privater/fremder IP ignorieren und den Datenkanal
            // immer zum Host der Steuerverbindung aufbauen (NAT-Server, und kein
            // Umlenken des Datenkanals auf Dritt-Hosts).
            stream.set_passive_nat_workaround(true);
            stream
                .transfer_type(FileType::Binary)
                .map_err(|e| FpError::Connect(e.to_string()))?;
            Ok(stream)
        })
        .await
        .map_err(|e| FpError::Connect(e.to_string()))??;

        Ok(FtpBackend {
            conn: tokio::sync::Mutex::new(Some(stream)),
            label,
            listing: Arc::new(AtomicU8::new(LISTING_UNKNOWN)),
        })
    }

    /// Führt `f` mit der Verbindung in einem Blocking-Task aus und legt
    /// die Verbindung danach zurück — Operationen laufen strikt seriell.
    async fn with_conn<T, F>(&self, f: F) -> Result<T, FpError>
    where
        T: Send + 'static,
        F: FnOnce(&mut RustlsFtpStream) -> Result<T, FpError> + Send + 'static,
    {
        let mut guard = self.conn.lock().await;
        let mut stream = guard
            .take()
            .ok_or_else(|| FpError::Connect("FTP-Verbindung ist geschlossen".into()))?;
        let (stream, result) = tokio::task::spawn_blocking(move || {
            let result = f(&mut stream);
            (stream, result)
        })
        .await
        .map_err(|e| FpError::Protocol(e.to_string()))?;
        *guard = Some(stream);
        result
    }
}

/// Blocking-Kopie mit Fortschritt, Abbruch und BLAKE3 — das synchrone
/// Gegenstück zu `copy_with_progress` für Backends ohne Async-IO.
fn copy_blocking<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    total: Option<u64>,
    ctl: &TransferCtl,
) -> Result<TransferResult, FpError> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut hasher = blake3::Hasher::new();
    loop {
        if ctl.cancel.is_cancelled() {
            return Err(FpError::Cancelled);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        done += n as u64;
        (ctl.progress)(done, total);
    }
    writer.flush()?;
    Ok(TransferResult {
        bytes: done,
        blake3: hasher.finalize().to_hex().to_string(),
    })
}

#[async_trait]
impl Backend for FtpBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        self.with_conn(|c| c.pwd().map_err(|e| map_ftp(".", e))).await
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let dir = path.to_string();
        let listing = self.listing.clone();
        self.with_conn(move |c| {
            // MLSD liefert Zeiten in UTC (RFC 3659); LIST hat keine Zeitzone.
            // Ob der Server MLSD kann, zeigt einmalig MLST über die Steuer-
            // verbindung — ein abgelehntes MLSD selbst ließe den schon
            // geöffneten TLS-Datenkanal bis zum Zeitlimit hängen.
            if listing.load(Ordering::Relaxed) == LISTING_UNKNOWN {
                match c.mlst(Some(&dir)) {
                    Ok(_) => listing.store(LISTING_MLSD, Ordering::Relaxed),
                    Err(e) if is_unsupported(&e) => listing.store(LISTING_LIST, Ordering::Relaxed),
                    Err(_) => {} // z. B. 550 für diesen Pfad — beim nächsten Listing erneut prüfen
                }
            }
            let lines = c.list(Some(&dir)).map_err(|e| map_ftp(&dir, e))?;
            // Art und Größe kommen weiter aus LIST: Manche Server melden in
            // MLSD einen Ordner-Symlink als „type=dir“ — rekursives Löschen
            // liefe dann ins Link-Ziel. MLSD liefert nur die UTC-Zeiten.
            let utc_times: HashMap<String, i64> = if listing.load(Ordering::Relaxed) == LISTING_MLSD {
                match c.mlsd(Some(&dir)) {
                    Ok(ml) => ml
                        .iter()
                        .filter_map(|l| parse_mlsd_line(&dir, l))
                        .filter_map(|e| e.modified.map(|m| (e.name, m)))
                        .collect(),
                    Err(_) => {
                        listing.store(LISTING_LIST, Ordering::Relaxed);
                        HashMap::new()
                    }
                }
            } else {
                HashMap::new()
            };
            let mut out = Vec::new();
            for line in lines {
                let Ok(f) = suppaftp::list::File::try_from(line.as_str()) else {
                    continue;
                };
                let name = f.name().to_string();
                if name == "." || name == ".." {
                    continue;
                }
                out.push(Entry {
                    path: rpath::join(&dir, &name),
                    kind: if f.is_directory() {
                        EntryKind::Dir
                    } else if f.is_symlink() {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    },
                    size: f.size() as u64,
                    modified: utc_times.get(&name).copied().or_else(|| {
                        f.modified()
                            .duration_since(UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs() as i64)
                    }),
                    name,
                });
            }
            Ok(out)
        })
        .await
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let remote = remote.to_string();
        let local = local.to_path_buf();
        let ctl = ctl.clone();
        self.with_conn(move |c| {
            let total = c.size(&remote).ok().map(|s| s as u64);
            let mut src = c.retr_as_stream(&remote).map_err(|e| map_ftp(&remote, e))?;
            let dst = std::fs::File::create(&local)?;
            let result = copy_blocking(&mut src, dst, total, &ctl);
            // Auch nach Abbruch/Fehler muss der Datenkanal sauber schließen,
            // sonst bleibt die Steuerverbindung in der Schwebe.
            let fin = c.finalize_retr_stream(src);
            let result = result?;
            fin.map_err(|e| map_ftp(&remote, e))?;
            Ok(result)
        })
        .await
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let remote = remote.to_string();
        let local = local.to_path_buf();
        let ctl = ctl.clone();
        self.with_conn(move |c| {
            let src = std::fs::File::open(&local)?;
            let total = src.metadata().ok().map(|m| m.len());
            let mut dst = c.put_with_stream(&remote).map_err(|e| map_ftp(&remote, e))?;
            let result = copy_blocking(src, &mut dst, total, &ctl);
            let fin = c.finalize_put_stream(dst);
            let result = result?;
            fin.map_err(|e| map_ftp(&remote, e))?;
            Ok(result)
        })
        .await
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        let path = path.to_string();
        self.with_conn(move |c| c.mkdir(&path).map_err(|e| map_ftp(&path, e))).await
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        if is_dir {
            // RMD löscht nur leere Verzeichnisse — Inhalt rekursiv abräumen.
            for entry in self.list(path).await? {
                Box::pin(self.remove(&entry.path, entry.kind == EntryKind::Dir)).await?;
            }
        }
        let path = path.to_string();
        self.with_conn(move |c| {
            if is_dir {
                c.rmdir(&path).map_err(|e| map_ftp(&path, e))
            } else {
                c.rm(&path).map_err(|e| map_ftp(&path, e))
            }
        })
        .await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        let from = from.to_string();
        let to = to.to_string();
        self.with_conn(move |c| c.rename(&from, &to).map_err(|e| map_ftp(&from, e))).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_conversion() {
        assert_eq!(chrono_free_utc("19700101000000"), Some(0));
        assert_eq!(chrono_free_utc("20000229120000"), Some(951_825_600));
        assert_eq!(chrono_free_utc("20261020013000"), Some(1_792_459_800));
        assert_eq!(chrono_free_utc("2026102001300"), None);
        assert_eq!(chrono_free_utc("20261320013000"), None);
    }

    #[test]
    fn mlsd_lines_from_real_servers() {
        // ProFTPD
        let e = parse_mlsd_line("/home", "modify=20261020013000;perm=adfrw;size=1234;type=file;unique=FD00U1;UNIX.group=1000;UNIX.mode=0644;UNIX.owner=1000; Bericht 2026; final.pdf").unwrap();
        assert_eq!(e.name, "Bericht 2026; final.pdf");
        assert_eq!(e.path, "/home/Bericht 2026; final.pdf");
        assert_eq!(e.kind, EntryKind::File);
        assert_eq!(e.size, 1234);
        assert_eq!(e.modified, Some(1_792_459_800));
        // FileZilla Server, Sekundenbruchteile
        let e = parse_mlsd_line("/", "type=dir;modify=20261020013000.123;perm=flcdmpe; Ordner mit  zwei Leerzeichen").unwrap();
        assert_eq!(e.kind, EntryKind::Dir);
        assert_eq!(e.name, "Ordner mit  zwei Leerzeichen");
        assert_eq!(e.modified, Some(1_792_459_800));
        // Symlink (ProFTPD: OS.unix=slink:<ziel>)
        let e = parse_mlsd_line("/", "type=OS.unix=slink:/srv/x;modify=20261020013000; link").unwrap();
        assert_eq!(e.kind, EntryKind::Symlink);
        // cdir/pdir und kaputte Zeilen werden ausgeblendet
        assert!(parse_mlsd_line("/", "type=cdir;modify=20261020013000; /home").is_none());
        assert!(parse_mlsd_line("/", "type=pdir;modify=20261020013000; ..").is_none());
        assert!(parse_mlsd_line("/", "kaputt").is_none());
        // Gemischte Groß-/Kleinschreibung der Fakten, CRLF
        let e = parse_mlsd_line("/", "Type=File;Size=5;Modify=20261020013000; a.txt\r\n").unwrap();
        assert_eq!((e.name.as_str(), e.size), ("a.txt", 5));
    }
}
