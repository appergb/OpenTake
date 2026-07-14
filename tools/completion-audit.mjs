import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function normalizePath(value) {
  return value.replaceAll("\\", "/").replace(/^\.\//, "");
}

export function stableId(prefix, value) {
  return `${prefix}-${createHash("sha256").update(value).digest("hex").slice(0, 16)}`;
}

export function readTrackedFiles(root) {
  return execFileSync("git", ["-C", root, "ls-files", "-z"])
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .map(normalizePath)
    .sort();
}

function writeJson(path, value) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function parseArgs(argv) {
  const [command, ...rest] = argv;
  const values = { command };
  for (let index = 0; index < rest.length; index += 2) {
    values[rest[index].replace(/^--/, "")] = rest[index + 1];
  }
  return values;
}

async function runCommand(command) {
  throw new Error(`unsupported command: ${command}`);
}

async function main(argv) {
  const args = parseArgs(argv);
  if (!args.command || !args.root || !args.out) {
    throw new Error("usage: completion-audit <command> --root <repo> --out <path>");
  }
  const root = resolve(args.root);
  const output = resolve(args.out);
  const result = await runCommand(args.command, root, args);
  writeJson(output, result);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
