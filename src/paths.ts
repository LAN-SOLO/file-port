/** Pfad-Helfer — verstehen `/` (remote & macOS) und `\` (Windows). */

export function parentPath(path: string): string {
  const sep = path.includes('\\') && !path.includes('/') ? '\\' : '/';
  const trimmed = path.length > 1 ? path.replace(/[/\\]+$/, '') : path;
  const idx = trimmed.lastIndexOf(sep);
  if (idx <= 0) return sep === '\\' ? trimmed.slice(0, 3) : '/';
  return trimmed.slice(0, idx);
}

export function joinPath(dir: string, name: string): string {
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return (dir === sep ? dir : dir.replace(/[/\\]+$/, '')) + sep + name;
}
