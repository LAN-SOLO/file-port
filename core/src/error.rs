use thiserror::Error;

/// Fehlerarten der Engine — Protokoll-Details werden auf sprechende,
/// UI-taugliche Varianten abgebildet statt roher Bibliotheksfehler.
#[derive(Debug, Error)]
pub enum FpError {
    #[error("Verbindung fehlgeschlagen: {0}")]
    Connect(String),
    #[error("Anmeldung fehlgeschlagen: {0}")]
    Auth(String),
    #[error("Nicht gefunden: {0}")]
    NotFound(String),
    #[error("Zugriff verweigert: {0}")]
    Denied(String),
    #[error("Übertragung abgebrochen")]
    Cancelled,
    #[error("E/A-Fehler: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Protocol(String),
}

impl FpError {
    /// Kurzform für Protokollfehler aus beliebigen Fehlertypen.
    pub fn proto(e: impl std::fmt::Display) -> Self {
        FpError::Protocol(e.to_string())
    }
}
