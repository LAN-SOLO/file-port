// Pfad-Helfer der UI — läuft ohne Zusatz-Abhängigkeiten mit dem Node-Testrunner:
//   pnpm test   (= node --experimental-strip-types --test tests/*.test.ts)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isValidName, joinPath, parentPath } from '../src/paths.ts';

test('parentPath: Unix-/Remote-Pfade', () => {
  assert.equal(parentPath('/a/b'), '/a');
  assert.equal(parentPath('/a'), '/');
  assert.equal(parentPath('/'), '/');
  assert.equal(parentPath('/a/b/'), '/a');
  // S3-Schlüssel dürfen Backslashes enthalten — Trenner bleibt „/"
  assert.equal(parentPath('/a\\b/c'), '/a\\b');
});

test('parentPath: Windows-Laufwerke bleiben auf der Wurzel', () => {
  assert.equal(parentPath('C:\\Users\\bob'), 'C:\\Users');
  assert.equal(parentPath('C:\\Users'), 'C:\\');
  assert.equal(parentPath('C:\\'), 'C:\\');
  assert.equal(parentPath('C:'), 'C:\\');
  assert.equal(parentPath('C:/Users'), 'C:/');
  assert.equal(parentPath('C:/Users/x'), 'C:/Users');
  assert.equal(parentPath('\\\\srv\\share\\dir'), '\\\\srv\\share');
});

test('joinPath', () => {
  assert.equal(joinPath('C:\\', 'x'), 'C:\\x');
  assert.equal(joinPath('C:', 'x'), 'C:\\x');
  assert.equal(joinPath('C:\\Users', 'x'), 'C:\\Users\\x');
  assert.equal(joinPath('C:/Users', 'x'), 'C:/Users/x');
  assert.equal(joinPath('/', 'x'), '/x');
  assert.equal(joinPath('/a/', 'x'), '/a/x');
  assert.equal(joinPath('/a', 'x'), '/a/x');
  assert.equal(joinPath('', 'x'), '/x');
});

test('isValidName', () => {
  assert.ok(isValidName('Bericht 2026.pdf'));
  assert.ok(!isValidName('a/b'));
  assert.ok(!isValidName('a\\b'));
  assert.ok(!isValidName('..'));
  assert.ok(!isValidName('.'));
  assert.ok(!isValidName('  '));
});
