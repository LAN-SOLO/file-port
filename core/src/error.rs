use thiserror::Error;

/// Fehlerarten der Engine — Protokoll-Details werden auf sprechende,
/// UI-taugliche Varianten abgebildet statt roher Bibliotheksfehler.
#[derive(Debug, Error)]
pub enum FpError {
    #[error("Verbindung fehlgeschlagen: {0}")]
    Connect(String),
    #[error("Anmeldung fehlgeschlagen: {0}")]
    Auth(String),
    /// SSH: der Server zeigt einen anderen Host-Key als den gespeicherten.
    /// `seen` ist der tatsächlich präsentierte Key (OpenSSH-Format), damit
    /// die App ihn anzeigen und auf ausdrücklichen Wunsch übernehmen kann.
    #[error("Host-Key von {host} stimmt nicht mit dem gespeicherten überein — möglicher Angriff (Man-in-the-Middle) oder neu aufgesetzter Server")]
    HostKeyMismatch { host: String, seen: String },
    #[error("Nicht gefunden: {0}")]
    NotFound(String),
    #[error("Zugriff verweigert: {0}")]
    Denied(String),
    #[error("Übertragung abgebrochen")]
    Cancelled,
    #[error("E/A-Fehler: {}", io_text(.0))]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Protocol(String),
}

/// Häufige Betriebssystem-Fehler in fester deutscher Formulierung — die UI
/// übersetzt diese Texte (src/errors.ts) für die englische Oberfläche. Der
/// System-Fehlercode hängt in Klammern an (für Support-Fälle).
pub fn io_text(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    // Windows: Freigabe-/Sperrverletzung (Virenscanner, OneDrive, Office) —
    // ERROR_SHARING_VIOLATION (32) / ERROR_LOCK_VIOLATION (33).
    let windows_locked = cfg!(windows) && matches!(e.raw_os_error(), Some(32) | Some(33));
    let text = if windows_locked {
        "Die Datei wird von einem anderen Programm verwendet"
    } else {
        match e.kind() {
            K::NotFound => "Datei oder Ordner nicht gefunden",
            K::PermissionDenied => "Keine Berechtigung",
            K::AlreadyExists => "Existiert bereits",
            K::StorageFull | K::QuotaExceeded => "Kein Speicherplatz mehr frei",
            K::DirectoryNotEmpty => "Ordner ist nicht leer",
            K::InvalidFilename => "Ungültiger oder zu langer Dateiname",
            K::ReadOnlyFilesystem => "Datenträger ist schreibgeschützt",
            K::TimedOut => "Zeitüberschreitung",
            K::IsADirectory => "Ziel ist ein Ordner",
            K::NotADirectory => "Ziel ist kein Ordner",
            K::CrossesDevices => "Verschieben über Laufwerksgrenzen nicht möglich",
            _ => return e.to_string(),
        }
    };
    match e.raw_os_error() {
        Some(code) => format!("{text} (OS-Fehler {code})"),
        None => text.to_string(),
    }
}

impl FpError {
    /// Kurzform für Protokollfehler aus beliebigen Fehlertypen.
    pub fn proto(e: impl std::fmt::Display) -> Self {
        FpError::Protocol(e.to_string())
    }
}
