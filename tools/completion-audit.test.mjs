import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  buildFileInventory,
  classifyFile,
  extractDocumentCandidates,
  normalizePath,
  stableId,
} from "./completion-audit.mjs";

test("normalizePath uses repository-relative POSIX paths", () => {
  assert.equal(normalizePath("./web\\src\\App.tsx"), "web/src/App.tsx");
});

test("stableId is deterministic and prefix scoped", () => {
  assert.equal(stableId("file", "web/src/App.tsx"), stableId("file", "web/src/App.tsx"));
  assert.notEqual(stableId("file", "web/src/App.tsx"), stableId("control", "web/src/App.tsx"));
});

test("extractDocumentCandidates captures headings, checkboxes, and gap signals", () => {
  const source = "# Plan\n## Export\n- [ ] Add HEVC\nKnown TODO: cancellation\n";
  const records = extractDocumentCandidates("docs/plan.md", source);
  assert.deepEqual(records.map((record) => [record.line, record.signal]), [
    [1, "heading"], [2, "heading"], [3, "unchecked"], [4, "gap-marker"],
  ]);
  assert.equal(records[2].heading, "Export");
});

test("classifyFile maps product and evidence files", () => {
  assert.deepEqual(classifyFile("crates/opentake-domain/src/clip.rs"), {
    domain: "opentake-domain",
    kind: "rust-source",
    material: true,
  });
  assert.deepEqual(classifyFile("web/src/App.tsx"), {
    domain: "web",
    kind: "tsx-source",
    material: true,
  });
  assert.equal(classifyFile("src-tauri/icons/icon.png").material, false);
  assert.equal(classifyFile(".github/workflows/ci.yml").domain, "ci");
});

test("classifyFile derives extensions from the basename only", () => {
  assert.deepEqual(classifyFile(".github/CODEOWNERS"), {
    domain: "ci",
    kind: "configuration",
    material: true,
  });
  assert.equal(classifyFile(".gitignore").kind, "configuration");
});

test("buildFileInventory marks its own output as self-referential", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(join(root, "source.txt"), "tracked source\n");
  writeFileSync(join(root, "inventory.json"), "placeholder\n");
  execFileSync("git", ["init", "--quiet", root]);
  execFileSync("git", ["-C", root, "add", "source.txt", "inventory.json"]);

  const files = buildFileInventory(root, "inventory.json");
  const source = files.find((file) => file.path === "source.txt");
  const inventory = files.find((file) => file.path === "inventory.json");

  assert.equal(
    source.sha256,
    createHash("sha256").update("tracked source\n").digest("hex"),
  );
  assert.equal(inventory.bytes, null);
  assert.equal(inventory.sha256, null);
  assert.equal(inventory.hashStatus, "self-reference");
  assert.match(inventory.reason, /cannot hash its own final bytes/);
});

test("files CLI writes tracked paths, hashes, and one self-reference", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-cli-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const inventoryPath = join(root, "inventory.json");
  writeFileSync(join(root, "source.txt"), "tracked source\n");
  writeFileSync(inventoryPath, "placeholder\n");
  execFileSync("git", ["init", "--quiet", root]);
  execFileSync("git", ["-C", root, "add", "source.txt", "inventory.json"]);

  const result = spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "files",
      "--root",
      root,
      "--out",
      inventoryPath,
    ],
    {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stderr);
  const inventory = JSON.parse(readFileSync(inventoryPath, "utf8"));
  assert.equal(inventory.schema, 1);
  assert.deepEqual(inventory.files.map((file) => file.path), [
    "inventory.json",
    "source.txt",
  ]);
  const source = inventory.files.find((file) => file.path === "source.txt");
  assert.equal(source.bytes, Buffer.byteLength("tracked source\n"));
  assert.equal(
    source.sha256,
    createHash("sha256").update("tracked source\n").digest("hex"),
  );
  const selfReferences = inventory.files.filter(
    (file) => file.hashStatus === "self-reference",
  );
  assert.equal(selfReferences.length, 1);
  assert.equal(selfReferences[0].path, "inventory.json");
  assert.equal(selfReferences[0].bytes, null);
  assert.equal(selfReferences[0].sha256, null);
});

test("docs CLI deterministically extracts tracked Markdown only", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-docs-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const firstOutput = join(root, "..", `${basename(root)}-first.json`);
  const secondOutput = join(root, "..", `${basename(root)}-second.json`);
  t.after(() => {
    rmSync(firstOutput, { force: true });
    rmSync(secondOutput, { force: true });
  });
  writeFileSync(join(root, "b.md"), "# Beta\nTODO: beta gap\n");
  writeFileSync(join(root, "a.md"), "# Alpha\n- [ ] alpha gap\n");
  writeFileSync(join(root, "tracked.txt"), "TODO: not Markdown\n");
  writeFileSync(join(root, "untracked.md"), "# Untracked\nTODO: ignore me\n");
  execFileSync("git", ["init", "--quiet", root]);
  execFileSync("git", ["-C", root, "add", "a.md", "b.md", "tracked.txt"]);

  const invoke = (output) => spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "docs",
      "--root",
      root,
      "--out",
      output,
    ],
    {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    },
  );

  const firstResult = invoke(firstOutput);
  const secondResult = invoke(secondOutput);
  assert.equal(firstResult.status, 0, firstResult.stderr);
  assert.equal(secondResult.status, 0, secondResult.stderr);
  assert.equal(readFileSync(firstOutput, "utf8"), readFileSync(secondOutput, "utf8"));

  const documentCandidates = JSON.parse(readFileSync(firstOutput, "utf8"));
  assert.equal(documentCandidates.schema, 1);
  assert.deepEqual(documentCandidates.candidates.map(({ path, line, signal }) => [
    path,
    line,
    signal,
  ]), [
    ["a.md", 1, "heading"],
    ["a.md", 2, "unchecked"],
    ["b.md", 1, "heading"],
    ["b.md", 2, "gap-marker"],
  ]);
  assert.equal(
    new Set(documentCandidates.candidates.map(({ id }) => id)).size,
    documentCandidates.candidates.length,
  );
});

test("requirements CLI creates complete shells and preserves reviewed records", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-requirements-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const candidatesPath = join(root, "candidates.json");
  const requirementsPath = join(root, "requirements.json");
  const candidates = [
    {
      id: stableId("doc", "docs/a.md:1:# Alpha"),
      path: "docs/a.md",
      line: 1,
      heading: "Alpha",
      text: "# Alpha",
      signal: "heading",
    },
    {
      id: stableId("doc", "docs/b.md:2:- [ ] Beta"),
      path: "docs/b.md",
      line: 2,
      heading: "Beta",
      text: "- [ ] Beta",
      signal: "unchecked",
    },
  ];
  const reviewed = {
    id: stableId("requirement", candidates[0].id),
    candidateId: candidates[0].id,
    source: { path: candidates[0].path, line: candidates[0].line },
    targetBehavior: "Alpha is visible",
    priority: "high",
    status: "complete",
    uiEntry: ["Home"],
    visibleResult: "Alpha",
    react: ["AlphaView"],
    storeApi: ["alphaStore"],
    tauri: ["alpha_command"],
    rust: ["alpha::command"],
    sideEffects: ["project update"],
    returnPath: ["snapshot"],
    automatedTests: ["alpha test"],
    runtimeEvidence: ["desktop pass"],
    provenance: ["OpenTake decision"],
    acceptanceCriteria: ["Alpha renders"],
    gapGroup: "alpha",
    finalDisposition: "verified",
    commit: "abc123",
    reviewNotes: ["preserve this evidence"],
  };
  writeFileSync(candidatesPath, `${JSON.stringify({ schema: 1, candidates }, null, 2)}\n`);
  writeFileSync(requirementsPath, `${JSON.stringify({
    schema: 1,
    records: [reviewed, {
      id: "requirement-stale",
      candidateId: "doc-stale",
      source: { path: "docs/stale.md", line: 1 },
    }],
  }, null, 2)}\n`);

  const invoke = () => spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "requirements",
      "--root",
      root,
      "--candidates",
      candidatesPath,
      "--out",
      requirementsPath,
    ],
    {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    },
  );

  const firstResult = invoke();
  assert.equal(firstResult.status, 0, firstResult.stderr);
  const firstBytes = readFileSync(requirementsPath, "utf8");
  const requirements = JSON.parse(firstBytes);
  assert.equal(requirements.schema, 1);
  assert.equal(requirements.records.length, candidates.length);
  assert.deepEqual(requirements.records.map(({ candidateId }) => candidateId), candidates.map(({ id }) => id));
  assert.equal(new Set(requirements.records.map(({ id }) => id)).size, requirements.records.length);
  assert.deepEqual(requirements.records[0], reviewed);
  assert.deepEqual(requirements.records[1], {
    id: stableId("requirement", candidates[1].id),
    candidateId: candidates[1].id,
    source: { path: candidates[1].path, line: candidates[1].line },
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
  });

  const secondResult = invoke();
  assert.equal(secondResult.status, 0, secondResult.stderr);
  assert.equal(readFileSync(requirementsPath, "utf8"), firstBytes);
});

test("requirements CLI rejects duplicate candidate identities", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-duplicates-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const candidatesPath = join(root, "candidates.json");
  const requirementsPath = join(root, "requirements.json");
  const candidate = {
    id: "doc-duplicate",
    path: "docs/duplicate.md",
    line: 1,
    heading: "Duplicate",
    text: "# Duplicate",
    signal: "heading",
  };
  writeFileSync(candidatesPath, `${JSON.stringify({
    schema: 1,
    candidates: [candidate, { ...candidate }],
  }, null, 2)}\n`);

  const result = spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "requirements",
      "--root",
      root,
      "--candidates",
      candidatesPath,
      "--out",
      requirementsPath,
    ],
    {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    },
  );

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /duplicate candidate id: doc-duplicate/);
});

test("module imports when process.argv[1] is unavailable", () => {
  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "--eval", 'await import("./completion-audit.mjs");'],
    {
      cwd: fileURLToPath(new URL(".", import.meta.url)),
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stderr);
});
