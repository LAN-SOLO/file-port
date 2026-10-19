// Übersetzung der deutschen Engine-Fehler für die englische UI.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { isExistsError, translateError } from '../src/errors.ts';

const en = (s: string) => translateError(s, 'en');

test('DE bleibt unverändert', () => {
  const msg = 'Verbindung fehlgeschlagen: Zeitüberschreitung beim Verbinden mit host';
  assert.equal(translateError(msg, 'de'), msg);
  assert.equal(translateError(new Error('Profil nicht gefunden'), 'de'), 'Profil nicht gefunden');
});

test('zusammengesetzte Meldungen', () => {
  assert.equal(
    en('Anmeldung fehlgeschlagen: Schlüsseldatei: Datei oder Ordner nicht gefunden (OS-Fehler 2)'),
    'Authentication failed: Key file: File or folder not found (OS error 2)'
  );
  assert.equal(
    en('E/A-Fehler: Die Datei wird von einem anderen Programm verwendet (OS-Fehler 32)'),
    'I/O error: The file is in use by another program (OS error 32)'
  );
  assert.equal(
    en('Verbindung fehlgeschlagen: Zeitüberschreitung beim Verbinden mit nas.local'),
    'Connection failed: Timed out connecting to nas.local'
  );
  assert.equal(
    en('Anmeldung fehlgeschlagen: Server hat die Anmeldung als bob abgelehnt'),
    'Authentication failed: The server rejected the login as bob'
  );
  assert.equal(en('Nicht gefunden: /a/Größe.txt'), 'Not found: /a/Größe.txt');
  assert.equal(en('E/A-Fehler: abgebrochen'), 'I/O error: cancelled');
  assert.equal(en('Übertragung abgebrochen'), 'Transfer cancelled');
});

test('Namens- und Kollisionsfehler aus engine.rs/transfers.rs', () => {
  assert.equal(
    en('Der Dateiname „a:b.txt“ ist auf diesem System nicht zulässig — bitte auf dem Server umbenennen'),
    'The file name “a:b.txt” is not allowed on this system — please rename it on the server'
  );
  assert.equal(
    en('Der Dateiname „CON“ ist auf diesem System nicht zulässig'),
    'The file name “CON” is not allowed on this system'
  );
  assert.equal(
    en('„Report.pdf“ existiert im Zielordner bereits'),
    '“Report.pdf” already exists in the target folder'
  );
  assert.equal(
    en('Ungültiger lokaler Zielpfad (enthält „..“): C:\\x\\..'),
    'Invalid local target path (contains “..”): C:\\x\\..'
  );
  assert.ok(isExistsError('„x“ existiert im Zielordner bereits'));
  assert.ok(!isExistsError('Profil nicht gefunden'));
});

test('Updater-Fehler', () => {
  assert.equal(en('Update-Prüfung fehlgeschlagen: network'), 'Update check failed: network');
  assert.equal(en('Kein Update verfügbar'), 'No update available');
});

test('englische Bibliotheksmeldungen bleiben stehen', () => {
  assert.equal(en('Connection refused (os error 61)'), 'Connection refused (os error 61)');
});

// Wächter: jede feste deutsche Fehlermeldung im Rust-Code (Literal in
// Err/FpError/format!/ok_or) muss in der englischen UI ohne deutsche Wörter
// ankommen. Platzhalter {…} werden durch Beispielwerte ersetzt.
test('alle deutschen Rust-Fehlertexte haben eine Übersetzung', () => {
  const root = join(import.meta.dirname, '..');
  const files = ['core/src', 'src-tauri/src'].flatMap((d) =>
    readdirSync(join(root, d))
      .filter((f) => f.endsWith('.rs'))
      .map((f) => join(root, d, f))
  );
  const german = /[äöüÄÖÜß„]|\b(nicht|kein|keine|keinen|fehlt|fehlgeschlagen|abgelehnt|ungültig|Ungültige?r?|bereits|Zugriff|Anmeldung|Verbindung|Datei|Ordner|Freigabe|bitte|Schlüssel\w*|Zeitüberschreitung)\b/;
  const missing: string[] = [];
  for (const file of files) {
    let src = readFileSync(file, 'utf8');
    // Testmodule nicht prüfen
    src = src.split('#[cfg(test)]')[0];
    // Rust-Zeilenfortsetzung „\<newline><spaces>“ zusammenziehen
    src = src.replace(/\\\n\s*/g, '');
    const re = /(?:Err\(|FpError::\w+\(|format!\(|ok_or\(|ok_or_else\(\|\|\s*|#\[error\(|=> )\s*"((?:[^"\\]|\\.)*)"/g;
    for (const m of src.matchAll(re)) {
      const lit = m[1];
      if (!german.test(lit)) continue;
      const sample = lit
        .replace(/\{\}|\{[^}]*\}/g, 'X')
        .replace(/\\"/g, '"');
      const out = en(sample);
      if (german.test(out.replace(/X/g, ''))) missing.push(`${file.slice(root.length + 1)}: ${lit} → ${out}`);
    }
  }
  assert.deepEqual(missing, []);
});
