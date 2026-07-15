import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  CONTROL_REVIEW_METADATA,
  PALMIER_REVIEWED_PATH_LEDGER,
  buildFileInventory,
  buildSourceEvidence,
  capturedOpenPullRequests,
  captureDirtyCheckout,
  captureGitSource,
  classifyFile,
  compareAuditText,
  extractControls,
  extractDocumentCandidates,
  expectedControlAcceptanceCriteria,
  normalizePath,
  renderSourceReport,
  reviewPalmierChangedPaths,
  stableId,
  validateSourceEvidenceCatalogs,
  validateSourceEvidenceShape,
  verifyAudit,
  verifyOpenPullRequests,
} from "./completion-audit.mjs";

const require = createRequire(new URL("../web/package.json", import.meta.url));
const ts = require("typescript");

function git(root, args) {
  return execFileSync("git", ["-C", root, ...args], { encoding: "utf8" }).trim();
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function createControlVerificationFixture(t, {
  auditRelative = "docs/audit/2026-07-14",
  sourcePath = "web/src/Panel.tsx",
  sourceText = "import React from \"react\";\nexport function Panel({ openAlpha }) {\n  return <button aria-label=\"Alpha\" onClick={openAlpha}>Alpha</button>;\n}\nexport function harmlessHelper() { return { props: {} }; }\n",
} = {}) {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-verify-controls-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Control Audit"]);
  git(root, ["config", "user.email", "control-audit@example.invalid"]);
  const audit = join(root, auditRelative);
  const testPath = "web/src/Panel.test.tsx";
  mkdirSync(join(root, "web", "src"), { recursive: true });
  mkdirSync(audit, { recursive: true });
  writeFileSync(join(root, ".gitignore"), "web/node_modules\n");
  symlinkSync(
    fileURLToPath(new URL("../web/node_modules", import.meta.url)),
    join(root, "web", "node_modules"),
    "dir",
  );
  writeFileSync(
    join(root, sourcePath),
    sourceText,
  );
  const [candidate] = extractControls(
    sourcePath,
    readFileSync(join(root, sourcePath), "utf8"),
    ts,
  );
  const exactTestName = `${candidate.id} Alpha invokes its exact handler`;
  writeFileSync(
    join(root, testPath),
    `import assert from "node:assert/strict";\nimport test from "node:test";\ntest(${JSON.stringify(exactTestName)}, () => {\n  let calls = 0;\n  const openAlpha = () => { calls += 1; };\n  const button = new EventTarget();\n  button.addEventListener("click", openAlpha);\n  button.dispatchEvent(new Event("click"));\n  assert.equal(calls, 1);\n});\n`,
  );
  const outcomes = Object.fromEntries([
    "success", "pending", "empty", "disabled", "cancel", "retry", "failure",
  ].map((name) => [name, `${name} state`]));
  const candidates = [candidate];
  const owningCodeEvidence = `code:${sourcePath}#Panel`;
  const record = {
    id: stableId("control-record", candidate.id),
    candidateId: candidate.id,
    candidateLabel: candidate.label,
    semanticName: "Alpha",
    source: { path: candidate.path, line: candidate.line, column: candidate.column },
    element: candidate.element,
    visibility: "Panel is mounted.",
    enabledWhen: "Always enabled.",
    inputs: ["click"],
    handler: candidate.handler,
    stateTransition: "openAlpha runs.",
    backendTrace: [owningCodeEvidence],
    outcomes,
    accessibility: { focus: "native button", label: "Alpha", shortcut: "none" },
    returnPath: ["remain in Panel"],
    automatedTests: ["supporting suite only"],
    runtimeEvidence: [],
    status: "incomplete",
    finalDisposition: "Direct interaction evidence is missing.",
    duplicateOf: [],
    acceptanceCriteria: [],
    gapGroup: "home-shell",
    commit: null,
  };
  record.acceptanceCriteria = expectedControlAcceptanceCriteria(record);
  const candidateLedgerText = `${JSON.stringify({ schema: 1, candidates }, null, 2)}\n`;
  writeFileSync(join(audit, "control-candidates.json"), candidateLedgerText);
  const verifierFiles = [
    "tools/completion-audit.mjs",
    "tools/completion-audit-controls.mjs",
    "tools/completion-audit.test.mjs",
  ];
  git(root, [
    "add",
    ".gitignore",
    sourcePath,
    testPath,
    `${auditRelative}/control-candidates.json`,
  ]);
  git(root, ["commit", "--quiet", "-m", "audited controls source"]);
  const auditedCommit = git(root, ["rev-parse", "HEAD"]);
  const auditedTree = git(root, ["rev-parse", "HEAD^{tree}"]);
  mkdirSync(join(root, "tools"), { recursive: true });
  for (const path of verifierFiles) {
    writeFileSync(join(root, path), `fixture verifier file: ${path}\n`);
  }
  git(root, ["add", ...verifierFiles]);
  git(root, ["commit", "--quiet", "-m", "fixture verifier code"]);
  const verifierCommit = git(root, ["rev-parse", "HEAD"]);
  const verifierTree = git(root, ["rev-parse", "HEAD^{tree}"]);
  const sourceHash = createHash("sha256")
    .update(readFileSync(join(root, sourcePath)))
    .digest("hex");
  const provenance = {
    auditedProductRevision: { commit: auditedCommit, tree: auditedTree },
    verifierRevision: { commit: verifierCommit, tree: verifierTree },
    candidateLedgerSha256: createHash("sha256").update(candidateLedgerText).digest("hex"),
    candidateSourceAggregateSha256: createHash("sha256")
      .update(`${sourcePath}\0${sourceHash}\n`)
      .digest("hex"),
    verifierFilesSha256: Object.fromEntries(verifierFiles.map((path) => [
      path,
      createHash("sha256").update(readFileSync(join(root, path))).digest("hex"),
    ])),
  };
  const runtime = {
    schema: 1,
    metadata: {
      schemaName: "completion-control-runtime-evidence",
      schemaVersion: 1,
      receiptIdDerivation: "stableId('control-runtime-receipt', key)",
      evidencePolicy: "Only direct receipts qualify.",
    },
    provenance: structuredClone(provenance),
    candidateLedger: `${auditRelative}/control-candidates.json`,
    controlsLedger: `${auditRelative}/controls.json`,
    summary: {
      receipts: 0,
      direct: 0,
      supporting: 0,
      passed: 0,
      failed: 0,
      partial: 0,
      notRun: 0,
      blocked: 0,
    },
    receipts: [],
  };
  const controls = {
    schema: 2,
    metadata: CONTROL_REVIEW_METADATA,
    provenance: structuredClone(provenance),
    scope: {
      included: "fixture",
      sourceLedger: `${auditRelative}/control-candidates.json`,
      candidateCount: 1,
      candidateIdsUnique: true,
      unverifiedCount: 0,
    },
    counts: { complete: 0, incomplete: 1, obsolete: 0, duplicate: 0, contradicted: 0 },
    gapCounts: { "home-shell": 1 },
    keyFindings: ["fixture"],
    codeEvidence: { [owningCodeEvidence]: sourceHash },
    records: [record],
  };
  const write = () => {
    runtime.summary = {
      receipts: runtime.receipts.length,
      direct: runtime.receipts.filter((receipt) => receipt.evidenceLevel === "direct").length,
      supporting: runtime.receipts.filter((receipt) => receipt.evidenceLevel === "supporting").length,
      passed: runtime.receipts.filter((receipt) => receipt.status === "passed").length,
      failed: runtime.receipts.filter((receipt) => receipt.status === "failed").length,
      partial: runtime.receipts.filter((receipt) => receipt.status === "partial").length,
      notRun: runtime.receipts.filter((receipt) => receipt.status === "not-run").length,
      blocked: runtime.receipts.filter((receipt) => receipt.status === "blocked").length,
    };
    writeFileSync(
      join(audit, "control-candidates.json"),
      `${JSON.stringify({ schema: 1, candidates }, null, 2)}\n`,
    );
    writeFileSync(join(audit, "controls.json"), `${JSON.stringify(controls, null, 2)}\n`);
    writeFileSync(join(audit, "runtime-evidence.json"), `${JSON.stringify(runtime, null, 2)}\n`);
    git(root, ["add", "-A"]);
  };
  write();
  return {
    root,
    audit,
    sourcePath,
    testPath,
    exactTestName,
    candidate,
    record,
    controls,
    runtime,
    candidates,
    write,
  };
}

function createGitSourceFixture(t) {
  const root = mkdtempSync(join(tmpdir(), "opentake-git-source-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Completion Audit"]);
  git(root, ["config", "user.email", "audit@example.invalid"]);
  git(root, ["remote", "add", "origin", "https://example.invalid/origin/OpenTake.git"]);
  git(root, ["remote", "add", "fork", "ssh://git@example.invalid/fork/OpenTake.git"]);
  writeFileSync(join(root, "source.txt"), "base\n");
  git(root, ["add", "source.txt"]);
  git(root, ["commit", "--quiet", "-m", "base"]);
  const base = git(root, ["rev-parse", "HEAD"]);
  writeFileSync(join(root, "source.txt"), "head\n");
  git(root, ["add", "source.txt"]);
  git(root, ["commit", "--quiet", "-m", "head"]);
  const head = git(root, ["rev-parse", "HEAD"]);
  return { root, base, head };
}

function createSourceEvidenceFixture() {
  const sha = "1".repeat(40);
  const tree = "2".repeat(40);
  const digest = "3".repeat(64);
  return {
    schema: 1,
    auditDate: "2026-07-14",
    generation: {
      deterministicOrdering: "UTF-8 byte ordering after slash normalization; filenames are not Unicode-normalized",
      hashAlgorithm: "SHA-256",
      sourceRefs: "full immutable 40-character commit SHAs",
    },
    fetchEvidence: {
      serializedOrder: ["git fetch --prune origin"],
      target: { pre: digest, post: digest },
      canonical: { pre: digest, post: digest },
      palmier: { pre: digest, post: digest },
      workingFilesUnchanged: true,
    },
    openPullRequests: capturedOpenPullRequests(),
    sources: [{
      name: "reviewed source",
      repository: "https://example.invalid/source.git",
      base: sha,
      head: sha,
      baseTree: tree,
      headTree: tree,
      mergeBase: sha,
      aheadCount: 0,
      behindCount: 0,
      status: "identical",
      changedPaths: [{
        status: "M",
        path: "reviewed.swift",
        destination: null,
        disposition: "portable",
        behavior: "Reviewed behavior.",
        openTakeEquivalent: "reviewed equivalent",
        rationale: "Reviewed rationale.",
        linkedRequirementIds: ["requirement-known"],
        linkedControlIds: ["control-known"],
        requirementGapIds: ["requirement-needed:reviewed-gap"],
      }],
    }],
    branchIndex: [],
    branchSummary: {
      totalNonMainHeads: 0,
      integratedTarget: 0,
      integratedForkMain: 0,
      empty: 0,
      emptyForkMainDeltas: 0,
      relevantUnmerged: 0,
    },
    canonicalDirtyCheckout: {
      repository: "https://example.invalid/canonical.git",
      head: sha,
      tree,
      statusSha256: digest,
      captureManifestSha256: digest,
      manifestSha256: digest,
      paths: [],
      relationshipCounts: {},
    },
  };
}

function createBuildSourceOperations({
  palmierPaths = PALMIER_REVIEWED_PATH_LEDGER.map(
    ({ status, path, destination }) => ({ status, path, destination, disposition: "unverified" }),
  ),
} = {}) {
  const tree = "a".repeat(40);
  const source = ({ name, base, head, remote = "origin" }, changedPaths, status = "changes") => ({
    name,
    repository: `https://example.invalid/${remote}.git`,
    remote,
    status,
    base,
    head,
    baseTree: tree,
    headTree: status === "equivalent-tree" ? tree : "b".repeat(40),
    mergeBase: base,
    commitCount: 1,
    aheadCount: 1,
    behindCount: 0,
    changedPaths,
  });
  const integratedPaths = [
    "src-tauri/capabilities/default.json",
    "src-tauri/src/mpv_bootstrap.rs",
    "src-tauri/tauri.conf.json",
    "web/src/components/home/HomeView.tsx",
    "web/src/components/media/MediaSearch.tsx",
    "web/src/components/shell/ViewMenu.tsx",
    "web/src/components/ui/PanelShell.tsx",
    "web/src/lib/mpvEdl.test.ts",
    "web/src/lib/mpvEdl.ts",
    "web/src/styles/global.css",
  ];
  const equivalentPaths = [
    "README.md",
    "web/package.json",
    "web/src/store/editActions.ts",
    "web/src/store/uiStore.ts",
  ];
  const stalePaths = [".github/workflows/ci.yml", "src-tauri/Cargo.toml"];
  const dirtyPaths = [
    ...integratedPaths.map((path) => ({ status: " M", path })),
    ...equivalentPaths.map((path) => ({ status: " M", path })),
    ...stalePaths.map((path) => ({ status: " M", path })),
    ...Array.from({ length: 4 }, (_, index) => ({
      status: "??",
      path: `fixture-untracked-${index}.txt`,
    })),
    ...Array.from({ length: 36 }, (_, index) => ({
      status: " M",
      path: `fixture-conflict-${String(index).padStart(2, "0")}.txt`,
    })),
  ].map((entry) => ({
    ...entry,
    source: null,
    tracked: entry.status !== "??",
    patchBytes: entry.status === "??" ? null : 1,
    patchSha256: entry.status === "??" ? null : "c".repeat(64),
    fileType: "regular",
    bytes: 1,
    contentSha256: "d".repeat(64),
    linkTargetSha256: null,
  }));
  const requirements = new Set([
    "requirement-10e720a4f5ddd734",
    "requirement-9bceb67f73cd51d4",
    ...PALMIER_REVIEWED_PATH_LEDGER.flatMap(({ linkedRequirementIds }) => linkedRequirementIds),
  ]);
  const controls = new Set([
    "control-record-64640989bd95e214",
    ...PALMIER_REVIEWED_PATH_LEDGER.flatMap(({ linkedControlIds }) => linkedControlIds),
  ]);

  return {
    assertPinnedRef() {},
    captureGitSource(input) {
      if (input.name === "target cloud main vs local start") {
        return source(input, [], "equivalent-tree");
      }
      if (input.name === "Palmier Pro refreshed main") {
        return source(input, palmierPaths);
      }
      throw new Error(`unexpected source capture: ${input.name}`);
    },
    branchIndexForRemote(_root, remote) {
      const count = remote === "H-Chris233" ? 10 : 11;
      return Array.from({ length: count }, (_, index) => ({
        ref: `${remote}/fixture-${index}`,
        remote,
        tip: String(index + 1).padStart(40, "0"),
        tree,
        targetMain: "e".repeat(40),
        forkMain: "f".repeat(40),
        targetMergeBase: String(index + 1).padStart(40, "0"),
        forkMainMergeBase: String(index + 1).padStart(40, "0"),
        targetAncestor: true,
        forkMainAncestor: true,
        targetUniqueCommitCount: 0,
        forkMainUniqueCommitCount: 0,
        forkMainChangedPathCount: index === 0 ? 0 : 1,
        forkMainDelta: index === 0 ? "empty" : "changes",
        status: "integrated-target",
        separatelyRelevant: false,
      }));
    },
    integratedForkMainSource(_root, remote) {
      return source({
        name: `${remote} main ancestry`,
        base: "f".repeat(40),
        head: "e".repeat(40),
        remote,
      }, [], "integrated-ancestor");
    },
    captureDirtyCheckout() {
      return {
        name: "canonical dirty OpenTake checkout",
        repository: "https://example.invalid/canonical.git",
        remote: "origin",
        status: "dirty",
        head: "c2f807aafd6e46088365eac2de45fe8803a7e1d0",
        tree: "6fa0ac1bd83719c8215e887e4edcb52c527623dd",
        statusSha256: "1157939da02403643b66eee0618db0ec04f9a42412a538d0a67d547d75e0dd00",
        manifestSha256: "e".repeat(64),
        paths: dirtyPaths,
      };
    },
    readSourceEvidenceCatalogs() {
      return {
        requirements: { records: [...requirements].map((id) => ({ id })) },
        controls: { records: [...controls].map((id) => ({ id })) },
      };
    },
  };
}

test("captureGitSource records exact immutable Git evidence", (t) => {
  const { root, base, head } = createGitSourceFixture(t);
  const before = execFileSync("git", ["-C", root, "status", "--porcelain=v1", "-z"]);

  const source = captureGitSource({
    name: "fixture",
    path: root,
    base,
    head: "HEAD",
    requireClean: true,
    expectChanges: true,
  });

  assert.equal(source.name, "fixture");
  assert.equal(source.repository, "https://example.invalid/origin/OpenTake.git");
  assert.equal(source.remote, "origin");
  assert.equal(source.status, "changes");
  assert.equal(source.base, base);
  assert.equal(source.head, head);
  assert.equal(source.baseTree, git(root, ["rev-parse", `${base}^{tree}`]));
  assert.equal(source.headTree, git(root, ["rev-parse", `${head}^{tree}`]));
  assert.equal(source.mergeBase, base);
  assert.equal(source.commitCount, 1);
  assert.equal(source.aheadCount, 1);
  assert.equal(source.behindCount, 0);
  assert.deepEqual(source.changedPaths, [
    { status: "M", path: "source.txt", destination: null, disposition: "unverified" },
  ]);
  assert.deepEqual(
    execFileSync("git", ["-C", root, "status", "--porcelain=v1", "-z"]),
    before,
  );
});

test("captureGitSource parses rename and copy records from NUL-delimited output", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-git-source-nul-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Completion Audit"]);
  git(root, ["config", "user.email", "audit@example.invalid"]);
  git(root, ["remote", "add", "origin", "https://example.invalid/nul.git"]);
  writeFileSync(join(root, "old\tname.txt"), "rename me\n");
  writeFileSync(join(root, "copy-source.txt"), "copy me\n");
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", "base"]);
  const base = git(root, ["rev-parse", "HEAD"]);
  git(root, ["mv", "old\tname.txt", "renamed\nname.txt"]);
  writeFileSync(join(root, "copy\tduplicate.txt"), readFileSync(join(root, "copy-source.txt")));
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", "rename and copy"]);

  const source = captureGitSource({
    name: "nul-fixture",
    path: root,
    base,
    head: "HEAD",
    requireClean: true,
    expectChanges: true,
  });

  assert.deepEqual(source.changedPaths, [
    {
      status: "C100",
      path: "copy-source.txt",
      destination: "copy\tduplicate.txt",
      disposition: "unverified",
    },
    {
      status: "R100",
      path: "old\tname.txt",
      destination: "renamed\nname.txt",
      disposition: "unverified",
    },
  ]);
});

test("captureGitSource uses the requested remote repository URL", (t) => {
  const { root, base } = createGitSourceFixture(t);
  const source = captureGitSource({
    name: "fork fixture",
    path: root,
    base,
    head: "HEAD",
    remote: "fork",
  });

  assert.equal(source.remote, "fork");
  assert.equal(source.repository, "ssh://git@example.invalid/fork/OpenTake.git");
});

test("captureGitSource rejects a dirty repository when clean evidence is required", (t) => {
  const { root, base } = createGitSourceFixture(t);
  writeFileSync(join(root, "untracked.txt"), "dirty\n");

  assert.throws(
    () => captureGitSource({
      name: "dirty fixture",
      path: root,
      base,
      head: "HEAD",
      requireClean: true,
    }),
    /dirty fixture: source repository is dirty/,
  );
});

test("captureGitSource output ignores transient worktree dirtiness", (t) => {
  const { root, base } = createGitSourceFixture(t);
  const clean = captureGitSource({ name: "stable fixture", path: root, base, head: "HEAD" });
  writeFileSync(join(root, "untracked.txt"), "dirty\n");
  const dirty = captureGitSource({ name: "stable fixture", path: root, base, head: "HEAD" });

  assert.deepEqual(dirty, clean);
  assert.equal(clean.status, "changes");
});

test("captureGitSource rejects missing refs and abbreviated SHA inputs", (t) => {
  const { root, base, head } = createGitSourceFixture(t);

  assert.throws(
    () => captureGitSource({ name: "missing fixture", path: root, base, head: "missing-ref" }),
    /missing fixture: git rev-parse missing-ref failed/,
  );
  assert.throws(
    () => captureGitSource({ name: "short fixture", path: root, base, head: head.slice(0, 12) }),
    /short fixture: abbreviated SHA ref is not immutable/,
  );
});

test("captureGitSource rejects equal refs when changes are expected", (t) => {
  const { root, head } = createGitSourceFixture(t);

  assert.throws(
    () => captureGitSource({
      name: "equal fixture",
      path: root,
      base: head,
      head: "HEAD",
      expectChanges: true,
    }),
    /equal fixture: expected base and head to differ/,
  );
});

test("Palmier reviewed ledger is an exact 132-row classification golden", () => {
  assert.equal(PALMIER_REVIEWED_PATH_LEDGER.length, 132);
  assert.equal(
    createHash("sha256")
      .update(JSON.stringify(PALMIER_REVIEWED_PATH_LEDGER))
      .digest("hex"),
    "eea0c2016f0e64f6025b0de94cfab0a21a4ef67ca0b74dd5f3b11508e82cf9a0",
  );
  assert.equal(
    new Set(PALMIER_REVIEWED_PATH_LEDGER.map(
      ({ status, path, destination }) => `${status}\0${path}\0${destination ?? ""}`,
    )).size,
    132,
  );

  const raw = PALMIER_REVIEWED_PATH_LEDGER.map(({ status, path, destination }) => ({
    status,
    path,
    destination,
    disposition: "unverified",
  }));
  const reviewed = reviewPalmierChangedPaths(raw);
  assert.equal(reviewed.length, 132);
  assert.equal(
    reviewed.find(({ path }) => path === "Metal/ChromaKey.metal").disposition,
    "requires-reconciliation",
  );
  assert.equal(
    reviewed.find(({ path }) => path === "Sources/PalmierPro/Export/ExportService.swift").disposition,
    "integrated",
  );
  assert.equal(
    reviewed.find(({ path }) => path === "Sources/PalmierPro/Search/Indexing/VisualIndexer.swift").disposition,
    "integrated",
  );
  assert.equal(
    reviewed.find(({ path }) => path === "Sources/PalmierPro/Generation/GenerationBackend.swift").disposition,
    "cloud-specific",
  );
  assert.deepEqual(
    reviewed.find(({ status }) => status === "C060"),
    {
      status: "C060",
      path: "Sources/PalmierPro/Generation/Edit/EditSubmitter.swift",
      destination: "Sources/PalmierPro/Generation/Edit/EditSubmitter+Rerun.swift",
      disposition: "portable",
      behavior: "Splits rerun behavior from the submitter while preserving prior generation inputs, including transform inputs.",
      openTakeEquivalent: "crates/opentake-project::GenerationLog; generation rerun UI none",
      rationale: "Generation log exists, but rerun execution/UI is not wired.",
      linkedRequirementIds: [],
      linkedControlIds: [],
      requirementGapIds: ["requirement-needed:generation-rerun"],
    },
  );

  assert.throws(
    () => reviewPalmierChangedPaths(raw.slice(1)),
    /expected 132 paths, found 131/,
  );
  assert.throws(
    () => reviewPalmierChangedPaths([
      { ...raw[0], path: "unexpected.swift" },
      ...raw.slice(1),
    ]),
    /not in reviewed ledger/,
  );
  const renameIndex = raw.findIndex(({ destination }) => destination);
  assert.notEqual(renameIndex, -1);
  assert.throws(
    () => reviewPalmierChangedPaths(raw.map((record, index) => (
      index === renameIndex ? { ...record, destination: `${record.destination}.wrong` } : record
    ))),
    /not in reviewed ledger/,
  );
});

test("immutable PR capture is independent of live verification changes", () => {
  const before = capturedOpenPullRequests();
  const reportBefore = renderSourceReport(createSourceEvidenceFixture());
  assert.deepEqual(before, {
    repository: "appergb/OpenTake",
    state: "open",
    transport: "immutable-capture",
    capturedAt: "2026-07-14T08:24:59Z",
    command: "gh pr list --repo appergb/OpenTake --state open --json number,title,headRefName,baseRefName,url",
    count: 0,
    items: [],
  });
  assert.deepEqual(verifyOpenPullRequests({ readLive: () => [] }), {
    status: "match",
    capturedAt: before.capturedAt,
    command: before.command,
    count: 0,
  });
  assert.throws(
    () => verifyOpenPullRequests({
      readLive: () => [{
        number: 999,
        title: "Cloud state changed",
        headRefName: "later",
        baseRefName: "main",
        url: "https://example.invalid/pr/999",
      }],
    }),
    /live open-PR state differs from immutable capture/,
  );
  assert.deepEqual(capturedOpenPullRequests(), before);
  assert.equal(renderSourceReport(createSourceEvidenceFixture()), reportBefore);
});

test("source evidence catalog validation accepts current links and rejects stale IDs", () => {
  const evidence = createSourceEvidenceFixture();
  const catalogs = {
    requirements: { records: [{ id: "requirement-known" }] },
    controls: { records: [{ id: "control-known" }] },
  };
  assert.equal(validateSourceEvidenceCatalogs(evidence, catalogs), evidence);

  const staleRequirement = structuredClone(evidence);
  staleRequirement.sources[0].changedPaths[0].linkedRequirementIds = ["requirement-stale"];
  assert.throws(
    () => validateSourceEvidenceCatalogs(staleRequirement, catalogs),
    /stale requirement id requirement-stale/,
  );

  const staleControl = structuredClone(evidence);
  staleControl.sources[0].changedPaths[0].linkedControlIds = ["control-stale"];
  assert.throws(
    () => validateSourceEvidenceCatalogs(staleControl, catalogs),
    /stale control id control-stale/,
  );
});

test("source report renders reviewed classifications and rejects malformed evidence", () => {
  const evidence = createSourceEvidenceFixture();
  const report = renderSourceReport(evidence);
  assert.match(report, /Target open-PR capture: \*\*0\*\* at `2026-07-14T08:24:59Z`/);
  assert.match(report, /Reviewed behavior\./);
  assert.match(report, /requirement-needed:reviewed-gap/);
  assert.match(report, /Reviewed rationale\./);
  assert.ok(report.endsWith("\n"));

  const malformed = structuredClone(evidence);
  malformed.sources[0].changedPaths = null;
  assert.throws(
    () => renderSourceReport(malformed),
    /changedPaths must be an array/,
  );
  assert.throws(
    () => validateSourceEvidenceShape(null),
    /source evidence must be an object/,
  );
});

test("buildSourceEvidence assembles fixture snapshots and fails closed on ledger drift", () => {
  const operations = createBuildSourceOperations();
  const evidence = buildSourceEvidence({
    root: "/fixture/target",
    palmierPath: "/fixture/palmier",
    canonicalPath: "/fixture/canonical",
    operations,
  });
  assert.equal(evidence.openPullRequests.transport, "immutable-capture");
  assert.equal(evidence.sources[1].changedPaths.length, 132);
  assert.equal(evidence.branchIndex.length, 21);
  assert.equal(evidence.canonicalDirtyCheckout.paths.length, 56);

  assert.throws(
    () => buildSourceEvidence({
      root: "/fixture/target",
      palmierPath: "/fixture/palmier",
      canonicalPath: "/fixture/canonical",
      operations: createBuildSourceOperations({
        palmierPaths: PALMIER_REVIEWED_PATH_LEDGER.slice(1).map(
          ({ status, path, destination }) => ({
            status,
            path,
            destination,
            disposition: "unverified",
          }),
        ),
      }),
    }),
    /expected 132 paths, found 131/,
  );
});

test("captureDirtyCheckout hashes every tracked dirty and untracked path", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-dirty-source-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Completion Audit"]);
  git(root, ["config", "user.email", "audit@example.invalid"]);
  git(root, ["remote", "add", "origin", "https://example.invalid/canonical.git"]);
  writeFileSync(join(root, "modified.txt"), "base\n");
  writeFileSync(join(root, "deleted.txt"), "delete\n");
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", "base"]);
  writeFileSync(join(root, "modified.txt"), "changed\n");
  rmSync(join(root, "deleted.txt"));
  writeFileSync(join(root, "untracked.txt"), "new\n");
  const before = execFileSync("git", ["-C", root, "status", "--porcelain=v1", "-z"]);

  const first = captureDirtyCheckout({ name: "canonical", path: root });
  const second = captureDirtyCheckout({ name: "canonical", path: root });

  assert.deepEqual(second, first);
  assert.equal(first.repository, "https://example.invalid/canonical.git");
  assert.equal(first.status, "dirty");
  assert.equal(first.head, git(root, ["rev-parse", "HEAD"]));
  assert.equal(first.tree, git(root, ["rev-parse", "HEAD^{tree}"]));
  assert.equal(first.statusSha256, createHash("sha256").update(before).digest("hex"));
  assert.deepEqual(first.paths.map(({ status, path, tracked }) => [status, path, tracked]), [
    [" D", "deleted.txt", true],
    [" M", "modified.txt", true],
    ["??", "untracked.txt", false],
  ]);
  assert.deepEqual(
    first.paths.map(({ path }) => path),
    first.paths.map(({ path }) => path).toSorted(compareAuditText),
  );
  const deleted = first.paths[0];
  const modified = first.paths[1];
  const untracked = first.paths[2];
  assert.match(deleted.patchSha256, /^[0-9a-f]{64}$/);
  assert.equal(deleted.contentSha256, null);
  assert.equal(deleted.bytes, null);
  assert.equal(modified.contentSha256, createHash("sha256").update("changed\n").digest("hex"));
  assert.equal(modified.bytes, Buffer.byteLength("changed\n"));
  assert.match(modified.patchSha256, /^[0-9a-f]{64}$/);
  assert.equal(untracked.patchSha256, null);
  assert.equal(untracked.contentSha256, createHash("sha256").update("new\n").digest("hex"));
  assert.equal(
    first.manifestSha256,
    createHash("sha256").update(JSON.stringify(first.paths)).digest("hex"),
  );
  assert.deepEqual(
    execFileSync("git", ["-C", root, "status", "--porcelain=v1", "-z"]),
    before,
  );
});

test("captureDirtyCheckout fails closed when the snapshot changes during capture", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-dirty-race-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Completion Audit"]);
  git(root, ["config", "user.email", "audit@example.invalid"]);
  git(root, ["remote", "add", "origin", "https://example.invalid/race.git"]);
  writeFileSync(join(root, "tracked.txt"), "base\n");
  git(root, ["add", "."]);
  git(root, ["commit", "--quiet", "-m", "base"]);
  writeFileSync(join(root, "tracked.txt"), "dirty\n");

  assert.throws(
    () => captureDirtyCheckout({
      name: "racing canonical",
      path: root,
      afterInitialCapture: () => writeFileSync(join(root, "tracked.txt"), "changed again\n"),
    }),
    /changed while its manifest was being captured/,
  );
});

test("audit ordering is UTF-8 byte deterministic without Unicode normalization", () => {
  const decomposed = "e\u0301.txt";
  const composed = "\u00e9.txt";
  assert.notEqual(compareAuditText(decomposed, composed), 0);
  assert.deepEqual([composed, decomposed].sort(compareAuditText), [decomposed, composed]);
});

test("extractControls records labels, handlers, and panel triggers", () => {
  const source = "export function View(){return <button aria-label=\"Export\" onClick={() => setExportOpen(true)}>Go</button>}";
  const [control] = extractControls("web/src/View.tsx", source, ts);
  assert.equal(control.element, "button");
  assert.equal(control.label, "Export");
  assert.match(control.handler, /setExportOpen/);
  assert.equal(control.panelTrigger, true);
});

test("extractControls fails closed with normalized parse diagnostic locations", () => {
  const source = "export function Broken(){return <button onClick={save}>X</div>}";

  assert.throws(
    () => extractControls("./web\\src\\Broken.tsx", source, ts),
    (error) => {
      assert.match(error.message, /^web\/src\/Broken\.tsx:1:\d+:/);
      assert.match(error.message, /button|closing tag/i);
      return true;
    },
  );
});

test("extractControls separates user interaction handlers from passive lifecycle callbacks", () => {
  const source = [
    "export const Callbacks = () => <section>",
    "  <img onLoad={loaded} onError={failed} />",
    "  <audio onTimeUpdate={timed} onPlay={played} onPause={paused} />",
    "  <video controls onLoadedData={loaded} onEnded={ended} />",
    "  <div onLoad={loaded} onClick={clicked} onPointerDownCapture={captured} onLostPointerCapture={lost} onError={failed} />",
    "  <CustomLifecycle onTime={timed} onDuration={duration} onPlayingChange={playing} onTerminalFailure={terminal} onSuccess={success} onStart={started} onError={failed} onLoad={loaded} onStatusChange={status} />",
    "  <CustomActions onClick={clicked} onContextMenu={contexted} onPointerDown={pointed} onMouseUp={moused} onDoubleClick={doubled} onSubmit={submitted} onChange={changed} onInput={input} onKeyDown={keyed} onSelect={selected} onValueChange={valued} onDelete={deleted} onCommit={committed} onClose={closed} onSeek={sought} onResize={resized} onToggle={toggled} />",
    "  <button onLoad={loaded} onError={failed}>Native remains</button>",
    "</section>;",
  ].join("\n");

  const controls = extractControls("web/src/Callbacks.tsx", source, ts);

  assert.deepEqual(controls.map(({ element }) => element), [
    "video", "div", "CustomActions", "button",
  ]);
  assert.equal(controls[0].handler, "");
  assert.equal(controls[1].handler, "{clicked} {captured} {lost}");
  assert.equal(controls[3].handler, "");
  for (const action of [
    "clicked", "contexted", "pointed", "moused", "doubled", "submitted", "changed",
    "input", "keyed", "selected", "valued", "deleted", "committed", "closed", "sought",
    "resized", "toggled",
  ]) {
    assert.match(controls[2].handler, new RegExp(`\\{${action}\\}`));
  }
  assert.doesNotMatch(controls.map(({ handler }) => handler).join(" "), /loaded|failed|timed|played|paused|ended|terminal|success|started|status/);
});

test("extractControls covers native, custom-handler, and ARIA controls in source order", () => {
  const source = [
    "export function View(){return <section>",
    "  <a title=\"Documentation\" onKeyDown={openDocs}>Read docs</a>",
    "  <input aria-label=\"Search\" onChange={(event) => setQuery(event.currentTarget.value)} disabled={busy} />",
    "  <select title=\"Format\" onValueChange={showFormatMenu} />",
    "  <textarea onInput={handleNotes}>Notes fallback</textarea>",
    "  <HoverButton onPress={toggleInspector}>Inspector</HoverButton>",
    "  <Card onSelect={() => setActivePanel(\"library\")}>Library</Card>",
    "  <div role=\"combobox\" aria-label=\"Model\" />",
    "  <div role=\"treeitem\">Assets</div>",
    "  <div>Not interactive</div>",
    "</section>}",
  ].join("\n");

  const controls = extractControls("./web\\src\\View.tsx", source, ts);

  assert.deepEqual(controls.map(({ element }) => element), [
    "a", "input", "select", "textarea", "HoverButton", "Card", "div", "div",
  ]);
  assert.deepEqual(controls.map(({ order }) => order), [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.deepEqual(controls.map(({ line }) => line), [2, 3, 4, 5, 6, 7, 8, 9]);
  assert.ok(controls.every(({ column }) => Number.isInteger(column) && column > 0));
  assert.ok(controls.every(({ path }) => path === "web/src/View.tsx"));
  assert.equal(controls[0].label, "Documentation");
  assert.equal(controls[3].label, "Notes fallback");
  assert.equal(controls[4].label, "Inspector");
  assert.equal(controls[7].label, "Assets");
  assert.equal(controls[1].disabled, "{busy}");
  assert.equal(controls[6].role, "combobox");
  assert.match(controls[1].handler, /^\{\(event\)/);
  assert.equal(controls[0].panelTrigger, true);
  assert.equal(controls[2].panelTrigger, true);
  assert.equal(controls[4].panelTrigger, true);
  assert.equal(controls[5].panelTrigger, true);
  assert.equal(controls[1].panelTrigger, false);
  assert.deepEqual(extractControls("web/src/View.tsx", source, ts), controls);
});

test("extractControls uses columns to distinguish same-line same-tag controls", () => {
  const source = "export const Pair = () => <div><button>One</button><button>Two</button></div>;";
  const controls = extractControls("web/src/Pair.tsx", source, ts);

  assert.equal(controls.length, 2);
  assert.equal(controls[0].line, controls[1].line);
  assert.notEqual(controls[0].column, controls[1].column);
  assert.notEqual(controls[0].id, controls[1].id);
  assert.equal(
    controls[0].id,
    stableId("control", `web/src/Pair.tsx:${controls[0].line}:${controls[0].column}:button`),
  );
  assert.deepEqual(controls.map(({ label }) => label), ["One", "Two"]);
});

test("extractControls includes every required interactive ARIA role", () => {
  const roles = [
    "button", "menuitem", "tab", "switch", "slider", "link", "checkbox", "radio",
    "option", "combobox", "spinbutton", "textbox", "treeitem",
  ];
  const source = `export const Roles = () => <>${roles.map((role) => `<div role="${role}" />`).join("")}<div role="status" /></>`;

  assert.deepEqual(
    extractControls("web/src/Roles.tsx", source, ts).map(({ role }) => role),
    roles,
  );
});

test("extractControls constrains toggle panel patterns to visible surfaces", () => {
  const source = [
    "export const Triggers = () => <>",
    "  <button onClick={setOpen} />",
    "  <button onClick={setSettingsOpen} />",
    "  <button onClick={toggleInspector} />",
    "  <button onClick={toggleAgent} />",
    "  <button onClick={toggleMedia} />",
    "  <button onClick={toggleKeyframesPanel} />",
    "  <button onClick={onToggleKeyframes} />",
    "  <button onClick={openDialog} />",
    "  <button onClick={showSettings} />",
    "  <button onClick={onOpen} />",
    "  <button onClick={handleOpen} />",
    "  <button onClick={togglePlay} />",
    "  <button onClick={toggle} />",
    "  <button onClick={onToggle} />",
    "  <button onClick={toggleCropEditingActive} />",
    "  <button onClick={setQuery} />",
    "</>;",
  ].join("\n");

  assert.deepEqual(
    extractControls("web/src/Triggers.tsx", source, ts).map(({ panelTrigger }) => panelTrigger),
    [
      true, true, true, true, true, true, true, true, true, true, true,
      false, false, false, false, false,
    ],
  );
});

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

test("extractDocumentCandidates ignores fenced code and preserves the outer heading", () => {
  const source = [
    "# Plan",
    "```md TODO",
    "# Fake backtick heading",
    "~~~",
    "TODO: fake after mismatched fence",
    "``",
    "- [ ] fake after short fence",
    "   ````",
    "- [ ] Real after backticks",
    "## Recovery",
    "  ~~~~typescript FIXME",
    "## Fake tilde heading",
    "```",
    "FIXME: fake after mismatched fence",
    "   ~~~",
    "* [ ] fake after short fence",
    " ~~~~~",
    "- [ ] Real after tildes",
    "",
  ].join("\r\n");

  const records = extractDocumentCandidates("docs/fences.md", source);

  assert.deepEqual(records.map(({ line, signal, heading }) => [line, signal, heading]), [
    [1, "heading", "Plan"],
    [9, "unchecked", "Plan"],
    [10, "heading", "Recovery"],
    [18, "unchecked", "Recovery"],
  ]);
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

test("controls CLI scans tracked web/src TSX files through a real subprocess", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-controls-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const firstOutput = join(root, "control-candidates.json");
  const secondOutput = join(root, "control-candidates-second.json");
  mkdirSync(join(root, "web", "src", "nested"), { recursive: true });
  writeFileSync(
    join(root, "web", "src", "nested", "B.tsx"),
    "export const B = () => <div role=\"switch\" title=\"Beta\" />;\n",
  );
  writeFileSync(
    join(root, "web", "src", "A.tsx"),
    "export const A = () => <><button>Alpha</button><button>Again</button></>;\n",
  );
  writeFileSync(join(root, "web", "src", "ignored.ts"), "// onClick is not TSX\n");
  writeFileSync(join(root, "web", "src", "untracked.tsx"), "export const Nope = () => <button />;\n");
  execFileSync("git", ["init", "--quiet", root]);
  execFileSync("git", ["-C", root, "add", "web/src/A.tsx", "web/src/nested/B.tsx", "web/src/ignored.ts"]);

  const invoke = (output) => spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "controls",
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
  const ledger = JSON.parse(readFileSync(firstOutput, "utf8"));
  assert.equal(ledger.schema, 1);
  assert.deepEqual(ledger.candidates.map(({ path, element, label }) => [path, element, label]), [
    ["web/src/A.tsx", "button", "Alpha"],
    ["web/src/A.tsx", "button", "Again"],
    ["web/src/nested/B.tsx", "div", "Beta"],
  ]);
  assert.deepEqual(ledger.candidates.map(({ ownerSymbol }) => ownerSymbol), ["A", "A", "B"]);
  assert.equal(new Set(ledger.candidates.map(({ id }) => id)).size, 3);
  assert.equal(
    new Set(ledger.candidates.map(({ path, line, column }) => `${path}:${line}:${column}`)).size,
    3,
  );
});

test("control-ledger CLI creates shells and preserves reviewed records", (t) => {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-control-ledger-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const candidatesPath = join(root, "control-candidates.json");
  const controlsPath = join(root, "controls.json");
  const candidates = [
    {
      id: stableId("control", "web/src/A.tsx:1:25:button"),
      path: "web/src/A.tsx",
      line: 1,
      column: 25,
      order: 1,
      element: "button",
      label: "Alpha",
      handler: "{openAlpha}",
      disabled: null,
      role: "",
      panelTrigger: true,
    },
    {
      id: stableId("control", "web/src/B.tsx:2:3:input"),
      path: "web/src/B.tsx",
      line: 2,
      column: 3,
      order: 1,
      element: "input",
      label: "Beta",
      handler: "{setBeta}",
      disabled: "{busy}",
      role: "",
      panelTrigger: false,
    },
  ];
  const reviewed = {
    id: stableId("control-record", candidates[0].id),
    candidateId: candidates[0].id,
    visibility: "Editor is open",
    enabledWhen: "A project is active",
    inputs: ["pointer", "keyboard"],
    handler: candidates[0].handler,
    stateTransition: "Alpha opens",
    backendTrace: ["alpha_command"],
    outcomes: { success: "Alpha shown", failure: "Inline error" },
    accessibility: { focus: "visible", label: "Alpha", shortcut: "Cmd+A" },
    returnPath: ["Close Alpha"],
    automatedTests: ["alpha test"],
    runtimeEvidence: ["desktop pass"],
    status: "complete",
    acceptanceCriteria: ["Alpha opens"],
    gapGroup: "alpha",
    finalDisposition: "verified",
    commit: "abc123",
    reviewNotes: ["preserve byte-for-byte by candidate ID"],
  };
  writeFileSync(candidatesPath, `${JSON.stringify({ schema: 1, candidates }, null, 2)}\n`);
  writeFileSync(controlsPath, `${JSON.stringify({
    schema: 1,
    records: [reviewed, { id: "stale", candidateId: "control-stale" }],
  }, null, 2)}\n`);

  const invoke = () => spawnSync(
    process.execPath,
    [
      "tools/completion-audit.mjs",
      "control-ledger",
      "--root",
      root,
      "--candidates",
      candidatesPath,
      "--out",
      controlsPath,
    ],
    {
      cwd: fileURLToPath(new URL("..", import.meta.url)),
      encoding: "utf8",
    },
  );

  const firstResult = invoke();
  assert.equal(firstResult.status, 0, firstResult.stderr);
  const firstBytes = readFileSync(controlsPath, "utf8");
  const controls = JSON.parse(firstBytes);
  assert.equal(controls.schema, 1);
  assert.equal(controls.records.length, candidates.length);
  assert.deepEqual(controls.records[0], reviewed);
  assert.deepEqual(controls.records[1], {
    id: stableId("control-record", candidates[1].id),
    candidateId: candidates[1].id,
    visibility: null,
    enabledWhen: null,
    inputs: [],
    handler: candidates[1].handler,
    stateTransition: null,
    backendTrace: [],
    outcomes: {
      success: null,
      pending: null,
      empty: null,
      disabled: null,
      cancel: null,
      retry: null,
      failure: null,
    },
    accessibility: { focus: null, label: null, shortcut: null },
    returnPath: [],
    automatedTests: [],
    runtimeEvidence: [],
    status: "unverified",
    acceptanceCriteria: [],
    gapGroup: null,
    finalDisposition: null,
    commit: null,
  });

  const secondResult = invoke();
  assert.equal(secondResult.status, 0, secondResult.stderr);
  assert.equal(readFileSync(controlsPath, "utf8"), firstBytes);
});

test("control-ledger CLI rejects duplicate candidate IDs and column locations", async (t) => {
  const invokeWith = (candidates, suffix) => {
    const root = mkdtempSync(join(tmpdir(), `opentake-completion-audit-control-${suffix}-`));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const candidatesPath = join(root, "control-candidates.json");
    const controlsPath = join(root, "controls.json");
    writeFileSync(candidatesPath, `${JSON.stringify({ schema: 1, candidates }, null, 2)}\n`);
    return spawnSync(
      process.execPath,
      [
        "tools/completion-audit.mjs",
        "control-ledger",
        "--root",
        root,
        "--candidates",
        candidatesPath,
        "--out",
        controlsPath,
      ],
      {
        cwd: fileURLToPath(new URL("..", import.meta.url)),
        encoding: "utf8",
      },
    );
  };
  const base = {
    id: "control-one",
    path: "web/src/A.tsx",
    line: 4,
    column: 12,
    element: "button",
  };

  await t.test("duplicate ID", () => {
    const result = invokeWith([base, { ...base, path: "web/src/B.tsx" }], "duplicate-id");
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /duplicate candidate id: control-one/);
  });

  await t.test("duplicate path-line-column", () => {
    const result = invokeWith([base, { ...base, id: "control-two" }], "duplicate-location");
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /duplicate candidate location: web\/src\/A\.tsx:4:12/);
  });
});

test("verifyAudit controls accepts an exact incomplete schema-2 ledger", (t) => {
  const fixture = createControlVerificationFixture(t);
  const result = verifyAudit(fixture.root, fixture.audit, "controls");
  assert.equal(result.passed, true, JSON.stringify(result.errors));
  assert.deepEqual(result.counts, {
    candidates: 1,
    records: 1,
    uniqueCandidateIds: 1,
    uniqueRecordIds: 1,
    missingCandidateIds: 0,
    orphanCandidateIds: 0,
    duplicateCandidateIds: 0,
    duplicateRecordIds: 0,
    unverified: 0,
    complete: 0,
    incomplete: 1,
    obsolete: 0,
    duplicate: 0,
    contradicted: 0,
  });
});

test("verifyAudit controls derives ledger bindings from an arbitrary audit directory", (t) => {
  const fixture = createControlVerificationFixture(t, {
    auditRelative: "quality/evidence/custom-control-audit",
  });
  const result = verifyAudit(fixture.root, fixture.audit, "controls");
  assert.equal(result.passed, true, JSON.stringify(result.errors));
});

test("verifyAudit controls rejects current-source and record identity drift", async (t) => {
  await t.test("current handler drift", (t) => {
    const fixture = createControlVerificationFixture(t);
    writeFileSync(
      join(fixture.root, fixture.sourcePath),
      "import React from \"react\";\nexport function Panel({ openBeta }) {\n  return <button aria-label=\"Alpha\" onClick={openBeta}>Alpha</button>;\n}\nexport function harmlessHelper() { return { props: {} }; }\n",
    );
    git(fixture.root, ["add", fixture.sourcePath]);
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "candidate-field-drift"));
  });
  await t.test("record source and stable id drift", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.id = "control-record-forged";
    fixture.record.source.column += 1;
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "record-id-mismatch"));
    assert.ok(result.errors.some(({ code }) => code === "record-candidate-drift"));
  });
});

test("verifyAudit controls fails closed on status, gap, obsolete, and duplicate claims", async (t) => {
  await t.test("unverified and incomplete without executable gap contract", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.status = "unverified";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.controls.counts = { complete: 0, incomplete: 0, obsolete: 0, duplicate: 0, contradicted: 0 };
    fixture.controls.gapCounts = {};
    fixture.controls.scope.unverifiedCount = 1;
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "unverified-status"));
  });
  await t.test("obsolete record still describes an action", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.status = "obsolete";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.controls.counts = { complete: 0, incomplete: 0, obsolete: 1, duplicate: 0, contradicted: 0 };
    fixture.controls.gapCounts = {};
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "obsolete-with-action"));
  });
  await t.test("duplicate points to itself", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.status = "duplicate";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.record.duplicateOf = [fixture.candidate.id];
    fixture.record.finalDisposition = `Duplicate of ${fixture.candidate.id}.`;
    fixture.controls.counts = { complete: 0, incomplete: 0, obsolete: 0, duplicate: 1, contradicted: 0 };
    fixture.controls.gapCounts = {};
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-duplicate-target"));
    assert.ok(result.errors.some(({ code }) => code === "duplicate-canonical-chain"));
  });
});

test("verifyAudit controls rejects generic proof and accepts exact tests or direct typed receipts", async (t) => {
  const makeComplete = (fixture) => {
    fixture.record.status = "complete";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.controls.counts = { complete: 1, incomplete: 0, obsolete: 0, duplicate: 0, contradicted: 0 };
    fixture.controls.gapCounts = {};
  };
  await t.test("generic suite string is not completion evidence", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = ["pnpm test passed"];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));
  });
  await t.test("declared exact test without a candidate binding cannot complete", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    writeFileSync(
      join(fixture.root, fixture.testPath),
      'import assert from "node:assert/strict";\nimport test from "node:test";\ntest("Alpha invokes its exact handler", () => assert.equal(1, 1));\n',
    );
    fixture.record.automatedTests = [`test:${fixture.testPath}#Alpha invokes its exact handler`];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));
  });
  await t.test("candidate-bound static test without a real assertion cannot complete", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import test from "node:test";\ntest(${JSON.stringify(fixture.exactTestName)}, () => true);\n`,
    );
    fixture.record.automatedTests = [`test:${fixture.testPath}#${fixture.exactTestName}`];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));
  });
  await t.test("tracked static test with a candidate-specific assertion still cannot complete", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = [`test:${fixture.testPath}#${fixture.exactTestName}`];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));
  });
  await t.test("supporting generic command receipt cannot prove one control", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = [];
    const key = "generic-suite";
    const id = stableId("control-runtime-receipt", key);
    fixture.record.runtimeEvidence = [`receipt:${id}`];
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm test",
      startedAt: null,
      endedAt: null,
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      limitations: ["No candidate-specific assertion."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));
  });
  await t.test("direct automated receipt needs and validates a named test and assertion", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = [];
    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  let calls = 0;\n  const element = Panel({ openAlpha: () => { calls += 1; } });\n  const view = render(element);\n  view.props.onClick();\n  expect(calls).toBe(1);\n});\n`,
    );
    const key = "alpha-direct-test";
    const id = stableId("control-runtime-receipt", key);
    fixture.record.runtimeEvidence = [`receipt:${id}`];
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "direct",
      command: {
        kind: "vitest",
        executable: "web/node_modules/.bin/vitest",
        argv: [
          "run",
          "src/Panel.test.tsx",
          "--testNamePattern",
          `^${escapeRegExp(fixture.exactTestName)}$`,
          "--reporter=verbose",
        ],
        timeoutMs: 30_000,
      },
      executedCheckoutRevision: {
        commit: fixture.runtime.provenance.verifierRevision.commit,
        tree: fixture.runtime.provenance.verifierRevision.tree,
      },
      result: { summary: "one owning-component interaction test passed", exitCode: 0 },
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [fixture.candidate.id],
      assertions: [{
        candidateId: fixture.candidate.id,
        event: fixture.record.inputs.join(" + "),
        handler: fixture.record.handler,
        backend: fixture.record.backendTrace.join(" -> "),
        visibleOutcome: fixture.record.outcomes.success,
        accessibility: `focus=${fixture.record.accessibility.focus}; label=${fixture.record.accessibility.label}; shortcut=${fixture.record.accessibility.shortcut}`,
        returnPath: fixture.record.returnPath.join(" -> "),
        artifactPaths: [fixture.testPath],
        testEvidence: [`test:${fixture.testPath}#${fixture.exactTestName}`],
      }],
      artifacts: [{
        path: fixture.testPath,
        availability: "tracked",
        sha256: createHash("sha256").update(readFileSync(join(fixture.root, fixture.testPath))).digest("hex"),
      }],
      cleanup: {
        required: false,
        status: "not-required",
        details: ["The bounded test process exited itself."],
      },
      limitations: [],
      testEvidence: [`test:${fixture.testPath}#${fixture.exactTestName}`],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.equal(result.passed, true, JSON.stringify(result.errors));

    fixture.runtime.receipts[0].executedCheckoutRevision = {
      commit: fixture.runtime.provenance.auditedProductRevision.commit,
      tree: fixture.runtime.provenance.auditedProductRevision.tree,
    };
    fixture.write();
    const wrongExecutionRevision = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(wrongExecutionRevision.errors.some(
      ({ code }) => code === "runtime-execution-record-incomplete",
    ));
    fixture.runtime.receipts[0].executedCheckoutRevision = {
      commit: fixture.runtime.provenance.verifierRevision.commit,
      tree: fixture.runtime.provenance.verifierRevision.tree,
    };

    const verifyDirectSource = (source) => {
      writeFileSync(join(fixture.root, fixture.testPath), source);
      fixture.runtime.receipts[0].artifacts[0].sha256 = createHash("sha256")
        .update(readFileSync(join(fixture.root, fixture.testPath)))
        .digest("hex");
      fixture.write();
      return verifyAudit(fixture.root, fixture.audit, "controls");
    };

    const noGuardConditionalReturn = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (true) return;\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(noGuardConditionalReturn.errors.some(
      ({ code }) => code === "control-test-missing-assertion-guard",
    ));

    const fakeExpectGuard = verifyDirectSource(
      `import { test } from "vitest";\nimport { Panel } from "./Panel";\nconst expect = Object.assign(() => ({ toBe() {} }), { hasAssertions() {} });\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (true) return;\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(fakeExpectGuard.errors.some(
      ({ code }) => code === "control-test-missing-vitest-expect",
    ));

    const mutatedVitestGuard = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nexpect.hasAssertions = () => {};\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (true) return;\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(mutatedVitestGuard.errors.some(
      ({ code }) => code === "control-test-mutates-vitest-expect",
    ));

    const branchedGuard = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  if (true) expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(branchedGuard.errors.some(
      ({ code }) => code === "control-test-missing-assertion-guard",
    ));

    const zeroAssertionGuard = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.assertions(0);\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(zeroAssertionGuard.errors.some(
      ({ code }) => code === "control-test-missing-assertion-guard",
    ));

    const guardAfterEvent = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  expect.hasAssertions();\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(guardAfterEvent.errors.some(
      ({ code }) => code === "control-test-missing-assertion-guard",
    ));

    const guardAfterReturn = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  return;\n  expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(guardAfterReturn.errors.some(
      ({ code }) => code === "control-test-missing-assertion-guard",
    ));

    const guardedConditionalReturn = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (true) return;\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(guardedConditionalReturn.errors.some(
      ({ code }) => code === "direct-test-reexecution-failed",
    ));

    const guardedConditionalThrow = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.assertions(1);\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (true) throw new Error("stop before assertion");\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.ok(guardedConditionalThrow.errors.some(
      ({ code }) => code === "direct-test-reexecution-failed",
    ));

    const guardedFalseBranch = verifyDirectSource(
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.assertions(1);\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  if (false) return;\n  expect(calls).toBe(1);\n});\n`,
    );
    assert.equal(guardedFalseBranch.passed, true, JSON.stringify(guardedFalseBranch.errors));

    const vitestCommand = fixture.runtime.receipts[0].command;
    fixture.runtime.receipts[0].command = {
      kind: "node-test",
      executable: "node",
      argv: [
        "--test",
        "--test-name-pattern",
        `^${escapeRegExp(fixture.exactTestName)}$`,
        fixture.testPath,
      ],
      timeoutMs: 30_000,
    };
    fixture.write();
    const nodeDirectRunner = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(nodeDirectRunner.errors.some(
      ({ code }) => code === "direct-runner-not-vitest",
    ));
    fixture.runtime.receipts[0].command = vitestCommand;

    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.assertions(1);\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  expect(calls).toBe(2);\n});\n`,
    );
    fixture.runtime.receipts[0].artifacts[0].sha256 = createHash("sha256")
      .update(readFileSync(join(fixture.root, fixture.testPath)))
      .digest("hex");
    fixture.write();
    const selfReportedOnly = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(selfReportedOnly.errors.some(
      ({ code }) => code === "direct-test-reexecution-failed",
    ));
  });
  await t.test("typed receipt timestamps, exact keys, and summary fail closed", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "malformed-supporting-receipt";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "partial",
      evidenceLevel: "supporting",
      command: null,
      startedAt: "not-a-timestamp",
      endedAt: null,
      exitCode: null,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      limitations: ["No per-candidate proof."],
      testEvidence: [],
      unexpected: true,
    }];
    fixture.write();
    fixture.runtime.summary.receipts = 0;
    writeFileSync(
      join(fixture.audit, "runtime-evidence.json"),
      `${JSON.stringify(fixture.runtime, null, 2)}\n`,
    );
    git(fixture.root, ["add", "docs/audit/2026-07-14/runtime-evidence.json"]);
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-runtime-receipt-schema"));
    assert.ok(result.errors.some(({ code }) => code === "invalid-runtime-timestamp"));
    assert.ok(result.errors.some(({ code }) => code === "runtime-summary-drift"));
  });
  await t.test("executed receipts require strict timezone timestamps", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "loose-timestamp-receipt";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "supporting",
      command: "node --test",
      startedAt: "2026-07-14",
      endedAt: null,
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      limitations: ["Generic suite only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-runtime-timestamp"));
  });
  await t.test("runtime metadata and cleanup use exact fail-closed contracts", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.runtime.metadata.unexpected = true;
    const key = "browser-without-cleanup";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm -C web dev --host 127.0.0.1 --port 1437",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:30:33+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      limitations: ["Generic browser reachability only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-runtime-ledger-metadata"));
    assert.ok(result.errors.some(({ code }) => code === "runtime-cleanup-unverified"));
  });
  await t.test("verified cleanup requires at least one concrete probe", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "browser-empty-cleanup-proof";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm -C web dev --host 127.0.0.1 --port 1437",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:30:33+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      cleanup: { required: true, status: "verified", details: [] },
      limitations: ["Generic browser reachability only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-runtime-cleanup"));
  });
  await t.test("declared runtime artifacts require a sha256 digest", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "browser-unhashed-artifact";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm -C web dev --host 127.0.0.1 --port 1437",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:30:33+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [{
        path: ".superpowers/sdd/missing-runtime-capture.png",
        availability: "local-ignored",
        sha256: null,
      }],
      limitations: ["Generic browser reachability only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "runtime-artifact-hash-required"));
  });
  await t.test("local-ignored artifact declarations must actually be ignored", (t) => {
    const fixture = createControlVerificationFixture(t);
    const artifactPath = ".superpowers/sdd/browser-capture.log";
    mkdirSync(join(fixture.root, ".superpowers", "sdd"), { recursive: true });
    writeFileSync(join(fixture.root, artifactPath), "browser capture\n");
    const key = "browser-false-ignored-artifact";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm -C web dev --host 127.0.0.1 --port 1437",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:30:33+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [{
        path: artifactPath,
        availability: "local-ignored",
        sha256: createHash("sha256").update("browser capture\n").digest("hex"),
      }],
      cleanup: {
        required: true,
        status: "verified",
        details: ["The browser and Vite server were stopped."],
      },
      limitations: ["Generic browser reachability only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "runtime-artifact-availability-mismatch"));
  });
  await t.test("hashed tracked browser artifacts with verified cleanup are accepted as supporting", (t) => {
    const fixture = createControlVerificationFixture(t);
    const artifactPath = "docs/audit/2026-07-14/browser-capture.log";
    writeFileSync(join(fixture.root, artifactPath), "browser capture\n");
    const key = "browser-valid-ignored-artifact";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "pnpm -C web dev --host 127.0.0.1 --port 1437",
      executedCheckoutRevision: {
        commit: fixture.runtime.provenance.auditedProductRevision.commit,
        tree: fixture.runtime.provenance.auditedProductRevision.tree,
      },
      result: { summary: "browser capture completed", exitCode: 0 },
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:30:33+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [{
        path: artifactPath,
        availability: "tracked",
        sha256: createHash("sha256").update("browser capture\n").digest("hex"),
      }],
      cleanup: {
        required: true,
        status: "verified",
        details: ["The browser and Vite server were stopped."],
      },
      limitations: ["Generic browser reachability only."],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.equal(result.passed, true, JSON.stringify(result.errors));

    fixture.runtime.receipts[0].limitations = [
      "All browser artifacts are local ignored files.",
    ];
    fixture.write();
    const contradictoryAvailability = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(contradictoryAvailability.errors.some(
      ({ code }) => code === "runtime-artifact-limitation-contradiction",
    ));
  });
  await t.test("automated receipt summary and limitation test counts must agree", (t) => {
    const fixture = createControlVerificationFixture(t);
    const artifactPath = "docs/audit/2026-07-14/full-suite-process.txt";
    writeFileSync(join(fixture.root, artifactPath), "163 tests passed\n");
    const key = "automated-count-mismatch";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "supporting",
      command: "node --test tools/completion-audit.test.mjs",
      executedCheckoutRevision: {
        commit: fixture.runtime.provenance.verifierRevision.commit,
        tree: fixture.runtime.provenance.verifierRevision.tree,
      },
      result: { summary: "163 audit tests passed and 0 failed.", exitCode: 0 },
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [{
        path: artifactPath,
        availability: "tracked",
        sha256: createHash("sha256").update("163 tests passed\n").digest("hex"),
      }],
      cleanup: {
        required: false,
        status: "not-required",
        details: ["The process exited."],
      },
      limitations: [
        "The 148 passing tests validate fail-closed audit behavior only.",
      ],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(
      ({ code }) => code === "runtime-limitation-count-mismatch",
    ));
  });
});

test("verifyAudit controls rejects static and unrelated completion proof", async (t) => {
  const makeComplete = (fixture) => {
    fixture.record.status = "complete";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.controls.counts = { complete: 1, incomplete: 0, obsolete: 0, duplicate: 0, contradicted: 0 };
    fixture.controls.gapCounts = {};
  };

  await t.test("candidate id plus assert.equal(1, 1) cannot prove complete", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import assert from "node:assert/strict";\nimport test from "node:test";\ntest(${JSON.stringify(fixture.exactTestName)}, () => assert.equal(1, 1));\n`,
    );
    fixture.record.automatedTests = [`test:${fixture.testPath}#${fixture.exactTestName}`];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
  });

  await t.test("an independent EventTarget does not exercise the owning component", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = [`test:${fixture.testPath}#${fixture.exactTestName}`];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "static-control-test-cannot-complete"));
  });

  await t.test("an unrelated render plus an independent owner identifier cannot complete", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    fixture.record.automatedTests = [];
    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  render("<div />");\n  void Panel;\n  expect(1).toBe(1);\n});\n`,
    );
    const key = "alpha-unrelated-render";
    const id = stableId("control-runtime-receipt", key);
    fixture.record.runtimeEvidence = [`receipt:${id}`];
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "direct",
      command: {
        kind: "vitest",
        executable: "web/node_modules/.bin/vitest",
        argv: [
          "run",
          "src/Panel.test.tsx",
          "--testNamePattern",
          `^${escapeRegExp(fixture.exactTestName)}$`,
          "--reporter=verbose",
        ],
        timeoutMs: 30_000,
      },
      executedCheckoutRevision: {
        commit: fixture.runtime.provenance.verifierRevision.commit,
        tree: fixture.runtime.provenance.verifierRevision.tree,
      },
      result: { summary: "one purported owning-component interaction test passed", exitCode: 0 },
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [fixture.candidate.id],
      assertions: [{
        candidateId: fixture.candidate.id,
        event: fixture.record.inputs.join(" + "),
        handler: fixture.record.handler,
        backend: fixture.record.backendTrace.join(" -> "),
        visibleOutcome: fixture.record.outcomes.success,
        accessibility: `focus=${fixture.record.accessibility.focus}; label=${fixture.record.accessibility.label}; shortcut=${fixture.record.accessibility.shortcut}`,
        returnPath: fixture.record.returnPath.join(" -> "),
        artifactPaths: [fixture.testPath],
        testEvidence: [`test:${fixture.testPath}#${fixture.exactTestName}`],
      }],
      artifacts: [{
        path: fixture.testPath,
        availability: "tracked",
        sha256: createHash("sha256").update(readFileSync(join(fixture.root, fixture.testPath))).digest("hex"),
      }],
      cleanup: {
        required: false,
        status: "not-required",
        details: ["The bounded test process exited itself."],
      },
      limitations: [],
      testEvidence: [`test:${fixture.testPath}#${fixture.exactTestName}`],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "control-test-does-not-exercise-owner"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-direct-verification"));

    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel, harmlessHelper } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  render(harmlessHelper());\n  void Panel;\n  expect(1).toBe(1);\n});\n`,
    );
    fixture.runtime.receipts[0].artifacts[0].sha256 = createHash("sha256")
      .update(readFileSync(join(fixture.root, fixture.testPath)))
      .digest("hex");
    fixture.write();
    const sameModuleHelper = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(sameModuleHelper.errors.some(
      ({ code }) => code === "control-test-does-not-exercise-owner",
    ));

    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  render(Panel({ openAlpha: () => {} }));\n  expect(1).toBe(1);\n});\n`,
    );
    fixture.runtime.receipts[0].artifacts[0].sha256 = createHash("sha256")
      .update(readFileSync(join(fixture.root, fixture.testPath)))
      .digest("hex");
    fixture.write();
    const unrelatedAssertion = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(unrelatedAssertion.errors.some(
      ({ code }) => code === "control-test-assertion-not-bound",
    ));

    writeFileSync(
      join(fixture.root, fixture.testPath),
      `import { expect, test } from "vitest";\nimport { Panel } from "./Panel";\nfunction render(value) { return value; }\ntest(${JSON.stringify(fixture.exactTestName)}, () => {\n  expect.hasAssertions();\n  let calls = 0;\n  const view = render(Panel({ openAlpha: () => { calls += 1; } }));\n  view.props.onClick();\n  return;\n  expect(calls).toBe(1);\n});\n`,
    );
    fixture.runtime.receipts[0].artifacts[0].sha256 = createHash("sha256")
      .update(readFileSync(join(fixture.root, fixture.testPath)))
      .digest("hex");
    fixture.write();
    const unreachableAssertion = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(unreachableAssertion.errors.some(
      ({ code }) => code === "control-test-assertion-not-bound",
    ));
  });

  await t.test("one unrelated screenshot cannot prove multiple controls", (t) => {
    const fixture = createControlVerificationFixture(t);
    makeComplete(fixture);
    const secondPath = "web/src/PanelBeta.tsx";
    writeFileSync(
      join(fixture.root, secondPath),
      "export function PanelBeta({ openBeta }) {\n  return <button aria-label=\"Beta\" onClick={openBeta}>Beta</button>;\n}\n",
    );
    const [secondCandidate] = extractControls(
      secondPath,
      readFileSync(join(fixture.root, secondPath), "utf8"),
      ts,
    );
    const secondRecord = {
      ...structuredClone(fixture.record),
      id: stableId("control-record", secondCandidate.id),
      candidateId: secondCandidate.id,
      candidateLabel: secondCandidate.label,
      semanticName: "Beta",
      source: {
        path: secondCandidate.path,
        line: secondCandidate.line,
        column: secondCandidate.column,
      },
      element: secondCandidate.element,
      handler: secondCandidate.handler,
    };
    fixture.candidates.push(secondCandidate);
    fixture.controls.records.push(secondRecord);
    fixture.controls.scope.candidateCount = 2;
    fixture.controls.counts.complete = 2;
    const artifactPath = "docs/audit/2026-07-14/unrelated.png";
    writeFileSync(join(fixture.audit, "unrelated.png"), "not a component capture\n");
    const key = "two-controls-one-unrelated-screenshot";
    const id = stableId("control-runtime-receipt", key);
    const evidence = `receipt:${id}`;
    fixture.record.automatedTests = [];
    fixture.record.runtimeEvidence = [evidence];
    secondRecord.automatedTests = [];
    secondRecord.runtimeEvidence = [evidence];
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "direct",
      command: "browser-control --session two-controls",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [fixture.candidate.id, secondCandidate.id],
      assertions: [
        {
          candidateId: fixture.candidate.id,
          event: "click Alpha",
          expected: "Alpha handler and visible outcome",
          observed: "unrelated screenshot exists",
        },
        {
          candidateId: secondCandidate.id,
          event: "click Beta",
          expected: "Beta handler and visible outcome",
          observed: "the same unrelated screenshot exists",
        },
      ],
      artifacts: [{
        path: artifactPath,
        availability: "tracked",
        sha256: createHash("sha256").update("not a component capture\n").digest("hex"),
      }],
      cleanup: {
        required: true,
        status: "verified",
        details: ["The browser process exited."],
      },
      limitations: [],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "direct-assertion-artifact-not-candidate-bound"));
  });
});

test("verifyAudit controls rejects missing artifacts, future receipts, and weak execution records", async (t) => {
  await t.test("a declared artifact must still exist", (t) => {
    const fixture = createControlVerificationFixture(t);
    const artifactPath = ".superpowers/sdd/deleted-capture.log";
    writeFileSync(join(fixture.root, ".gitignore"), ".superpowers/\n");
    mkdirSync(join(fixture.root, ".superpowers", "sdd"), { recursive: true });
    writeFileSync(join(fixture.root, artifactPath), "capture\n");
    const key = "deleted-artifact";
    fixture.runtime.receipts = [{
      id: stableId("control-runtime-receipt", key),
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "supporting",
      command: "browser-control --session deleted-artifact",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [{
        path: artifactPath,
        availability: "local-ignored",
        sha256: createHash("sha256").update("capture\n").digest("hex"),
      }],
      cleanup: {
        required: true,
        status: "verified",
        details: ["The browser process exited."],
      },
      limitations: [],
      testEvidence: [],
    }];
    fixture.write();
    rmSync(join(fixture.root, artifactPath));
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "runtime-artifact-missing"));
  });

  await t.test("a receipt from 2099 is rejected", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "future-receipt";
    fixture.runtime.receipts = [{
      id: stableId("control-runtime-receipt", key),
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "supporting",
      command: "node --test",
      startedAt: "2099-01-01T00:00:00+00:00",
      endedAt: "2099-01-01T00:00:01+00:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      cleanup: {
        required: false,
        status: "not-required",
        details: ["The process exited."],
      },
      limitations: [],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "runtime-timestamp-in-future"));
  });

  await t.test("passed execution needs source revision, result, and a process artifact", (t) => {
    const fixture = createControlVerificationFixture(t);
    const key = "weak-passed-execution";
    fixture.runtime.receipts = [{
      id: stableId("control-runtime-receipt", key),
      key,
      kind: "automated",
      status: "passed",
      evidenceLevel: "supporting",
      command: "node --test",
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [],
      assertions: [],
      artifacts: [],
      cleanup: {
        required: false,
        status: "not-required",
        details: ["The process exited."],
      },
      limitations: [],
      testEvidence: [],
    }];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "runtime-execution-record-incomplete"));
  });
});

test("verifyAudit controls requires immutable provenance and executable criteria", async (t) => {
  await t.test("a production candidate cannot omit its lexical owner", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.candidate.ownerSymbol = null;
    fixture.write();
    const candidateLedgerHash = createHash("sha256")
      .update(readFileSync(join(fixture.audit, "control-candidates.json")))
      .digest("hex");
    fixture.controls.provenance.candidateLedgerSha256 = candidateLedgerHash;
    fixture.runtime.provenance.candidateLedgerSha256 = candidateLedgerHash;
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-candidate"));
  });

  await t.test("an explicitly obsolete test fixture candidate may omit a lexical owner", (t) => {
    const fixture = createControlVerificationFixture(t, {
      sourcePath: "web/src/FixtureCandidate.test.tsx",
      sourceText: `import React from "react";\nimport { it } from "vitest";\nit("renders a fixture", () => {\n  const openAlpha = () => {};\n  return <button aria-label="Alpha" onClick={openAlpha}>Alpha</button>;\n});\n`,
    });
    assert.equal(fixture.candidate.ownerSymbol, null);
    fixture.record.semanticName = "test-only fixture control";
    fixture.record.stateTransition = "N/A — test fixture only.";
    fixture.record.backendTrace = ["N/A — test fixture only."];
    fixture.record.outcomes = Object.fromEntries(
      Object.keys(fixture.record.outcomes).map((key) => [key, "N/A — test fixture only."]),
    );
    fixture.record.status = "obsolete";
    fixture.record.finalDisposition = "Test-fixture JSX is not a production user-visible control.";
    fixture.record.acceptanceCriteria = [];
    fixture.record.gapGroup = null;
    fixture.controls.codeEvidence = {};
    fixture.controls.counts = {
      complete: 0,
      incomplete: 0,
      obsolete: 1,
      duplicate: 0,
      contradicted: 0,
    };
    fixture.controls.gapCounts = {};
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.equal(result.passed, true, JSON.stringify(result.errors));

    const artifactPath = `docs/audit/2026-07-14/${fixture.candidate.id}-fixture.png`;
    writeFileSync(join(fixture.root, artifactPath), "test-fixture capture\n");
    const key = "ownerless-test-fixture-direct";
    const id = stableId("control-runtime-receipt", key);
    fixture.runtime.receipts = [{
      id,
      key,
      kind: "browser",
      status: "passed",
      evidenceLevel: "direct",
      command: "fixture browser capture",
      executedCheckoutRevision: {
        commit: fixture.runtime.provenance.auditedProductRevision.commit,
        tree: fixture.runtime.provenance.auditedProductRevision.tree,
      },
      result: { summary: "fixture browser capture passed", exitCode: 0 },
      startedAt: "2026-07-14T19:10:33+08:00",
      endedAt: "2026-07-14T19:10:34+08:00",
      exitCode: 0,
      candidateIds: [fixture.candidate.id],
      assertions: [{
        candidateId: fixture.candidate.id,
        event: fixture.record.inputs.join(" + "),
        handler: fixture.record.handler,
        backend: fixture.record.backendTrace.join(" -> "),
        visibleOutcome: fixture.record.outcomes.success,
        accessibility: `focus=${fixture.record.accessibility.focus}; label=${fixture.record.accessibility.label}; shortcut=${fixture.record.accessibility.shortcut}`,
        returnPath: fixture.record.returnPath.join(" -> "),
        artifactPaths: [artifactPath],
        testEvidence: [],
      }],
      artifacts: [{
        path: artifactPath,
        availability: "tracked",
        sha256: createHash("sha256").update("test-fixture capture\n").digest("hex"),
      }],
      cleanup: {
        required: true,
        status: "verified",
        details: ["The browser was stopped."],
      },
      limitations: ["Test-fixture observation only."],
      testEvidence: [],
    }];
    fixture.write();
    const directResult = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(directResult.errors.some(
      ({ code }) => code === "direct-evidence-for-ownerless-fixture",
    ));
  });

  await t.test("missing source provenance is rejected", (t) => {
    const fixture = createControlVerificationFixture(t);
    delete fixture.controls.provenance;
    delete fixture.runtime.provenance;
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "missing-control-provenance"));
  });

  await t.test("backend source hash drift is rejected even when the JSX handler is unchanged", (t) => {
    const fixture = createControlVerificationFixture(t);
    const backendPath = "web/src/backend.ts";
    writeFileSync(join(fixture.root, backendPath), "export function applyAlpha() { return 'v1'; }\n");
    fixture.record.backendTrace = [`code:${backendPath}#applyAlpha`];
    fixture.controls.codeEvidence = {
      [`code:${backendPath}#applyAlpha`]: createHash("sha256")
        .update("export function applyAlpha() { return 'v1'; }\n")
        .digest("hex"),
    };
    fixture.write();
    writeFileSync(join(fixture.root, backendPath), "export function applyAlpha() { return 'v2'; }\n");
    git(fixture.root, ["add", backendPath]);
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "backend-source-hash-mismatch"));
  });

  await t.test("verifier file drift is rejected independently of product source", (t) => {
    const fixture = createControlVerificationFixture(t);
    const verifierPath = "tools/completion-audit-controls.mjs";
    writeFileSync(join(fixture.root, verifierPath), "fixture verifier file drifted\n");
    git(fixture.root, ["add", verifierPath]);
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "verifier-file-hash-mismatch"));
  });

  await t.test("TODO is not an executable candidate-specific acceptance contract", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.acceptanceCriteria = ["TODO"];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-control-acceptance-criteria"));
  });

  await t.test("criteria copied from another candidate are rejected", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.acceptanceCriteria = [
      "Candidate: control-0000000000000000.",
      "Test: web/src/Other.interaction.test.tsx#control-0000000000000000 other control.",
      "Initial state: other state.",
      "Event: click other.",
      "Exact call/state/backend: other call.",
      "Visible/accessibility/return path: other result.",
      "Outcome matrix: success=other; pending=other; empty=other; disabled=other; cancel=other; retry=other; failure=other.",
    ];
    fixture.write();
    const result = verifyAudit(fixture.root, fixture.audit, "controls");
    assert.ok(result.errors.some(({ code }) => code === "invalid-control-acceptance-criteria"));
  });

  await t.test("malformed criteria source fields fail closed instead of throwing", (t) => {
    const fixture = createControlVerificationFixture(t);
    fixture.record.outcomes = null;
    fixture.write();
    let result;
    assert.doesNotThrow(() => {
      result = verifyAudit(fixture.root, fixture.audit, "controls");
    });
    assert.ok(result.errors.some(({ code }) => code === "invalid-field-type"));
    assert.ok(result.errors.some(({ code }) => code === "invalid-control-acceptance-criteria"));
  });
});

test("verify controls CLI writes failure output and exits nonzero", (t) => {
  const fixture = createControlVerificationFixture(t);
  fixture.record.status = "unverified";
  fixture.controls.scope.unverifiedCount = 1;
  fixture.controls.counts.incomplete = 0;
  fixture.write();
  const output = join(fixture.audit, "control-verification.json");
  const result = spawnSync(process.execPath, [
    "tools/completion-audit.mjs",
    "verify",
    "--root",
    fixture.root,
    "--audit",
    fixture.audit,
    "--scope",
    "controls",
    "--out",
    output,
  ], {
    cwd: fileURLToPath(new URL("..", import.meta.url)),
    encoding: "utf8",
  });
  assert.notEqual(result.status, 0);
  const verification = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(verification.passed, false);
  assert.ok(verification.errors.some(({ code }) => code === "unverified-status"));
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

function createDocumentVerificationFixture(t) {
  const root = mkdtempSync(join(tmpdir(), "opentake-completion-audit-verify-documents-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  execFileSync("git", ["init", "--quiet", root]);
  git(root, ["config", "user.name", "Completion Audit"]);
  git(root, ["config", "user.email", "audit@example.invalid"]);
  const audit = join(root, "docs", "audit", "2026-07-14");
  const sourcePath = "docs/requirements.md";
  const implementationPath = "web/src/Complete.ts";
  const testPath = "web/src/Complete.test.ts";
  mkdirSync(audit, { recursive: true });
  mkdirSync(join(root, "web", "src"), { recursive: true });
  writeFileSync(join(root, sourcePath), "# Complete behavior\n- [ ] Incomplete behavior\n");
  writeFileSync(
    join(root, implementationPath),
    "export function completeBehavior() { return true; }\n",
  );
  writeFileSync(
    join(root, testPath),
    'test("renders complete behavior", () => completeBehavior());\n',
  );
  git(root, ["add", sourcePath, implementationPath, testPath]);
  git(root, ["commit", "--quiet", "-m", "verification fixture"]);
  const candidates = extractDocumentCandidates(
    sourcePath,
    readFileSync(join(root, sourcePath), "utf8"),
  );
  const [completeCandidate, incompleteCandidate] = candidates;
  const records = [{
    id: stableId("requirement", completeCandidate.id),
    candidateId: completeCandidate.id,
    source: { path: completeCandidate.path, line: completeCandidate.line },
    targetBehavior: "The complete behavior is available.",
    priority: null,
    status: "complete",
    uiEntry: [],
    visibleResult: null,
    react: [`code:${implementationPath}#completeBehavior`],
    storeApi: [],
    tauri: [],
    rust: [],
    sideEffects: [],
    returnPath: [],
    automatedTests: [`test:${testPath}#renders complete behavior`],
    runtimeEvidence: [],
    provenance: [`source:${completeCandidate.path}:${completeCandidate.line}:heading`],
    acceptanceCriteria: [],
    gapGroup: null,
    finalDisposition: "verified-complete",
    commit: null,
  }, {
    id: stableId("requirement", incompleteCandidate.id),
    candidateId: incompleteCandidate.id,
    source: { path: incompleteCandidate.path, line: incompleteCandidate.line },
    targetBehavior: "The incomplete behavior becomes available.",
    priority: null,
    status: "incomplete",
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
    provenance: [`source:${incompleteCandidate.path}:${incompleteCandidate.line}:unchecked`],
    acceptanceCriteria: ["A focused test proves the incomplete behavior."],
    gapGroup: "documentation",
    finalDisposition: "active-gap:documentation",
    commit: null,
  }];
  const write = (nextCandidates = candidates, nextRecords = records) => {
    writeFileSync(
      join(audit, "document-candidates.json"),
      `${JSON.stringify({ schema: 1, candidates: nextCandidates }, null, 2)}\n`,
    );
    writeFileSync(
      join(audit, "requirements.json"),
      `${JSON.stringify({ schema: 1, records: nextRecords }, null, 2)}\n`,
    );
  };
  write();
  return {
    root,
    audit,
    sourcePath,
    implementationPath,
    testPath,
    completeCandidate,
    incompleteCandidate,
    candidates,
    records,
    write,
  };
}

test("verifyAudit documents accepts a complete, planned ledger", (t) => {
  const fixture = createDocumentVerificationFixture(t);
  const result = verifyAudit(fixture.root, fixture.audit, "documents");

  assert.equal(result.passed, true);
  assert.deepEqual(result.errors, []);
  assert.deepEqual(result.counts, {
    candidates: 2,
    records: 2,
    uniqueCandidateIds: 2,
    uniqueRecordIds: 2,
    missingCandidateIds: 0,
    orphanCandidateIds: 0,
    duplicateCandidateIds: 0,
    duplicateRecordIds: 0,
    unverified: 0,
    complete: 1,
    incomplete: 1,
    contradicted: 0,
    obsolete: 0,
    duplicate: 0,
  });
});

test("verifyAudit documents rejects missing and duplicate identities", async (t) => {
  await t.test("missing candidate ID", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    fixture.write(fixture.candidates, fixture.records.slice(0, 1));
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.equal(result.passed, false);
    assert.ok(result.errors.some(({ code }) => code === "missing-candidate-id"));
  });

  await t.test("duplicate record ID", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    fixture.write(fixture.candidates, [fixture.records[0], {
      ...fixture.records[1],
      id: fixture.records[0].id,
    }]);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "duplicate-record-id"));
  });

  await t.test("duplicate record candidateId", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    fixture.write(fixture.candidates, [fixture.records[0], {
      ...fixture.records[1],
      candidateId: fixture.records[0].candidateId,
    }]);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "duplicate-candidate-id"));
  });

  await t.test("orphan requirement candidateId", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    fixture.write(fixture.candidates, [...fixture.records, {
      ...fixture.records[1],
      id: "requirement-orphan",
      candidateId: "doc-orphan",
      source: { path: "docs/orphan.md", line: 1 },
    }]);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "orphan-candidate-id"));
  });
});

test("verifyAudit documents rejects unresolved or unsupported dispositions", async (t) => {
  const mutate = (t, change) => {
    const fixture = createDocumentVerificationFixture(t);
    const records = structuredClone(fixture.records);
    change(records);
    fixture.write(fixture.candidates, records);
    return verifyAudit(fixture.root, fixture.audit, "documents");
  };

  await t.test("unverified", (t) => {
    const result = mutate(t, (records) => { records[0].status = "unverified"; });
    assert.ok(result.errors.some(({ code }) => code === "unverified-status"));
  });

  await t.test("complete without implementation", (t) => {
    const result = mutate(t, (records) => {
      records[0].react = [];
      records[0].provenance = [];
    });
    assert.ok(result.errors.some(({ code }) => code === "complete-without-implementation"));
  });

  await t.test("complete without verification", (t) => {
    const result = mutate(t, (records) => { records[0].automatedTests = []; });
    assert.ok(result.errors.some(({ code }) => code === "complete-without-verification"));
  });

  await t.test("record ID does not derive from candidate ID", (t) => {
    const result = mutate(t, (records) => { records[0].id = "requirement-wrong"; });
    assert.ok(result.errors.some(({ code }) => code === "record-id-mismatch"));
  });

  await t.test("complete without target behavior", (t) => {
    const result = mutate(t, (records) => { records[0].targetBehavior = null; });
    assert.ok(result.errors.some(({ code }) => code === "requirement-without-target-behavior"));
  });

  await t.test("incomplete without final disposition", (t) => {
    const result = mutate(t, (records) => { records[1].finalDisposition = null; });
    assert.ok(result.errors.some(({ code }) => code === "requirement-without-final-disposition"));
  });

  await t.test("incomplete without acceptance criteria", (t) => {
    const result = mutate(t, (records) => { records[1].acceptanceCriteria = []; });
    assert.ok(result.errors.some(({ code }) => code === "incomplete-without-acceptance-criteria"));
  });

  await t.test("invalid status", (t) => {
    const result = mutate(t, (records) => { records[0].status = "maybe"; });
    assert.ok(result.errors.some(({ code }) => code === "invalid-status"));
  });

  await t.test("invalid gap group", (t) => {
    const result = mutate(t, (records) => { records[1].gapGroup = "miscellaneous"; });
    assert.ok(result.errors.some(({ code }) => code === "invalid-gap-group"));
  });

  await t.test("incomplete without gap group", (t) => {
    const result = mutate(t, (records) => { records[1].gapGroup = null; });
    assert.ok(result.errors.some(({ code }) => code === "incomplete-without-gap-group"));
  });

  await t.test("contradicted without target behavior", (t) => {
    const result = mutate(t, (records) => {
      records[0].status = "contradicted";
      records[0].targetBehavior = null;
    });
    assert.ok(result.errors.some(({ code }) => code === "disposition-without-target-behavior"));
  });

  await t.test("obsolete without final disposition", (t) => {
    const result = mutate(t, (records) => {
      records[0].status = "obsolete";
      records[0].finalDisposition = null;
    });
    assert.ok(result.errors.some(({ code }) => code === "disposition-without-final-disposition"));
  });

  await t.test("duplicate without provenance", (t) => {
    const result = mutate(t, (records) => {
      records[0].status = "duplicate";
      records[0].provenance = [];
    });
    assert.ok(result.errors.some(({ code }) => code === "disposition-without-provenance"));
  });

  await t.test("candidate source drift", (t) => {
    const result = mutate(t, (records) => { records[0].source.line = 5; });
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-drift"));
  });

  await t.test("candidate source path drift", (t) => {
    const result = mutate(t, (records) => { records[0].source.path = "docs/elsewhere.md"; });
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-drift"));
  });

  await t.test("non-array evidence field", (t) => {
    const result = mutate(t, (records) => { records[0].react = "web/src/Complete.ts"; });
    assert.ok(result.errors.some(({ code, candidateId, message }) => (
      code === "invalid-field-type"
      && candidateId != null
      && message === "react must be an array of non-empty strings"
    )));
  });

  await t.test("non-string evidence item", (t) => {
    const result = mutate(t, (records) => { records[0].automatedTests = [123]; });
    assert.ok(result.errors.some(({ code }) => code === "invalid-field-type"));
  });

  await t.test("non-string scalar field", (t) => {
    const result = mutate(t, (records) => { records[0].targetBehavior = 42; });
    assert.ok(result.errors.some(({ code }) => code === "invalid-field-type"));
  });

  await t.test("non-array return path", (t) => {
    const result = mutate(t, (records) => { records[0].returnPath = "snapshot"; });
    assert.ok(result.errors.some(({ code, message }) => (
      code === "invalid-field-type" && message.startsWith("returnPath must")
    )));
  });

  await t.test("non-string nullable metadata", (t) => {
    const result = mutate(t, (records) => { records[0].priority = 1; });
    assert.ok(result.errors.some(({ code, message }) => (
      code === "invalid-field-type" && message === "priority must be a string or null"
    )));
  });

  await t.test("malformed candidate source fields fail closed", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const candidates = structuredClone(fixture.candidates);
    candidates[0].path = 42;
    candidates[0].line = "4";
    fixture.write(candidates, fixture.records);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code, candidateId }) => (
      code === "invalid-candidate" && candidateId === fixture.completeCandidate.id
    )));
  });

  await t.test("unsupported audit schema fails closed", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    writeFileSync(
      join(fixture.audit, "requirements.json"),
      `${JSON.stringify({ schema: 2, records: fixture.records }, null, 2)}\n`,
    );
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code, message }) => (
      code === "invalid-audit-schema" && message.startsWith("requirements.json")
    )));
  });
});

test("verifyAudit documents rejects repositories and candidate sources that drift", async (t) => {
  await t.test("audit directory outside repository", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const outside = mkdtempSync(join(tmpdir(), "opentake-completion-audit-outside-"));
    t.after(() => rmSync(outside, { recursive: true, force: true }));
    writeFileSync(
      join(outside, "document-candidates.json"),
      `${JSON.stringify({ schema: 1, candidates: fixture.candidates }, null, 2)}\n`,
    );
    writeFileSync(
      join(outside, "requirements.json"),
      `${JSON.stringify({ schema: 1, records: fixture.records }, null, 2)}\n`,
    );
    const result = verifyAudit(fixture.root, outside, "documents");
    assert.ok(result.errors.some(({ code }) => code === "audit-directory-outside-root"));
  });

  await t.test("internal audit symlink escaping the repository", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const outside = mkdtempSync(join(tmpdir(), "opentake-completion-audit-linked-"));
    t.after(() => rmSync(outside, { recursive: true, force: true }));
    writeFileSync(
      join(outside, "document-candidates.json"),
      `${JSON.stringify({ schema: 1, candidates: fixture.candidates }, null, 2)}\n`,
    );
    writeFileSync(
      join(outside, "requirements.json"),
      `${JSON.stringify({ schema: 1, records: fixture.records }, null, 2)}\n`,
    );
    const linkedAudit = join(fixture.root, "docs", "audit", "linked");
    symlinkSync(outside, linkedAudit, "dir");
    const result = verifyAudit(fixture.root, linkedAudit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "audit-directory-symlink-escape"));
  });

  await t.test("missing source file", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    rmSync(join(fixture.root, fixture.sourcePath));
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-missing"));
  });

  await t.test("untracked source file", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    git(fixture.root, ["rm", "--cached", fixture.sourcePath]);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-untracked"));
  });

  await t.test("tracked candidate source leaf symlink", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const outside = mkdtempSync(join(tmpdir(), "opentake-candidate-leaf-link-"));
    t.after(() => rmSync(outside, { recursive: true, force: true }));
    const outsideSource = join(outside, "requirements.md");
    writeFileSync(outsideSource, "# Complete behavior\n- [ ] Incomplete behavior\n");
    rmSync(join(fixture.root, fixture.sourcePath));
    symlinkSync(outsideSource, join(fixture.root, fixture.sourcePath));
    git(fixture.root, ["add", "-f", fixture.sourcePath]);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-symlink"));
  });

  await t.test("candidate source ancestor symlink escape", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const outside = mkdtempSync(join(tmpdir(), "opentake-candidate-ancestor-link-"));
    t.after(() => rmSync(outside, { recursive: true, force: true }));
    writeFileSync(join(outside, "requirements.md"), "# Complete behavior\n- [ ] Incomplete behavior\n");
    const linked = join(fixture.root, "docs", "linked");
    symlinkSync(outside, linked, "dir");
    git(fixture.root, ["add", "docs/linked"]);
    const linkedPath = "docs/linked/requirements.md";
    const candidates = extractDocumentCandidates(
      linkedPath,
      readFileSync(join(outside, "requirements.md"), "utf8"),
    );
    const records = structuredClone(fixture.records);
    for (let index = 0; index < records.length; index += 1) {
      records[index].candidateId = candidates[index].id;
      records[index].id = stableId("requirement", candidates[index].id);
      records[index].source = { path: linkedPath, line: candidates[index].line };
    }
    fixture.write(candidates, records);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-source-symlink-escape"));
  });

  await t.test("forged candidate ID", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const candidates = structuredClone(fixture.candidates);
    const records = structuredClone(fixture.records);
    candidates[0].id = "doc-forged";
    records[0].candidateId = candidates[0].id;
    records[0].id = stableId("requirement", candidates[0].id);
    fixture.write(candidates, records);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-id-mismatch"));
  });

  await t.test("source text drift", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const candidates = structuredClone(fixture.candidates);
    candidates[0].text = "# Different behavior";
    fixture.write(candidates, fixture.records);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-text-drift"));
  });

  await t.test("source signal drift", (t) => {
    const fixture = createDocumentVerificationFixture(t);
    const candidates = structuredClone(fixture.candidates);
    candidates[0].signal = "unchecked";
    fixture.write(candidates, fixture.records);
    const result = verifyAudit(fixture.root, fixture.audit, "documents");
    assert.ok(result.errors.some(({ code }) => code === "candidate-signal-drift"));
  });
});

test("verifyAudit documents rejects invalid implementation, test, and runtime evidence", async (t) => {
  const mutate = (t, change) => {
    const fixture = createDocumentVerificationFixture(t);
    const records = structuredClone(fixture.records);
    change(records[0], fixture);
    fixture.write(fixture.candidates, records);
    return verifyAudit(fixture.root, fixture.audit, "documents");
  };

  const implementationCases = [
    ["missing implementation path", "code:web/src/Missing.ts#completeBehavior", "implementation-path-missing"],
    ["missing implementation symbol", "code:web/src/Complete.ts#missingSymbol", "implementation-symbol-missing"],
    ["implementation directory", "code:web/src#completeBehavior", "invalid-implementation-evidence"],
    ["combined implementation symbols", "code:web/src/Complete.ts#completeBehavior/other", "invalid-implementation-evidence"],
    ["invalid implementation symbol grammar", "code:web/src/Complete.ts#complete-behavior", "invalid-implementation-evidence"],
    ["natural-language implementation", "web/src/Complete.ts has complete behavior", "invalid-implementation-evidence"],
  ];
  for (const [name, evidence, code] of implementationCases) {
    await t.test(name, (t) => {
      const result = mutate(t, (record) => { record.react = [evidence]; });
      assert.ok(result.errors.some((error) => error.code === code));
    });
  }

  const testCases = [
    ["missing test path", "test:web/src/Missing.test.ts#renders complete behavior", "test-path-missing"],
    ["missing test name", "test:web/src/Complete.test.ts#missing behavior", "test-name-missing"],
    ["test file only", "web/src/Complete.test.ts", "invalid-test-evidence"],
    ["natural-language test", "Complete test passes", "invalid-test-evidence"],
  ];
  for (const [name, evidence, code] of testCases) {
    await t.test(name, (t) => {
      const result = mutate(t, (record) => { record.automatedTests = [evidence]; });
      assert.ok(result.errors.some((error) => error.code === code));
    });
  }

  await t.test("tracked implementation leaf symlink", (t) => {
    const result = mutate(t, (record, fixture) => {
      const outside = mkdtempSync(join(tmpdir(), "opentake-implementation-leaf-link-"));
      t.after(() => rmSync(outside, { recursive: true, force: true }));
      const outsideSource = join(outside, "Complete.ts");
      writeFileSync(outsideSource, "export function completeBehavior() { return true; }\n");
      rmSync(join(fixture.root, fixture.implementationPath));
      symlinkSync(outsideSource, join(fixture.root, fixture.implementationPath));
      git(fixture.root, ["add", "-f", fixture.implementationPath]);
    });
    assert.ok(result.errors.some(({ code }) => code === "implementation-path-symlink"));
  });

  await t.test("implementation ancestor symlink escape", (t) => {
    const result = mutate(t, (record, fixture) => {
      const outside = mkdtempSync(join(tmpdir(), "opentake-implementation-ancestor-link-"));
      t.after(() => rmSync(outside, { recursive: true, force: true }));
      writeFileSync(join(outside, "Complete.ts"), "export function completeBehavior() { return true; }\n");
      symlinkSync(outside, join(fixture.root, "web", "linked"), "dir");
      git(fixture.root, ["add", "web/linked"]);
      record.react = ["code:web/linked/Complete.ts#completeBehavior"];
    });
    assert.ok(result.errors.some(({ code }) => code === "implementation-path-escape"));
  });

  await t.test("tracked test leaf symlink", (t) => {
    const result = mutate(t, (record, fixture) => {
      const outside = mkdtempSync(join(tmpdir(), "opentake-test-leaf-link-"));
      t.after(() => rmSync(outside, { recursive: true, force: true }));
      const outsideSource = join(outside, "Complete.test.ts");
      writeFileSync(outsideSource, 'test("renders complete behavior", () => true);\n');
      rmSync(join(fixture.root, fixture.testPath));
      symlinkSync(outsideSource, join(fixture.root, fixture.testPath));
      git(fixture.root, ["add", "-f", fixture.testPath]);
    });
    assert.ok(result.errors.some(({ code }) => code === "test-path-symlink"));
  });

  await t.test("test ancestor symlink escape", (t) => {
    const result = mutate(t, (record, fixture) => {
      const outside = mkdtempSync(join(tmpdir(), "opentake-test-ancestor-link-"));
      t.after(() => rmSync(outside, { recursive: true, force: true }));
      writeFileSync(join(outside, "Complete.test.ts"), 'test("renders complete behavior", () => true);\n');
      symlinkSync(outside, join(fixture.root, "web", "linked-tests"), "dir");
      git(fixture.root, ["add", "web/linked-tests"]);
      record.automatedTests = ["test:web/linked-tests/Complete.test.ts#renders complete behavior"];
    });
    assert.ok(result.errors.some(({ code }) => code === "test-path-escape"));
  });

  await t.test("symbol use without declaration", (t) => {
    const result = mutate(t, (record, fixture) => {
      writeFileSync(
        join(fixture.root, fixture.implementationPath),
        "export function wrapper() { return completeBehavior(); }\n",
      );
    });
    assert.ok(result.errors.some(({ code }) => code === "implementation-symbol-missing"));
  });

  for (const [name, source] of [
    ["commented TypeScript declaration", "// export function completeBehavior() { return true; }\n"],
    ["string-contained TypeScript declaration", 'export const note = "function completeBehavior() {}";\n'],
  ]) {
    await t.test(name, (t) => {
      const result = mutate(t, (record, fixture) => {
        writeFileSync(join(fixture.root, fixture.implementationPath), source);
      });
      assert.ok(result.errors.some(({ code }) => code === "implementation-symbol-missing"));
    });
  }

  await t.test("aliased TypeScript test call is unsupported", (t) => {
    const result = mutate(t, (record, fixture) => {
      writeFileSync(
        join(fixture.root, fixture.testPath),
        'const check = test; check("renders complete behavior", () => true);\n',
      );
    });
    assert.ok(result.errors.some(({ code }) => code === "test-name-missing"));
  });

  for (const [name, source] of [
    ["commented Rust declaration", "// pub fn real_implementation() {}\n"],
    ["string-contained Rust declaration", 'pub const NOTE: &str = r#"pub fn real_implementation() {}"#;\n'],
    ["macro-token Rust pseudo declaration", "stringify!(fn real_implementation() {});\n"],
    [
      "macro_rules nested body pseudo declaration",
      "macro_rules! blueprint { (($value:expr)) => {{ let _ = [($value)]; fn real_implementation() {} }} }\n",
    ],
  ]) {
    await t.test(name, (t) => {
      const result = mutate(t, (record, fixture) => {
        const implementationPath = "crates/example/src/lib.rs";
        mkdirSync(join(fixture.root, "crates", "example", "src"), { recursive: true });
        writeFileSync(join(fixture.root, implementationPath), source);
        git(fixture.root, ["add", implementationPath]);
        record.react = [];
        record.rust = [`code:${implementationPath}#real_implementation`];
      });
      assert.ok(result.errors.some(({ code }) => code === "implementation-symbol-missing"));
    });
  }

  await t.test("macro_rules declaration name is supported without exposing its body", (t) => {
    const result = mutate(t, (record, fixture) => {
      const implementationPath = "crates/example/src/lib.rs";
      mkdirSync(join(fixture.root, "crates", "example", "src"), { recursive: true });
      writeFileSync(
        join(fixture.root, implementationPath),
        "macro_rules! blueprint { (($value:expr)) => {{ let _ = [($value)]; fn forged_symbol() {} }} }\n",
      );
      git(fixture.root, ["add", implementationPath]);
      record.react = [];
      record.rust = [`code:${implementationPath}#blueprint`];
    });
    assert.equal(result.errors.length, 0, JSON.stringify(result.errors));
  });

  await t.test("macro_rules nested body test attribute is not an automated test", (t) => {
    const result = mutate(t, (record, fixture) => {
      const testPath = "crates/example/tests/macro_body.rs";
      mkdirSync(join(fixture.root, "crates", "example", "tests"), { recursive: true });
      writeFileSync(
        join(fixture.root, testPath),
        "macro_rules! blueprint { (($value:expr)) => {{ let _ = [($value)]; #[test] fn forged_test() {} }} }\n",
      );
      git(fixture.root, ["add", testPath]);
      record.automatedTests = [`test:${testPath}#forged_test`];
    });
    assert.ok(result.errors.some(({ code }) => code === "test-name-missing"));
  });

  for (const [name, source] of [
    ["commented TypeScript test", '// test("renders complete behavior", () => completeBehavior());\n'],
    ["string-contained TypeScript test", 'export const note = `test("renders complete behavior", () => true)`;\n'],
  ]) {
    await t.test(name, (t) => {
      const result = mutate(t, (record, fixture) => {
        writeFileSync(join(fixture.root, fixture.testPath), source);
      });
      assert.ok(result.errors.some(({ code }) => code === "test-name-missing"));
    });
  }

  await t.test("ordinary Rust function is not an automated test", (t) => {
    const result = mutate(t, (record, fixture) => {
      const implementationPath = "crates/example/src/lib.rs";
      const testPath = "crates/example/tests/helper.rs";
      mkdirSync(join(fixture.root, "crates", "example", "src"), { recursive: true });
      mkdirSync(join(fixture.root, "crates", "example", "tests"), { recursive: true });
      writeFileSync(join(fixture.root, implementationPath), "pub fn real_implementation() {}\n");
      writeFileSync(join(fixture.root, testPath), "fn ordinary_helper() {}\n");
      git(fixture.root, ["add", implementationPath, testPath]);
      record.react = [];
      record.rust = [`code:${implementationPath}#real_implementation`];
      record.automatedTests = [`test:${testPath}#ordinary_helper`];
    });
    assert.ok(result.errors.some(({ code }) => code === "test-name-missing"));
  });

  for (const [name, testSource] of [
    ["Rust ignore attribute is not a test", "#[ignore]\nfn ordinary_helper() {}\n"],
    ["Rust test attribute cannot attach across another function", "#[test]\nfn different_test() {}\nfn ordinary_helper() {}\n"],
  ]) {
    await t.test(name, (t) => {
      const result = mutate(t, (record, fixture) => {
        const implementationPath = "crates/example/src/lib.rs";
        const testPath = "crates/example/tests/helper.rs";
        mkdirSync(join(fixture.root, "crates", "example", "src"), { recursive: true });
        mkdirSync(join(fixture.root, "crates", "example", "tests"), { recursive: true });
        writeFileSync(join(fixture.root, implementationPath), "pub fn real_implementation() {}\n");
        writeFileSync(join(fixture.root, testPath), testSource);
        git(fixture.root, ["add", implementationPath, testPath]);
        record.react = [];
        record.rust = [`code:${implementationPath}#real_implementation`];
        record.automatedTests = [`test:${testPath}#ordinary_helper`];
      });
      assert.ok(result.errors.some(({ code }) => code === "test-name-missing"));
    });
  }

  for (const [name, attribute] of [
    ["tokio test attribute is supported", "#[tokio::test]\nasync"],
    ["async-std test attribute is supported", "#[async_std::test]\nasync"],
  ]) {
    await t.test(name, (t) => {
      const result = mutate(t, (record, fixture) => {
        const implementationPath = "crates/example/src/lib.rs";
        const testPath = "crates/example/tests/supported.rs";
        mkdirSync(join(fixture.root, "crates", "example", "src"), { recursive: true });
        mkdirSync(join(fixture.root, "crates", "example", "tests"), { recursive: true });
        writeFileSync(join(fixture.root, implementationPath), "pub fn real_implementation() {}\n");
        writeFileSync(join(fixture.root, testPath), `${attribute} fn supported_test() {}\n`);
        git(fixture.root, ["add", implementationPath, testPath]);
        record.react = [];
        record.rust = [`code:${implementationPath}#real_implementation`];
        record.automatedTests = [`test:${testPath}#supported_test`];
      });
      assert.equal(result.errors.length, 0, JSON.stringify(result.errors));
    });
  }

  await t.test("runtime commit is unsupported self-report", (t) => {
    const result = mutate(t, (record, fixture) => {
      record.runtimeEvidence = [`commit:${git(fixture.root, ["rev-parse", "HEAD"])}`];
    });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
  });

  await t.test("matching runtime hash is unsupported self-report", (t) => {
    const result = mutate(t, (record, fixture) => {
      const digest = createHash("sha256")
        .update(readFileSync(join(fixture.root, fixture.implementationPath)))
        .digest("hex");
      record.runtimeEvidence = [`hash:${fixture.implementationPath}#sha256=${digest}`];
      record.automatedTests = [];
    });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
    assert.ok(result.errors.some(({ code }) => code === "complete-without-verification"));
  });

  await t.test("matching zero-exit runtime receipt is unsupported self-report", (t) => {
    const result = mutate(t, (record, fixture) => {
      const receiptPath = "receipts/test.json";
      mkdirSync(join(fixture.root, "receipts"), { recursive: true });
      writeFileSync(join(fixture.root, receiptPath), '{"exitCode":0}\n');
      git(fixture.root, ["add", receiptPath]);
      const digest = createHash("sha256")
        .update(readFileSync(join(fixture.root, receiptPath)))
        .digest("hex");
      record.runtimeEvidence = [`receipt:${receiptPath}#sha256=${digest}#exit=0`];
    });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
  });

  for (const status of [
    "incomplete",
    "obsolete",
    "duplicate",
    "contradicted",
    "unverified",
  ]) {
    await t.test(`${status} matching runtime commit is unsupported self-report`, (t) => {
      const result = mutate(t, (record, fixture) => {
        record.status = status;
        record.runtimeEvidence = [`commit:${git(fixture.root, ["rev-parse", "HEAD"])}`];
        record.gapGroup = status === "incomplete" ? "documentation" : null;
        record.acceptanceCriteria = status === "incomplete"
          ? ["A deterministic automated test proves the behavior."]
          : [];
      });
      assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
    });
  }

  await t.test("obsolete tracked historical report provenance is accepted", (t) => {
    const result = mutate(t, (record, fixture) => {
      record.status = "obsolete";
      record.gapGroup = null;
      record.runtimeEvidence = [];
      record.provenance.push(`historical-report:${fixture.implementationPath}`);
    });
    assert.equal(result.errors.length, 0, JSON.stringify(result.errors));
  });

  await t.test("historical report provenance rejects a non-tracked regular path", (t) => {
    const result = mutate(t, (record) => {
      record.status = "obsolete";
      record.gapGroup = null;
      record.runtimeEvidence = [];
      record.provenance.push("historical-report:reports/missing.md");
    });
    assert.ok(result.errors.some(({ code }) => (
      code === "invalid-historical-report-provenance"
    )));
  });

  await t.test("forged runtime commit", (t) => {
    const result = mutate(t, (record) => { record.runtimeEvidence = [`commit:${"0".repeat(40)}`]; });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
  });

  await t.test("forged runtime hash", (t) => {
    const result = mutate(t, (record, fixture) => {
      record.runtimeEvidence = [`hash:${fixture.implementationPath}#sha256=${"0".repeat(64)}`];
    });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
  });

  await t.test("forged runtime receipt", (t) => {
    const result = mutate(t, (record) => {
      record.runtimeEvidence = [
        `receipt:web/src/Complete.test.ts#sha256=${"0".repeat(64)}#exit=0`,
      ];
    });
    assert.ok(result.errors.some(({ code }) => code === "unsupported-runtime-evidence"));
  });
});

test("review demotions are unique and semantically bound to frozen candidates", () => {
  const root = fileURLToPath(new URL("..", import.meta.url));
  const audit = join(root, "docs", "audit", "2026-07-14");
  const candidates = JSON.parse(
    readFileSync(join(audit, "document-candidates.json"), "utf8"),
  ).candidates;
  const records = JSON.parse(
    readFileSync(join(audit, "requirements.json"), "utf8"),
  ).records;
  const candidatesById = new Map(candidates.map((candidate) => [candidate.id, candidate]));
  const provenance = "review-provenance:demoted-from-complete@6e32952ccf4fa7a2fd3cdd66f327702024f4a463";
  const demoted = records.filter((record) => record.provenance.includes(provenance));
  assert.equal(demoted.length, 289);
  assert.equal(new Set(demoted.map((record) => record.targetBehavior)).size, demoted.length);
  assert.equal(
    new Set(demoted.map((record) => JSON.stringify(record.acceptanceCriteria))).size,
    demoted.length,
  );

  const labels = [
    "Source binding:",
    "Expected behavior:",
    "Deterministic test:",
    "Initial state/input/event:",
    "Code/store/API/Rust effect:",
    "Visible/returned assertion:",
    "Evidence required:",
  ];
  for (const record of demoted) {
    const candidate = candidatesById.get(record.candidateId);
    assert.ok(candidate, record.candidateId);
    assert.equal(record.acceptanceCriteria.length, labels.length, record.candidateId);
    labels.forEach((label, index) => {
      assert.ok(record.acceptanceCriteria[index].startsWith(label), `${record.candidateId} ${label}`);
    });
    assert.match(record.targetBehavior, new RegExp(`${escapeRegExp(candidate.path)}:${candidate.line}`));
    assert.ok(record.acceptanceCriteria[0].includes(candidate.text), record.candidateId);
    assert.ok(record.acceptanceCriteria[0].includes(`signal=${candidate.signal}`), record.candidateId);
    assert.ok(
      record.acceptanceCriteria[2].includes(record.candidateId.replace(/^doc-/, "")),
      record.candidateId,
    );
    assert.match(record.acceptanceCriteria[2], /test:[^#]+#[A-Za-z_][A-Za-z0-9_]*$/);
    assert.match(record.acceptanceCriteria[6], /code:<tracked-file>#<declared-symbol>/);
    assert.match(record.acceptanceCriteria[6], /test:<tracked-test-file>#<exact-test-name>/);
  }

  const agent = demoted.filter(({ source }) => (
    /^docs\/specs\/agent\/(?:1-mcp-server|4-execution-shell|10-implementation)\.md$/.test(source.path)
  ));
  assert.ok(agent.length > 0);
  assert.equal(new Set(agent.map(({ targetBehavior }) => targetBehavior)).size, agent.length);
  for (const record of agent) {
    const candidate = candidatesById.get(record.candidateId);
    assert.ok(record.targetBehavior.includes(candidate.text), record.candidateId);
    assert.ok(record.acceptanceCriteria.join(" ").includes(candidate.text), record.candidateId);
  }

  const exactAgentTerms = new Map([
    ["docs/specs/agent/1-mcp-server.md:1", ["loopback", "stateless", "origin", "content-type", "protocol version", "route", "capabilities"]],
    ["docs/specs/agent/1-mcp-server.md:26", ["rmcp", "axum", "tower", "streamablehttpservice", "/mcp", "well-known"]],
    ["docs/specs/agent/1-mcp-server.md:47", ["127.0.0.1", "default enabled", "idempotent", "disabled", "0.0.0.0"]],
    ["docs/specs/agent/1-mcp-server.md:73", ["origin", "host", "403", "application/json", "415", "protocol", "400"]],
    ["docs/specs/agent/1-mcp-server.md:90", ["name", "version", "instructions", "opentake://models/video", "opentake://models/image", "capabilities"]],
    ["docs/specs/agent/4-execution-shell.md:1", ["execution sequence", "snapshot", "undo", "context signal", "shorten", "no panic"]],
    ["docs/specs/agent/4-execution-shell.md:5", ["toolname", "snapshot", "expand", "run", "undo", "context signal", "shorten", "serialized"]],
    ["docs/specs/agent/4-execution-shell.md:31", ["three-layer", "unknown fields", "non-finite", "serde path", "validation order"]],
    ["docs/specs/agent/4-execution-shell.md:33", ["unknown fields", "nested", "allowed", "entries[3]", "no mutation"]],
    ["docs/specs/agent/4-execution-shell.md:42", ["first non-finite", "array", "object", "value must be finite"]],
    ["docs/specs/agent/4-execution-shell.md:55", ["keynotfound", "typemismatch", "valuenotfound", "datacorrupted", "entries[3].startframe"]],
    ["docs/specs/agent/4-execution-shell.md:84", ["per-tool guards", "exact error messages", "no mutation", "mixed trackindex"]],
    ["docs/specs/agent/4-execution-shell.md:97", ["assistant-only undo", "session undo stack", "user undo", "conflict", "not undoing"]],
    ["docs/specs/agent/4-execution-shell.md:113", ["toolresult", "text", "image", "is_error", "rmcp", "calltoolresult"]],
    ["docs/specs/agent/10-implementation.md:5", ["workspace", "lib.rs exports", "module tree", "desktop shell", "tauri"]],
    ["docs/specs/agent/10-implementation.md:51", ["serde_path_to_error", "allowedkeys", "non-finite", "entries[3].startframe", "exact wording"]],
    ["docs/specs/agent/10-implementation.md:85", ["os keychain", "project.json", "logs", "telemetry", "webview", "plaintext"]],
    ["docs/specs/agent/10-implementation.md:88", ["untrusted", "plugin:{id}", "system prompt", "prompt injection", "source label"]],
  ]);
  for (const [key, terms] of exactAgentTerms) {
    const [path, line] = key.split(/:(?=\d+$)/);
    const record = agent.find((item) => item.source.path === path && item.source.line === Number(line));
    assert.ok(record, key);
    const contract = `${record.targetBehavior} ${record.acceptanceCriteria.slice(1, 6).join(" ")}`.toLowerCase();
    for (const term of terms) assert.ok(contract.includes(term), `${key} missing ${term}`);
  }
  for (const line of [97, 113]) {
    const record = agent.find((item) => (
      item.source.path === "docs/specs/agent/4-execution-shell.md" && item.source.line === line
    ));
    assert.doesNotMatch(
      record.acceptanceCriteria.slice(1, 6).join(" ").toLowerCase(),
      /malformed input|unknown fields|non-finite/,
    );
  }
});

test("verify CLI writes failed document verification before exiting nonzero", (t) => {
  const fixture = createDocumentVerificationFixture(t);
  fixture.records[0].status = "unverified";
  fixture.write();
  const output = join(fixture.audit, "document-verification.json");
  const result = spawnSync(process.execPath, [
    "tools/completion-audit.mjs",
    "verify",
    "--root",
    fixture.root,
    "--audit",
    fixture.audit,
    "--scope",
    "documents",
    "--out",
    output,
  ], {
    cwd: fileURLToPath(new URL("..", import.meta.url)),
    encoding: "utf8",
  });

  assert.notEqual(result.status, 0);
  const verification = JSON.parse(readFileSync(output, "utf8"));
  assert.equal(verification.passed, false);
  assert.ok(verification.errors.some(({ code }) => code === "unverified-status"));
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
