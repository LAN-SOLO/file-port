/** Pfad-Helfer — verstehen `/` (remote & macOS) und `\` (Windows). */

/** Windows-Laufwerkswurzel wie `C:\`, `C:/` oder `C:`. */
const DRIVE_ROOT = /^[A-Za-z]:[\\/]?$/;
const DRIVE = /^[A-Za-z]:/;

function sepOf(path: string): '/' | '\\' {
  if (DRIVE.test(path)) return path.includes('/') && !path.includes('\\') ? '/' : '\\';
  return path.includes('\\') && !path.includes('/') ? '\\' : '/';
}

export function parentPath(path: string): string {
  // Laufwerkswurzel bleibt Laufwerkswurzel (nie das nackte `C:` — das wäre
  // unter Windows das aktuelle Verzeichnis des Laufwerks, nicht die Wurzel).
  if (DRIVE_ROOT.test(path)) return path.slice(0, 2) + sepOf(path);
  const sep = sepOf(path);
  const trimmed = path.length > 1 ? path.replace(/[/\\]+$/, '') : path;
  if (DRIVE.test(trimmed)) {
    // C:\Users → C:\ ; C:\Users\bob → C:\Users
    const idx = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
    return idx <= 2 ? trimmed.slice(0, 2) + sep : trimmed.slice(0, idx);
  }
  const idx = trimmed.lastIndexOf(sep);
  if (idx <= 0) return sep === '\\' ? trimmed.slice(0, 3) || '\\' : '/';
  return trimmed.slice(0, idx);
}

export function joinPath(dir: string, name: string): string {
  const sep = sepOf(dir);
  if (dir === '') return sep + name;
  if (DRIVE_ROOT.test(dir)) return dir.slice(0, 2) + sep + name;
  if (dir === sep) return dir + name;
  return dir.replace(/[/\\]+$/, '') + sep + name;
}

/** Neuer Datei-/Ordnername aus einem Eingabefeld: keine Pfadtrenner, nicht `.`/`..`. */
export function isValidName(name: string): boolean {
  const n = name.trim();
  return n !== '' && n !== '.' && n !== '..' && !/[/\\\0]/.test(n);
}
