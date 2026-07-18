import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

const require = createRequire(new URL("../web/package.json", import.meta.url));

export function createControlAuditTools(dependencies) {
  const {
    assertUniqueCandidates,
    compareAuditText,
    DOCUMENT_GAP_GROUPS,
    DOCUMENT_GAP_GROUP_SET,
    gitRead,
    gitText,
    normalizePath,
    normalizeSemanticText,
    pathInsideRoot,
    pushVerificationError,
    readTrackedFiles,
    realPathWithinRoot,
    stableId,
    trackedRegularFile,
    validateImplementationEvidence,
    validateTestEvidence,
  } = dependencies;

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

function lexicalControlOwnerSymbol(node, ts) {
  let current = node.parent;
  while (current) {
    if (
      (ts.isFunctionDeclaration(current) || ts.isClassDeclaration(current))
      && current.name
      && ts.isIdentifier(current.name)
    ) {
      return current.name.text;
    }
    if (ts.isArrowFunction(current) || ts.isFunctionExpression(current)) {
      const declaration = current.parent;
      if (ts.isVariableDeclaration(declaration) && ts.isIdentifier(declaration.name)) {
        return declaration.name.text;
      }
      if (ts.isPropertyAssignment(declaration) && ts.isIdentifier(declaration.name)) {
        return declaration.name.text;
      }
    }
    current = current.parent;
  }
  return null;
}

function extractControls(path, source, ts) {
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
          ownerSymbol: lexicalControlOwnerSymbol(opening, ts),
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
  const ordinals = new Map();
  return controls.map((control) => {
    const semanticKey = JSON.stringify([
      control.path,
      normalizeSemanticText(control.ownerSymbol),
      control.element,
      normalizeSemanticText(control.label),
      normalizeSemanticText(control.role),
      normalizeSemanticText(control.handler),
    ]);
    const semanticOrdinal = (ordinals.get(semanticKey) ?? 0) + 1;
    ordinals.set(semanticKey, semanticOrdinal);
    return {
      id: stableId("control", JSON.stringify([semanticKey, semanticOrdinal])),
      semanticFingerprint: createHash("sha256").update(semanticKey).digest("hex"),
      semanticOrdinal,
      ...control,
    };
  });
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


const CONTROL_REVIEW_METADATA = Object.freeze({
  schemaName: "completion-control-review",
  schemaVersion: 2,
  recordIdDerivation: "stableId('control-record', candidateId)",
  recordFieldTypes: Object.freeze({
    id: "string",
    candidateId: "string",
    candidateLabel: "string",
    semanticName: "string",
    source: "{path:string,line:number,column:number}",
    element: "string",
    visibility: "string",
    enabledWhen: "string",
    inputs: "string[]",
    handler: "string",
    stateTransition: "string",
    backendTrace: "string[]",
    outcomes: "{success:string,pending:string,empty:string,disabled:string,cancel:string,retry:string,failure:string}",
    accessibility: "{focus:string,label:string,shortcut:string}",
    returnPath: "string[]",
    automatedTests: "string[]",
    runtimeEvidence: "string[]",
    status: "complete|incomplete|obsolete|duplicate|contradicted",
    finalDisposition: "string",
    duplicateOf: "candidateId[]",
    acceptanceCriteria: "string[]",
    gapGroup: "legal-gap|null",
    commit: "string|null",
  }),
});

const CONTROL_STATUSES = new Set([
  "complete",
  "incomplete",
  "contradicted",
  "obsolete",
  "duplicate",
]);
const CONTROL_RECORD_FIELDS = Object.keys(CONTROL_REVIEW_METADATA.recordFieldTypes);
const CONTROL_OUTCOME_FIELDS = [
  "success",
  "pending",
  "empty",
  "disabled",
  "cancel",
  "retry",
  "failure",
];
const CONTROL_ACCESSIBILITY_FIELDS = ["focus", "label", "shortcut"];
const CONTROL_ARRAY_FIELDS = [
  "inputs",
  "backendTrace",
  "returnPath",
  "automatedTests",
  "runtimeEvidence",
  "duplicateOf",
  "acceptanceCriteria",
];
const CONTROL_RUNTIME_KINDS = new Set([
  "automated",
  "browser",
  "native",
  "static",
  "not-run",
  "external-limitation",
]);
const CONTROL_RUNTIME_STATUSES = new Set([
  "passed",
  "failed",
  "partial",
  "not-run",
  "blocked",
]);
const CONTROL_RUNTIME_RECEIPT_FIELDS = [
  "id",
  "key",
  "kind",
  "status",
  "evidenceLevel",
  "command",
  "executedCheckoutRevision",
  "result",
  "startedAt",
  "endedAt",
  "exitCode",
  "candidateIds",
  "assertions",
  "artifacts",
  "cleanup",
  "limitations",
  "testEvidence",
];
const CONTROL_PROVENANCE_FIELDS = [
  "auditedProductRevision",
  "verifierRevision",
  "candidateLedgerSha256",
  "candidateSourceAggregateSha256",
  "verifierFilesSha256",
];
const CONTROL_RUNTIME_SOURCE_REVISION_FIELDS = ["commit", "tree"];
const CONTROL_VERIFIER_FILES = [
  "tools/completion-audit.mjs",
  "tools/completion-audit-controls.mjs",
  "tools/completion-audit-plan-map.json",
  "tools/completion-audit.test.mjs",
];
const CONTROL_RUNTIME_RESULT_FIELDS = ["summary", "exitCode"];
const CONTROL_RUNTIME_ASSERTION_FIELDS = [
  "candidateId",
  "event",
  "handler",
  "backend",
  "visibleOutcome",
  "accessibility",
  "returnPath",
  "artifactPaths",
  "testEvidence",
];
const CONTROL_RUNTIME_METADATA_FIELDS = [
  "schemaName",
  "schemaVersion",
  "receiptIdDerivation",
  "evidencePolicy",
];
const CONTROL_RUNTIME_CLEANUP_FIELDS = ["required", "status", "details"];
const CONTROL_RUNTIME_SUMMARY_FIELDS = [
  "receipts",
  "direct",
  "supporting",
  "passed",
  "failed",
  "partial",
  "notRun",
  "blocked",
];

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() !== "";
}

function nonEmptyStringArray(value) {
  return Array.isArray(value) && value.some(nonEmptyString);
}

function normalizedControlExpression(value) {
  return typeof value === "string" ? value.replace(/\s+/g, " ").trim() : value;
}

function isStrictRuntimeTimestamp(value) {
  return nonEmptyString(value)
    && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:\d{2})$/.test(value)
    && !Number.isNaN(Date.parse(value));
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function candidateSourceAggregate(candidates, readSource) {
  const manifest = [...new Set(candidates.map((candidate) => candidate?.path).filter(nonEmptyString))]
    .sort(compareAuditText)
    .map((path) => `${path}\0${sha256(readSource(path))}\n`)
    .join("");
  return sha256(manifest);
}

function expectedControlAcceptanceCriteria(record) {
  const sourcePath = typeof record?.source?.path === "string" ? record.source.path : "";
  const testPath = sourcePath.replace(/(?:\.test)?\.tsx$/, ".interaction.test.tsx");
  const testName = `${String(record?.candidateId ?? "")} ${String(record?.semanticName ?? "")}`;
  const inputs = Array.isArray(record?.inputs) ? record.inputs : [];
  const backendTrace = Array.isArray(record?.backendTrace) ? record.backendTrace : [];
  const accessibility = record?.accessibility && typeof record.accessibility === "object"
    ? record.accessibility
    : {};
  const returnPath = Array.isArray(record?.returnPath) ? record.returnPath : [];
  const outcomes = record?.outcomes && typeof record.outcomes === "object" ? record.outcomes : {};
  return [
    `Candidate: ${String(record?.candidateId ?? "")}.`,
    `Test: ${testPath}#${testName}.`,
    `Initial state: visibility=${String(record?.visibility ?? "")}; enabledWhen=${String(record?.enabledWhen ?? "")}.`,
    `Event: inputs=${JSON.stringify(inputs)}; handler=${String(record?.handler ?? "")}.`,
    `Exact call/state/backend: stateTransition=${String(record?.stateTransition ?? "")}; backendTrace=${JSON.stringify(backendTrace)}.`,
    `Visible/accessibility/return path: success=${String(outcomes.success ?? "")}; accessibility=${JSON.stringify(accessibility)}; returnPath=${JSON.stringify(returnPath)}.`,
    `Outcome matrix: ${JSON.stringify(outcomes)}.`,
  ];
}

function duplicateCount(values) {
  const counts = new Map();
  for (const value of values) {
    if (typeof value !== "string") continue;
    counts.set(value, (counts.get(value) ?? 0) + 1);
  }
  return [...counts.values()].reduce(
    (total, count) => total + Math.max(0, count - 1),
    0,
  );
}


function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

let cachedTypeScript;

function typeScriptCompiler() {
  if (!cachedTypeScript) {
    const require = createRequire(new URL("../web/package.json", import.meta.url));
    cachedTypeScript = require("typescript");
  }
  return cachedTypeScript;
}

function typeScriptSourceFile(path, source) {
  const ts = typeScriptCompiler();
  const kind = path.endsWith(".tsx")
    ? ts.ScriptKind.TSX
    : path.endsWith(".jsx")
      ? ts.ScriptKind.JSX
      : path.endsWith(".js") || path.endsWith(".mjs") || path.endsWith(".cjs")
        ? ts.ScriptKind.JS
        : ts.ScriptKind.TS;
  return ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true, kind);
}


function sourceTestHasAssertion(path, source, name) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  let found = false;
  const isAssertionCall = (node) => {
    if (!ts.isCallExpression(node)) return false;
    if (ts.isIdentifier(node.expression)) {
      return node.expression.text === "expect" || node.expression.text === "assert";
    }
    if (ts.isPropertyAccessExpression(node.expression)) {
      let owner = node.expression.expression;
      while (ts.isPropertyAccessExpression(owner)) owner = owner.expression;
      return ts.isIdentifier(owner) && owner.text === "assert";
    }
    return false;
  };
  const visit = (node) => {
    if (found) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const [first, body] = node.arguments;
      if (
        (node.expression.text === "test" || node.expression.text === "it")
        && first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
        && body
      ) {
        const findAssertion = (child) => {
          if (found) return;
          if (isAssertionCall(child)) {
            found = true;
            return;
          }
          ts.forEachChild(child, findAssertion);
        };
        findAssertion(body);
        return;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return found;
}

function sourceTestUsesVitestExpect(path, source, name) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  const importsExpect = sourceFile.statements.some((statement) => {
    if (
      !ts.isImportDeclaration(statement)
      || !ts.isStringLiteral(statement.moduleSpecifier)
      || statement.moduleSpecifier.text !== "vitest"
    ) return false;
    const bindings = statement.importClause?.namedBindings;
    return Boolean(bindings && ts.isNamedImports(bindings) && bindings.elements.some((element) => (
      (element.propertyName?.text ?? element.name.text) === "expect"
      && element.name.text === "expect"
    )));
  });
  if (!importsExpect) return false;

  let exactCallback = null;
  const findExactTest = (node) => {
    if (exactCallback) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const [first, callback] = node.arguments;
      if (
        ["test", "it"].includes(node.expression.text)
        && first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
        && callback
        && (ts.isArrowFunction(callback) || ts.isFunctionExpression(callback))
      ) {
        exactCallback = callback;
        return;
      }
    }
    ts.forEachChild(node, findExactTest);
  };
  findExactTest(sourceFile);
  if (!exactCallback) return false;
  if (exactCallback.parameters.some((parameter) => (
    ts.isIdentifier(parameter.name) && parameter.name.text === "expect"
  ))) return false;

  let shadowed = false;
  const findShadow = (node) => {
    if (shadowed) return;
    if (
      (ts.isVariableDeclaration(node) || ts.isParameter(node))
      && ts.isIdentifier(node.name)
      && node.name.text === "expect"
    ) {
      shadowed = true;
      return;
    }
    if (
      (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node))
      && node.name?.text === "expect"
    ) {
      shadowed = true;
      return;
    }
    if (
      node !== exactCallback.body
      && (
        ts.isArrowFunction(node)
        || ts.isFunctionExpression(node)
        || ts.isFunctionDeclaration(node)
      )
    ) return;
    ts.forEachChild(node, findShadow);
  };
  findShadow(exactCallback.body);
  return !shadowed;
}

function sourceTestHasUnmodifiedVitestExpect(path, source) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  let valid = true;
  const visit = (node) => {
    if (!valid) return;
    if (ts.isIdentifier(node) && node.text === "expect") {
      const parent = node.parent;
      const imported = ts.isImportSpecifier(parent)
        && parent.name === node
        && (parent.propertyName?.text ?? parent.name.text) === "expect";
      const assertionCall = ts.isCallExpression(parent) && parent.expression === node;
      const guardCall = ts.isPropertyAccessExpression(parent)
        && parent.expression === node
        && ["hasAssertions", "assertions"].includes(parent.name.text)
        && ts.isCallExpression(parent.parent)
        && parent.parent.expression === parent;
      if (!imported && !assertionCall && !guardCall) {
        valid = false;
        return;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return valid;
}

function sourceTestHasTopLevelAssertionGuard(path, source, name, proof) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  let exactBody = null;
  const findExactTest = (node) => {
    if (exactBody) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const [first, callback] = node.arguments;
      if (
        ["test", "it"].includes(node.expression.text)
        && first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
        && callback
        && (ts.isArrowFunction(callback) || ts.isFunctionExpression(callback))
        && ts.isBlock(callback.body)
      ) {
        exactBody = callback.body;
        return;
      }
    }
    ts.forEachChild(node, findExactTest);
  };
  findExactTest(sourceFile);
  if (!exactBody) return false;

  const eventTokens = String(proof?.event ?? "")
    .toLowerCase()
    .split(/[^a-z0-9_$]+/)
    .filter((token) => token.length >= 3);
  const statementContainsCandidateEvent = (statement) => {
    let found = false;
    const visit = (node) => {
      if (found) return;
      if (
        node !== statement
        && (
          ts.isArrowFunction(node)
          || ts.isFunctionExpression(node)
          || ts.isFunctionDeclaration(node)
        )
      ) return;
      if (
        ts.isCallExpression(node)
        && eventTokens.some((token) => node.getText(sourceFile).toLowerCase().includes(token))
      ) {
        found = true;
        return;
      }
      ts.forEachChild(node, visit);
    };
    visit(statement);
    return found;
  };
  const isAssertionGuard = (statement) => {
    if (!ts.isExpressionStatement(statement) || !ts.isCallExpression(statement.expression)) {
      return false;
    }
    const call = statement.expression;
    if (
      !ts.isPropertyAccessExpression(call.expression)
      || !ts.isIdentifier(call.expression.expression)
      || call.expression.expression.text !== "expect"
    ) return false;
    if (call.expression.name.text === "hasAssertions") return call.arguments.length === 0;
    if (call.expression.name.text !== "assertions" || call.arguments.length !== 1) return false;
    const [count] = call.arguments;
    if (!ts.isNumericLiteral(count)) return false;
    const value = Number(count.text);
    return Number.isInteger(value) && value >= 1;
  };

  const statements = exactBody.statements;
  const eventIndex = statements.findIndex(statementContainsCandidateEvent);
  for (let index = 0; index < statements.length; index += 1) {
    const statement = statements[index];
    if (ts.isReturnStatement(statement) || ts.isThrowStatement(statement)) return false;
    if (
      ts.isIfStatement(statement)
      || ts.isSwitchStatement(statement)
      || ts.isTryStatement(statement)
      || ts.isForStatement(statement)
      || ts.isForInStatement(statement)
      || ts.isForOfStatement(statement)
      || ts.isWhileStatement(statement)
      || ts.isDoStatement(statement)
    ) return false;
    if (isAssertionGuard(statement)) return eventIndex === -1 || index < eventIndex;
    if (index === eventIndex) return false;
  }
  return false;
}

function sourceTestHasBoundReachableAssertion(path, source, name, proof) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  const handlerTokens = new Set(
    String(proof?.handler ?? "").match(/[A-Za-z_$][A-Za-z0-9_$]*/g) ?? [],
  );
  const eventTokens = String(proof?.event ?? "")
    .toLowerCase()
    .split(/[^a-z0-9_$]+/)
    .filter((token) => token.length >= 3);
  const isAssertionCall = (node) => {
    if (!ts.isCallExpression(node)) return false;
    if (ts.isIdentifier(node.expression)) {
      return node.expression.text === "expect" || node.expression.text === "assert";
    }
    if (ts.isPropertyAccessExpression(node.expression)) {
      let owner = node.expression.expression;
      while (ts.isPropertyAccessExpression(owner)) owner = owner.expression;
      return ts.isIdentifier(owner) && owner.text === "assert";
    }
    return false;
  };
  const identifiersWithin = (node) => {
    const identifiers = new Set();
    const visit = (child) => {
      if (ts.isIdentifier(child)) identifiers.add(child.text);
      ts.forEachChild(child, visit);
    };
    visit(node);
    return identifiers;
  };
  let exactBody = null;
  const findExactTest = (node) => {
    if (exactBody) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const [first, body] = node.arguments;
      if (
        ["test", "it"].includes(node.expression.text)
        && first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
        && body
        && (ts.isArrowFunction(body) || ts.isFunctionExpression(body))
      ) {
        exactBody = body.body;
        return;
      }
    }
    ts.forEachChild(node, findExactTest);
  };
  findExactTest(sourceFile);
  if (!exactBody) return false;
  const handlerStateIdentifiers = new Set();
  const collectHandlerState = (node) => {
    if (
      ts.isPropertyAssignment(node)
      && ts.isIdentifier(node.name)
      && handlerTokens.has(node.name.text)
      && (ts.isArrowFunction(node.initializer) || ts.isFunctionExpression(node.initializer))
    ) {
      for (const identifier of identifiersWithin(node.initializer.body)) {
        if (!handlerTokens.has(identifier)) handlerStateIdentifiers.add(identifier);
      }
      return;
    }
    ts.forEachChild(node, collectHandlerState);
  };
  collectHandlerState(exactBody);
  const bodyText = exactBody.getText(sourceFile).toLowerCase();
  if (
    handlerTokens.size === 0
    || handlerStateIdentifiers.size === 0
    || eventTokens.length === 0
    || !eventTokens.some((token) => bodyText.includes(token))
  ) return false;
  const statements = ts.isBlock(exactBody) ? exactBody.statements : [exactBody];
  for (const statement of statements) {
    if (ts.isReturnStatement(statement) || ts.isThrowStatement(statement)) break;
    let bound = false;
    const visitReachable = (node) => {
      if (bound) return;
      if (
        node !== statement
        && (
          ts.isArrowFunction(node)
          || ts.isFunctionExpression(node)
          || ts.isFunctionDeclaration(node)
        )
      ) return;
      if (isAssertionCall(node)) {
        const assertedIdentifiers = new Set(
          node.arguments.flatMap((argument) => [...identifiersWithin(argument)]),
        );
        if ([...assertedIdentifiers].some((identifier) => handlerStateIdentifiers.has(identifier))) {
          bound = true;
          return;
        }
      }
      ts.forEachChild(node, visitReachable);
    };
    visitReachable(statement);
    if (bound) return true;
  }
  return false;
}

function sourceOwnerExportKinds(path, source, ownerSymbol) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path) || !nonEmptyString(ownerSymbol)) return new Set();
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  const kinds = new Set();
  const hasModifier = (node, kind) => node.modifiers?.some(
    (modifier) => modifier.kind === kind,
  ) ?? false;
  for (const statement of sourceFile.statements) {
    if (
      (ts.isFunctionDeclaration(statement) || ts.isClassDeclaration(statement))
      && statement.name?.text === ownerSymbol
      && hasModifier(statement, ts.SyntaxKind.ExportKeyword)
    ) {
      kinds.add(hasModifier(statement, ts.SyntaxKind.DefaultKeyword) ? "default" : "named");
    }
    if (
      ts.isVariableStatement(statement)
      && hasModifier(statement, ts.SyntaxKind.ExportKeyword)
      && statement.declarationList.declarations.some(
        (declaration) => ts.isIdentifier(declaration.name) && declaration.name.text === ownerSymbol,
      )
    ) {
      kinds.add("named");
    }
    if (ts.isExportDeclaration(statement) && statement.exportClause && ts.isNamedExports(statement.exportClause)) {
      for (const element of statement.exportClause.elements) {
        const localName = element.propertyName?.text ?? element.name.text;
        if (localName === ownerSymbol && element.name.text === ownerSymbol) kinds.add("named");
      }
    }
    if (
      ts.isExportAssignment(statement)
      && !statement.isExportEquals
      && ts.isIdentifier(statement.expression)
      && statement.expression.text === ownerSymbol
    ) {
      kinds.add("default");
    }
  }
  return kinds;
}

function sourceTestExercisesOwningComponent(
  testPath,
  source,
  name,
  candidatePath,
  ownerSymbol,
  ownerExportKinds,
) {
  if (!/\.(?:[cm]?[jt]sx?)$/.test(testPath)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(testPath, source);
  const candidateWithoutExtension = candidatePath.replace(/\.(?:[cm]?[jt]sx?)$/, "");
  const importedNames = new Set();
  for (const statement of sourceFile.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue;
    const specifier = statement.moduleSpecifier.text;
    if (!specifier.startsWith(".")) continue;
    const resolvedImport = normalizePath(resolve("/", dirname(testPath), specifier).slice(1));
    if (resolvedImport !== candidateWithoutExtension && resolvedImport !== candidatePath) continue;
    const clause = statement.importClause;
    if (clause?.name && ownerExportKinds.has("default")) importedNames.add(clause.name.text);
    if (clause?.namedBindings && ts.isNamedImports(clause.namedBindings)) {
      for (const element of clause.namedBindings.elements) {
        const importedName = element.propertyName?.text ?? element.name.text;
        if (ownerExportKinds.has("named") && importedName === ownerSymbol) {
          importedNames.add(element.name.text);
        }
      }
    }
  }
  if (importedNames.size === 0) return false;
  const unwrapExpression = (node) => {
    let current = node;
    while (
      ts.isParenthesizedExpression(current)
      || ts.isAsExpression(current)
      || ts.isTypeAssertionExpression(current)
      || ts.isNonNullExpression(current)
    ) {
      current = current.expression;
    }
    return current;
  };
  const jsxContainsOwningComponent = (node) => {
    let found = false;
    const visit = (child) => {
      if (found) return;
      if (
        (ts.isJsxOpeningElement(child) || ts.isJsxSelfClosingElement(child))
        && ts.isIdentifier(child.tagName)
        && importedNames.has(child.tagName.text)
      ) {
        found = true;
        return;
      }
      ts.forEachChild(child, visit);
    };
    visit(node);
    return found;
  };
  const isCreateElementCall = (node) => {
    if (!ts.isCallExpression(node)) return false;
    if (ts.isIdentifier(node.expression)) return node.expression.text === "createElement";
    return ts.isPropertyAccessExpression(node.expression)
      && node.expression.name.text === "createElement";
  };
  const renderArgumentContainsOwningComponent = (
    rawNode,
    constInitializers,
    resolving = new Set(),
  ) => {
    const node = unwrapExpression(rawNode);
    if (ts.isIdentifier(node)) {
      if (importedNames.has(node.text) || resolving.has(node.text)) return false;
      const initializer = constInitializers.get(node.text);
      if (!initializer) return false;
      const nextResolving = new Set(resolving);
      nextResolving.add(node.text);
      return renderArgumentContainsOwningComponent(initializer, constInitializers, nextResolving);
    }
    if (ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node)) {
      return jsxContainsOwningComponent(node);
    }
    if (ts.isCallExpression(node)) {
      const callee = unwrapExpression(node.expression);
      if (ts.isIdentifier(callee) && importedNames.has(callee.text)) return true;
      if (!isCreateElementCall(node)) return false;
      const [component, _props, ...children] = node.arguments;
      const owningComponent = component && unwrapExpression(component);
      if (
        owningComponent
        && ts.isIdentifier(owningComponent)
        && importedNames.has(owningComponent.text)
      ) {
        return true;
      }
      return children.some((child) => renderArgumentContainsOwningComponent(
        child,
        constInitializers,
        new Set(resolving),
      ));
    }
    if (ts.isConditionalExpression(node)) {
      return renderArgumentContainsOwningComponent(node.whenTrue, constInitializers, new Set(resolving))
        || renderArgumentContainsOwningComponent(node.whenFalse, constInitializers, new Set(resolving));
    }
    if (ts.isArrayLiteralExpression(node)) {
      return node.elements.some((element) => renderArgumentContainsOwningComponent(
        element,
        constInitializers,
        new Set(resolving),
      ));
    }
    return false;
  };
  let found = false;
  const inspectTestBody = (body) => {
    const executableBody = (
      ts.isArrowFunction(body) || ts.isFunctionExpression(body)
    ) ? body.body : body;
    const constInitializers = new Map();
    if (ts.isBlock(executableBody)) {
      for (const statement of executableBody.statements) {
        if (
          !ts.isVariableStatement(statement)
          || !(statement.declarationList.flags & ts.NodeFlags.Const)
        ) continue;
        for (const declaration of statement.declarationList.declarations) {
          if (ts.isIdentifier(declaration.name) && declaration.initializer) {
            constInitializers.set(declaration.name.text, declaration.initializer);
          }
        }
      }
    }
    let rendered = false;
    const visit = (node) => {
      if (rendered) return;
      if (ts.isCallExpression(node)) {
        const expression = node.expression;
        const [argument] = node.arguments;
        if (
          ts.isIdentifier(expression)
          && ["render", "mount"].includes(expression.text)
          && argument
          && renderArgumentContainsOwningComponent(argument, constInitializers)
        ) {
          rendered = true;
          return;
        }
      }
      ts.forEachChild(node, visit);
    };
    visit(executableBody);
    return rendered;
  };
  const visit = (node) => {
    if (found) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const [first, body] = node.arguments;
      if (
        ["test", "it"].includes(node.expression.text)
        && first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
        && body
        && inspectTestBody(body)
      ) {
        found = true;
        return;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return found;
}


function validateControlTestEvidence(root, trackedPaths, evidence, candidateId, errors) {
  if (!validateTestEvidence(root, trackedPaths, evidence, candidateId, errors)) return false;
  const [, path, name] = /^test:([^#\n]+)#([^#\n]+)$/.exec(evidence);
  const file = trackedRegularFile(root, trackedPaths, path);
  const source = readFileSync(file.real, "utf8");
  if (!name.includes(candidateId) || !sourceTestHasAssertion(path, source, name)) {
    pushVerificationError(
      errors,
      "invalid-control-test-evidence",
      `control test must bind ${candidateId} in its exact name and contain a real assertion: ${evidence}`,
      candidateId,
    );
    return false;
  }
  return true;
}

function validateExecutedControlTestEvidence(
  root,
  trackedPaths,
  evidence,
  candidate,
  proof,
  errors,
) {
  if (!validateControlTestEvidence(root, trackedPaths, evidence, candidate.id, errors)) return null;
  const [, path, name] = /^test:([^#\n]+)#([^#\n]+)$/.exec(evidence);
  const file = trackedRegularFile(root, trackedPaths, path);
  const source = readFileSync(file.real, "utf8");
  const ownerFile = trackedRegularFile(root, trackedPaths, candidate.path);
  const ownerSource = ownerFile.status === "valid" ? readFileSync(ownerFile.real, "utf8") : "";
  const ownerExportKinds = sourceOwnerExportKinds(
    candidate.path,
    ownerSource,
    candidate.ownerSymbol,
  );
  if (
    ownerExportKinds.size === 0
    || !sourceTestExercisesOwningComponent(
      path,
      source,
      name,
      candidate.path,
      candidate.ownerSymbol,
      ownerExportKinds,
    )
  ) {
    pushVerificationError(
      errors,
      "control-test-does-not-exercise-owner",
      `executed control test must import and render its owning component ${candidate.path}: ${evidence}`,
      candidate.id,
    );
    return null;
  }
  if (!sourceTestUsesVitestExpect(path, source, name)) {
    pushVerificationError(
      errors,
      "control-test-missing-vitest-expect",
      `executed control test must use the unshadowed named expect import from Vitest: ${evidence}`,
      candidate.id,
    );
    return null;
  }
  if (!sourceTestHasUnmodifiedVitestExpect(path, source)) {
    pushVerificationError(
      errors,
      "control-test-mutates-vitest-expect",
      `executed control test may use Vitest expect only for assertions and assertion-count guards: ${evidence}`,
      candidate.id,
    );
    return null;
  }
  if (!sourceTestHasTopLevelAssertionGuard(path, source, name, proof)) {
    pushVerificationError(
      errors,
      "control-test-missing-assertion-guard",
      `executed control test needs a reachable top-level expect.hasAssertions() or expect.assertions(N >= 1) before its candidate event: ${evidence}`,
      candidate.id,
    );
    return null;
  }
  if (!sourceTestHasBoundReachableAssertion(path, source, name, proof)) {
    pushVerificationError(
      errors,
      "control-test-assertion-not-bound",
      `executed control test needs a reachable assertion bound to its structured event/handler proof: ${evidence}`,
      candidate.id,
    );
    return null;
  }
  return { path, name };
}


function readControlAuditJson(root, auditDirectory, name, errors) {
  const path = resolve(auditDirectory, name);
  if (!existsSync(path)) {
    pushVerificationError(errors, "missing-audit-file", `${name} is missing`);
    return null;
  }
  const real = realPathWithinRoot(root, path);
  if (!lstatSync(path).isFile() || !real) {
    pushVerificationError(
      errors,
      "audit-file-symlink-escape",
      `${name} must be a real regular file confined to the repository`,
    );
    return null;
  }
  try {
    return JSON.parse(readFileSync(real, "utf8"));
  } catch (error) {
    pushVerificationError(errors, "invalid-audit-file", `${name}: ${error.message}`);
    return null;
  }
}

function hasExactKeys(value, expected) {
  return value
    && typeof value === "object"
    && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...expected].sort());
}


function validateControlProvenance(
  repositoryRoot,
  auditDirectory,
  trackedPaths,
  candidates,
  controlsProvenance,
  runtimeProvenance,
  errors,
) {
  if (
    !hasExactKeys(controlsProvenance, CONTROL_PROVENANCE_FIELDS)
    || !hasExactKeys(runtimeProvenance, CONTROL_PROVENANCE_FIELDS)
  ) {
    pushVerificationError(
      errors,
      "missing-control-provenance",
      "controls.json and runtime-evidence.json need the exact immutable source provenance contract",
    );
    return null;
  }
  if (JSON.stringify(controlsProvenance) !== JSON.stringify(runtimeProvenance)) {
    pushVerificationError(errors, "control-provenance-drift", "controls and runtime provenance differ");
    return null;
  }
  const provenance = controlsProvenance;
  if (
    !hasExactKeys(provenance.auditedProductRevision, CONTROL_RUNTIME_SOURCE_REVISION_FIELDS)
    || !hasExactKeys(provenance.verifierRevision, CONTROL_RUNTIME_SOURCE_REVISION_FIELDS)
    || !/^[0-9a-f]{40}$/.test(provenance.auditedProductRevision.commit)
    || !/^[0-9a-f]{40}$/.test(provenance.auditedProductRevision.tree)
    || !/^[0-9a-f]{40}$/.test(provenance.verifierRevision.commit)
    || !/^[0-9a-f]{40}$/.test(provenance.verifierRevision.tree)
    || !/^[0-9a-f]{64}$/.test(provenance.candidateLedgerSha256)
    || !/^[0-9a-f]{64}$/.test(provenance.candidateSourceAggregateSha256)
    || !hasExactKeys(provenance.verifierFilesSha256, CONTROL_VERIFIER_FILES)
    || Object.values(provenance.verifierFilesSha256).some(
      (digest) => !/^[0-9a-f]{64}$/.test(digest),
    )
  ) {
    pushVerificationError(errors, "invalid-control-provenance", "control provenance contains an invalid digest");
    return null;
  }
  const validateRevision = (label, revision) => {
    try {
      const resolvedCommit = gitText(
        `${label} provenance`,
        repositoryRoot,
        ["rev-parse", "--verify", `${revision.commit}^{commit}`],
      );
      const resolvedTree = gitText(
        `${label} provenance`,
        repositoryRoot,
        ["rev-parse", "--verify", `${revision.commit}^{tree}`],
      );
      execFileSync(
        "git",
        ["-C", repositoryRoot, "merge-base", "--is-ancestor", revision.commit, "HEAD"],
        { stdio: "ignore" },
      );
      if (resolvedCommit !== revision.commit || resolvedTree !== revision.tree) {
        pushVerificationError(
          errors,
          "control-provenance-tree-mismatch",
          `${label} commit/tree binding is invalid`,
        );
        return false;
      }
      return true;
    } catch {
      pushVerificationError(
        errors,
        "invalid-control-provenance-revision",
        `${label} commit must exist and be an ancestor of current HEAD`,
      );
      return false;
    }
  };
  const productRevisionValid = validateRevision(
    "audited product",
    provenance.auditedProductRevision,
  );
  const verifierRevisionValid = validateRevision("verifier", provenance.verifierRevision);
  if (verifierRevisionValid) {
    const head = gitText("control provenance", repositoryRoot, ["rev-parse", "HEAD"]);
    const parent = head === provenance.verifierRevision.commit
      ? head
      : gitText("control provenance", repositoryRoot, ["rev-parse", "HEAD^"]);
    if (parent !== provenance.verifierRevision.commit) {
      pushVerificationError(
        errors,
        "verifier-revision-not-two-stage-parent",
        "verifierRevision must be current HEAD or the direct parent of the evidence commit",
      );
    }
  }
  const candidateLedgerPath = normalizePath(
    relative(repositoryRoot, resolve(auditDirectory, "control-candidates.json")),
  );
  const ledgerFile = trackedRegularFile(repositoryRoot, trackedPaths, candidateLedgerPath);
  if (ledgerFile.status !== "valid") return provenance;
  const currentLedger = readFileSync(ledgerFile.real);
  if (sha256(currentLedger) !== provenance.candidateLedgerSha256) {
    pushVerificationError(errors, "candidate-ledger-hash-mismatch", "candidate ledger differs from audited provenance");
  }
  try {
    const currentAggregate = candidateSourceAggregate(candidates, (path) => {
      const file = trackedRegularFile(repositoryRoot, trackedPaths, path);
      if (file.status !== "valid") throw new Error(`candidate source unavailable: ${path}`);
      return readFileSync(file.real);
    });
    if (currentAggregate !== provenance.candidateSourceAggregateSha256) {
      pushVerificationError(errors, "candidate-source-aggregate-mismatch", "candidate TSX sources differ from audited provenance");
    }
    if (productRevisionValid) {
      const auditedAggregate = candidateSourceAggregate(
        candidates,
        (path) => gitRead(
          "control provenance",
          repositoryRoot,
          ["show", `${provenance.auditedProductRevision.commit}:${path}`],
          null,
        ),
      );
      if (auditedAggregate !== provenance.candidateSourceAggregateSha256) {
        pushVerificationError(errors, "candidate-source-revision-mismatch", "candidate TSX sources were not captured by the audited product revision");
      }
    }
  } catch (error) {
    pushVerificationError(errors, "candidate-source-provenance-unavailable", error.message);
  }
  if (verifierRevisionValid) {
    for (const path of CONTROL_VERIFIER_FILES) {
      const file = trackedRegularFile(repositoryRoot, trackedPaths, path);
      const expectedHash = provenance.verifierFilesSha256[path];
      if (file.status !== "valid" || sha256(readFileSync(file.real)) !== expectedHash) {
        pushVerificationError(errors, "verifier-file-hash-mismatch", `current verifier file differs: ${path}`);
        continue;
      }
      try {
        const revisionHash = sha256(gitRead(
          "control verifier provenance",
          repositoryRoot,
          ["show", `${provenance.verifierRevision.commit}:${path}`],
          null,
        ));
        if (revisionHash !== expectedHash) {
          pushVerificationError(errors, "verifier-file-revision-mismatch", `verifier file was not captured by verifierRevision: ${path}`);
        }
      } catch (error) {
        pushVerificationError(errors, "verifier-file-revision-missing", error.message);
      }
    }
  }
  return provenance;
}

function executeDirectControlTest(repositoryRoot, runner, test, receiptId, candidateId, errors) {
  const fail = (message) => {
    pushVerificationError(
      errors,
      "direct-test-reexecution-failed",
      `${receiptId}: ${message}`,
      candidateId,
    );
    return { receiptId, candidateId, path: test.path, name: test.name, status: "failed" };
  };
  if (
    !hasExactKeys(runner, ["kind", "executable", "argv", "timeoutMs"])
    || !Array.isArray(runner?.argv)
    || runner.argv.some((argument) => typeof argument !== "string")
    || !Number.isInteger(runner?.timeoutMs)
    || runner.timeoutMs < 1_000
    || runner.timeoutMs > 60_000
  ) {
    return fail("direct automated evidence needs an exact allowlisted runner contract");
  }
  const exactPattern = `^${escapeRegExp(test.name)}$`;
  let executable;
  let cwd = repositoryRoot;
  let expectedArgv;
  if (
    runner.kind === "vitest"
    && runner.executable === "web/node_modules/.bin/vitest"
    && test.path.startsWith("web/")
    && /\.(?:[cm]?[jt]sx?)$/.test(test.path)
  ) {
    executable = resolve(repositoryRoot, runner.executable);
    cwd = resolve(repositoryRoot, "web");
    expectedArgv = [
      "run",
      normalizePath(relative(cwd, resolve(repositoryRoot, test.path))),
      "--testNamePattern",
      exactPattern,
      "--reporter=verbose",
    ];
  } else {
    return fail("direct control proof requires the exact allowlisted Vitest runner and test path");
  }
  if (JSON.stringify(runner.argv) !== JSON.stringify(expectedArgv)) {
    return fail("runner argv must select only the exact tracked test and exact test name");
  }
  try {
    const output = execFileSync(executable, runner.argv, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      timeout: runner.timeoutMs,
      maxBuffer: 4 * 1024 * 1024,
    });
    if (!output.includes(test.name)) return fail("runner exited zero without reporting the exact test name");
    return { receiptId, candidateId, path: test.path, name: test.name, status: "passed" };
  } catch (error) {
    const detail = error.stderr?.toString("utf8").trim() || error.message;
    return fail(`exact test rerun failed: ${detail}`);
  }
}

function validateControlRuntimeLedger(
  repositoryRoot,
  auditDirectory,
  trackedPaths,
  ledger,
  candidateIds,
  candidatesById,
  recordsByCandidateId,
  provenance,
  errors,
) {
  const directCandidates = new Map();
  const receiptsById = new Map();
  const directTestExecutions = [];
  const auditRelative = normalizePath(relative(repositoryRoot, auditDirectory)) || ".";
  const expectedCandidateLedger = normalizePath(join(auditRelative, "control-candidates.json"));
  const expectedControlsLedger = normalizePath(join(auditRelative, "controls.json"));
  if (!ledger || ledger.schema !== 1) {
    pushVerificationError(errors, "invalid-runtime-ledger-schema", "runtime-evidence.json schema must equal 1");
    return { receiptsById, directCandidates, directTestExecutions };
  }
  if (
    !hasExactKeys(ledger.metadata, CONTROL_RUNTIME_METADATA_FIELDS)
    || ledger.metadata?.schemaName !== "completion-control-runtime-evidence"
    || ledger.metadata?.schemaVersion !== 1
    || ledger.metadata?.receiptIdDerivation !== "stableId('control-runtime-receipt', key)"
    || !nonEmptyString(ledger.metadata?.evidencePolicy)
  ) {
    pushVerificationError(errors, "invalid-runtime-ledger-metadata", "runtime-evidence.json metadata contract is invalid");
  }
  if (
    !hasExactKeys(ledger, [
      "schema",
      "metadata",
      "provenance",
      "candidateLedger",
      "controlsLedger",
      "summary",
      "receipts",
    ])
    || ledger.candidateLedger !== expectedCandidateLedger
    || ledger.controlsLedger !== expectedControlsLedger
  ) {
    pushVerificationError(errors, "invalid-runtime-ledger-envelope", "runtime-evidence.json envelope or ledger bindings are invalid");
  }
  if (!Array.isArray(ledger.receipts)) {
    pushVerificationError(errors, "invalid-runtime-ledger", "runtime-evidence.json receipts must be an array");
    return { receiptsById, directCandidates, directTestExecutions };
  }
  for (const receipt of ledger.receipts) {
    if (!receipt || typeof receipt !== "object" || Array.isArray(receipt)) {
      pushVerificationError(errors, "invalid-runtime-receipt", "runtime receipt must be an object");
      continue;
    }
    if (!hasExactKeys(receipt, CONTROL_RUNTIME_RECEIPT_FIELDS)) {
      pushVerificationError(errors, "invalid-runtime-receipt-schema", "runtime receipt keys differ from the typed schema");
    }
    const receiptId = typeof receipt.id === "string" ? receipt.id : null;
    const key = nonEmptyString(receipt.key) ? receipt.key : null;
    if (!receiptId || !key || receiptId !== stableId("control-runtime-receipt", key)) {
      pushVerificationError(errors, "runtime-receipt-id-mismatch", "runtime receipt id must derive from its non-empty key");
      continue;
    }
    if (receiptsById.has(receiptId)) {
      pushVerificationError(errors, "duplicate-runtime-receipt-id", `duplicate runtime receipt: ${receiptId}`);
      continue;
    }
    receiptsById.set(receiptId, receipt);
    if (!CONTROL_RUNTIME_KINDS.has(receipt.kind)) {
      pushVerificationError(errors, "invalid-runtime-kind", `unsupported runtime kind: ${String(receipt.kind)}`);
    }
    if (!CONTROL_RUNTIME_STATUSES.has(receipt.status)) {
      pushVerificationError(errors, "invalid-runtime-status", `unsupported runtime status: ${String(receipt.status)}`);
    }
    if (!["supporting", "direct"].includes(receipt.evidenceLevel)) {
      pushVerificationError(errors, "invalid-runtime-evidence-level", `runtime receipt ${receiptId} needs supporting or direct evidenceLevel`);
    }
    for (const field of ["candidateIds", "assertions", "artifacts", "limitations", "testEvidence"]) {
      if (!Array.isArray(receipt[field])) {
        pushVerificationError(errors, "invalid-runtime-receipt-field", `${receiptId}: ${field} must be an array`);
      }
    }
    const directAutomated = receipt.evidenceLevel === "direct" && receipt.kind === "automated";
    if (
      (!directAutomated && receipt.command != null && typeof receipt.command !== "string")
      || (directAutomated && (!receipt.command || typeof receipt.command !== "object" || Array.isArray(receipt.command)))
    ) {
      pushVerificationError(
        errors,
        "invalid-runtime-receipt-field",
        `${receiptId}: supporting command must be string|null and direct automated command must be a structured runner`,
      );
    }
    for (const field of ["startedAt", "endedAt"]) {
      if (!isStrictRuntimeTimestamp(receipt[field])) {
        pushVerificationError(errors, "invalid-runtime-timestamp", `${receiptId}: ${field} must be an ISO-8601 timestamp with a timezone`);
      }
    }
    if (
      isStrictRuntimeTimestamp(receipt.startedAt)
      && isStrictRuntimeTimestamp(receipt.endedAt)
      && Date.parse(receipt.endedAt) < Date.parse(receipt.startedAt)
    ) {
      pushVerificationError(errors, "runtime-timestamp-order", `${receiptId}: endedAt precedes startedAt`);
    }
    const futureLimit = Date.now() + (5 * 60 * 1000);
    if (
      (isStrictRuntimeTimestamp(receipt.startedAt) && Date.parse(receipt.startedAt) > futureLimit)
      || (isStrictRuntimeTimestamp(receipt.endedAt) && Date.parse(receipt.endedAt) > futureLimit)
    ) {
      pushVerificationError(errors, "runtime-timestamp-in-future", `${receiptId}: receipt timestamp is more than five minutes in the future`);
    }
    if (receipt.exitCode != null && !Number.isInteger(receipt.exitCode)) {
      pushVerificationError(errors, "invalid-runtime-receipt-field", `${receiptId}: exitCode must be integer|null`);
    }
    const executedCheckoutRevision = receipt.executedCheckoutRevision;
    const expectedExecutionRevision = receipt.kind === "automated"
      ? provenance?.verifierRevision
      : provenance?.auditedProductRevision;
    const result = receipt.result;
    const executed = ["automated", "browser", "native"].includes(receipt.kind)
      && ["passed", "partial"].includes(receipt.status);
    if (
      executed
      && (
        !(directAutomated
          ? receipt.command && typeof receipt.command === "object" && !Array.isArray(receipt.command)
          : nonEmptyString(receipt.command))
        || !hasExactKeys(executedCheckoutRevision, CONTROL_RUNTIME_SOURCE_REVISION_FIELDS)
        || executedCheckoutRevision.commit !== expectedExecutionRevision?.commit
        || executedCheckoutRevision.tree !== expectedExecutionRevision?.tree
        || !hasExactKeys(result, CONTROL_RUNTIME_RESULT_FIELDS)
        || !nonEmptyString(result.summary)
        || result.exitCode !== receipt.exitCode
        || !Array.isArray(receipt.artifacts)
        || receipt.artifacts.length === 0
      )
    ) {
      pushVerificationError(
        errors,
        "runtime-execution-record-incomplete",
        `${receiptId}: passed/partial execution needs command, the exact executedCheckoutRevision, result, and at least one process/capture artifact`,
      );
    }
    const cleanup = receipt.cleanup;
    if (
      !hasExactKeys(cleanup, CONTROL_RUNTIME_CLEANUP_FIELDS)
      || typeof cleanup.required !== "boolean"
      || !["not-required", "verified"].includes(cleanup.status)
      || !nonEmptyStringArray(cleanup.details)
      || cleanup.details.some((entry) => !nonEmptyString(entry))
      || (cleanup.required && cleanup.status !== "verified")
      || (!cleanup.required && cleanup.status !== "not-required")
    ) {
      pushVerificationError(errors, "invalid-runtime-cleanup", `${receiptId}: cleanup needs exact required/status/details fields and a consistent status`);
    }
    if (
      ["browser", "native"].includes(receipt.kind)
      && (!cleanup?.required || cleanup?.status !== "verified")
    ) {
      pushVerificationError(errors, "runtime-cleanup-unverified", `${receiptId}: browser/native receipt cleanup must be required and verified`);
    }
    const receiptCandidateIds = Array.isArray(receipt.candidateIds) ? receipt.candidateIds : [];
    if (
      receiptCandidateIds.some((candidateId) => !nonEmptyString(candidateId))
      || new Set(receiptCandidateIds).size !== receiptCandidateIds.length
    ) {
      pushVerificationError(errors, "invalid-runtime-candidate-ids", `${receiptId}: candidateIds must be unique non-empty strings`);
    }
    for (const candidateId of receiptCandidateIds) {
      if (!candidateIds.has(candidateId)) {
        pushVerificationError(errors, "orphan-runtime-candidate-id", `${receiptId}: unknown candidateId ${candidateId}`, candidateId);
      }
    }
    const assertions = Array.isArray(receipt.assertions) ? receipt.assertions : [];
    for (const assertion of assertions) {
      if (
        !hasExactKeys(assertion, CONTROL_RUNTIME_ASSERTION_FIELDS)
        || !nonEmptyString(assertion.candidateId)
        || !nonEmptyString(assertion.event)
        || !nonEmptyString(assertion.handler)
        || !nonEmptyString(assertion.backend)
        || !nonEmptyString(assertion.visibleOutcome)
        || !nonEmptyString(assertion.accessibility)
        || !nonEmptyString(assertion.returnPath)
        || !Array.isArray(assertion.artifactPaths)
        || assertion.artifactPaths.some((path) => !nonEmptyString(path))
        || !Array.isArray(assertion.testEvidence)
        || assertion.testEvidence.some((evidence) => !nonEmptyString(evidence))
      ) {
        pushVerificationError(errors, "invalid-runtime-assertion", `${receiptId}: assertion contract is incomplete`);
      } else if (!receiptCandidateIds.includes(assertion.candidateId)) {
        pushVerificationError(errors, "runtime-assertion-candidate-mismatch", `${receiptId}: assertion candidate is not listed`, assertion.candidateId);
      }
    }
    const artifacts = Array.isArray(receipt.artifacts) ? receipt.artifacts : [];
    const artifactsByPath = new Map();
    for (const artifact of artifacts) {
      if (
        !artifact
        || typeof artifact !== "object"
        || !hasExactKeys(artifact, ["path", "availability", "sha256"])
        || !nonEmptyString(artifact.path)
      ) {
        pushVerificationError(errors, "invalid-runtime-artifact", `${receiptId}: artifact needs path, availability, and sha256`);
        continue;
      }
      if (!/^[0-9a-f]{64}$/.test(artifact.sha256)) {
        pushVerificationError(errors, "runtime-artifact-hash-required", `${receiptId}: artifact needs a lowercase sha256 digest: ${artifact.path}`);
        continue;
      }
      const inside = pathInsideRoot(repositoryRoot, artifact.path);
      if (!inside) {
        pushVerificationError(errors, "runtime-artifact-path-escape", `${receiptId}: artifact path escapes repository`);
        continue;
      }
      if (artifact.availability !== "tracked") {
        pushVerificationError(
          errors,
          "runtime-artifact-availability-mismatch",
          `${receiptId}: every declared artifact must be tracked: ${artifact.path}`,
        );
      }
      const file = trackedRegularFile(repositoryRoot, trackedPaths, artifact.path);
      if (file.status === "missing") {
        pushVerificationError(errors, "runtime-artifact-missing", `${receiptId}: artifact is missing: ${artifact.path}`);
        continue;
      }
      if (file.status !== "valid") {
        pushVerificationError(errors, "runtime-artifact-not-tracked", `${receiptId}: artifact is not a tracked confined regular file: ${artifact.path}`);
        continue;
      }
      const digest = sha256(readFileSync(file.real));
      if (artifact.sha256 !== digest) {
        pushVerificationError(errors, "runtime-artifact-hash-mismatch", `${receiptId}: artifact hash drifted: ${artifact.path}`);
        continue;
      }
      if (artifactsByPath.has(inside.normalized)) {
        pushVerificationError(errors, "duplicate-runtime-artifact", `${receiptId}: duplicate artifact path: ${artifact.path}`);
      } else {
        artifactsByPath.set(inside.normalized, artifact);
      }
    }
    for (const assertion of assertions) {
      if (!Array.isArray(assertion?.artifactPaths)) continue;
      for (const path of assertion.artifactPaths) {
        if (!artifactsByPath.has(normalizePath(path))) {
          pushVerificationError(
            errors,
            "runtime-assertion-artifact-mismatch",
            `${receiptId}: assertion references an undeclared or invalid artifact: ${path}`,
            assertion.candidateId,
          );
        }
      }
    }
    const testEvidence = Array.isArray(receipt.testEvidence) ? receipt.testEvidence : [];
    for (const field of ["limitations", "testEvidence"]) {
      if (Array.isArray(receipt[field]) && receipt[field].some((entry) => !nonEmptyString(entry))) {
        pushVerificationError(errors, "invalid-runtime-receipt-field", `${receiptId}: ${field} entries must be non-empty strings`);
      }
    }
    const limitations = Array.isArray(receipt.limitations) ? receipt.limitations : [];
    if (
      artifacts.some((artifact) => artifact?.availability === "tracked")
      && limitations.some((limitation) => (
        nonEmptyString(limitation)
        && /\ball\s+(?:browser\s+)?artifacts\s+are\s+local[- ]ignored\s+files\b/i.test(limitation)
      ))
    ) {
      pushVerificationError(
        errors,
        "runtime-artifact-limitation-contradiction",
        `${receiptId}: limitations claim all artifacts are local ignored files while the receipt declares tracked artifacts`,
      );
    }
    const passedCountMatch = nonEmptyString(result?.summary)
      ? result.summary.match(/\b(\d+)\s+(?:audit\s+)?tests?\s+passed\b/i)
      : null;
    if (passedCountMatch) {
      const passedCount = Number.parseInt(passedCountMatch[1], 10);
      for (const limitation of limitations) {
        const limitationCountMatch = nonEmptyString(limitation)
          ? limitation.match(/\b(\d+)\s+passing\s+tests?\b/i)
          : null;
        if (
          limitationCountMatch
          && Number.parseInt(limitationCountMatch[1], 10) !== passedCount
        ) {
          pushVerificationError(
            errors,
            "runtime-limitation-count-mismatch",
            `${receiptId}: limitation test count does not match result.summary`,
          );
        }
      }
    }
    const validatedTestEvidence = new Set();
    for (const evidence of testEvidence) {
      if (validateTestEvidence(
        repositoryRoot,
        trackedPaths,
        evidence,
        receiptCandidateIds[0] ?? null,
        errors,
      )) validatedTestEvidence.add(evidence);
    }
    if (receipt.kind === "automated" && receipt.status === "passed" && receipt.exitCode !== 0) {
      pushVerificationError(errors, "runtime-command-exit-mismatch", `${receiptId}: passed automated receipt needs exitCode 0`);
    }
    if (["not-run", "external-limitation"].includes(receipt.kind) && receipt.evidenceLevel === "direct") {
      pushVerificationError(errors, "nonexecution-marked-direct", `${receiptId}: non-execution receipt cannot be direct evidence`);
    }
    if (receipt.evidenceLevel === "direct") {
      const directKind = ["automated", "browser", "native"].includes(receipt.kind);
      const directVitestRunner = receipt.kind !== "automated" || (
        receipt.command?.kind === "vitest"
        && receipt.command?.executable === "web/node_modules/.bin/vitest"
      );
      const everyCandidateAsserted = receiptCandidateIds.length > 0 && receiptCandidateIds.every(
        (candidateId) => assertions.filter((assertion) => assertion?.candidateId === candidateId).length === 1,
      );
      let assertionsValid = everyCandidateAsserted && assertions.length === receiptCandidateIds.length;
      if (!directVitestRunner) {
        assertionsValid = false;
        pushVerificationError(
          errors,
          "direct-runner-not-vitest",
          `${receiptId}: direct automated control proof requires the exact Vitest runner; node:test remains supporting only`,
        );
      }
      for (const candidateId of receiptCandidateIds) {
        if (!nonEmptyString(candidatesById.get(candidateId)?.ownerSymbol)) {
          assertionsValid = false;
          pushVerificationError(
            errors,
            "direct-evidence-for-ownerless-fixture",
            `${receiptId}: direct evidence cannot promote an ownerless test-fixture candidate`,
            candidateId,
          );
        }
      }
      const claimedArtifactPaths = new Set();
      for (const assertion of assertions) {
        const record = recordsByCandidateId.get(assertion?.candidateId);
        if (!record) {
          assertionsValid = false;
          continue;
        }
        const recordContractValid = Array.isArray(record.inputs)
          && Array.isArray(record.backendTrace)
          && record.outcomes
          && typeof record.outcomes === "object"
          && record.accessibility
          && typeof record.accessibility === "object"
          && Array.isArray(record.returnPath);
        if (!recordContractValid) {
          assertionsValid = false;
          pushVerificationError(
            errors,
            "direct-assertion-contract-mismatch",
            `${receiptId}: direct assertion references a malformed control contract`,
            assertion.candidateId,
          );
          continue;
        }
        const expectedAccessibility = `focus=${record.accessibility.focus}; label=${record.accessibility.label}; shortcut=${record.accessibility.shortcut}`;
        const contractMatches = assertion.event === record.inputs.join(" + ")
          && normalizedControlExpression(assertion.handler) === normalizedControlExpression(record.handler)
          && assertion.backend === record.backendTrace.join(" -> ")
          && assertion.visibleOutcome === record.outcomes.success
          && assertion.accessibility === expectedAccessibility
          && assertion.returnPath === record.returnPath.join(" -> ");
        if (!contractMatches) {
          assertionsValid = false;
          pushVerificationError(
            errors,
            "direct-assertion-contract-mismatch",
            `${receiptId}: direct assertion does not bind the record's event, handler/backend, visible outcome, accessibility, and return path`,
            assertion.candidateId,
          );
        }
        if (receipt.kind === "automated") {
          if (!directVitestRunner) continue;
          const exactTests = Array.isArray(assertion.testEvidence) ? assertion.testEvidence : [];
          const exactArtifacts = Array.isArray(assertion.artifactPaths) ? assertion.artifactPaths : [];
          if (exactTests.length !== 1) {
            assertionsValid = false;
          } else {
            const test = validateExecutedControlTestEvidence(
              repositoryRoot,
              trackedPaths,
              exactTests[0],
              candidatesById.get(record.candidateId) ?? {
                id: record.candidateId,
                path: record.source.path,
                ownerSymbol: null,
              },
              assertion,
              errors,
            );
            if (
              !test
              || !validatedTestEvidence.has(exactTests[0])
              || !exactArtifacts.includes(test.path)
              || !artifactsByPath.has(normalizePath(test.path))
            ) {
              assertionsValid = false;
              pushVerificationError(
                errors,
                "direct-automated-test-not-bound",
                `${receiptId}: each automated assertion needs an executed exact owning-component test and its tracked file hash`,
                assertion.candidateId,
              );
            } else {
              const execution = executeDirectControlTest(
                repositoryRoot,
                receipt.command,
                test,
                receiptId,
                assertion.candidateId,
                errors,
              );
              directTestExecutions.push(execution);
              if (execution.status !== "passed") assertionsValid = false;
            }
          }
        } else {
          const exactArtifacts = Array.isArray(assertion.artifactPaths) ? assertion.artifactPaths : [];
          const candidateBound = exactArtifacts.length > 0 && exactArtifacts.every((path) => (
            path.includes(assertion.candidateId)
            && artifactsByPath.has(normalizePath(path))
            && !claimedArtifactPaths.has(normalizePath(path))
          ));
          if (!candidateBound || (Array.isArray(assertion.testEvidence) && assertion.testEvidence.length !== 0)) {
            assertionsValid = false;
            pushVerificationError(
              errors,
              "direct-assertion-artifact-not-candidate-bound",
              `${receiptId}: each browser/native assertion needs its own candidate-named tracked artifact`,
              assertion.candidateId,
            );
          }
          for (const path of exactArtifacts) claimedArtifactPaths.add(normalizePath(path));
        }
      }
      if (receipt.status !== "passed" || !directKind || !everyCandidateAsserted || !assertionsValid) {
        pushVerificationError(
          errors,
          "invalid-direct-runtime-evidence",
          `${receiptId}: direct evidence needs passed execution and one fully bound, independently reproducible assertion per candidate`,
        );
      } else {
        if (receipt.kind === "automated") {
          for (const candidateId of receiptCandidateIds) {
            if (!directCandidates.has(candidateId)) directCandidates.set(candidateId, new Set());
            directCandidates.get(candidateId).add(receiptId);
          }
        }
      }
    }
  }
  const computedSummary = {
    receipts: ledger.receipts.length,
    direct: ledger.receipts.filter((receipt) => receipt?.evidenceLevel === "direct").length,
    supporting: ledger.receipts.filter((receipt) => receipt?.evidenceLevel === "supporting").length,
    passed: ledger.receipts.filter((receipt) => receipt?.status === "passed").length,
    failed: ledger.receipts.filter((receipt) => receipt?.status === "failed").length,
    partial: ledger.receipts.filter((receipt) => receipt?.status === "partial").length,
    notRun: ledger.receipts.filter((receipt) => receipt?.status === "not-run").length,
    blocked: ledger.receipts.filter((receipt) => receipt?.status === "blocked").length,
  };
  if (
    !hasExactKeys(ledger.summary, CONTROL_RUNTIME_SUMMARY_FIELDS)
    || JSON.stringify(ledger.summary) !== JSON.stringify(computedSummary)
  ) {
    pushVerificationError(errors, "runtime-summary-drift", "runtime-evidence.json summary does not match receipts");
  }
  return { receiptsById, directCandidates, directTestExecutions };
}

function verifyControlAudit(root, auditDir) {
  const requestedRepositoryRoot = resolve(root);
  const requestedAuditDirectory = resolve(requestedRepositoryRoot, auditDir);
  const repositoryRoot = realpathSync(requestedRepositoryRoot);
  const errors = [];
  const auditRelative = relative(requestedRepositoryRoot, requestedAuditDirectory);
  const auditOutsideRoot = auditRelative === ".."
    || auditRelative.startsWith(`..${sep}`)
    || isAbsolute(auditRelative);
  const auditDirectory = auditOutsideRoot
    ? requestedAuditDirectory
    : resolve(repositoryRoot, auditRelative);
  if (auditOutsideRoot) {
    pushVerificationError(errors, "audit-directory-outside-root", "audit directory must remain inside the repository root");
  }
  let auditSymlinkEscape = false;
  if (!auditOutsideRoot && existsSync(auditDirectory) && !realPathWithinRoot(repositoryRoot, auditDirectory)) {
    auditSymlinkEscape = true;
    pushVerificationError(errors, "audit-directory-symlink-escape", "audit directory resolves outside the repository root");
  }
  const candidatesLedger = auditOutsideRoot || auditSymlinkEscape
    ? null
    : readControlAuditJson(repositoryRoot, auditDirectory, "control-candidates.json", errors);
  const controlsLedger = auditOutsideRoot || auditSymlinkEscape
    ? null
    : readControlAuditJson(repositoryRoot, auditDirectory, "controls.json", errors);
  const runtimeLedger = auditOutsideRoot || auditSymlinkEscape
    ? null
    : readControlAuditJson(repositoryRoot, auditDirectory, "runtime-evidence.json", errors);
  let trackedPaths = new Set();
  try {
    trackedPaths = new Set(readTrackedFiles(repositoryRoot));
  } catch (error) {
    pushVerificationError(errors, "repository-index-unavailable", error.message);
  }
  for (const name of ["control-candidates.json", "controls.json", "runtime-evidence.json"]) {
    const relativePath = normalizePath(relative(repositoryRoot, resolve(auditDirectory, name)));
    if (!trackedPaths.has(relativePath)) {
      pushVerificationError(errors, "audit-file-untracked", `${name} must be tracked`);
    }
  }

  if (candidatesLedger?.schema !== 1 || !Array.isArray(candidatesLedger?.candidates)) {
    pushVerificationError(errors, "invalid-candidate-ledger", "control-candidates.json needs schema 1 and candidates[]");
  }
  const candidates = Array.isArray(candidatesLedger?.candidates) ? candidatesLedger.candidates : [];
  if (
    controlsLedger?.schema !== 2
    || !Array.isArray(controlsLedger?.records)
    || !hasExactKeys(controlsLedger, [
      "schema",
      "metadata",
      "provenance",
      "scope",
      "counts",
      "gapCounts",
      "keyFindings",
      "codeEvidence",
      "records",
    ])
  ) {
    pushVerificationError(errors, "invalid-control-ledger-schema", "controls.json must use the exact schema-2 envelope");
  }
  if (JSON.stringify(controlsLedger?.metadata) !== JSON.stringify(CONTROL_REVIEW_METADATA)) {
    pushVerificationError(errors, "invalid-control-ledger-metadata", "controls.json metadata differs from the common control schema");
  }
  const records = Array.isArray(controlsLedger?.records) ? controlsLedger.records : [];
  const currentCandidates = [...trackedPaths]
    .filter((path) => path.startsWith("web/src/") && path.endsWith(".tsx"))
    .sort(compareAuditText)
    .flatMap((path) => extractControls(
      path,
      readFileSync(resolve(repositoryRoot, path), "utf8"),
      typeScriptCompiler(),
    ));
  if (JSON.stringify(candidates) !== JSON.stringify(currentCandidates)) {
    pushVerificationError(
      errors,
      "control-candidate-ledger-drift",
      "control-candidates.json differs from a full current-source re-extraction",
    );
  }
  const candidateIds = candidates.map((candidate) => candidate?.id).filter((id) => typeof id === "string");
  const recordIds = records.map((record) => record?.id).filter((id) => typeof id === "string");
  const recordCandidateIds = records.map((record) => record?.candidateId).filter((id) => typeof id === "string");
  if (duplicateCount(candidateIds)) pushVerificationError(errors, "duplicate-candidate-definition-id", "control candidates contain duplicate IDs");
  if (duplicateCount(recordIds)) pushVerificationError(errors, "duplicate-record-id", "controls contain duplicate record IDs");
  if (duplicateCount(recordCandidateIds)) pushVerificationError(errors, "duplicate-candidate-id", "controls contain duplicate candidateIds");
  const provenance = validateControlProvenance(
    repositoryRoot,
    auditDirectory,
    trackedPaths,
    candidates,
    controlsLedger?.provenance,
    runtimeLedger?.provenance,
    errors,
  );
  const codeEvidence = controlsLedger?.codeEvidence;
  if (!codeEvidence || typeof codeEvidence !== "object" || Array.isArray(codeEvidence)) {
    pushVerificationError(errors, "invalid-control-code-evidence", "controls.json codeEvidence must be an object");
  } else {
    for (const [evidence, expectedHash] of Object.entries(codeEvidence)) {
      if (!validateImplementationEvidence(repositoryRoot, trackedPaths, evidence, null, errors)) continue;
      const [, path] = /^code:([^#\n]+)#/.exec(evidence);
      const file = trackedRegularFile(repositoryRoot, trackedPaths, path);
      const currentHash = sha256(readFileSync(file.real));
      if (!/^[0-9a-f]{64}$/.test(expectedHash) || expectedHash !== currentHash) {
        pushVerificationError(errors, "backend-source-hash-mismatch", `backend source hash drifted: ${evidence}`);
      }
      if (provenance) {
        try {
          const auditedHash = sha256(gitRead(
            "control code evidence",
            repositoryRoot,
            ["show", `${provenance.auditedProductRevision.commit}:${path}`],
            null,
          ));
          if (expectedHash !== auditedHash) {
            pushVerificationError(errors, "backend-source-revision-mismatch", `backend source was not captured by audited commit: ${evidence}`);
          }
        } catch (error) {
          pushVerificationError(errors, "backend-source-revision-missing", error.message);
        }
      }
    }
  }

  const candidatesById = new Map();
  const recordsByCandidateId = new Map(records.map((record) => [record?.candidateId, record]));
  const derivedByPath = new Map();
  for (const candidate of candidates) {
    const candidateId = typeof candidate?.id === "string" ? candidate.id : null;
    const record = candidateId ? recordsByCandidateId.get(candidateId) : null;
    const ownerlessTestFixture = candidate?.ownerSymbol === null
      && /(?:^|\/)(?:__tests__|fixtures?)(?:\/|$)|\.(?:test|spec)\.[cm]?[jt]sx?$/.test(candidate?.path ?? "")
      && record?.status === "obsolete"
      && [record?.semanticName, record?.finalDisposition].some((value) => (
        nonEmptyString(value) && /\btest(?:-only| fixture)\b/i.test(value)
      ));
    if (!hasExactKeys(candidate, [
      "id", "semanticFingerprint", "semanticOrdinal", "ownerSymbol", "path", "line", "column",
      "order", "element", "label", "handler", "disabled", "role", "panelTrigger",
    ])) {
      pushVerificationError(errors, "invalid-candidate", "control candidate keys differ from semantic identity schema", candidateId);
    }
    if (
      !candidateId
      || !nonEmptyString(candidate.path)
      || !Number.isInteger(candidate.line)
      || !Number.isInteger(candidate.column)
      || !nonEmptyString(candidate.element)
      || (!nonEmptyString(candidate.ownerSymbol) && !ownerlessTestFixture)
      || typeof candidate.handler !== "string"
    ) {
      pushVerificationError(
        errors,
        "invalid-candidate",
        "control candidate needs id/path/line/column/element/lexical ownerSymbol/handler; null owner is limited to explicitly obsolete test fixtures",
        candidateId,
      );
      continue;
    }
    candidatesById.set(candidateId, candidate);
    const sourceFile = trackedRegularFile(repositoryRoot, trackedPaths, candidate.path);
    if (sourceFile.status !== "valid") {
      pushVerificationError(errors, "candidate-source-invalid", `candidate source is not a tracked regular file: ${candidate.path}`, candidateId);
      continue;
    }
    if (!derivedByPath.has(candidate.path)) {
      derivedByPath.set(
        candidate.path,
        extractControls(candidate.path, readFileSync(sourceFile.real, "utf8"), typeScriptCompiler()),
      );
    }
    const derived = derivedByPath.get(candidate.path).find((item) => item.id === candidate.id);
    if (!derived) {
      pushVerificationError(errors, "candidate-source-signal-missing", `control is absent at ${candidate.path}:${candidate.line}:${candidate.column}`, candidateId);
      const semanticDrift = derivedByPath.get(candidate.path).find(
        (item) => item.line === candidate.line && item.column === candidate.column,
      );
      if (semanticDrift) {
        pushVerificationError(errors, "candidate-field-drift", "control at the cited location has a different semantic identity", candidateId);
      }
      continue;
    }
    for (const field of ["path", "line", "column", "element", "ownerSymbol", "handler", "label", "disabled", "role", "panelTrigger", "semanticFingerprint", "semanticOrdinal", "id"]) {
      if (candidate[field] !== derived[field]) {
        pushVerificationError(errors, "candidate-field-drift", `current source changed candidate field ${field}`, candidateId);
      }
    }
  }

  const candidateIdSet = new Set(candidateIds);
  const recordCandidateIdSet = new Set(recordCandidateIds);
  const missingCandidateIds = candidateIds.filter((candidateId) => !recordCandidateIdSet.has(candidateId));
  const orphanCandidateIds = [...new Set(recordCandidateIds)].filter((candidateId) => !candidateIdSet.has(candidateId));
  for (const candidateId of missingCandidateIds) pushVerificationError(errors, "missing-candidate-id", "candidate has no control record", candidateId);
  for (const candidateId of orphanCandidateIds) pushVerificationError(errors, "orphan-candidate-id", "control record has no candidate", candidateId);
  if (
    records.length === candidates.length
    && records.some((record, index) => record?.candidateId !== candidates[index]?.id)
  ) {
    pushVerificationError(errors, "control-record-order-drift", "controls records must preserve candidate ledger order");
  }

  const { receiptsById, directCandidates, directTestExecutions } = validateControlRuntimeLedger(
    repositoryRoot,
    auditDirectory,
    trackedPaths,
    runtimeLedger,
    candidateIdSet,
    candidatesById,
    recordsByCandidateId,
    provenance,
    errors,
  );
  const statusCounts = {
    unverified: 0,
    complete: 0,
    incomplete: 0,
    obsolete: 0,
    duplicate: 0,
    contradicted: 0,
  };
  for (const record of records) {
    const candidateId = typeof record?.candidateId === "string" ? record.candidateId : null;
    const candidate = candidateId ? candidatesById.get(candidateId) : null;
    if (!record || typeof record !== "object" || Array.isArray(record)) {
      pushVerificationError(errors, "invalid-record", "control record must be an object");
      continue;
    }
    if (!hasExactKeys(record, CONTROL_RECORD_FIELDS)) {
      pushVerificationError(errors, "invalid-record-schema", "control record keys differ from common schema", candidateId);
    }
    if (!candidateId || record.id !== stableId("control-record", candidateId)) {
      pushVerificationError(errors, "record-id-mismatch", "control record id must derive from candidateId", candidateId);
    }
    for (const field of CONTROL_ARRAY_FIELDS) {
      if (!Array.isArray(record[field]) || record[field].some((item) => !nonEmptyString(item))) {
        pushVerificationError(errors, "invalid-field-type", `${field} must be an array of non-empty strings`, candidateId);
      }
    }
    for (const field of ["semanticName", "visibility", "enabledWhen", "handler", "stateTransition", "finalDisposition"]) {
      if (!nonEmptyString(record[field])) pushVerificationError(errors, "invalid-field-type", `${field} must be a non-empty string`, candidateId);
    }
    if (typeof record.candidateLabel !== "string" || typeof record.element !== "string") {
      pushVerificationError(errors, "invalid-field-type", "candidateLabel and element must be strings", candidateId);
    }
    if (record.commit != null && !/^[0-9a-f]{40}$/.test(record.commit)) {
      pushVerificationError(errors, "invalid-field-type", "commit must be a full SHA or null", candidateId);
    }
    if (!hasExactKeys(record.source, ["path", "line", "column"])) {
      pushVerificationError(errors, "invalid-field-type", "source needs exact path/line/column keys", candidateId);
    }
    if (!hasExactKeys(record.outcomes, CONTROL_OUTCOME_FIELDS)) {
      pushVerificationError(errors, "invalid-field-type", "outcomes keys differ from common schema", candidateId);
    } else if (Object.values(record.outcomes).some((value) => !nonEmptyString(value))) {
      pushVerificationError(errors, "invalid-field-type", "outcomes values must be non-empty strings", candidateId);
    }
    if (!hasExactKeys(record.accessibility, CONTROL_ACCESSIBILITY_FIELDS)) {
      pushVerificationError(errors, "invalid-field-type", "accessibility keys differ from common schema", candidateId);
    } else if (Object.values(record.accessibility).some((value) => !nonEmptyString(value))) {
      pushVerificationError(errors, "invalid-field-type", "accessibility values must be non-empty strings", candidateId);
    }
    if (candidate) {
      const fallbackCandidateLabel = `${candidate.element}@${candidate.path}:${candidate.line}`;
      const candidateLabelMatches = record.candidateLabel === candidate.label
        || (candidate.label === "" && record.candidateLabel === fallbackCandidateLabel);
      const handlerMatches = normalizedControlExpression(record.handler)
        === normalizedControlExpression(candidate.handler)
        || (candidate.handler === "" && record.handler === "No handler declared");
      if (
        !candidateLabelMatches
        || record.element !== candidate.element
        || !handlerMatches
        || record.source?.path !== candidate.path
        || record.source?.line !== candidate.line
        || record.source?.column !== candidate.column
      ) {
        pushVerificationError(errors, "record-candidate-drift", "record source/element/label/handler differs from candidate", candidateId);
      }
    }
    if (record.status === "unverified") {
      statusCounts.unverified += 1;
      pushVerificationError(errors, "unverified-status", "unverified control status is forbidden", candidateId);
      continue;
    }
    if (!CONTROL_STATUSES.has(record.status)) {
      pushVerificationError(errors, "invalid-status", `unsupported control status: ${String(record.status)}`, candidateId);
      continue;
    }
    statusCounts[record.status] += 1;
    const backendCodeRefs = Array.isArray(record.backendTrace)
      ? record.backendTrace.filter((entry) => /^code:[^#\n]+#[A-Za-z_$][A-Za-z0-9_$]*$/.test(entry))
      : [];
    if (["complete", "incomplete", "contradicted"].includes(record.status)) {
      if (backendCodeRefs.length === 0) {
        pushVerificationError(
          errors,
          "control-backend-code-evidence-missing",
          "actionable control needs at least one hashed code:<tracked-path>#<symbol> trace",
          candidateId,
        );
      }
      for (const evidence of backendCodeRefs) {
        if (!Object.prototype.hasOwnProperty.call(codeEvidence ?? {}, evidence)) {
          pushVerificationError(
            errors,
            "control-backend-code-evidence-unbound",
            `control code trace has no source hash binding: ${evidence}`,
            candidateId,
          );
        }
      }
    }
    if (record.status === "incomplete") {
      if (JSON.stringify(record.acceptanceCriteria) !== JSON.stringify(expectedControlAcceptanceCriteria(record))) {
        pushVerificationError(
          errors,
          "invalid-control-acceptance-criteria",
          "incomplete control needs the exact candidate/test/initial/event/call/backend/visible/accessibility/return/outcome contract",
          candidateId,
        );
      }
      if (!DOCUMENT_GAP_GROUP_SET.has(record.gapGroup)) {
        pushVerificationError(errors, "invalid-gap-group", `unsupported control gap group: ${String(record.gapGroup)}`, candidateId);
      }
    } else {
      if (record.gapGroup !== null) pushVerificationError(errors, "invalid-gap-group", "only incomplete controls may have a gap group", candidateId);
      if (record.acceptanceCriteria.length !== 0) pushVerificationError(errors, "unexpected-acceptance-criteria", "only incomplete controls may have acceptance criteria", candidateId);
    }
    if (record.status === "obsolete") {
      if (record.duplicateOf.length !== 0) pushVerificationError(errors, "obsolete-with-duplicate-target", "obsolete control cannot point to a canonical", candidateId);
      if (
        !record.stateTransition.includes("N/A")
        || !record.backendTrace.some((entry) => entry.includes("N/A"))
        || !Object.values(record.outcomes).every((value) => value.includes("N/A"))
      ) {
        pushVerificationError(errors, "obsolete-with-action", "obsolete control must explicitly prove no independent state/backend/outcome action", candidateId);
      }
    } else if (record.status !== "duplicate" && record.duplicateOf.length !== 0) {
      pushVerificationError(errors, "unexpected-duplicate-target", "only duplicate controls may have duplicateOf", candidateId);
    }
    if (record.status === "duplicate") {
      if (record.duplicateOf.length === 0 || record.duplicateOf.includes(candidateId)) {
        pushVerificationError(errors, "invalid-duplicate-target", "duplicate needs at least one non-self canonical", candidateId);
      }
      for (const canonicalId of record.duplicateOf) {
        const canonical = recordsByCandidateId.get(canonicalId);
        if (!canonical || !["complete", "incomplete"].includes(canonical.status) || canonical.duplicateOf?.length) {
          pushVerificationError(errors, "duplicate-canonical-chain", `duplicate canonical is absent, non-action, or another duplicate: ${canonicalId}`, candidateId);
        }
        if (!record.finalDisposition.includes(canonicalId)) {
          pushVerificationError(errors, "duplicate-without-equivalence-binding", `duplicate disposition does not bind canonical ${canonicalId}`, candidateId);
        }
      }
    }
    if (record.status === "complete") {
      if (!nonEmptyString(record.handler) || !nonEmptyStringArray(record.backendTrace)) {
        pushVerificationError(errors, "complete-without-handler-trace", "complete control needs its current handler and exact trace", candidateId);
      }
      if (record.automatedTests.length !== 0) {
        pushVerificationError(
          errors,
          "static-control-test-cannot-complete",
          "static automatedTests declarations cannot prove a control complete; use a direct executed typed receipt",
          candidateId,
        );
      }
      let directRuntime = false;
      for (const evidence of record.runtimeEvidence) {
        const match = /^receipt:(control-runtime-receipt-[0-9a-f]{16})$/.exec(evidence);
        if (!match || !receiptsById.has(match[1])) {
          pushVerificationError(errors, "invalid-complete-runtime-evidence", `complete runtime evidence must reference a typed receipt: ${evidence}`, candidateId);
          continue;
        }
        directRuntime = directCandidates.get(candidateId)?.has(match[1]) || directRuntime;
      }
      if (!directRuntime) {
        pushVerificationError(errors, "complete-without-direct-verification", "complete control needs an exit-zero direct automated typed receipt with candidate-bound structured assertions and tracked test hash", candidateId);
      }
    }
  }

  const computedCounts = {
    complete: statusCounts.complete,
    incomplete: statusCounts.incomplete,
    obsolete: statusCounts.obsolete,
    duplicate: statusCounts.duplicate,
    contradicted: statusCounts.contradicted,
  };
  if (JSON.stringify(controlsLedger?.counts) !== JSON.stringify(computedCounts)) {
    pushVerificationError(errors, "control-counts-drift", "controls.json counts do not match records");
  }
  const computedGapCounts = Object.fromEntries(
    [...new Set(records.map((record) => record?.gapGroup).filter(Boolean))].sort().map((group) => [
      group,
      records.filter((record) => record?.gapGroup === group).length,
    ]),
  );
  if (JSON.stringify(controlsLedger?.gapCounts) !== JSON.stringify(computedGapCounts)) {
    pushVerificationError(errors, "control-gap-counts-drift", "controls.json gapCounts do not match records");
  }
  if (
    controlsLedger?.scope?.sourceLedger !== normalizePath(
      join(normalizePath(relative(repositoryRoot, auditDirectory)) || ".", "control-candidates.json"),
    )
    || controlsLedger?.scope?.candidateCount !== candidates.length
    || controlsLedger?.scope?.candidateIdsUnique !== (new Set(candidateIds).size === candidates.length)
    || controlsLedger?.scope?.unverifiedCount !== statusCounts.unverified
  ) {
    pushVerificationError(errors, "control-scope-drift", "controls.json scope does not match candidates/statuses");
  }
  const counts = {
    candidates: candidates.length,
    records: records.length,
    uniqueCandidateIds: new Set(recordCandidateIds).size,
    uniqueRecordIds: new Set(recordIds).size,
    missingCandidateIds: missingCandidateIds.length,
    orphanCandidateIds: orphanCandidateIds.length,
    duplicateCandidateIds: duplicateCount(recordCandidateIds),
    duplicateRecordIds: duplicateCount(recordIds),
    ...statusCounts,
  };
  return {
    schema: 1,
    scope: "controls",
    auditDirectory: normalizePath(relative(repositoryRoot, auditDirectory)) || ".",
    passed: errors.length === 0,
    legalGapGroups: DOCUMENT_GAP_GROUPS,
    runtimeReceipts: {
      total: receiptsById.size,
      directCandidateCount: directCandidates.size,
      testExecutions: directTestExecutions,
    },
    counts,
    errors,
  };
}


  return {
    CONTROL_REVIEW_METADATA,
    expectedControlAcceptanceCriteria,
    extractControls,
    mergeControlLedger,
    verifyControlAudit,
  };
}
