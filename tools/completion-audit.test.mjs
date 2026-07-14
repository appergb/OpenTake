import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
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

  await t.test("symbol use without declaration", (t) => {
    const result = mutate(t, (record, fixture) => {
      writeFileSync(
        join(fixture.root, fixture.implementationPath),
        "export function wrapper() { return completeBehavior(); }\n",
      );
    });
    assert.ok(result.errors.some(({ code }) => code === "implementation-symbol-missing"));
  });

  await t.test("valid runtime commit", (t) => {
    const result = mutate(t, (record, fixture) => {
      record.runtimeEvidence = [`commit:${git(fixture.root, ["rev-parse", "HEAD"])}`];
    });
    assert.equal(result.errors.length, 0);
  });

  await t.test("valid runtime hash", (t) => {
    const result = mutate(t, (record, fixture) => {
      const digest = createHash("sha256")
        .update(readFileSync(join(fixture.root, fixture.implementationPath)))
        .digest("hex");
      record.runtimeEvidence = [`hash:${fixture.implementationPath}#sha256=${digest}`];
    });
    assert.equal(result.errors.length, 0);
  });

  await t.test("valid zero-exit runtime receipt", (t) => {
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
    assert.equal(result.errors.length, 0);
  });

  await t.test("forged runtime commit", (t) => {
    const result = mutate(t, (record) => { record.runtimeEvidence = [`commit:${"0".repeat(40)}`]; });
    assert.ok(result.errors.some(({ code }) => code === "runtime-commit-invalid"));
  });

  await t.test("forged runtime hash", (t) => {
    const result = mutate(t, (record, fixture) => {
      record.runtimeEvidence = [`hash:${fixture.implementationPath}#sha256=${"0".repeat(64)}`];
    });
    assert.ok(result.errors.some(({ code }) => code === "runtime-hash-invalid"));
  });

  await t.test("forged runtime receipt", (t) => {
    const result = mutate(t, (record) => {
      record.runtimeEvidence = [
        `receipt:web/src/Complete.test.ts#sha256=${"0".repeat(64)}#exit=0`,
      ];
    });
    assert.ok(result.errors.some(({ code }) => code === "runtime-receipt-invalid"));
  });
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
