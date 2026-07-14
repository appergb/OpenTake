# OpenTake Completion Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a fail-closed, reviewable ledger that proves every tracked file, planning requirement, upstream/downstream change, interface, and interactive control has been inspected and either verified complete or assigned to an exact implementation plan.

**Architecture:** A dependency-free Node 20 audit CLI generates deterministic candidate inventories from Git, Markdown, and the TypeScript compiler API. Human/agent review adds evidence-backed dispositions in stable JSON ledgers, and a verifier refuses completion while any source file, document candidate, upstream/downstream change, or UI control lacks a final record. Product fixes are then split into subsystem plans from the verified gap set.

**Tech Stack:** Node.js 20 built-ins, TypeScript 5 compiler API from `web/node_modules`, Git, JSON, Markdown, React 18, Tauri 2, Rust workspace tooling.

## Global Constraints

- Start from cloud tree `d992a0e3a7657d4d9b6ca66afb977433ca6b5e6a` on branch `audit/opentake-completion-20260714`.
- Preserve `../OpenTake` exactly; its 52 dirty paths are read-only evidence and must never be reset, checked out, or bulk copied.
- Never edit `../palmier-pro-upstream`; refresh and inspect it read-only.
- Priority labels do not remove valid work from scope.
- A requirement is complete only when requirement, implementation, user-visible behavior, and verification evidence agree.
- Generated/vendor content is inventoried but reviewed line by line only when shipped or relevant to reproducibility/security.
- No force pushes, destructive Git commands, silent no-ops, placeholder controls, or weakened data/filesystem/platform safety.
- Every product implementation task derived from this audit must use TDD, a separate review agent, and the strongest relevant local, installed-app, and cloud checks.

---

### Task 1: Deterministic audit CLI core

**Files:**
- Create: `tools/completion-audit.mjs`
- Create: `tools/completion-audit.test.mjs`

**Interfaces:**
- Produces: `normalizePath(path: string): string`
- Produces: `stableId(prefix: string, value: string): string`
- Produces: `readTrackedFiles(root: string): string[]`
- Produces: CLI subcommands `files`, `docs`, `controls`, `sources`, and `verify`

- [ ] **Step 1: Write failing core tests**

```js
import assert from "node:assert/strict";
import test from "node:test";
import { normalizePath, stableId } from "./completion-audit.mjs";

test("normalizePath uses repository-relative POSIX paths", () => {
  assert.equal(normalizePath("./web\\src\\App.tsx"), "web/src/App.tsx");
});

test("stableId is deterministic and prefix scoped", () => {
  assert.equal(stableId("file", "web/src/App.tsx"), stableId("file", "web/src/App.tsx"));
  assert.notEqual(stableId("file", "web/src/App.tsx"), stableId("control", "web/src/App.tsx"));
});
```

- [ ] **Step 2: Run the tests and confirm the module is missing**

Run: `node --test tools/completion-audit.test.mjs`  
Expected: FAIL because `tools/completion-audit.mjs` does not exist.

- [ ] **Step 3: Implement the core module and argument parser**

```js
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function normalizePath(value) {
  return value.replaceAll("\\\\", "/").replace(/^\.\//, "");
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
```

- [ ] **Step 4: Add a temporary unsupported-command implementation**

```js
async function runCommand(command) {
  throw new Error(`unsupported command: ${command}`);
}
```

- [ ] **Step 5: Run tests and syntax check**

Run: `node --check tools/completion-audit.mjs && node --test tools/completion-audit.test.mjs`  
Expected: 2 tests PASS.

- [ ] **Step 6: Commit the core**

```bash
git add tools/completion-audit.mjs tools/completion-audit.test.mjs
git commit -m "test(audit): add deterministic completion audit core"
```

### Task 2: Tracked-file inventory

**Files:**
- Modify: `tools/completion-audit.mjs`
- Modify: `tools/completion-audit.test.mjs`
- Create: `docs/audit/2026-07-14/repository-files.json`

**Interfaces:**
- Consumes: `readTrackedFiles(root)`
- Produces: `classifyFile(path: string): {domain: string, kind: string, material: boolean}`
- Produces: `buildFileInventory(root: string): FileRecord[]`

- [ ] **Step 1: Add failing classification tests**

```js
import { classifyFile } from "./completion-audit.mjs";

test("classifyFile maps product and evidence files", () => {
  assert.deepEqual(classifyFile("crates/opentake-domain/src/clip.rs"), {
    domain: "opentake-domain", kind: "rust-source", material: true,
  });
  assert.deepEqual(classifyFile("web/src/App.tsx"), {
    domain: "web", kind: "tsx-source", material: true,
  });
  assert.equal(classifyFile("src-tauri/icons/icon.png").material, false);
  assert.equal(classifyFile(".github/workflows/ci.yml").domain, "ci");
});
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `node --test --test-name-pattern='classifyFile' tools/completion-audit.test.mjs`  
Expected: FAIL because `classifyFile` is not exported.

- [ ] **Step 3: Implement classification and SHA-256 inventory**

```js
export function classifyFile(path) {
  const segments = normalizePath(path).split("/");
  const extension = path.includes(".") ? path.slice(path.lastIndexOf(".") + 1) : "";
  const domain = path.startsWith("crates/") ? segments[1]
    : path.startsWith("web/") ? "web"
    : path.startsWith("src-tauri/") ? "src-tauri"
    : path.startsWith("docs/") ? "docs"
    : path.startsWith(".github/") ? "ci"
    : "repository";
  const kind = extension === "rs" ? "rust-source"
    : extension === "tsx" ? "tsx-source"
    : extension === "ts" ? "typescript-source"
    : extension === "md" ? "markdown"
    : ["png", "jpg", "ico", "icns"].includes(extension) ? "image"
    : extension || "configuration";
  return { domain, kind, material: kind !== "image" };
}

export function buildFileInventory(root) {
  return readTrackedFiles(root).map((path) => {
    const bytes = readFileSync(resolve(root, path));
    return {
      id: stableId("file", path),
      path,
      ...classifyFile(path),
      bytes: bytes.length,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    };
  });
}
```

- [ ] **Step 4: Route the `files` command**

```js
async function runCommand(command, root, args) {
  if (command === "files") return { schema: 1, files: buildFileInventory(root) };
  throw new Error(`unsupported command: ${command}`);
}
```

- [ ] **Step 5: Run tests and generate the inventory**

Run:

```bash
node --test tools/completion-audit.test.mjs
node tools/completion-audit.mjs files --root . --out docs/audit/2026-07-14/repository-files.json
jq '.files | length' docs/audit/2026-07-14/repository-files.json
```

Expected: tests PASS; count equals `git ls-files | wc -l` after the new tracked files are staged and the inventory is regenerated.

- [ ] **Step 6: Commit the inventory**

```bash
git add tools/completion-audit.mjs tools/completion-audit.test.mjs docs/audit/2026-07-14/repository-files.json
git commit -m "feat(audit): inventory every tracked OpenTake file"
```

### Task 3: Planning-document candidate extraction

**Files:**
- Modify: `tools/completion-audit.mjs`
- Modify: `tools/completion-audit.test.mjs`
- Create: `docs/audit/2026-07-14/document-candidates.json`
- Create: `docs/audit/2026-07-14/requirements.json`

**Interfaces:**
- Produces: `extractDocumentCandidates(path: string, source: string): DocumentCandidate[]`
- Produces: candidate fields `id`, `path`, `line`, `heading`, `text`, and `signal`
- Produces: final requirement fields specified by the approved design section 5

- [ ] **Step 1: Add failing Markdown extraction tests**

```js
import { extractDocumentCandidates } from "./completion-audit.mjs";

test("extractDocumentCandidates captures headings, checkboxes, and gap signals", () => {
  const source = "# Plan\n## Export\n- [ ] Add HEVC\nKnown TODO: cancellation\n";
  const records = extractDocumentCandidates("docs/plan.md", source);
  assert.deepEqual(records.map((record) => [record.line, record.signal]), [
    [1, "heading"], [2, "heading"], [3, "unchecked"], [4, "gap-marker"],
  ]);
  assert.equal(records[2].heading, "Export");
});
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `node --test --test-name-pattern='extractDocumentCandidates' tools/completion-audit.test.mjs`  
Expected: FAIL because the function is missing.

- [ ] **Step 3: Implement deterministic Markdown extraction**

```js
export function extractDocumentCandidates(path, source) {
  let heading = "";
  const records = [];
  source.split(/\r?\n/).forEach((text, index) => {
    const line = index + 1;
    const headingMatch = text.match(/^#{1,6}\s+(.+)$/);
    if (headingMatch) heading = headingMatch[1].trim();
    const signal = headingMatch ? "heading"
      : /^\s*[-*]\s+\[\s\]/.test(text) ? "unchecked"
      : /\b(TODO|FIXME|TBD|stub|placeholder|not implemented|unimplemented)\b|未完成|待办|缺失/i.test(text)
        ? "gap-marker" : null;
    if (signal) {
      records.push({
        id: stableId("doc", `${path}:${line}:${text}`), path, line, heading,
        text: text.trim(), signal,
      });
    }
  });
  return records;
}
```

- [ ] **Step 4: Route `docs` and create final requirement shells**

The `docs` command reads every tracked `.md` file, emits all candidates, and
creates `requirements.json` records with `status: "unverified"`, the candidate
ID, source path/line, and all section-5 fields initialized to `null` or empty
arrays. Existing reviewed records are merged by ID so reruns do not discard
evidence.

```js
function mergeLedger(candidates, existing, createRecord) {
  const prior = new Map((existing?.records ?? []).map((record) => [record.candidateId, record]));
  return candidates.map((candidate) => prior.get(candidate.id) ?? createRecord(candidate));
}

if (command === "docs") {
  const candidates = readTrackedFiles(root)
    .filter((path) => path.endsWith(".md"))
    .flatMap((path) => extractDocumentCandidates(path, readFileSync(resolve(root, path), "utf8"));
  return { schema: 1, candidates };
}
if (command === "requirements") {
  const candidates = JSON.parse(readFileSync(resolve(args.candidates), "utf8")).candidates;
  const existing = existsSync(resolve(args.out))
    ? JSON.parse(readFileSync(resolve(args.out), "utf8")) : null;
  const records = mergeLedger(candidates, existing, (candidate) => ({
    id: stableId("requirement", candidate.id), candidateId: candidate.id,
    source: { path: candidate.path, line: candidate.line },
    targetBehavior: null, priority: null, status: "unverified",
    uiEntry: [], react: [], storeApi: [], tauri: [], rust: [], sideEffects: [],
    returnPath: [], automatedTests: [], runtimeEvidence: [], provenance: [],
    acceptanceCriteria: [], gapGroup: null, finalDisposition: null, commit: null,
  }));
  return { schema: 1, records };
}
```

- [ ] **Step 5: Generate candidates and requirements**

Run:

```bash
node tools/completion-audit.mjs docs --root . --out docs/audit/2026-07-14/document-candidates.json
node tools/completion-audit.mjs requirements --root . --candidates docs/audit/2026-07-14/document-candidates.json --out docs/audit/2026-07-14/requirements.json
```

Expected: every Markdown heading, unchecked box, and gap marker has a stable candidate ID; no record is silently dropped.

- [ ] **Step 6: Commit document extraction**

```bash
git add tools/completion-audit.mjs tools/completion-audit.test.mjs docs/audit/2026-07-14/document-candidates.json docs/audit/2026-07-14/requirements.json
git commit -m "feat(audit): extract every planning requirement candidate"
```

### Task 4: TypeScript AST interactive-control inventory

**Files:**
- Modify: `tools/completion-audit.mjs`
- Modify: `tools/completion-audit.test.mjs`
- Create: `docs/audit/2026-07-14/control-candidates.json`
- Create: `docs/audit/2026-07-14/controls.json`

**Interfaces:**
- Produces: `extractControls(path: string, source: string, ts: typeof import("typescript")): ControlCandidate[]`
- Detects: native controls, ARIA button/menu roles, and JSX nodes with click, context-menu, pointer, double-click, submit, change, or keyboard handlers

- [ ] **Step 1: Add failing TSX extraction test**

```js
import { createRequire } from "node:module";
import { extractControls } from "./completion-audit.mjs";
const require = createRequire(new URL("../web/package.json", import.meta.url));
const ts = require("typescript");

test("extractControls records labels, handlers, and panel triggers", () => {
  const source = `export function View(){return <button aria-label="Export" onClick={() => setExportOpen(true)}>Go</button>}`;
  const [control] = extractControls("web/src/View.tsx", source, ts);
  assert.equal(control.element, "button");
  assert.equal(control.label, "Export");
  assert.match(control.handler, /setExportOpen/);
  assert.equal(control.panelTrigger, true);
});
```

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `node --test --test-name-pattern='extractControls' tools/completion-audit.test.mjs`  
Expected: FAIL because `extractControls` is missing.

- [ ] **Step 3: Implement compiler-API traversal**

Use `ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)`, visit every `JsxElement` and `JsxSelfClosingElement`, inspect attributes for `onClick`, `onContextMenu`, `onPointerDown`, `onDoubleClick`, `onSubmit`, `onChange`, `onKeyDown`, `role`, `aria-label`, `title`, and `disabled`, and record the source line with `sourceFile.getLineAndCharacterOfPosition(node.getStart()).line + 1`. A control is included when its tag is `button`, `input`, `select`, `textarea`, `HoverButton`, or when it has an interaction handler or an ARIA control role.

```js
export function extractControls(path, source, ts) {
  const file = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const controls = [];
  function visit(node) {
    if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
      const opening = ts.isJsxElement(node) ? node.openingElement : node;
      const element = opening.tagName.getText(file);
      const attributes = Object.fromEntries(opening.attributes.properties
        .filter(ts.isJsxAttribute)
        .map((attribute) => [attribute.name.getText(file), attribute.initializer?.getText(file) ?? "true"]));
      const handlers = Object.entries(attributes).filter(([name]) => /^on[A-Z]/.test(name));
      const role = attributes.role?.replace(/[{}"']/g, "") ?? "";
      const interactive = ["button", "input", "select", "textarea", "HoverButton"].includes(element)
        || handlers.length > 0 || ["button", "menuitem", "tab", "switch", "slider"].includes(role);
      if (interactive) {
        const line = file.getLineAndCharacterOfPosition(opening.getStart(file)).line + 1;
        const handler = handlers.map(([, value]) => value).join(" ");
        const rawLabel = attributes["aria-label"] ?? attributes.title ?? "";
        controls.push({
          id: stableId("control", `${path}:${line}:${element}`), path, line, element,
          label: rawLabel.replace(/[{}"']/g, ""), handler,
          disabled: attributes.disabled ?? null,
          panelTrigger: /set[A-Za-z0-9]*(Open|Visible|Panel|Dialog)|toggle[A-Za-z0-9]*(Panel|View)/.test(handler),
        });
      }
    }
    ts.forEachChild(node, visit);
  }
  visit(file);
  return controls;
}
```

- [ ] **Step 4: Route `controls` and preserve reviewed records by stable ID**

Load TypeScript through `createRequire(new URL("../web/package.json", import.meta.url))`, scan every tracked `web/src/**/*.tsx`, emit candidates, and merge candidate shells into `controls.json` without overwriting reviewed evidence.

```js
if (command === "controls") {
  const require = createRequire(new URL("../web/package.json", import.meta.url));
  const ts = require("typescript");
  const candidates = readTrackedFiles(root)
    .filter((path) => path.startsWith("web/src/") && path.endsWith(".tsx"))
    .flatMap((path) => extractControls(path, readFileSync(resolve(root, path), "utf8"), ts));
  return { schema: 1, candidates };
}
if (command === "control-ledger") {
  const candidates = JSON.parse(readFileSync(resolve(args.candidates), "utf8")).candidates;
  const existing = existsSync(resolve(args.out))
    ? JSON.parse(readFileSync(resolve(args.out), "utf8")) : null;
  const records = mergeLedger(candidates, existing, (candidate) => ({
    id: stableId("control-record", candidate.id), candidateId: candidate.id,
    visibility: null, enabledWhen: null, inputs: [], handler: candidate.handler,
    stateTransition: null, backendTrace: [], outcomes: {}, accessibility: {},
    returnPath: [], automatedTests: [], runtimeEvidence: [], status: "unverified",
    acceptanceCriteria: [], gapGroup: null, finalDisposition: null, commit: null,
  }));
  return { schema: 1, records };
}
```

- [ ] **Step 5: Generate the inventories and inspect counts**

Run:

```bash
node --test tools/completion-audit.test.mjs
node tools/completion-audit.mjs controls --root . --out docs/audit/2026-07-14/control-candidates.json
node tools/completion-audit.mjs control-ledger --root . --candidates docs/audit/2026-07-14/control-candidates.json --out docs/audit/2026-07-14/controls.json
jq '.candidates | length' docs/audit/2026-07-14/control-candidates.json
```

Expected: tests PASS and every TSX interactive candidate has a stable record.

- [ ] **Step 6: Commit control extraction**

```bash
git add tools/completion-audit.mjs tools/completion-audit.test.mjs docs/audit/2026-07-14/control-candidates.json docs/audit/2026-07-14/controls.json
git commit -m "feat(audit): inventory every React interaction control"
```

### Task 5: Immutable upstream and downstream source capture

**Files:**
- Modify: `tools/completion-audit.mjs`
- Modify: `tools/completion-audit.test.mjs`
- Create: `docs/audit/2026-07-14/sources.json`
- Create: `docs/audit/2026-07-14/upstream-downstream.md`

**Interfaces:**
- Produces: `captureGitSource({name, path, base, head}): SourceRecord`
- Produces: exact repo URL, base/head SHA, tree SHA, merge base, commit count, changed paths, and status

- [ ] **Step 1: Add a failing source-capture test using a temporary Git repository**

The test creates two commits with `git init`, `git add`, and `git commit`, then asserts that `captureGitSource` returns the exact full SHAs, one changed path, and `commitCount: 1`.

- [ ] **Step 2: Run the focused test and confirm failure**

Run: `node --test --test-name-pattern='captureGitSource' tools/completion-audit.test.mjs`  
Expected: FAIL because the function is missing.

- [ ] **Step 3: Implement fail-closed Git capture**

Use only `git -C <path>` read commands: `remote get-url`, `rev-parse <ref>`, `rev-parse <ref>^{tree}`, `merge-base`, `rev-list --count <base>..<head>`, and `diff --name-status -z <base>..<head>`. Reject missing refs, dirty source repositories where a clean snapshot is required, abbreviated SHAs, and head/base equality when the caller expects updates.

```js
function git(path, args) {
  return execFileSync("git", ["-C", path, ...args], { encoding: "utf8" }).trim();
}

export function captureGitSource({ name, path, base, head }) {
  const baseSha = git(path, ["rev-parse", base]);
  const headSha = git(path, ["rev-parse", head]);
  const fullSha = /^[0-9a-f]{40}$/;
  if (!fullSha.test(baseSha) || !fullSha.test(headSha)) {
    throw new Error(`${name}: refs must resolve to full SHAs`);
  }
  const raw = execFileSync("git", ["-C", path, "diff", "--name-status", "-z", `${baseSha}..${headSha}`]);
  const parts = raw.toString("utf8").split("\0").filter(Boolean);
  const changedPaths = [];
  for (let index = 0; index < parts.length;) {
    const status = parts[index++];
    const first = normalizePath(parts[index++]);
    const second = /^[RC]/.test(status) ? normalizePath(parts[index++]) : null;
    changedPaths.push({ status, path: first, destination: second, disposition: "unverified" });
  }
  return {
    name, repository: git(path, ["remote", "get-url", "origin"]),
    base: baseSha, head: headSha,
    baseTree: git(path, ["rev-parse", `${baseSha}^{tree}`]),
    headTree: git(path, ["rev-parse", `${headSha}^{tree}`]),
    mergeBase: git(path, ["merge-base", baseSha, headSha]),
    commitCount: Number(git(path, ["rev-list", "--count", `${baseSha}..${headSha}`])),
    changedPaths,
  };
}
```

- [ ] **Step 4: Refresh remotes without touching source worktrees**

Run:

```bash
git -C ../palmier-pro-upstream fetch --prune origin
git fetch --prune origin H-Chris233 cuic19053-hue
```

Expected: remote-tracking refs update; neither source worktree gains file changes. If Git transport is unavailable, retrieve refs/trees through authenticated GitHub Git Data API, store the transport limitation, and require exact SHA/tree readback before continuing.

- [ ] **Step 5: Generate `sources.json` from explicit immutable refs**

Capture:

- target cloud main and local starting tree;
- previous audited Palmier commit and current `palmier-io/palmier-pro` main;
- current main plus relevant unmerged refs from `H-Chris233/OpenTake`;
- current main plus relevant unmerged refs from `cuic19053-hue/OpenTake`;
- the canonical dirty checkout HEAD plus a SHA-256 patch manifest, without modifying it.

- [ ] **Step 6: Write the readable comparison report**

For each changed path, record behavior, OpenTake equivalent, disposition, and linked requirement/control IDs. No path may remain with disposition `unverified` at task completion.

- [ ] **Step 7: Commit source evidence**

```bash
git add tools/completion-audit.mjs tools/completion-audit.test.mjs docs/audit/2026-07-14/sources.json docs/audit/2026-07-14/upstream-downstream.md
git commit -m "docs(audit): pin upstream and downstream evidence"
```

### Task 6: Reconcile all planning requirements

**Files:**
- Modify: `docs/audit/2026-07-14/requirements.json`
- Create: `docs/audit/2026-07-14/document-reconciliation.md`
- Modify as evidence requires: `docs/architecture/HANDOFF-2026-07.md`
- Modify as evidence requires: `docs/architecture/ROADMAP.md`
- Modify as evidence requires: `docs/architecture/BUGS.md`
- Modify as evidence requires: `docs/architecture/PORT-1TO1-GAP.md`
- Modify as evidence requires: `docs/需求与问题汇总.md`

**Interfaces:**
- Consumes: every record in `document-candidates.json`
- Produces: one final requirement or non-requirement disposition for every candidate ID

- [ ] **Step 1: Partition candidates by source authority and subsystem**

Run `jq` queries that group candidates by document, heading, and signal. Assign reviewers non-overlapping groups: architecture/roadmap, modules/specs, Superpowers plans, port/upstream analysis, and historical issue/handoff documents.

- [ ] **Step 2: Trace each candidate to current implementation and tests**

For each candidate, record exact files/symbols and test names. Set status to one of `complete`, `incomplete`, `contradicted`, `obsolete`, or `duplicate`; `unverified` is forbidden at task completion. A `complete` record requires at least one implementation path and verification path. An `incomplete` record requires an exact acceptance criterion and subsystem plan assignment.

- [ ] **Step 3: Independently review every non-complete disposition**

The review agent checks source authority, newer decisions, and runtime evidence. Any disagreement remains `unverified` until resolved by stronger evidence.

- [ ] **Step 4: Update authoritative documents only after reconciliation**

Remove stale missing-feature claims, add still-valid gaps, and link every active item to its ledger ID. Preserve archived reports as history and append corrections rather than rewriting their original result.

- [ ] **Step 5: Run the document coverage verifier**

Run: `node tools/completion-audit.mjs verify --root . --audit docs/audit/2026-07-14 --scope documents --out docs/audit/2026-07-14/document-verification.json`  
Expected: exit 0, zero missing candidate IDs, zero `unverified` records, zero `complete` records without implementation and verification evidence.

- [ ] **Step 6: Commit reconciled plans**

```bash
git add docs/audit/2026-07-14 docs/architecture docs/需求与问题汇总.md
git commit -m "docs(audit): reconcile every OpenTake planning requirement"
```

### Task 7: Trace and exercise every interface control

**Files:**
- Modify: `docs/audit/2026-07-14/controls.json`
- Create: `docs/audit/2026-07-14/interface-traces.md`
- Create: `docs/audit/2026-07-14/runtime-evidence.json`

**Interfaces:**
- Consumes: every candidate in `control-candidates.json`
- Produces: visibility/enabled condition, handler, store/API/Tauri/Rust path, success/failure states, accessibility contract, runtime evidence, and final status for every control

- [ ] **Step 1: Build static traces screen by screen**

Audit Home, editor shell/title bar, Media/Library, Preview, Timeline, Inspector, Agent, Settings, Export/Save As, dialogs, dropdowns, popovers, context menus, and empty/error states. Resolve each handler through stores/actions/API/Tauri commands to Rust symbols and return events/snapshots.

- [ ] **Step 2: Add component tests for candidate controls with missing deterministic coverage**

For each control lacking a component or interaction test, first write a failing Vitest test that renders the owning component, invokes the exact pointer/keyboard event, and asserts the expected store/API call plus visible state. Then implement only real defects found by that test. Product fixes discovered here receive their own commit and requirement/control IDs.

- [ ] **Step 3: Exercise browser-capable paths**

Run the Vite fallback in a signed local browser context. Exercise every deterministic control state, including disabled, cancel, retry, empty, and error paths. Record evidence handles in `runtime-evidence.json`.

- [ ] **Step 4: Exercise native-only paths in the installed application**

Build and install the exact candidate bundle. Test native menus, open/save dialogs, filesystem capabilities, media import/relink, playback/seek/pause, export/cancel, project reopen, Agent/MCP, and process cleanup. Use small fixtures first and real media for final render/playback/export evidence.

- [ ] **Step 5: Run accessibility and keyboard audits**

Verify accessible names, roles, focus order, shortcut parity, menu/panel return paths, and disabled explanations. Record capture limitations explicitly; lack of screenshot permission cannot stand in for interaction proof.

- [ ] **Step 6: Independently review all controls marked complete or intentionally disabled**

The reviewer samples source paths and reruns high-risk interactions. Any silent no-op, placeholder panel, missing handler, or unproven state returns to `incomplete`.

- [ ] **Step 7: Run control coverage verification**

Run: `node tools/completion-audit.mjs verify --root . --audit docs/audit/2026-07-14 --scope controls --out docs/audit/2026-07-14/control-verification.json`  
Expected: exit 0, every candidate ID covered, no unverified controls, no complete control without a handler trace and runtime or deterministic interaction evidence.

- [ ] **Step 8: Commit interface evidence and any independently reviewed fixes**

```bash
git add web src-tauri crates docs/audit/2026-07-14
git commit -m "test(ui): verify every OpenTake interface control"
```

### Task 8: Fail-closed coverage verifier and gap grouping

**Files:**
- Modify: `tools/completion-audit.mjs`
- Modify: `tools/completion-audit.test.mjs`
- Create: `docs/audit/2026-07-14/completion-ledger.json`
- Create: `docs/audit/2026-07-14/completion-report.md`
- Create: `docs/audit/2026-07-14/implementation-plan-index.md`

**Interfaces:**
- Produces: `verifyAudit(root: string, auditDir: string, scope: string): VerificationResult`
- Produces: gap groups `data-safety`, `command-contracts`, `media-render-playback-export`, `home-shell`, `media-library`, `preview-timeline`, `inspector-text-keyframes`, `agent-settings-generation`, `accessibility-polish`, and `documentation`

- [ ] **Step 1: Add failing verifier tests**

Fixtures must prove the verifier rejects:

- a tracked file absent from `repository-files.json`;
- a document candidate absent from requirements;
- a control candidate absent from controls;
- any `unverified` record;
- a complete requirement without implementation and verification evidence;
- a complete control without a handler/backend trace and behavior evidence;
- a changed upstream/downstream path without disposition; and
- an incomplete record without acceptance criteria and gap group.

- [ ] **Step 2: Run focused verifier tests and confirm failure**

Run: `node --test --test-name-pattern='verifyAudit' tools/completion-audit.test.mjs`  
Expected: FAIL because `verifyAudit` is missing.

- [ ] **Step 3: Implement verifier and nonzero exit behavior**

`verifyAudit` returns `{ok, errors, counts}`. The CLI writes the result and sets `process.exitCode = 1` when `ok` is false. Error messages include stable IDs and exact source paths.

```js
function missingIds(candidates, records) {
  const covered = new Set(records.map((record) => record.candidateId));
  return candidates.filter((candidate) => !covered.has(candidate.id)).map((candidate) => candidate.id);
}

export function verifyAudit(root, auditDir, scope = "all") {
  const load = (name) => JSON.parse(readFileSync(resolve(auditDir, name), "utf8"));
  const errors = [];
  const files = load("repository-files.json").files;
  const documents = load("document-candidates.json").candidates;
  const requirements = load("requirements.json").records;
  const controls = load("control-candidates.json").candidates;
  const controlRecords = load("controls.json").records;
  const sources = load("sources.json").sources;
  const tracked = readTrackedFiles(root);
  const inventoried = new Set(files.map((record) => record.path));
  if (scope === "all" || scope === "files") {
    for (const path of tracked) if (!inventoried.has(path)) errors.push(`missing file: ${path}`);
  }
  if (scope === "all" || scope === "documents") {
    for (const id of missingIds(documents, requirements)) errors.push(`missing requirement: ${id}`);
    for (const record of requirements) {
      if (record.status === "unverified") errors.push(`unverified requirement: ${record.id}`);
      if (record.status === "complete" && (!record.rust.length && !record.react.length || !record.automatedTests.length && !record.runtimeEvidence.length)) {
        errors.push(`unsupported complete requirement: ${record.id}`);
      }
      if (record.status === "incomplete" && (!record.acceptanceCriteria.length || !record.gapGroup)) {
        errors.push(`unplanned gap: ${record.id}`);
      }
    }
  }
  if (scope === "all" || scope === "controls") {
    for (const id of missingIds(controls, controlRecords)) errors.push(`missing control: ${id}`);
    for (const record of controlRecords) {
      if (record.status === "unverified") errors.push(`unverified control: ${record.id}`);
      if (record.status === "complete" && (!record.handler || !record.backendTrace.length && !record.stateTransition || !record.automatedTests.length && !record.runtimeEvidence.length)) {
        errors.push(`unsupported complete control: ${record.id}`);
      }
    }
  }
  if (scope === "all" || scope === "sources") {
    for (const source of sources) {
      for (const change of source.changedPaths ?? []) {
        if (!change.disposition || change.disposition === "unverified") {
          errors.push(`unverified source change: ${source.name}:${change.path}`);
        }
      }
    }
  }
  return { ok: errors.length === 0, errors, counts: {
    tracked: tracked.length, files: files.length, documents: documents.length,
    requirements: requirements.length, controls: controls.length,
    controlRecords: controlRecords.length, sources: sources.length,
  } };
}
```

- [ ] **Step 4: Regenerate final inventories after all audit files are tracked**

Run:

```bash
node tools/completion-audit.mjs files --root . --out docs/audit/2026-07-14/repository-files.json
node tools/completion-audit.mjs docs --root . --out docs/audit/2026-07-14/document-candidates.json
node tools/completion-audit.mjs controls --root . --out docs/audit/2026-07-14/control-candidates.json
```

- [ ] **Step 5: Build the completion ledger and readable report**

Join file, document, control, source, runtime, and test evidence by stable ID. The report lists verified-complete areas, contradicted/obsolete documents, exact gaps, external limitations, and the proof required to close every gap.

- [ ] **Step 6: Create the implementation-plan index**

Assign every incomplete record to exactly one gap group. For each non-empty group, create a separate Superpowers design/implementation plan using exact discovered files, symbols, failing tests, expected behavior, and validation commands. Empty groups are recorded with the verifier evidence that proves no work remains.

- [ ] **Step 7: Run all audit checks**

Run:

```bash
node --check tools/completion-audit.mjs
node --test tools/completion-audit.test.mjs
node tools/completion-audit.mjs verify --root . --audit docs/audit/2026-07-14 --scope all --out docs/audit/2026-07-14/final-verification.json
git diff --check
```

Expected: all tests PASS; verifier exits 0; zero uncovered files, candidates, controls, or source changes; no unverified records.

- [ ] **Step 8: Independent audit review**

A fresh reviewer receives the exact commit/tree and checks spec coverage, candidate-generation completeness, verifier fail-closed behavior, source SHA provenance, and a stratified sample from every gap group. Critical/important findings must be zero before implementation plans begin.

- [ ] **Step 9: Commit the verified audit**

```bash
git add tools docs/audit docs/superpowers/plans docs/superpowers/specs docs/architecture docs/需求与问题汇总.md
git commit -m "docs(audit): publish verified OpenTake completion ledger"
```

### Task 9: Execute subsystem plans and close the program

**Files:**
- Consume: `docs/audit/2026-07-14/implementation-plan-index.md`
- Modify: product and test files named by each generated subsystem plan
- Modify: `docs/audit/2026-07-14/completion-ledger.json`
- Modify: `docs/audit/2026-07-14/completion-report.md`

**Interfaces:**
- Consumes: exact incomplete records and acceptance criteria
- Produces: verified product behavior and final status for every record

- [ ] **Step 1: Execute plans in dependency order**

Use subagent-driven development with fresh implementer and two-stage review gates for each independently testable task. Order is data safety, contracts, engines, shell, Media/Library, Preview/Timeline, Inspector, Agent/Settings/Generation, accessibility, then documentation.

- [ ] **Step 2: Run focused tests before and after every change**

Each task must demonstrate its regression test failing before implementation and passing afterward. Run the owning module suite and required cross-platform checks before commit.

- [ ] **Step 3: Re-audit affected controls and requirements after every subsystem**

Regenerate candidates, update evidence, and run the verifier. A new control or document requirement cannot enter the tree without a ledger record.

- [ ] **Step 4: Run full product verification**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
pnpm -C web build
pnpm -C web test
node --test tools/completion-audit.test.mjs
node tools/completion-audit.mjs verify --root . --audit docs/audit/2026-07-14 --scope all --out docs/audit/2026-07-14/final-verification.json
```

Expected: every command succeeds, followed by installed-app real-media and Agent/MCP smoke tests and mandatory native Windows CI.

- [ ] **Step 5: Final independent exact-tree review**

The reviewer checks every completion criterion in the approved design against current files, command output, runtime evidence, cloud state, and ledger coverage. Critical/important/minor actionable findings must be zero.

- [ ] **Step 6: Publish without rewriting history**

Create a reviewed PR from the exact candidate tree, wait for every required job, merge through a non-force compare-and-swap path, rerun final main CI, and read back commit/tree/parents/PR states. Preserve all local audit state and canonical dirty work.
