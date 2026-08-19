use std::path::Path;

use async_trait::async_trait;
use serde::Serialize;

use crate::error::FpError;
use crate::transfer::{TransferCtl, TransferResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Dir,
    File,
    Symlink,
}

/// Ein Verzeichniseintrag, wie ihn jedes Backend liefert.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub name: String,
    /// Voller Pfad auf der Gegenstelle (Unix-Notation, `/`-getrennt).
    pub path: String,
    pub kind: EntryKind,
    pub size: u64,
    /// Änderungszeit als Unix-Sekunden, wenn das Protokoll sie liefert.
    pub modified: Option<i64>,
}

/// Gemeinsame Abstraktion über alle Gegenstellen. Pfade sind immer
/// absolut in Unix-Notation; Transfers laufen gegen das lokale Dateisystem.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Menschenlesbarer Name der Verbindung (für Logs und UI).
    fn label(&self) -> String;

    /// Startverzeichnis nach dem Verbinden (z. B. Home-Verzeichnis).
    async fn initial_dir(&self) -> Result<String, FpError>;

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError>;

    /// Lädt `remote` in die lokale Datei `local` (überschreibt sie).
    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError>;

    /// Lädt die lokale Datei `local` nach `remote` hoch (überschreibt).
    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError>;

    async fn mkdir(&self, path: &str) -> Result<(), FpError>;

    /// Entfernt eine Datei oder ein (leeres) Verzeichnis.
    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError>;

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError>;
}
