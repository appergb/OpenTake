import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  buildFileInventory,
  classifyFile,
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
