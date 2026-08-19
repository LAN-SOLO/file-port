//! fileport-core — die Protokoll-Engine hinter file/port.
//!
//! Alle Gegenstellen (lokales Dateisystem, SFTP, FTP(S), WebDAV, S3 …)
//! implementieren denselben [`Backend`]-Trait; Transfers laufen immer
//! zwischen einem Backend und dem lokalen Dateisystem, mit Fortschritts-
//! Callback, Abbruch-Token und BLAKE3-Prüfsumme über die übertragenen Bytes.

mod backend;
mod error;
mod ftp;
mod local;
mod rpath;
mod sftp;
mod transfer;
mod webdav;

pub use backend::{Backend, Entry, EntryKind};
pub use error::FpError;
pub use ftp::{FtpBackend, FtpConfig, FtpSecurity};
pub use local::LocalBackend;
pub use rpath::{file_name as rfile_name, join as rjoin, parent as rparent};
pub use sftp::{SftpAuth, SftpBackend, SftpConfig};
pub use transfer::{copy_with_progress, CancelToken, TransferCtl, TransferResult};
pub use webdav::{WebdavBackend, WebdavConfig};
