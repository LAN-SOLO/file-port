//! fileport-core — die Protokoll-Engine hinter fileport.
//!
//! Alle Gegenstellen (lokales Dateisystem, SFTP, FTPS, WebDAV, S3,
//! Azure Blob, Google Cloud Storage …) — ausschließlich verschlüsselte
//! Verbindungen — implementieren denselben [`Backend`]-Trait; Transfers laufen immer
//! zwischen einem Backend und dem lokalen Dateisystem, mit Fortschritts-
//! Callback, Abbruch-Token und BLAKE3-Prüfsumme über die übertragenen Bytes.

mod backend;
mod error;
mod ftp;
mod local;
mod objstore;
mod rpath;
mod sftp;
mod transfer;
mod webdav;

pub use backend::{Backend, Entry, EntryKind};
pub use error::FpError;
pub use ftp::{FtpBackend, FtpConfig, FtpSecurity};
pub use local::LocalBackend;
pub use objstore::{AzureConfig, GcsConfig, ObjectBackend, S3Config};
pub use rpath::{file_name as rfile_name, join as rjoin, parent as rparent};
pub use sftp::{SftpAuth, SftpBackend, SftpConfig};
pub use transfer::{copy_with_progress, CancelToken, TransferCtl, TransferResult};
pub use webdav::{WebdavBackend, WebdavConfig};
