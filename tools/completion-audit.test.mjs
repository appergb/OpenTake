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
  buildFileInventory,
  classifyFile,
  extractControls,
  extractDocumentCandidates,
  normalizePath,
  stableId,
} from "./completion-audit.mjs";

const require = createRequire(new URL("../web/package.json", import.meta.url));
const ts = require("typescript");

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
