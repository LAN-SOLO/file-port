//! Kleine Helfer für Remote-Pfade in Unix-Notation (`/`-getrennt).

/// Hängt `name` an `dir` an, ohne doppelte Schrägstriche zu erzeugen.
pub fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir == "/" {
        format!("/{name}")
    } else {
        format!("{}/{}", dir.trim_end_matches('/'), name)
    }
}

/// Übergeordnetes Verzeichnis; `/` bleibt `/`.
pub fn parent(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(idx) => trimmed[..idx].to_string(),
    }
}

/// Letzte Pfadkomponente (Dateiname).
pub fn file_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_handles_root_and_trailing_slash() {
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("", "a"), "/a");
        assert_eq!(join("/x", "a"), "/x/a");
        assert_eq!(join("/x/", "a"), "/x/a");
    }

    #[test]
    fn parent_walks_up_to_root() {
        assert_eq!(parent("/a/b"), "/a");
        assert_eq!(parent("/a"), "/");
        assert_eq!(parent("/"), "/");
        assert_eq!(parent("/a/b/"), "/a");
    }

    #[test]
    fn file_name_takes_last_component() {
        assert_eq!(file_name("/a/b.txt"), "b.txt");
        assert_eq!(file_name("/a/dir/"), "dir");
    }
}
