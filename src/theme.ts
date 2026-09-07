/** Dark/Light-Modus — persistiert in localStorage (fileport hat keine
 *  Settings-Datei). Default ist dark; Anwenden über <html data-theme>. */
export type Theme = 'dark' | 'light';

const KEY = 'theme';

export function readTheme(): Theme {
  try {
    return localStorage.getItem(KEY) === 'light' ? 'light' : 'dark';
  } catch {
    return 'dark';
  }
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // localStorage nicht verfügbar — Theme gilt dann nur für diese Sitzung
  }
}
