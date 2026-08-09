import {readFile, writeFile} from 'node:fs/promises';

const bundle = new URL('../bundle/runner.html', import.meta.url);
const source = await readFile(bundle, 'utf8');
const normalized = `${source.replace(/[\t ]+$/gm, '').trimEnd()}\n`;
await writeFile(bundle, normalized);
