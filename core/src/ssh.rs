//! Gemeinsame SSH-Schicht für SFTP, SCP und rsync-über-SSH: Verbindungsaufbau
//! mit Passwort-/Schlüssel-Auth und Trust-on-first-use-Host-Key, plus
//! Exec-Helfer für Backends, die Dateioperationen über Shell-Kommandos
//! auf der Gegenstelle abwickeln (SCP, rsync).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use russh::client;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg};
use russh::ChannelMsg;

use crate::backend::{Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;

pub enum SshAuth {
    Password(String),
    KeyFile {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
    /// Bekannter Host-Key (OpenSSH-Format) aus einer früheren Verbindung.
    /// `None` = Trust-on-first-use: der Key wird akzeptiert und über
    /// `host_key()` zurückgemeldet, damit die App ihn speichert.
    pub expected_host_key: Option<String>,
}

/// Prüft den Server-Key gegen den gespeicherten (TOFU) und reicht den
/// tatsächlich gesehenen Key nach außen.
pub(crate) struct HostKeyCheck {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
}

impl client::Handler for HostKeyCheck {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let openssh = key.to_openssh().map_err(|_| russh::Error::UnknownKey)?;
        *self.seen.lock().unwrap() = Some(openssh.clone());
        match &self.expected {
            Some(expected) => Ok(expected.trim() == openssh.trim()),
            None => Ok(true),
        }
    }
}

/// Eine authentifizierte SSH-Session — Grundlage für das SFTP-Subsystem
/// wie für die Exec-Backends (SCP, rsync).
pub(crate) struct SshSession {
    pub handle: client::Handle<HostKeyCheck>,
    pub host_key: String,
}

pub(crate) async fn connect_ssh(cfg: &SshConfig) -> Result<SshSession, FpError> {
    let seen = Arc::new(Mutex::new(None));
    let handler = HostKeyCheck {
        expected: cfg.expected_host_key.clone(),
        seen: seen.clone(),
    };
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (cfg.host.as_str(), cfg.port), handler)
        .await
        .map_err(|e| match e {
            russh::Error::UnknownKey => FpError::Connect(format!(
                "Host-Key von {} stimmt nicht mit dem gespeicherten überein",
                cfg.host
            )),
            other => FpError::Connect(other.to_string()),
        })?;

    let authed = match &cfg.auth {
        SshAuth::Password(pw) => handle
            .authenticate_password(&cfg.user, pw)
            .await
            .map_err(|e| FpError::Auth(e.to_string()))?,
        SshAuth::KeyFile { path, passphrase } => {
            let key = russh::keys::load_secret_key(path, passphrase.as_deref())
                .map_err(|e| FpError::Auth(format!("Schlüsseldatei: {e}")))?;
            let hash: Option<HashAlg> = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|e| FpError::Auth(e.to_string()))?
                .flatten();
            handle
                .authenticate_publickey(&cfg.user, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await
                .map_err(|e| FpError::Auth(e.to_string()))?
        }
    };
    if !authed.success() {
        return Err(FpError::Auth(format!(
            "Server hat die Anmeldung als {} abgelehnt",
            cfg.user
        )));
    }

    let host_key = seen.lock().unwrap().clone().unwrap_or_default();
    Ok(SshSession { handle, host_key })
}

/// Shell-sicheres Quoting für Pfade in Remote-Kommandos: `'a'\''b'`.
pub(crate) fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Ordnet typische Shell-Fehlermeldungen den Fehlerklassen zu.
fn map_exec(path: &str, stderr: &str) -> FpError {
    let msg = stderr.trim();
    if msg.contains("No such file or directory") {
        FpError::NotFound(path.to_string())
    } else if msg.contains("Permission denied") || msg.contains("Operation not permitted") {
        FpError::Denied(path.to_string())
    } else {
        FpError::Protocol(format!("{path}: {msg}"))
    }
}

impl SshSession {
    /// Führt ein Kommando auf der Gegenstelle aus und sammelt
    /// stdout, stderr und Exit-Code ein.
    pub async fn run(&self, cmd: &str) -> Result<(Vec<u8>, String, u32), FpError> {
        let mut ch = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        ch.exec(true, cmd)
            .await
            .map_err(|e| FpError::Protocol(e.to_string()))?;
        ch.eof().await.map_err(|e| FpError::Protocol(e.to_string()))?;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut exit = 0u32;
        while let Some(msg) = ch.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, ext: 1 } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => exit = exit_status,
                _ => {}
            }
        }
        Ok((stdout, String::from_utf8_lossy(&stderr).into_owned(), exit))
    }

    /// Wie [`run`](Self::run), schlägt aber bei Exit-Code ≠ 0 fehl.
    pub async fn run_ok(&self, cmd: &str, path: &str) -> Result<Vec<u8>, FpError> {
        let (stdout, stderr, exit) = self.run(cmd).await?;
        if exit != 0 {
            return Err(map_exec(path, &stderr));
        }
        Ok(stdout)
    }

    /// Startverzeichnis: das Home-Verzeichnis der Login-Shell.
    pub async fn initial_dir(&self) -> Result<String, FpError> {
        let out = self.run_ok("pwd", ".").await?;
        let dir = String::from_utf8_lossy(&out).trim().to_string();
        if dir.starts_with('/') {
            Ok(dir)
        } else {
            Ok("/".to_string())
        }
    }

    /// Verzeichnis-Listing über `ls -la` — die Zeilen haben dasselbe
    /// Unix-Format wie FTP-LIST-Antworten, deshalb parst sie der
    /// suppaftp-Parser gleich mit (unparsbare Zeilen wie `total …`
    /// werden übersprungen).
    pub async fn list_dir(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let cmd = format!("LC_ALL=C ls -la {}", shell_quote(path));
        let out = self.run_ok(&cmd, path).await?;
        let mut entries = Vec::new();
        for line in String::from_utf8_lossy(&out).lines() {
            // `total <n>`-Kopfzeile von ls überspringen — der Parser würde
            // sie sonst als Datei namens „total n" durchreichen.
            if line
                .strip_prefix("total ")
                .is_some_and(|rest| rest.trim().parse::<u64>().is_ok())
            {
                continue;
            }
            let Ok(f) = suppaftp::list::File::try_from(line) else {
                continue;
            };
            let name = f.name().to_string();
            if name == "." || name == ".." {
                continue;
            }
            entries.push(Entry {
                path: rpath::join(path, &name),
                name,
                kind: if f.is_directory() {
                    EntryKind::Dir
                } else if f.is_symlink() {
                    EntryKind::Symlink
                } else {
                    EntryKind::File
                },
                size: f.size() as u64,
                modified: f
                    .modified()
                    .duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64),
            });
        }
        Ok(entries)
    }

    pub async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        self.run_ok(&format!("mkdir {}", shell_quote(path)), path)
            .await
            .map(|_| ())
    }

    pub async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        let cmd = if is_dir {
            format!("rm -r {}", shell_quote(path))
        } else {
            format!("rm {}", shell_quote(path))
        };
        self.run_ok(&cmd, path).await.map(|_| ())
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        let cmd = format!("mv {} {}", shell_quote(from), shell_quote(to));
        self.run_ok(&cmd, from).await.map(|_| ())
    }

    /// Größe einer Datei auf der Gegenstelle in Bytes.
    pub async fn file_size(&self, path: &str) -> Result<u64, FpError> {
        let out = self
            .run_ok(&format!("wc -c < {}", shell_quote(path)), path)
            .await?;
        String::from_utf8_lossy(&out)
            .trim()
            .parse()
            .map_err(|_| FpError::Protocol(format!("{path}: unlesbare Dateigröße")))
    }
}
