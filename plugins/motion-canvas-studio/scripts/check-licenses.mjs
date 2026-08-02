import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const lock = JSON.parse(await readFile(new URL('../package-lock.json', import.meta.url), 'utf8'));
const allowed = new Set([
  'Apache-2.0',
  'BSD-3-Clause',
  'ISC',
  'MIT',
  '(BSD-3-Clause AND Apache-2.0)',
]);
const rejected = [];
for (const [path, metadata] of Object.entries(lock.packages)) {
  if (!path) continue;
  const license = metadata.license ?? (metadata.link ? lock.packages[metadata.resolved]?.license : undefined);
  if (!license || !allowed.has(license)) {
    rejected.push(`${path}: ${license ?? 'missing license'}`);
  }
}
assert.deepEqual(rejected, [], `unapproved dependency licenses:\n${rejected.join('\n')}`);
console.log(`verified ${Object.keys(lock.packages).length - 1} locked package license records`);
