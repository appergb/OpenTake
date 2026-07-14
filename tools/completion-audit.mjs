import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
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

const CONTROL_ELEMENTS = new Set([
  "a",
  "button",
  "input",
  "select",
  "textarea",
  "HoverButton",
]);

const INTERACTIVE_ROLES = new Set([
  "button",
  "menuitem",
  "tab",
  "switch",
  "slider",
  "link",
  "checkbox",
  "radio",
  "option",
  "combobox",
  "spinbutton",
  "textbox",
  "treeitem",
]);

const MEDIA_ELEMENTS = new Set(["audio", "video"]);

const DOM_INTERACTION_HANDLER_PATTERN = /^on(?:(?:AuxClick|BeforeInput|Blur|Cancel|Change|Click|Close|Composition(?:End|Start|Update)|ContextMenu|Copy|Cut|DoubleClick|Drag(?:End|Enter|Exit|Leave|Over|Start)?|Drop|Focus|Input|Key(?:Down|Press|Up)|Mouse(?:Down|Enter|Leave|Move|Out|Over|Up)|Paste|Pointer(?:Cancel|Down|Enter|Leave|Move|Out|Over|Up)|Reset|Resize|Scroll(?:End)?|Seek|Select|Submit|Touch(?:Cancel|End|Move|Start)|Wheel)(?:Capture)?|GotPointerCapture|LostPointerCapture)$/;

const EXPLICIT_INTERACTION_CALLBACKS = new Set([
  "onClose",
  "onCommit",
  "onDelete",
  "onOpen",
  "onOpenChange",
  "onPress",
  "onResize",
  "onSeek",
  "onToggle",
  "onValueChange",
]);

const PASSIVE_CUSTOM_CALLBACKS = new Set([
  "onAbort",
  "onAnimationEnd",
  "onAnimationIteration",
  "onAnimationStart",
  "onBusyChange",
  "onCanPlay",
  "onCanPlayThrough",
  "onComplete",
  "onCompleted",
  "onData",
  "onDuration",
  "onDurationChange",
  "onEmptied",
  "onEnded",
  "onError",
  "onEvent",
  "onFailure",
  "onFrame",
  "onLoad",
  "onLoadedData",
  "onLoadedMetadata",
  "onLoadingChange",
  "onLoadStart",
  "onMessage",
  "onMount",
  "onPause",
  "onPendingChange",
  "onPlay",
  "onPlaying",
  "onPlayingChange",
  "onProgress",
  "onRateChange",
  "onReady",
  "onRender",
  "onSeeked",
  "onSeeking",
  "onStalled",
  "onStart",
  "onStateChange",
  "onStatus",
  "onStatusChange",
  "onSuccess",
  "onSuspend",
  "onTerminalFailure",
  "onTick",
  "onTime",
  "onTimeUpdate",
  "onTransitionCancel",
  "onTransitionEnd",
  "onTransitionRun",
  "onTransitionStart",
  "onUnmount",
  "onUpdate",
  "onVolumeChange",
  "onWaiting",
]);

const PANEL_SETTER_PATTERN = /^set(?:[A-Za-z0-9_$]*(?:Open|Visible|Visibility|Panel|Dialog|Modal|Popover|Menu|Drawer|Sheet)|(?:Show|Showing)[A-Za-z0-9_$]*)$/;
const PANEL_OPEN_SHOW_PATTERN = /^(?:(?:open|show)|(?:on|handle)(?:Open|Show))(?:$|[A-Z0-9_$][A-Za-z0-9_$]*)$/;
const PANEL_TOGGLE_PATTERN = /^(?:toggle|(?:on|handle)Toggle)([A-Za-z0-9_$]*)$/;
const PANEL_TOGGLE_SURFACE_PATTERN = /(?:Agent|Inspector|Media|Keyframes?|Panel|View|Menu|Dialog|Modal|Popover|Drawer|Sheet|Sidebar|Settings|Export|Library|Overlay|Window|Picker|Browser|History|Details|Preferences|Account|About|Fullscreen|Visibility|Visible|Open)/;

function isCustomElement(element) {
  return /^[A-Z]/.test(element) || element.includes(".");
}

function isInteractionHandler(name, element) {
  if (!/^on[A-Z]/.test(name)) return false;
  if (DOM_INTERACTION_HANDLER_PATTERN.test(name) || EXPLICIT_INTERACTION_CALLBACKS.has(name)) {
    return true;
  }
  return isCustomElement(element) && !PASSIVE_CUSTOM_CALLBACKS.has(name);
}

function isPanelTrigger(handler) {
  const identifiers = handler.match(/[A-Za-z_$][A-Za-z0-9_$]*/g) ?? [];
  return identifiers.some((identifier) => {
    if (PANEL_SETTER_PATTERN.test(identifier) || PANEL_OPEN_SHOW_PATTERN.test(identifier)) {
      return true;
    }
    const toggle = identifier.match(PANEL_TOGGLE_PATTERN);
    return toggle !== null && PANEL_TOGGLE_SURFACE_PATTERN.test(toggle[1]);
  });
}

function jsxAttributeValue(attribute, file, ts) {
  const initializer = attribute.initializer;
  if (!initializer) return { raw: "true", readable: "true", staticValue: null };
  const raw = initializer.getText(file);
  if (ts.isStringLiteral(initializer)) {
    return { raw, readable: initializer.text, staticValue: initializer.text };
  }
  const expression = ts.isJsxExpression(initializer) ? initializer.expression : null;
  if (
    expression
    && (
      ts.isStringLiteral(expression)
      || ts.isNoSubstitutionTemplateLiteral(expression)
      || ts.isNumericLiteral(expression)
    )
  ) {
    return { raw, readable: expression.text, staticValue: expression.text };
  }
  return {
    raw,
    readable: raw.replace(/^\{([\s\S]*)\}$/, "$1").replace(/^["']|["']$/g, "").trim(),
    staticValue: null,
  };
}

function directStaticJsxText(node, ts) {
  if (!ts.isJsxElement(node)) return "";
  return node.children
    .flatMap((child) => {
      if (ts.isJsxText(child)) return [child.getText()];
      if (!ts.isJsxExpression(child) || !child.expression) return [];
      if (
        ts.isStringLiteral(child.expression)
        || ts.isNoSubstitutionTemplateLiteral(child.expression)
        || ts.isNumericLiteral(child.expression)
      ) {
        return [child.expression.text];
      }
      return [];
    })
    .map((text) => text.replace(/\s+/g, " ").trim())
    .filter(Boolean)
    .join(" ");
}

export function extractControls(path, source, ts) {
  const normalizedPath = normalizePath(path);
  const file = ts.createSourceFile(
    normalizedPath,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  if (file.parseDiagnostics.length > 0) {
    const diagnostics = file.parseDiagnostics.map((diagnostic) => {
      const position = file.getLineAndCharacterOfPosition(diagnostic.start ?? 0);
      const message = ts.flattenDiagnosticMessageText(diagnostic.messageText, " ");
      return `${normalizedPath}:${position.line + 1}:${position.character + 1}: ${message}`;
    });
    throw new Error(diagnostics.join("\n"));
  }
  const controls = [];

  function visit(node) {
    if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node)) {
      const opening = ts.isJsxElement(node) ? node.openingElement : node;
      const element = opening.tagName.getText(file);
      const attributes = new Map();
      for (const property of opening.attributes.properties) {
        if (!ts.isJsxAttribute(property)) continue;
        attributes.set(
          property.name.getText(file),
          jsxAttributeValue(property, file, ts),
        );
      }
      const handlers = [...attributes.entries()].filter(
        ([name]) => isInteractionHandler(name, element),
      );
      const roleAttribute = attributes.get("role");
      const role = roleAttribute?.readable ?? "";
      const interactiveRole = roleAttribute?.staticValue != null
        && INTERACTIVE_ROLES.has(roleAttribute.staticValue);
      const interactive = MEDIA_ELEMENTS.has(element)
        ? attributes.has("controls") || handlers.length > 0
        : CONTROL_ELEMENTS.has(element) || handlers.length > 0 || interactiveRole;
      if (interactive) {
        const position = file.getLineAndCharacterOfPosition(opening.getStart(file));
        const line = position.line + 1;
        const column = position.character + 1;
        const handler = handlers.map(([, value]) => value.raw).join(" ");
        const label = attributes.get("aria-label")?.readable
          ?? attributes.get("title")?.readable
          ?? directStaticJsxText(node, ts);
        controls.push({
          id: stableId("control", `${normalizedPath}:${line}:${column}:${element}`),
          path: normalizedPath,
          line,
          column,
          order: controls.length + 1,
          element,
          label,
          handler,
          disabled: attributes.get("disabled")?.raw ?? null,
          role,
          panelTrigger: isPanelTrigger(handler),
        });
      }
    }
    ts.forEachChild(node, visit);
  }

  visit(file);
  return controls;
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
    const location = candidate.column == null
      ? `${path}:${candidate.line}`
      : `${path}:${candidate.line}:${candidate.column}`;
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

function createControlRecord(candidate) {
  return {
    id: stableId("control-record", candidate.id),
    candidateId: candidate.id,
    visibility: null,
    enabledWhen: null,
    inputs: [],
    handler: candidate.handler,
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
  };
}

function mergeControlLedger(candidates, existing) {
  assertUniqueCandidates(candidates);
  const existingRecords = existing?.records ?? [];
  if (!Array.isArray(existingRecords)) {
    throw new Error("control ledger must contain a records array");
  }

  const priorByCandidateId = new Map();
  const priorRecordIds = new Set();
  for (const record of existingRecords) {
    if (!record || typeof record.candidateId !== "string") {
      throw new Error("control candidateId must be a string");
    }
    if (priorByCandidateId.has(record.candidateId)) {
      throw new Error(`duplicate control candidateId: ${record.candidateId}`);
    }
    if (typeof record.id !== "string") {
      throw new Error("control record id must be a string");
    }
    if (priorRecordIds.has(record.id)) {
      throw new Error(`duplicate control record id: ${record.id}`);
    }
    priorByCandidateId.set(record.candidateId, record);
    priorRecordIds.add(record.id);
  }

  const records = candidates.map(
    (candidate) => priorByCandidateId.get(candidate.id) ?? createControlRecord(candidate),
  );
  const recordIds = new Set(records.map(({ id }) => id));
  if (recordIds.size !== records.length) {
    throw new Error("generated control record ids are not unique");
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
  if (command === "controls") {
    const require = createRequire(new URL("../web/package.json", import.meta.url));
    const ts = require("typescript");
    const candidates = readTrackedFiles(root)
      .filter((path) => path.startsWith("web/src/") && path.endsWith(".tsx"))
      .flatMap((path) => extractControls(
        path,
        readFileSync(resolve(root, path), "utf8"),
        ts,
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
  if (command === "control-ledger") {
    if (!args.candidates) {
      throw new Error("control-ledger command needs --candidates <path>");
    }
    const candidateLedger = JSON.parse(readFileSync(resolve(args.candidates), "utf8"));
    const existing = existsSync(resolve(args.out))
      ? JSON.parse(readFileSync(resolve(args.out), "utf8"))
      : null;
    return {
      schema: 1,
      records: mergeControlLedger(candidateLedger.candidates, existing),
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
