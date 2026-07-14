import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_FILE_INVENTORY_PATH = "docs/audit/2026-07-14/repository-files.json";

export function normalizePath(value) {
  return value.replaceAll("\\", "/").replace(/^\.\//, "");
}

export function stableId(prefix, value) {
  return `${prefix}-${createHash("sha256").update(value).digest("hex").slice(0, 16)}`;
}

export function extractDocumentCandidates(path, source) {
  const normalizedPath = normalizePath(path);
  let heading = "";
  let fence = null;
  const records = [];
  source.split(/\r?\n/).forEach((text, index) => {
    const line = index + 1;
    if (fence) {
      const closingMatch = text.match(/^ {0,3}(`{3,}|~{3,})[ \t]*$/);
      if (
        closingMatch
        && closingMatch[1][0] === fence.character
        && closingMatch[1].length >= fence.length
      ) {
        fence = null;
      }
      return;
    }

    const openingMatch = text.match(/^ {0,3}(`{3,}|~{3,})(.*)$/);
    if (
      openingMatch
      && (openingMatch[1][0] !== "`" || !openingMatch[2].includes("`"))
    ) {
      fence = {
        character: openingMatch[1][0],
        length: openingMatch[1].length,
      };
      return;
    }

    const headingMatch = text.match(/^#{1,6}\s+(.+)$/);
    if (headingMatch) heading = headingMatch[1].trim();
    const signal = headingMatch
      ? "heading"
      : /^\s*[-*]\s+\[\s\]/.test(text)
        ? "unchecked"
        : /\b(TODO|FIXME|TBD|stub|placeholder|not implemented|unimplemented)\b|未完成|待办|缺失/i.test(text)
          ? "gap-marker"
          : null;
    if (signal) {
      records.push({
        id: stableId("doc", `${normalizedPath}:${line}:${text}`),
        path: normalizedPath,
        line,
        heading,
        text: text.trim(),
        signal,
      });
    }
  });
  return records;
}

export function readTrackedFiles(root) {
  return execFileSync("git", ["-C", root, "ls-files", "-z"])
    .toString("utf8")
    .split("\0")
    .filter(Boolean)
    .map(normalizePath)
    .sort();
}

export function classifyFile(path) {
  const normalizedPath = normalizePath(path);
  const segments = normalizedPath.split("/");
  const basename = segments[segments.length - 1];
  const extensionSeparator = basename.lastIndexOf(".");
  const extension = extensionSeparator > 0 ? basename.slice(extensionSeparator + 1) : "";
  const domain = normalizedPath.startsWith("crates/")
    ? segments[1]
    : normalizedPath.startsWith("web/")
      ? "web"
      : normalizedPath.startsWith("src-tauri/")
        ? "src-tauri"
        : normalizedPath.startsWith("docs/")
          ? "docs"
          : normalizedPath.startsWith(".github/")
            ? "ci"
            : "repository";
  const kind = extension === "rs"
    ? "rust-source"
    : extension === "tsx"
      ? "tsx-source"
      : extension === "ts"
        ? "typescript-source"
        : extension === "md"
          ? "markdown"
          : ["png", "jpg", "ico", "icns"].includes(extension)
            ? "image"
            : extension || "configuration";
  return { domain, kind, material: kind !== "image" };
}

export function buildFileInventory(root, selfPath = DEFAULT_FILE_INVENTORY_PATH) {
  const normalizedSelfPath = normalizePath(selfPath);
  return readTrackedFiles(root).map((path) => {
    const record = {
      id: stableId("file", path),
      path,
      ...classifyFile(path),
    };
    if (path === normalizedSelfPath) {
      return {
        ...record,
        bytes: null,
        sha256: null,
        hashStatus: "self-reference",
        reason: "inventory cannot hash its own final bytes",
      };
    }
    const bytes = readFileSync(resolve(root, path));
    return {
      ...record,
      bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    };
  });
}

function assertUniqueCandidates(candidates) {
  if (!Array.isArray(candidates)) {
    throw new Error("candidate ledger must contain a candidates array");
  }
  const ids = new Set();
  const locations = new Set();
  for (const candidate of candidates) {
    if (!candidate || typeof candidate.id !== "string") {
      throw new Error("candidate id must be a string");
    }
    if (ids.has(candidate.id)) {
      throw new Error(`duplicate candidate id: ${candidate.id}`);
    }
    ids.add(candidate.id);
    const path = normalizePath(candidate.path);
    const location = `${path}:${candidate.line}`;
    if (locations.has(location)) {
      throw new Error(`duplicate candidate location: ${location}`);
    }
    locations.add(location);
  }
}

function createRequirementRecord(candidate) {
  return {
    id: stableId("requirement", candidate.id),
    candidateId: candidate.id,
    source: { path: candidate.path, line: candidate.line },
    targetBehavior: null,
    priority: null,
    status: "unverified",
    uiEntry: [],
    visibleResult: null,
    react: [],
    storeApi: [],
    tauri: [],
    rust: [],
    sideEffects: [],
    returnPath: [],
    automatedTests: [],
    runtimeEvidence: [],
    provenance: [],
    acceptanceCriteria: [],
    gapGroup: null,
    finalDisposition: null,
    commit: null,
  };
}

function mergeRequirementLedger(candidates, existing) {
  assertUniqueCandidates(candidates);
  const existingRecords = existing?.records ?? [];
  if (!Array.isArray(existingRecords)) {
    throw new Error("requirement ledger must contain a records array");
  }

  const priorByCandidateId = new Map();
  const priorRecordIds = new Set();
  for (const record of existingRecords) {
    if (!record || typeof record.candidateId !== "string") {
      throw new Error("requirement candidateId must be a string");
    }
    if (priorByCandidateId.has(record.candidateId)) {
      throw new Error(`duplicate requirement candidateId: ${record.candidateId}`);
    }
    if (typeof record.id !== "string") {
      throw new Error("requirement id must be a string");
    }
    if (priorRecordIds.has(record.id)) {
      throw new Error(`duplicate requirement id: ${record.id}`);
    }
    priorByCandidateId.set(record.candidateId, record);
    priorRecordIds.add(record.id);
  }

  const records = candidates.map(
    (candidate) => priorByCandidateId.get(candidate.id) ?? createRequirementRecord(candidate),
  );
  const recordIds = new Set(records.map(({ id }) => id));
  if (recordIds.size !== records.length) {
    throw new Error("generated requirement ids are not unique");
  }
  return records;
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

async function runCommand(command, root, args) {
  if (command === "files") {
    const selfPath = normalizePath(relative(root, resolve(args.out)));
    return { schema: 1, files: buildFileInventory(root, selfPath) };
  }
  if (command === "docs") {
    const candidates = readTrackedFiles(root)
      .filter((path) => path.endsWith(".md"))
      .flatMap((path) => extractDocumentCandidates(
        path,
        readFileSync(resolve(root, path), "utf8"),
      ));
    assertUniqueCandidates(candidates);
    return { schema: 1, candidates };
  }
  if (command === "requirements") {
    if (!args.candidates) {
      throw new Error("requirements command needs --candidates <path>");
    }
    const candidateLedger = JSON.parse(readFileSync(resolve(args.candidates), "utf8"));
    const existing = existsSync(resolve(args.out))
      ? JSON.parse(readFileSync(resolve(args.out), "utf8"))
      : null;
    return {
      schema: 1,
      records: mergeRequirementLedger(candidateLedger.candidates, existing),
    };
  }
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

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
