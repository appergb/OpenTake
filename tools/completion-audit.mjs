import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { createControlAuditTools } from "./completion-audit-controls.mjs";

const DEFAULT_FILE_INVENTORY_PATH = "docs/audit/2026-07-14/repository-files.json";
const REVIEWED_PLAN_MAP = JSON.parse(readFileSync(new URL("./completion-audit-plan-map.json", import.meta.url), "utf8"));

const TRACKED_PLAN_PROVENANCE_FIELDS = Object.freeze([
  ["coreReportPath", "coreReportSha256"],
  ["coreValidationPath", "coreValidationSha256"],
  ["uiReportPath", "uiReportSha256"],
  ["uiValidationPath", "uiValidationSha256"],
  ["ledgerReportPath", "ledgerReportSha256"],
  ["ledgerValidationPath", "ledgerValidationSha256"],
  ["correctionsPath", "correctionsSha256"],
  ["capabilityDedupePath", "capabilityDedupeSha256"],
]);

export function validateTrackedPlanProvenance(planMap, repositoryRoot) {
  const root = resolve(repositoryRoot);
  const provenance = planMap?.provenance;
  if (!provenance || typeof provenance !== "object") {
    throw new Error("reviewed plan tracked provenance is missing");
  }
  for (const [pathKey, hashKey] of TRACKED_PLAN_PROVENANCE_FIELDS) {
    const path = provenance[pathKey];
    const expectedSha256 = provenance[hashKey];
    if (
      typeof path !== "string" || !path.startsWith("docs/audit/2026-07-14/task8-")
      || isAbsolute(path) || path.split("/").includes("..")
      || typeof expectedSha256 !== "string" || !/^[0-9a-f]{64}$/.test(expectedSha256)
    ) {
      throw new Error(`reviewed plan tracked provenance field is invalid: ${pathKey}/${hashKey}`);
    }
    const absolutePath = resolve(root, path);
    const relativePath = relative(root, absolutePath);
    if (relativePath === ".." || relativePath.startsWith(`..${sep}`) || isAbsolute(relativePath)) {
      throw new Error(`reviewed plan tracked provenance escapes repository root: ${path}`);
    }
    let bytes;
    try {
      bytes = readFileSync(absolutePath);
    } catch (error) {
      throw new Error(`reviewed plan tracked provenance file is unreadable: ${path}`, { cause: error });
    }
    const actualSha256 = createHash("sha256").update(bytes).digest("hex");
    if (actualSha256 !== expectedSha256) {
      throw new Error(`reviewed plan tracked provenance hash mismatch: ${path}`);
    }
  }
  const linkedFields = [
    [planMap.exactSlices?.sourceSha256, provenance.coreReportSha256, "core report"],
    [planMap.exactSlices?.uiSourceSha256, provenance.uiReportSha256, "UI report"],
    [planMap.ledgerOwnership?.sourcePath, provenance.ledgerReportPath, "ledger report path"],
    [planMap.ledgerOwnership?.sourceSha256, provenance.ledgerReportSha256, "ledger report hash"],
    [planMap.ledgerOwnership?.validationPath, provenance.ledgerValidationPath, "ledger validation path"],
    [planMap.ledgerOwnership?.validationSha256, provenance.ledgerValidationSha256, "ledger validation hash"],
    [planMap.corrections?.sourcePath, provenance.correctionsPath, "corrections path"],
    [planMap.corrections?.sourceSha256, provenance.correctionsSha256, "corrections hash"],
    [planMap.capabilityDedupe?.sourcePath, provenance.capabilityDedupePath, "capability dedupe path"],
    [planMap.capabilityDedupe?.sourceSha256, provenance.capabilityDedupeSha256, "capability dedupe hash"],
    [planMap.capabilityDedupe?.sourceInputs?.reportMap?.path, provenance.uiReportPath, "dedupe UI report path"],
    [planMap.capabilityDedupe?.sourceInputs?.reportMap?.sha256, provenance.uiReportSha256, "dedupe UI report hash"],
    [planMap.capabilityDedupe?.sourceInputs?.ledgerMap?.path, provenance.ledgerReportPath, "dedupe ledger path"],
    [planMap.capabilityDedupe?.sourceInputs?.ledgerMap?.sha256, provenance.ledgerReportSha256, "dedupe ledger hash"],
  ];
  for (const [actual, expected, label] of linkedFields) {
    if (actual !== expected) throw new Error(`reviewed plan tracked provenance link drifted: ${label}`);
  }
  return structuredClone(provenance);
}

const REVIEWED_TRACKED_PLAN_PROVENANCE = validateTrackedPlanProvenance(
  REVIEWED_PLAN_MAP,
  fileURLToPath(new URL("..", import.meta.url)),
);
const REVIEWED_PLAN_RECORD_IDS = new Set(REVIEWED_PLAN_MAP.currentReviewedRecordIds);
const REVIEWED_PLAN_SOURCE_RECORD_IDS = new Set([
  ...REVIEWED_PLAN_MAP.currentReviewedRecordIds,
  ...REVIEWED_PLAN_MAP.sourceOnlyReviewedRecordIds,
]);

function normalizeReviewedRecordId(recordId) {
  return String(recordId).startsWith("requirement-") ? String(recordId) : `requirement-${recordId}`;
}

function validateReviewedExactSlices(planMap) {
  if (
    planMap.currentReviewedRecordIds.length !== 346
    || REVIEWED_PLAN_SOURCE_RECORD_IDS.size !== 353
    || planMap.sourceOnlyReviewedRecordIds.some((recordId) => REVIEWED_PLAN_RECORD_IDS.has(recordId))
  ) {
    throw new Error("reviewed source/current record provenance is invalid");
  }
  const exact = planMap.exactSlices;
  if (
    !exact
    || exact.schema !== 1
    || exact.sourceSha256 !== planMap.provenance.coreReportSha256
    || exact.uiSourceSha256 !== planMap.provenance.uiReportSha256
  ) {
    throw new Error("reviewed exact slice map schema or core report provenance is invalid");
  }
  const allowedClassifications = new Set([
    "implementation", "composite-acceptance", "evidence-closure", "contradicted", "duplicate-or-taxonomy",
  ]);
  const sourceSeen = new Set();
  const currentSeen = new Set();
  for (const [group, definition] of Object.entries(exact.groups ?? {})) {
    if (!Number.isInteger(definition.sourceRecordCount) || !Number.isInteger(definition.currentRecordCount)) {
      throw new Error(`reviewed exact slice counts are invalid for ${group}`);
    }
    const seen = new Set();
    let currentCount = 0;
    for (const slice of definition.slices ?? []) {
      if (!slice.sliceKey || !allowedClassifications.has(slice.classification) || !Array.isArray(slice.recordIds)) {
        throw new Error(`reviewed exact slice schema is invalid for ${group}`);
      }
      for (const rawId of slice.recordIds) {
        const recordId = normalizeReviewedRecordId(rawId);
        if (seen.has(recordId)) throw new Error(`reviewed exact slice record is duplicated in ${group}: ${recordId}`);
        if (sourceSeen.has(recordId)) throw new Error(`reviewed exact slice record is duplicated globally: ${recordId}`);
        if (!REVIEWED_PLAN_SOURCE_RECORD_IDS.has(recordId)) {
          throw new Error(`reviewed exact slice record is outside report provenance: ${recordId}`);
        }
        if (slice.targetStatus === "complete" ? REVIEWED_PLAN_RECORD_IDS.has(recordId) : !REVIEWED_PLAN_RECORD_IDS.has(recordId)) {
          throw new Error(`reviewed exact slice current membership is invalid: ${recordId}`);
        }
        seen.add(recordId);
        sourceSeen.add(recordId);
        if (slice.targetStatus !== "complete") {
          currentCount += 1;
          currentSeen.add(recordId);
        }
      }
    }
    if (seen.size !== definition.sourceRecordCount || currentCount !== definition.currentRecordCount) {
      throw new Error(`reviewed exact slice counts drifted for ${group}`);
    }
    if (definition.plannedGroupRecordCount !== undefined) {
      const plannedCount = definition.slices.reduce((count, slice) => (
        count + (slice.targetStatus === "complete" || slice.targetGroup ? 0 : slice.recordIds.length)
      ), 0);
      if (plannedCount !== definition.plannedGroupRecordCount) {
        throw new Error(`reviewed planned group count drifted for ${group}`);
      }
    }
  }
  if (
    sourceSeen.size !== 353
    || currentSeen.size !== 346
    || [...sourceSeen].some((recordId) => !REVIEWED_PLAN_SOURCE_RECORD_IDS.has(recordId))
    || [...REVIEWED_PLAN_SOURCE_RECORD_IDS].some((recordId) => !sourceSeen.has(recordId))
    || [...currentSeen].some((recordId) => !REVIEWED_PLAN_RECORD_IDS.has(recordId))
    || [...REVIEWED_PLAN_RECORD_IDS].some((recordId) => !currentSeen.has(recordId))
  ) {
    throw new Error("reviewed exact slice global source/current union drifted");
  }
  const correctionRegistry = planMap.corrections;
  const declaredCorrections = new Set(correctionRegistry?.appliedCorrectionIds ?? []);
  const appliedCorrections = new Set([
    ...Object.values(exact.groups).flatMap(({ slices }) => slices.flatMap(({ correctionIds = [] }) => correctionIds)),
    ...Object.values(planMap.ownershipGroups ?? {}).flatMap(({ slices }) => slices.flatMap(({ correctionIds = [] }) => correctionIds)),
  ]);
  if (
    correctionRegistry?.schema !== 1
    || correctionRegistry.expectedCount !== 9
    || declaredCorrections.size !== 9
    || appliedCorrections.size !== 9
    || [...declaredCorrections].some((correctionId) => !appliedCorrections.has(correctionId))
  ) {
    throw new Error("reviewed map correction coverage is not applied 9/9");
  }
  return exact;
}

const REVIEWED_EXACT_SLICES = validateReviewedExactSlices(REVIEWED_PLAN_MAP);
const REVIEWED_EXACT_SLICE_BY_KEY = new Map(Object.values(REVIEWED_EXACT_SLICES.groups).flatMap(({ slices }) => (
  slices.map((slice) => [slice.sliceKey, slice])
)));

export function reviewedPlanExactSlices() {
  return structuredClone(REVIEWED_EXACT_SLICES);
}

function validateReviewedOwnershipGroups(planMap, exact) {
  const groups = planMap.ownershipGroups ?? {};
  for (const [group, ownership] of Object.entries(groups)) {
    const expected = exact.groups[group];
    if (!expected || !Number.isInteger(ownership.recordCount) || !Array.isArray(ownership.slices)) {
      throw new Error(`reviewed ownership group schema is invalid for ${group}`);
    }
    const expectedSlices = expected.slices;
    const expectedIds = new Set(expectedSlices.flatMap(({ recordIds }) => recordIds.map(normalizeReviewedRecordId)));
    const expectedKeys = new Set(expectedSlices.map(({ sliceKey }) => sliceKey));
    const seen = new Set();
    const seenKeys = new Set();
    for (const slice of ownership.slices) {
      if (!slice.sliceKey || seenKeys.has(slice.sliceKey) || !Array.isArray(slice.recordIds)) {
        throw new Error(`reviewed ownership slice schema is invalid for ${group}`);
      }
      seenKeys.add(slice.sliceKey);
      for (const product of slice.products ?? []) {
        if (!product || typeof product !== "object" || !product.path || !("symbol" in product) || typeof product.planned !== "boolean") {
          throw new Error(`reviewed ownership product schema is invalid for ${group}/${slice.sliceKey}`);
        }
      }
      for (const test of slice.tests ?? []) {
        if (
          !test || typeof test !== "object" || !test.path || !test.name
          || !["existing-owned", "reviewed-planned"].includes(test.evidenceClass)
        ) {
          throw new Error(`reviewed ownership test schema is invalid for ${group}/${slice.sliceKey}`);
        }
      }
      for (const rawId of slice.recordIds) {
        const recordId = normalizeReviewedRecordId(rawId);
        if (seen.has(recordId)) throw new Error(`reviewed ownership record is duplicated for ${group}: ${recordId}`);
        seen.add(recordId);
      }
    }
    if (
      ownership.recordCount !== expectedIds.size
      || seen.size !== expectedIds.size
      || seenKeys.size !== expectedKeys.size
      || [...seen].some((recordId) => !expectedIds.has(recordId))
      || [...expectedIds].some((recordId) => !seen.has(recordId))
      || [...seenKeys].some((sliceKey) => !expectedKeys.has(sliceKey))
    ) {
      throw new Error(`reviewed ownership exact ID or slice union drifted for ${group}`);
    }
  }
  return groups;
}

function validateLedgerOwnership(planMap) {
  const ledger = planMap.ledgerOwnership;
  if (
    !ledger || ledger.schema !== 1 || ledger.recordCount !== 111 || ledger.sliceCount !== 34
    || !/^[0-9a-f]{64}$/.test(ledger.sourceSha256)
    || !/^[0-9a-f]{64}$/.test(ledger.validationSha256)
    || !Array.isArray(ledger.slices) || ledger.slices.length !== ledger.sliceCount
  ) {
    throw new Error("validated ledger ownership schema or provenance is invalid");
  }
  const groupByRecord = new Map();
  const groupBySlice = new Map();
  for (const [group, definition] of Object.entries(ledger.groups ?? {})) {
    if (
      !Number.isInteger(definition.recordCount) || definition.recordIds.length !== definition.recordCount
      || !Array.isArray(definition.sliceKeys) || definition.sliceKeys.length === 0
      || (definition.uncoveredRecordIds ?? []).length !== 0
    ) {
      throw new Error(`validated ledger group schema is invalid for ${group}`);
    }
    for (const recordId of definition.recordIds) {
      if (groupByRecord.has(recordId)) throw new Error(`validated ledger record is duplicated across groups: ${recordId}`);
      if (REVIEWED_PLAN_RECORD_IDS.has(recordId)) throw new Error(`validated ledger record overlaps report provenance: ${recordId}`);
      groupByRecord.set(recordId, group);
    }
    for (const sliceKey of definition.sliceKeys) {
      if (groupBySlice.has(sliceKey)) throw new Error(`validated ledger slice is duplicated across groups: ${sliceKey}`);
      groupBySlice.set(sliceKey, group);
    }
  }
  const seen = new Set();
  const seenKeys = new Set();
  for (const slice of ledger.slices) {
    const group = groupBySlice.get(slice.sliceKey);
    if (
      !group || seenKeys.has(slice.sliceKey) || !Array.isArray(slice.recordIds) || slice.recordIds.length === 0
      || !["implementation", "composite-acceptance", "evidence-closure"].includes(slice.classification)
      || !Array.isArray(slice.products) || slice.products.length === 0
      || !Array.isArray(slice.tests) || slice.tests.length === 0
    ) {
      throw new Error(`validated ledger slice schema is invalid: ${String(slice.sliceKey)}`);
    }
    seenKeys.add(slice.sliceKey);
    for (const product of slice.products) {
      if (!product || typeof product !== "object" || !product.path || !("symbol" in product) || typeof product.planned !== "boolean") {
        throw new Error(`validated ledger product schema is invalid: ${slice.sliceKey}`);
      }
    }
    for (const test of slice.tests) {
      if (
        !test || typeof test !== "object" || !test.path || !test.name
        || !["existing-owned", "reviewed-planned"].includes(test.evidenceClass)
      ) {
        throw new Error(`validated ledger test schema is invalid: ${slice.sliceKey}`);
      }
    }
    for (const recordId of slice.recordIds) {
      if (groupByRecord.get(recordId) !== group) {
        throw new Error(`validated ledger record is assigned to the wrong group: ${recordId}`);
      }
      if (seen.has(recordId)) throw new Error(`validated ledger record is duplicated across slices: ${recordId}`);
      seen.add(recordId);
    }
  }
  if (
    groupByRecord.size !== ledger.recordCount || seen.size !== ledger.recordCount
    || groupBySlice.size !== ledger.sliceCount || seenKeys.size !== ledger.sliceCount
    || [...groupByRecord.keys()].some((recordId) => !seen.has(recordId))
    || [...groupBySlice.keys()].some((sliceKey) => !seenKeys.has(sliceKey))
  ) {
    throw new Error("validated ledger exact ID or slice union drifted");
  }
  return ledger;
}

function validateCapabilityDedupe(planMap, ledger) {
  const dedupe = planMap.capabilityDedupe;
  const reportProof = dedupe?.coverageProof?.report;
  const ledgerProof = dedupe?.coverageProof?.ledger;
  if (
    !dedupe || dedupe.schema !== "opentake-task8-ui-ledger-capability-dedupe/v1"
    || dedupe.status !== "complete" || dedupe.duplicateClusterCount !== 16
    || dedupe.duplicateClusters.length !== 16
    || dedupe.sourceInputs?.reportMap?.sha256 !== planMap.provenance.uiReportSha256
    || dedupe.sourceInputs?.ledgerMap?.sha256 !== ledger.sourceSha256
    || reportProof?.all176AccountedExactlyOnce !== true
    || ledgerProof?.all111AccountedExactlyOnce !== true
    || dedupe.coverageProof?.crossSource?.noCrossSourceLossOrDuplication !== true
    || dedupe.coverageProof.crossSource.totalUniqueSourceRecordCount !== 287
  ) {
    throw new Error("UI/ledger capability dedupe schema, provenance, or coverage proof is invalid");
  }
  const ledgerByKey = new Map(ledger.slices.map((slice) => [slice.sliceKey, slice]));
  const clusteredReportKeys = new Set();
  const clusteredLedgerKeys = new Set();
  for (const cluster of dedupe.duplicateClusters) {
    if (
      !cluster.capabilityId || !dedupe.primaryGroups.includes(cluster.primaryGroup)
      || cluster.products.length === 0 || cluster.tests.length === 0
    ) {
      throw new Error(`UI/ledger capability cluster schema is invalid: ${String(cluster.capabilityId)}`);
    }
    const sourceRecords = [];
    for (const sliceKey of cluster.reportSliceKeys) {
      const slice = REVIEWED_EXACT_SLICE_BY_KEY.get(sliceKey);
      if (!slice || clusteredReportKeys.has(sliceKey)) throw new Error(`UI report capability slice is missing or duplicated: ${sliceKey}`);
      clusteredReportKeys.add(sliceKey);
      sourceRecords.push(...slice.recordIds.map(normalizeReviewedRecordId));
    }
    for (const sliceKey of cluster.ledgerSliceKeys) {
      const slice = ledgerByKey.get(sliceKey);
      if (!slice || clusteredLedgerKeys.has(sliceKey)) throw new Error(`ledger capability slice is missing or duplicated: ${sliceKey}`);
      clusteredLedgerKeys.add(sliceKey);
      sourceRecords.push(...slice.recordIds);
    }
    const clusterRecords = new Set(cluster.recordIds);
    if (
      sourceRecords.length !== clusterRecords.size
      || sourceRecords.some((recordId) => !clusterRecords.has(recordId))
      || cluster.products.some((product) => (
        !product?.path || !("symbol" in product) || typeof product.planned !== "boolean"
      ))
      || cluster.tests.some((test) => (
        !test?.path || !test.name || !["existing-owned", "reviewed-planned"].includes(test.evidenceClass)
      ))
    ) {
      throw new Error(`UI/ledger capability exact union drifted: ${cluster.capabilityId}`);
    }
  }
  const standaloneReportKeys = new Set(dedupe.standalone.reportSlices.map(({ reportSliceKey }) => reportSliceKey));
  const standaloneLedgerKeys = new Set(dedupe.standalone.ledgerSlices.map(({ ledgerSliceKey }) => ledgerSliceKey));
  const movedReportKeys = new Set(dedupe.taxonomyMoves.map(({ reportSliceKey }) => reportSliceKey));
  const completeReportKeys = new Set(dedupe.completedNonProduct.map(({ reportSliceKey }) => reportSliceKey));
  const allUiReportKeys = new Set(["preview-timeline", "inspector-text-keyframes", "agent-settings-generation", "accessibility-polish"].flatMap((group) => (
    REVIEWED_EXACT_SLICES.groups[group].slices.map(({ sliceKey }) => sliceKey)
  )));
  const partitionedReportKeys = new Set([
    ...clusteredReportKeys, ...standaloneReportKeys, ...movedReportKeys, ...completeReportKeys,
  ]);
  const partitionedLedgerKeys = new Set([...clusteredLedgerKeys, ...standaloneLedgerKeys]);
  if (
    partitionedReportKeys.size !== allUiReportKeys.size
    || [...allUiReportKeys].some((sliceKey) => !partitionedReportKeys.has(sliceKey))
    || partitionedLedgerKeys.size !== ledgerByKey.size
    || [...ledgerByKey.keys()].some((sliceKey) => !partitionedLedgerKeys.has(sliceKey))
  ) {
    throw new Error("UI/ledger capability slice partition drifted");
  }
  return dedupe;
}

const REVIEWED_OWNERSHIP_GROUPS = validateReviewedOwnershipGroups(REVIEWED_PLAN_MAP, REVIEWED_EXACT_SLICES);
const REVIEWED_LEDGER_OWNERSHIP = validateLedgerOwnership(REVIEWED_PLAN_MAP);
const REVIEWED_LEDGER_RECORD_IDS = new Set(Object.values(REVIEWED_LEDGER_OWNERSHIP.groups).flatMap(({ recordIds }) => recordIds));
const REVIEWED_CAPABILITY_DEDUPE = validateCapabilityDedupe(REVIEWED_PLAN_MAP, REVIEWED_LEDGER_OWNERSHIP);
const REVIEWED_CAPABILITY_CLUSTER_BY_RULE_KEY = new Map(REVIEWED_CAPABILITY_DEDUPE.duplicateClusters.flatMap((cluster) => (
  [...cluster.reportSliceKeys, ...cluster.ledgerSliceKeys].map((sliceKey) => [sliceKey, cluster])
)));

export function reviewedPlanOwnershipMaps() {
  return structuredClone({
    provenance: REVIEWED_TRACKED_PLAN_PROVENANCE,
    ownershipGroups: REVIEWED_OWNERSHIP_GROUPS,
    ledger: REVIEWED_LEDGER_OWNERSHIP,
    capabilityDedupe: REVIEWED_CAPABILITY_DEDUPE,
  });
}

export function normalizePath(value) {
  return value.replaceAll("\\", "/").replace(/^\.\//, "");
}

export function compareAuditText(left, right) {
  return Buffer.compare(
    Buffer.from(String(left), "utf8"),
    Buffer.from(String(right), "utf8"),
  );
}

export function stableId(prefix, value) {
  return `${prefix}-${createHash("sha256").update(value).digest("hex").slice(0, 16)}`;
}

export function normalizeSemanticText(value) {
  return String(value ?? "").replace(/\s+/gu, " ").trim();
}

function gitRead(name, path, args, encoding = "utf8") {
  try {
    return execFileSync("git", ["-C", path, ...args], {
      encoding,
      maxBuffer: 128 * 1024 * 1024,
      stdio: ["ignore", "pipe", "pipe"],
    });
  } catch (error) {
    const detail = error.stderr?.toString("utf8").trim();
    const suffix = detail ? `: ${detail}` : "";
    throw new Error(`${name}: git ${args.join(" ")} failed${suffix}`);
  }
}

function gitText(name, path, args) {
  return gitRead(name, path, args).trim();
}

function assertFullSha(name, label, value) {
  if (!/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${name}: ${label} must resolve to a full SHA`);
  }
  return value;
}

function parseCount(name, label, value) {
  if (!/^\d+$/.test(value)) {
    throw new Error(`${name}: ${label} is not an integer`);
  }
  return Number(value);
}

function rejectAbbreviatedSha(name, ref) {
  if (/^[0-9a-f]{4,39}$/i.test(ref)) {
    throw new Error(`${name}: abbreviated SHA ref is not immutable: ${ref}`);
  }
}

function parseNameStatus(name, raw) {
  const parts = raw.toString("utf8").split("\0");
  if (parts.at(-1) !== "") {
    throw new Error(`${name}: Git name-status output was not NUL terminated`);
  }
  parts.pop();
  const changedPaths = [];
  for (let index = 0; index < parts.length;) {
    const status = parts[index++];
    if (!/^(?:[ACDMRTUXB]|[RC]\d{1,3})$/.test(status)) {
      throw new Error(`${name}: invalid Git name-status token: ${status}`);
    }
    const first = parts[index++];
    if (first == null || first === "") {
      throw new Error(`${name}: Git name-status record is missing a path`);
    }
    let destination = null;
    if (/^[RC]/.test(status)) {
      const second = parts[index++];
      if (second == null || second === "") {
        throw new Error(`${name}: Git ${status} record is missing a destination`);
      }
      destination = normalizePath(second);
    }
    changedPaths.push({
      status,
      path: normalizePath(first),
      destination,
      disposition: "unverified",
    });
  }
  return changedPaths;
}

export function captureGitSource({
  name,
  path,
  base,
  head,
  remote = "origin",
  requireClean = false,
  expectChanges = false,
}) {
  if (![name, path, base, head, remote].every((value) => typeof value === "string" && value)) {
    throw new Error("captureGitSource requires non-empty name, path, base, head, and remote strings");
  }
  rejectAbbreviatedSha(name, base);
  rejectAbbreviatedSha(name, head);

  if (requireClean) {
    const porcelain = gitRead(
      name,
      path,
      ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
      null,
    );
    if (porcelain.length !== 0) {
      throw new Error(`${name}: source repository is dirty`);
    }
  }

  const baseSha = assertFullSha(name, "base", gitText(name, path, ["rev-parse", base]));
  const headSha = assertFullSha(name, "head", gitText(name, path, ["rev-parse", head]));
  if (expectChanges && baseSha === headSha) {
    throw new Error(`${name}: expected base and head to differ`);
  }

  const baseTree = assertFullSha(
    name,
    "base tree",
    gitText(name, path, ["rev-parse", `${baseSha}^{tree}`]),
  );
  const headTree = assertFullSha(
    name,
    "head tree",
    gitText(name, path, ["rev-parse", `${headSha}^{tree}`]),
  );
  const mergeBase = assertFullSha(
    name,
    "merge base",
    gitText(name, path, ["merge-base", baseSha, headSha]),
  );
  const commitCount = parseCount(
    name,
    "commit count",
    gitText(name, path, ["rev-list", "--count", `${baseSha}..${headSha}`]),
  );
  const behindCount = parseCount(
    name,
    "behind count",
    gitText(name, path, ["rev-list", "--count", `${headSha}..${baseSha}`]),
  );
  const raw = gitRead(
    name,
    path,
    [
      "diff",
      "--name-status",
      "-z",
      "--find-renames",
      "--find-copies-harder",
      `${baseSha}..${headSha}`,
    ],
    null,
  );
  const changedPaths = parseNameStatus(name, raw);
  const status = baseSha === headSha
    ? "identical"
    : baseTree === headTree
      ? "equivalent-tree"
      : commitCount > 0 && behindCount > 0
        ? "diverged"
        : "changes";

  return {
    name,
    repository: gitText(name, path, ["remote", "get-url", remote]),
    remote,
    status,
    base: baseSha,
    head: headSha,
    baseTree,
    headTree,
    mergeBase,
    commitCount,
    aheadCount: commitCount,
    behindCount,
    changedPaths,
  };
}

function parsePorcelain(name, raw) {
  const parts = raw.toString("utf8").split("\0");
  if (parts.at(-1) !== "") {
    throw new Error(`${name}: Git porcelain output was not NUL terminated`);
  }
  parts.pop();
  const records = [];
  for (let index = 0; index < parts.length;) {
    const record = parts[index++];
    if (record.length < 4 || record[2] !== " ") {
      throw new Error(`${name}: invalid Git porcelain record`);
    }
    const status = record.slice(0, 2);
    const path = normalizePath(record.slice(3));
    if (!path) {
      throw new Error(`${name}: Git porcelain record is missing a path`);
    }
    const source = /[RC]/.test(status)
      ? normalizePath(parts[index++] ?? "")
      : null;
    if (/[RC]/.test(status) && !source) {
      throw new Error(`${name}: Git rename/copy record is missing a source`);
    }
    records.push({ status, path, source });
  }
  return records;
}

function worktreeContent(path) {
  if (!existsSync(path)) {
    return {
      fileType: "missing",
      bytes: null,
      contentSha256: null,
      linkTargetSha256: null,
    };
  }
  const stat = lstatSync(path);
  if (stat.isSymbolicLink()) {
    const target = readlinkSync(path, { encoding: "buffer" });
    return {
      fileType: "symlink",
      bytes: null,
      contentSha256: null,
      linkTargetSha256: createHash("sha256").update(target).digest("hex"),
    };
  }
  if (!stat.isFile()) {
    return {
      fileType: stat.isDirectory() ? "directory" : "other",
      bytes: null,
      contentSha256: null,
      linkTargetSha256: null,
    };
  }
  const content = readFileSync(path);
  return {
    fileType: "regular",
    bytes: content.length,
    contentSha256: createHash("sha256").update(content).digest("hex"),
    linkTargetSha256: null,
  };
}

function resolveConfined(root, path) {
  const absoluteRoot = resolve(root);
  const absolutePath = resolve(absoluteRoot, path);
  if (absolutePath !== absoluteRoot && !absolutePath.startsWith(`${absoluteRoot}${sep}`)) {
    throw new Error(`dirty manifest path escapes checkout root: ${path}`);
  }
  return absolutePath;
}

function readDirtySnapshot(name, path) {
  return {
    head: assertFullSha(name, "HEAD", gitText(name, path, ["rev-parse", "HEAD"])),
    tree: assertFullSha(name, "HEAD tree", gitText(name, path, ["rev-parse", "HEAD^{tree}"])),
    rawStatus: gitRead(
      name,
      path,
      ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
      null,
    ),
  };
}

function captureDirtyPaths(name, path, rawStatus) {
  return parsePorcelain(name, rawStatus)
    .sort((left, right) => (
      compareAuditText(left.path, right.path)
      || compareAuditText(left.source ?? "", right.source ?? "")
      || compareAuditText(left.status, right.status)
    ))
    .map(({ status, path: dirtyPath, source }) => {
      const tracked = status !== "??" && status !== "!!";
      let patchSha256 = null;
      let patchBytes = null;
      if (tracked) {
        const patchPaths = source ? [source, dirtyPath] : [dirtyPath];
        const patch = gitRead(
          name,
          path,
          ["diff", "HEAD", "--binary", "--no-ext-diff", "--", ...patchPaths],
          null,
        );
        patchBytes = patch.length;
        patchSha256 = createHash("sha256").update(patch).digest("hex");
      }
      return {
        status,
        path: dirtyPath,
        source,
        tracked,
        patchBytes,
        patchSha256,
        ...worktreeContent(resolveConfined(path, dirtyPath)),
      };
    });
}

export function captureDirtyCheckout({
  name,
  path,
  remote = "origin",
  afterInitialCapture = null,
}) {
  if (![name, path, remote].every((value) => typeof value === "string" && value)) {
    throw new Error("captureDirtyCheckout requires non-empty name, path, and remote strings");
  }
  if (afterInitialCapture !== null && typeof afterInitialCapture !== "function") {
    throw new Error("captureDirtyCheckout afterInitialCapture must be a function");
  }
  const initial = readDirtySnapshot(name, path);
  const { rawStatus } = initial;
  const paths = captureDirtyPaths(name, path, rawStatus);
  afterInitialCapture?.();
  const final = readDirtySnapshot(name, path);
  const finalPaths = captureDirtyPaths(name, path, final.rawStatus);
  if (initial.head !== final.head
      || initial.tree !== final.tree
      || !initial.rawStatus.equals(final.rawStatus)
      || JSON.stringify(paths) !== JSON.stringify(finalPaths)) {
    throw new Error(`${name}: checkout changed while its manifest was being captured`);
  }
  return {
    name,
    repository: gitText(name, path, ["remote", "get-url", remote]),
    remote,
    status: paths.length === 0 ? "clean" : "dirty",
    head: initial.head,
    tree: initial.tree,
    statusSha256: createHash("sha256").update(rawStatus).digest("hex"),
    manifestSha256: createHash("sha256").update(JSON.stringify(paths)).digest("hex"),
    paths,
  };
}

const SOURCE_PINS = Object.freeze({
  targetStart: "f94a171753c7fe5361044eabf1c8f3be8177d238",
  targetStartTree: "d992a0e3a7657d4d9b6ca66afb977433ca6b5e6a",
  targetMain: "925736a1c0871f2de1f668d1587a5607e45ab1f9",
  targetMainTree: "d992a0e3a7657d4d9b6ca66afb977433ca6b5e6a",
  palmierPrevious: "404e14f4c449bd24576e52fa24f8e50694a5da13",
  palmierPreviousTree: "b6eb2625b43f1b10884b18c14d0a5f9fe23ec578",
  palmierMain: "092bc9e0fd4c9f0e865799299d34b0dcfee63c7e",
  palmierMainTree: "e397e177b5673ea23fb55b18e8463146ccee0cf4",
  canonicalHead: "c2f807aafd6e46088365eac2de45fe8803a7e1d0",
  canonicalTree: "6fa0ac1bd83719c8215e887e4edcb52c527623dd",
  forkMains: Object.freeze({
    "H-Chris233": Object.freeze({
      sha: "b89f1fc14fa4f2a4d6f988b8a3680ee6406b8483",
      tree: "06c6868c0ac278877aca9427a468c588d1c2067c",
    }),
    "cuic19053-hue": Object.freeze({
      sha: "83482cf365f113241182d0504138751d569dd359",
      tree: "b761a799d999fcbdac3b80c3478c4694e48d7d35",
    }),
  }),
});

const FETCH_PORCELAIN_SHA256 = Object.freeze({
  target: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  canonical: "1157939da02403643b66eee0618db0ec04f9a42412a538d0a67d547d75e0dd00",
  palmier: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
});

const OPEN_PULL_REQUEST_COMMAND = "gh pr list --repo appergb/OpenTake --state open --json number,title,headRefName,baseRefName,url";

const CAPTURED_OPEN_PULL_REQUESTS = Object.freeze({
  repository: "appergb/OpenTake",
  state: "open",
  transport: "immutable-capture",
  capturedAt: "2026-07-14T08:24:59Z",
  command: OPEN_PULL_REQUEST_COMMAND,
  count: 0,
  items: Object.freeze([]),
});

export function capturedOpenPullRequests() {
  return {
    ...CAPTURED_OPEN_PULL_REQUESTS,
    items: CAPTURED_OPEN_PULL_REQUESTS.items.map((item) => ({ ...item })),
  };
}

const REQUIREMENT = Object.freeze({
  chromaKey: "requirement-559211270c1ef341",
  lutImport: "requirement-2156cc0bdb849391",
  voiceIsolation: "requirement-a61a89d25c504355",
  generationTools: "requirement-1c40dd077c50436b",
  generationContract: "requirement-40bc3f68e4a7ed80",
  elevenLabs: "requirement-d4cf765d956089f4",
  generationError: "requirement-43d222897dd2b2dc",
  manifest: "requirement-6afe067d8c7d1190",
  projectBundle: "requirement-21603ea32a296b68",
  agentUndo: "requirement-11f2c5fee86dd78a",
  toolCalls: "requirement-97c6d65f7a0f7dc5",
  telemetry: "requirement-cd85f58d3c84be13",
  telemetryDecision: "requirement-a9bb7231b1ead0d4",
  playback: "requirement-1f4c2338bf4f5188",
  playbackLifecycle: "requirement-3aa21ae6148b5fcd",
});

const CONTROL = Object.freeze({
  chromaKey: Object.freeze([
    "control-record-7f7a127fc222b6f6",
    "control-record-a7f4988fdf17e70b",
    "control-record-679cbe8e200cd71f",
    "control-record-44ecdc8071f40679",
  ]),
  exportCancel: Object.freeze(["control-record-c6c9e81870a0cc68"]),
  generation: Object.freeze(["control-record-8715ffdcc0c6bf4d"]),
  searchIndex: Object.freeze(["control-record-ab960caf193a5615"]),
});

export const PALMIER_REVIEWED_PATH_LEDGER = Object.freeze(
  [
  {
    "status": "M",
    "path": ".github/workflows/ci.yml",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Replaces a separate clean build plus test sequence with trait-aware swift test, reducing duplicate compilation.",
    "openTakeEquivalent": ".github/workflows/ci.yml",
    "rationale": "The CI optimization is Swift-specific; only the invariant of testing production feature combinations is portable.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "A",
    "path": ".swift-version",
    "destination": null,
    "disposition": "platform-specific",
    "behavior": "Pins the Xcode Swift language version for reproducible clean CI builds.",
    "openTakeEquivalent": "rust-toolchain.toml",
    "rationale": "OpenTake uses Rust/Node toolchains, so the file is not portable.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:toolchain-pin"
    ]
  },
  {
    "status": "M",
    "path": "Package.resolved",
    "destination": null,
    "disposition": "platform-specific",
    "behavior": "Prunes/resolves dependencies after moving speech and telemetry dependencies behind package traits.",
    "openTakeEquivalent": "Cargo.lock; package-lock.json",
    "rationale": "Lockfile churn belongs to SwiftPM, though dependency closure should be verified in OpenTake builds.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "M",
    "path": "Package.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Adds BundledSpeech and ProductionTelemetry traits and conditionally enables their dependencies/imports.",
    "openTakeEquivalent": "Cargo.toml feature tables; src-tauri/Cargo.toml",
    "rationale": "Product-level feature gating is relevant, but SwiftPM declarations are not.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "M",
    "path": "README.md",
    "destination": null,
    "disposition": "obsolete",
    "behavior": "Replaces the public star-history image token with a sealed token URL.",
    "openTakeEquivalent": "README.md",
    "rationale": "Palmier repository analytics artwork has no OpenTake product behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Resources/Info.plist",
    "destination": null,
    "disposition": "obsolete",
    "behavior": "Advances marketing/build versions from 0.6.4/65 through 0.6.6/67.",
    "openTakeEquivalent": "src-tauri/tauri.conf.json; package.json",
    "rationale": "Upstream release numbers must not be copied into OpenTake.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:release-metadata"
    ]
  },
  {
    "status": "M",
    "path": "appcast.xml",
    "destination": null,
    "disposition": "obsolete",
    "behavior": "Publishes signed Sparkle entries for Palmier 0.6.5 and 0.6.6.",
    "openTakeEquivalent": "none",
    "rationale": "OpenTake has no equivalent Sparkle feed in this tree and cannot reuse Palmier signatures or URLs.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:release-metadata"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.ar.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Arabic README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.bn.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Bengali README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.es.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Spanish README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.fr.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates French README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.hi.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Hindi README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.it.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Italian README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.ja.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Japanese README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.ko.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Korean README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.pt-BR.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Brazilian Portuguese README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.ru.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Russian README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.tr.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Turkish README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.vi.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Vietnamese README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.zh-CN.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Simplified Chinese README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "docs/readme/README.zh-TW.md",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates Traditional Chinese README star-history artwork to the sealed token URL.",
    "openTakeEquivalent": "none",
    "rationale": "Documentation-only upstream analytics change.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-doc-only"
    ]
  },
  {
    "status": "M",
    "path": "scripts/bundle.sh",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Builds the bundle with speech always enabled, production telemetry only for release, and adds a fast code-sign verification.",
    "openTakeEquivalent": "src-tauri/tauri.conf.json; OpenTake release workflows",
    "rationale": "Feature composition and signing checks are portable release invariants; the script is macOS/Swift-specific.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/AgentService.swift",
    "destination": null,
    "disposition": "cloud-specific",
    "behavior": "Defaults hosted chat to Sonnet 5 and restores in-app activation telemetry without double-counting restored chats.",
    "openTakeEquivalent": "crates/opentake-agent/src; none for hosted model/activation telemetry",
    "rationale": "OpenTake is currently BYOK/local-agent oriented; hosted model choice and activation accounting need an explicit service decision.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:hosted-chat-model"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/MCP/MCPHTTPServer.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Separates HTTP connection lifetime from recognized MCP-session activation counting.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/server.rs; src-tauri/src/mcp.rs",
    "rationale": "OpenTake has MCP transport but no equivalent activation telemetry boundary.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:mcp-session-activation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/MCP/MCPService.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Tracks per-session project selection, consolidates project tools, and records MCP activation only on the first recognized tool call.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs::Dispatcher; crates/opentake-agent/src/tools/names.rs::ToolName",
    "rationale": "The dispatcher is session-scoped but does not pin/project-authorize writes or count first-use activation.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-project",
      "requirement-needed:mcp-session-activation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/AgentInstructions.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Rewrites instructions for manage_tracks, consolidated manage_project, and queued manage_exports.",
    "openTakeEquivalent": "crates/opentake-agent/src/tools/descriptions.rs::description",
    "rationale": "OpenTake's advertised 31-tool set predates all three upstream contract changes.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-project",
      "requirement-needed:agent-manage-tracks-stable-id",
      "requirement-needed:agent-manage-exports"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolDefinitions.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds/updates schemas for stable track addressing, project management, export queue control, and audio cleanup/dubbing source arguments.",
    "openTakeEquivalent": "crates/opentake-agent/src/tools/names.rs::ToolName; crates/opentake-agent/src/tools/descriptions.rs::schema",
    "rationale": "Missing tool names/schemas are a direct contract gap; generation tools are also still unwired.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-tool-contract-refresh"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Clips.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Resolves track targets by trackId or exact legacy integer index and rejects fractional/out-of-zone indices.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs::{remove_tracks,add_clips,move_clips}",
    "rationale": "OpenTake still exposes index-based mutations even though get_timeline returns track IDs.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-tracks-stable-id"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Export.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Routes export commands into the shared queue and implements manage_exports list/cancel/remove/clear operations.",
    "openTakeEquivalent": "src-tauri/src/export.rs::{ExportControl,cancel_export}; web/src/components/shell/ExportDialog.tsx",
    "rationale": "Single-export cancellation exists, but queue state and the agent management surface do not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-c6c9e81870a0cc68"
    ],
    "requirementGapIds": [
      "requirement-needed:agent-manage-exports",
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Generate.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Accepts sourceMediaRef and targetLanguage, validates audio transforms, and submits cleanup/dubbing generations.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs::run_body; crates/opentake-gen/src/provider/elevenlabs.rs::ElevenLabsAdapter",
    "rationale": "Provider plumbing exists, but agent generation is explicitly not implemented and lacks these arguments.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Import.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Routes imported asset metadata through the new batched manifest updater.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs::EditorSession::import_media_file_checked",
    "rationale": "OpenTake imports transactionally but directly mutates the manifest rather than batching metadata.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+ProjectSettings.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Moves project setting mutations under consolidated project management and selected-project write authority.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs::EditorSession; src-tauri/src/commands.rs project settings commands",
    "rationale": "Core settings exist, but no session-pinned agent project authority wraps them.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-project"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Projects.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Replaces separate list/create/open/close tools with manage_project; allows inactive reads, blocks inactive writes, and saves before close.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs::{new_project,open_project,save_project}; crates/opentake-agent/src/tools/names.rs::ToolName",
    "rationale": "Project primitives exist, but the consolidated agent operation and authority/persistence guarantees do not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-project",
      "requirement-needed:project-close-save-barrier"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+ShortId.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Includes stable track IDs and audio-transform source references in short-ID expansion/shortening.",
    "openTakeEquivalent": "crates/opentake-agent/src/tools/short_id.rs::{current_id_universe,expand_id_prefixes,shorten_ids}",
    "rationale": "OpenTake has a short-ID engine but its universe/contracts lack the new tool surfaces.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-tracks-stable-id",
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor+Timeline.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Returns track receipts for reorder/removal and wraps timeline creation/duplication in explicit single agent undo groups.",
    "openTakeEquivalent": "crates/opentake-agent/src/tools/encode_timeline.rs::encode_timeline; crates/opentake-agent/src/mcp/dispatch.rs::{apply,undo}",
    "rationale": "Assistant-only undo already exists, but receipts/consolidated track operations and event-group parity do not.",
    "linkedRequirementIds": [
      "requirement-4780410070918f21"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-tracks-stable-id"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Agent/Tools/ToolExecutor.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Centralizes tool telemetry with source/status/duration/change/failure fields, registers new project/export tools, and fixes NSUndoManager group boundaries.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs::{dispatch,apply,undo}",
    "rationale": "The uniform dispatcher and undo stack exist; telemetry and new tool registrations do not.",
    "linkedRequirementIds": [
      "requirement-11f2c5fee86dd78a",
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-tool-contract-refresh"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/App/AppState.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Makes visible/frontmost project state available to agent session authorization and project listing.",
    "openTakeEquivalent": "src-tauri/src/playback/session.rs::PlaybackSessionRegistry; web project store",
    "rationale": "OpenTake tracks project epochs/sessions, but not the same multi-window agent visibility/write rule.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-project-authority"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Telemetry/Analytics.swift",
    "destination": null,
    "disposition": "cloud-specific",
    "behavior": "Adds tool-call dimensions and MCP/in-app activation events, conditionally compiled for production telemetry.",
    "openTakeEquivalent": "crates/opentake-core/src/events.rs; none for analytics sink",
    "rationale": "OpenTake describes telemetry as a possible observer but ships no analytics capture implementation.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Telemetry/Telemetry.swift",
    "destination": null,
    "disposition": "cloud-specific",
    "behavior": "Makes telemetry imports/initialization conditional on the production package trait.",
    "openTakeEquivalent": "none",
    "rationale": "Requires an OpenTake privacy/consent and deployment decision before implementation.",
    "linkedRequirementIds": [
      "requirement-a9bb7231b1ead0d4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:telemetry-consent-policy"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Agent/AnalyticsSessionActivationTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Verifies one activation per recognized MCP session and avoids connection/restoration double counts.",
    "openTakeEquivalent": "none",
    "rationale": "Test evidence for a telemetry behavior OpenTake does not implement.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:mcp-session-activation"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Agent/ExportProjectToolTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Updates agent project export tests for enqueue/queue-result semantics and cancellation.",
    "openTakeEquivalent": "src-tauri/src/export.rs tests; none for agent export queue",
    "rationale": "OpenTake tests single-operation export but lacks the tested agent queue behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-exports",
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Agent/ManageProjectToolTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Covers consolidated project actions, selected-project pinning, inactive-write rejection, save-before-close, and activation counting.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs tests; none for agent project tool",
    "rationale": "Core project tests do not establish agent/multi-window authority semantics.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-project",
      "requirement-needed:project-close-save-barrier"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Agent/ManageTracksTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Covers stable-ID addressing, exact legacy indices, invalid fractional/index cases, reorder and removal receipts.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs tests; crates/opentake-agent/src/tools/encode_timeline.rs tests",
    "rationale": "Existing OpenTake tests cover separate index-based tools, not the new contract.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-manage-tracks-stable-id"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Agent/ToolExecutorTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Refreshes registered tool/schema expectations for exports, projects, tracks, and generation sources.",
    "openTakeEquivalent": "crates/opentake-agent/src/tools/names.rs tests; crates/opentake-agent/src/tools/descriptions.rs tests",
    "rationale": "This is contract evidence; OpenTake's pinned 31-tool expectation is now stale relative to head.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:agent-tool-contract-refresh"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Agent/UndoToolTests.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Regression-tests automatic event-group closure and one-step undo for agent-created/duplicated timelines.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs::undo; crates/opentake-ops/src/command.rs tests",
    "rationale": "OpenTake snapshots each changed command and keeps a dispatcher-local agent undo stack, avoiding NSUndoManager event groups.",
    "linkedRequirementIds": [
      "requirement-4780410070918f21",
      "requirement-11f2c5fee86dd78a"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Metal/ChromaKey.metal",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Adds a chroma-distance floor to suppress near-black key noise and adjusts matte/alpha output.",
    "openTakeEquivalent": "crates/opentake-render/src/gpu/shader.wgsl chroma branch",
    "rationale": "OpenTake's RGB similarity/smoothness/spill shader is real but not proven visually equivalent to the new chroma-space floor.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-matte-parity"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Compositing/EffectRegistry.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Changes chroma defaults, including softness 0.1, to match sampled-key workflow.",
    "openTakeEquivalent": "crates/opentake-domain/src/grade.rs::ChromaKey; web/src/components/inspector/Inspector.tsx::completeChromaKey",
    "rationale": "OpenTake defaults smoothness to 0.35 and includes spill, so a blind default copy would change its visual model.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [
      "control-record-679cbe8e200cd71f"
    ],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Compositing/FrameRenderer.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Temporarily disables the selected clip's chroma effect while sampling and premultiplies alpha after transforms/effects.",
    "openTakeEquivalent": "crates/opentake-render/src/gpu/compositor.rs; crates/opentake-render/src/gpu/shader.wgsl",
    "rationale": "Sampling needs an unkeyed frame; alpha-order parity is render-critical and needs GPU image tests.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Compositing/LUTLoader.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Raises accepted .cube dimension ceiling from 64 to 128, allowing standard 65-point LUTs.",
    "openTakeEquivalent": "none",
    "rationale": "OpenTake has no .cube LUT loader or import path.",
    "linkedRequirementIds": [
      "requirement-2156cc0bdb849391"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/EditorWindowController.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Cancels chroma sampling on selection/window changes and contributes frontmost-window project state for agent authorization.",
    "openTakeEquivalent": "web selection state; none for eyedropper/project authority",
    "rationale": "Both lifecycle cancellations and window authority are missing as explicit contracts.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper",
      "requirement-needed:agent-project-authority"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ChromaKey.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds sampling mode lifecycle; averages a 9×9 active-frame region, derives hue, and commits hue/tolerance/softness defaults.",
    "openTakeEquivalent": "web/src/components/inspector/Inspector.tsx::ChromaKeySection; none for sampling",
    "rationale": "Existing manual color input does not replace click-to-sample behavior.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Inspector/InspectorView.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Wires the chroma eyedropper action into the inspector and also exposes new audio transform edit state.",
    "openTakeEquivalent": "web/src/components/inspector/Inspector.tsx::ChromaKeySection",
    "rationale": "Manual swatch is integrated; eyedropper and transform actions are absent.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-7f7a127fc222b6f6"
    ],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper",
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Inspector/Tabs/AdjustTab.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Adds the eyedropper control and aligns chroma tolerance/softness labels/defaults.",
    "openTakeEquivalent": "web/src/components/inspector/Inspector.tsx::ChromaKeySection",
    "rationale": "Enable and numeric controls exist; sampling control and semantic parity do not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-e2dc11ccd269093f",
      "control-record-a7f4988fdf17e70b",
      "control-record-679cbe8e200cd71f"
    ],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Preview/ChromaKeySamplerOverlayView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds crosshair/hover overlay that converts preview clicks into sampled image coordinates and handles escape/tab cancellation.",
    "openTakeEquivalent": "none",
    "rationale": "This is missing user-visible preview interaction, not a macOS-only product behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Preview/PreviewContainerView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Installs/removes the sampler overlay with selection/playhead state and suppresses conflicting preview interactions.",
    "openTakeEquivalent": "web/src/components/preview/Preview.tsx",
    "rationale": "OpenTake preview has scrub/zoom interaction but no sampling mode.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Preview/PreviewHitTester.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Exposes hit-test mapping needed to identify the selected clip pixel under the preview point.",
    "openTakeEquivalent": "web/src/components/preview; crates/opentake-render frame mapping",
    "rationale": "Coordinate mapping must be adapted to DOM/canvas and Rust-render paths.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Preview/VideoEngine.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Produces unkeyed active-frame samples for the eyedropper and initializes audible scrub support.",
    "openTakeEquivalent": "web/src/components/preview/previewEngine.ts; src-tauri/src/playback",
    "rationale": "OpenTake preview is split between web and native paths; both behaviors cross that boundary.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-eyedropper",
      "requirement-needed:audible-scrub"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/UI/AppTheme.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Adds shared colors/layout constants used by eyedropper, audio meter, and export activity UI.",
    "openTakeEquivalent": "web/src/index.css; component-local tokens",
    "rationale": "Theme constants are implementation details; only new controls/feedback need behavioral coverage.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:upstream-ui-token-delta"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Rendering/ChromaKeyKernelTests.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Adds regression vectors for black retention and keyed-color matte behavior.",
    "openTakeEquivalent": "crates/opentake-render/tests/gpu_effects.rs::{chroma_key_removes_green,chroma_key_keeps_non_key_color}",
    "rationale": "OpenTake has GPU chroma tests, but not the new near-black regression/vector semantics.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-matte-parity"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Rendering/CompositorRenderTests.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Verifies corrected post-effect alpha premultiplication and compositing output.",
    "openTakeEquivalent": "crates/opentake-render/tests; crates/opentake-render/src/gpu/compositor.rs",
    "rationale": "The invariant is cross-platform and needs image-based parity coverage.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:premultiplied-alpha-order"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Rendering/HistogramTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Adjusts render histogram expectations after chroma/alpha output correction.",
    "openTakeEquivalent": "none",
    "rationale": "Evidence is tied to Palmier renderer output; OpenTake needs its own fixtures.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:chroma-key-matte-parity"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Rendering/LUTLoaderTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Proves 65-point LUT parsing and upper-bound validation.",
    "openTakeEquivalent": "none",
    "rationale": "Test accompanies missing LUT behavior.",
    "linkedRequirementIds": [
      "requirement-2156cc0bdb849391"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Timeline/ClipMutationsTests.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Updates chroma mutation/default expectations used by edit and undo paths.",
    "openTakeEquivalent": "crates/opentake-ops/src/command.rs chroma tests",
    "rationale": "Chroma mutation exists, but upstream default/field model changed.",
    "linkedRequirementIds": [
      "requirement-559211270c1ef341"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/App/AppDelegate.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Begins termination gating and waits for active MLX speaker/VAD work to drain before exit.",
    "openTakeEquivalent": "src-tauri/src/playback/session.rs; none for inference-operation gate",
    "rationale": "Graceful worker drain is portable, but OpenTake uses different inference/runtime processes.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:ml-worker-shutdown"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Audio/Analysis/SpeakerIdentity.swift",
    "destination": null,
    "disposition": "platform-specific",
    "behavior": "Wraps MLX speaker embedding operations in the termination gate and conditionally compiles bundled speech.",
    "openTakeEquivalent": "OpenTake transcription/speaker analysis paths; none for MLX",
    "rationale": "MLX-specific code cannot port directly; shutdown and feature-gate invariants remain.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:ml-worker-shutdown",
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Audio/Analysis/VoiceActivity.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Gates VAD MLX work, adds chunk cancellation checks, and conditionally compiles the bundled model.",
    "openTakeEquivalent": "OpenTake transcription/VAD paths",
    "rationale": "Per-chunk cancellation is relevant but must target OpenTake's actual backend.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:ml-worker-shutdown"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Audio/AudioEnhancer.swift",
    "destination": null,
    "disposition": "platform-specific",
    "behavior": "Makes speech-enhancement implementation conditional on the bundled-speech package trait.",
    "openTakeEquivalent": "none",
    "rationale": "No exact OpenTake enhancer surface exists.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:build-feature-gating"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Audio/AudioMeter.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds 48 kHz stereo peak analysis, dB mapping, decay, 1.5 s peak hold, clipping indication/reset, and coalesced invalidation.",
    "openTakeEquivalent": "none",
    "rationale": "OpenTake has playback audio but no level-meter state or analysis.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-meter"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Audio/AudioMeterView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds the timeline-side level/peak/clipping visual meter.",
    "openTakeEquivalent": "none",
    "rationale": "User-visible monitoring is absent.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-meter"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/EditorView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Places the audio meter beside the timeline and binds it to preview playback/scrub audio.",
    "openTakeEquivalent": "web/src/components/timeline/TimelineContainer.tsx; web/src/components/preview/Preview.tsx",
    "rationale": "Timeline/preview controls exist but expose no meter.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-2334c496315c4fb0"
    ],
    "requirementGapIds": [
      "requirement-needed:audio-meter"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Preview/ScrubAudioEngine.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Implements cached PCM windows and short direction-aware faded grains for step/scrub playback; later delegates device work off-main.",
    "openTakeEquivalent": "web/src/components/preview/previewEngine.ts::scrubTo; src-tauri/src/playback/audio.rs",
    "rationale": "OpenTake explicitly silences audio during scrub; native playback audio can be reused but not assumed safe.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-ddec81c32f6fc580"
    ],
    "requirementGapIds": [
      "requirement-needed:audible-scrub"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Preview/ScrubAudioOutput.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Owns Core Audio queue operations off the main thread, coalesces to latest pending grain, and invalidates on lifecycle/device changes.",
    "openTakeEquivalent": "src-tauri/src/playback/audio.rs",
    "rationale": "Threading/lifecycle invariant is portable; Core Audio implementation is not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audible-scrub"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Search/Indexing/VisualIndexer.swift",
    "destination": null,
    "disposition": "integrated",
    "behavior": "Makes visual indexing wait for shared export activity to drain.",
    "openTakeEquivalent": "crates/opentake-media/src/index_coordinator.rs::ExportPause",
    "rationale": "OpenTake already provides a shared reference-counted export pause primitive for indexing.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:search-export-coordination"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Search/Models/VisualEmbedder.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Makes model preflight usable from detached work and returns immutable spec/readiness data for later revalidation.",
    "openTakeEquivalent": "crates/opentake-media/src/search/embedder.rs::EmbedderSpec; crates/opentake-media/src/index_coordinator.rs::work_needed",
    "rationale": "Spec types exist; the actual detached preflight runtime is explicitly deferred.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:search-preflight-concurrency"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Search/SearchIndexCoordinator.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Runs disk/model preflight off-main, snapshots asset/spec, revalidates before commit, reschedules stale work, drains cancellation, waits export, and reduces production logs.",
    "openTakeEquivalent": "crates/opentake-media/src/index_coordinator.rs::{work_needed,ExportPause}",
    "rationale": "The file itself documents that OpenTake ships only the scheduling kernel and defers the queue/runtime.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:search-preflight-concurrency",
      "requirement-needed:search-export-coordination"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Transcription/TranscriptionBackend.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Exposes source-audio extraction support used by the new audio transformation generation path.",
    "openTakeEquivalent": "crates/opentake-media/src/transcribe; src-tauri/src/playback/audio.rs",
    "rationale": "Transcription exists, but extraction for provider upload is a different lifecycle/format contract.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Utilities/Log.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Compiles debug autoclosures and stderr mirroring only in debug, reducing production overhead.",
    "openTakeEquivalent": "Rust tracing/logging configuration",
    "rationale": "Rust logging already uses a different compile/runtime filtering model; no line-for-line port is justified.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:production-logging-policy"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Utilities/MLXRuntime.swift",
    "destination": null,
    "disposition": "platform-specific",
    "behavior": "Adds a gate rejecting new MLX operations after termination starts and awaits the active-operation count reaching zero.",
    "openTakeEquivalent": "none",
    "rationale": "Exact runtime is Apple MLX; behavior should be reconsidered against OpenTake's workers/processes.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:ml-worker-shutdown"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Audio/AudioMeterTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Verifies peak/dB conversion, decay, hold, clipping reset, and invalidation coalescing.",
    "openTakeEquivalent": "none",
    "rationale": "Defines acceptance tests for missing meter behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-meter"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Audio/ScrubAudioOutputStateTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Verifies latest-grain coalescing, invalidation, and output state transitions without main-thread Core Audio calls.",
    "openTakeEquivalent": "src-tauri/src/playback/audio.rs tests",
    "rationale": "OpenTake playback tests do not cover scrub-grain ownership/state.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audible-scrub"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Search/SearchIndexCoordinatorTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Adds stale-snapshot/reschedule and off-main preflight regression coverage.",
    "openTakeEquivalent": "crates/opentake-media/src/index_coordinator.rs tests",
    "rationale": "Current OpenTake tests cover eligibility/export-pause only.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:search-preflight-concurrency"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Utilities/MLXOperationGateTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Covers rejection after termination and waiting for active MLX operations to drain.",
    "openTakeEquivalent": "none",
    "rationale": "Platform-specific test evidence; portable shutdown acceptance still needs definition.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:ml-worker-shutdown"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/TitleBarView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds export activity/history affordance and status dot linked to shared queue jobs.",
    "openTakeEquivalent": "web/src/components/shell/TitleBar.tsx export menu",
    "rationale": "Export entry exists; queue activity/history feedback does not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-f628e22a4f21d719"
    ],
    "requirementGapIds": [
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "D",
    "path": "Sources/PalmierPro/Export/ExportCoordinator.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Removes the prior single-export coordinator in favor of shared FIFO job management.",
    "openTakeEquivalent": "src-tauri/src/export.rs::ExportControl",
    "rationale": "OpenTake's control is deliberately operation-scoped and should be extended, not deleted blindly.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Export/ExportQueue.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds FIFO jobs, per-project status/progress/history, active/waiting cancellation, destination reservation, remove/clear, and completion publication.",
    "openTakeEquivalent": "src-tauri/src/export.rs::{ExportControl,ExportGuard}; web/src/components/shell/ExportDialog.tsx",
    "rationale": "Safe single-operation primitives are a base, but queue semantics are missing.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-c6c9e81870a0cc68"
    ],
    "requirementGapIds": [
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Export/ExportService.swift",
    "destination": null,
    "disposition": "integrated",
    "behavior": "Makes all export formats queue-executable and cancellation-aware while preserving existing destination content until staged commit.",
    "openTakeEquivalent": "src-tauri/src/export.rs::{export_video,ExportGuard} staged-output helpers",
    "rationale": "OpenTake already uses generation-scoped cancellation and staged commit to protect prior output.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-f350ffbc0ca5cf8f",
      "control-record-c6c9e81870a0cc68"
    ],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Export/ExportView.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Replaces modal-only progress with queue submission plus log/cancel/remove/clear UI.",
    "openTakeEquivalent": "web/src/components/shell/ExportDialog.tsx",
    "rationale": "Form and live cancel exist; multi-job log/history controls do not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [
      "control-record-f350ffbc0ca5cf8f",
      "control-record-c6c9e81870a0cc68"
    ],
    "requirementGapIds": [
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Export/HDRVideoExporter.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Threads cooperative cancellation through HDR export work.",
    "openTakeEquivalent": "src-tauri/src/export.rs::ExportGuard",
    "rationale": "OpenTake cancellation checks are format-agnostic in its export operation, though HDR parity itself is separate.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Export/PalmierProjectExporter.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Makes project-bundle export cancellable and transactional within queue jobs.",
    "openTakeEquivalent": "src-tauri/src/export.rs project archive path; web/src/components/shell/ExportDialog.tsx",
    "rationale": "OpenTake UI explicitly says project bundling has no cooperative cancel even though video export does.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety",
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Models/MediaResolver.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Makes resolver/export reads cancellation-aware for queued export operations.",
    "openTakeEquivalent": "crates/opentake-render media resolver; src-tauri/src/export.rs",
    "rationale": "OpenTake guards decode/render work with operation cancellation through a different resolver design.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Export/ExportQueueTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Exercises FIFO order, destination conflicts, per-project views, waiting/active cancellation, remove, clear, and progress.",
    "openTakeEquivalent": "src-tauri/src/export.rs cancellation tests; none for queue",
    "rationale": "Existing tests prove control safety, not scheduler behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-queue"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Export/ExportServiceRoundTripTests.swift",
    "destination": null,
    "disposition": "integrated",
    "behavior": "Adds cancellation/staging round trips proving an existing destination survives aborted export.",
    "openTakeEquivalent": "src-tauri/src/export.rs staged-output and stale-generation tests",
    "rationale": "OpenTake already has high-signal tests for generation isolation and commit refusal after cancel.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety"
    ]
  },
  {
    "status": "M",
    "path": "Tests/PalmierProTests/Export/PalmierProjectExportTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Adds project-bundle cancellation and existing-output preservation coverage.",
    "openTakeEquivalent": "src-tauri/src/export.rs archive tests; none for bundle cooperative cancel",
    "rationale": "The bundle-specific cancellation gap remains despite shared staging concepts.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:export-cancellation-safety"
    ]
  },
  {
    "status": "R100",
    "path": "Sources/PalmierPro/Account/BackendConfig.swift",
    "destination": "Sources/PalmierPro/Backend/BackendConfig.swift",
    "disposition": "equivalent",
    "behavior": "Moves backend configuration into a shared backend namespace for generation workflows; content is unchanged.",
    "openTakeEquivalent": "crates/opentake-gen/src/keys.rs; provider configuration",
    "rationale": "OpenTake already centralizes provider configuration independently of account UI.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Backend/BackendError.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Introduces shared backend error decoding used by storage and generation requests.",
    "openTakeEquivalent": "crates/opentake-gen/src/error.rs",
    "rationale": "OpenTake already has provider-neutral generation errors.",
    "linkedRequirementIds": [
      "requirement-43d222897dd2b2dc"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "R084",
    "path": "Sources/PalmierPro/Generation/BackendStorage.swift",
    "destination": "Sources/PalmierPro/Backend/BackendStorage.swift",
    "disposition": "requires-reconciliation",
    "behavior": "Moves storage upload/download into shared backend code and extends it for transform source media.",
    "openTakeEquivalent": "crates/opentake-gen/src/transport.rs; provider adapters",
    "rationale": "BYOK adapters own transport, but reusable source upload/lifetime semantics need work.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/ViewModel/EditorViewModel+AIEdit.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Allows AI Edit on standalone audio or linked A/V and dispatches cleanup/dubbing edit kinds.",
    "openTakeEquivalent": "web/src/components/inspector; none for AI audio transforms",
    "rationale": "No equivalent edit action exists in the current web editor.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Catalog/AudioModelConfig.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds cleanup/dubbing input capability, source requirements, target-language validation, and duration-preservation rules.",
    "openTakeEquivalent": "crates/opentake-gen/src/catalog/entry.rs; crates/opentake-gen/src/params.rs",
    "rationale": "Catalog machinery exists, but these model capabilities/constraints are absent.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Catalog/CostEstimator.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Accounts for source-duration cleanup/dubbing requests in estimated cost.",
    "openTakeEquivalent": "crates/opentake-gen/src/catalog; none for transform costing",
    "rationale": "Money-impacting estimates must match provider rules before exposing the workflow.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Catalog/ModelCatalog.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Registers ElevenLabs voice-isolation and dubbing models/categories.",
    "openTakeEquivalent": "crates/opentake-gen/src/catalog/entry.rs ElevenLabs entries",
    "rationale": "ElevenLabs TTS/music/SFX exist, but cleanup/dubbing catalog entries do not.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Edit/AIEditMenu.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds cleanup/dub actions with media eligibility and target-language flow.",
    "openTakeEquivalent": "none",
    "rationale": "Missing user entry point.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Edit/AudioTransformEditKind.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Defines cleanup versus dubbing behavior, titles, language requirements, and model mapping.",
    "openTakeEquivalent": "none",
    "rationale": "Product/domain concept is missing.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Edit/EditSubmitter+AudioTransform.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Extracts the selected/linked source, submits transform generation, and places the result while preserving source length.",
    "openTakeEquivalent": "crates/opentake-gen/src/client.rs; src-tauri media import/edit paths",
    "rationale": "Requires a new vertical path across extraction, provider submission, import, and timeline placement.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "C060",
    "path": "Sources/PalmierPro/Generation/Edit/EditSubmitter.swift",
    "destination": "Sources/PalmierPro/Generation/Edit/EditSubmitter+Rerun.swift",
    "disposition": "portable",
    "behavior": "Splits rerun behavior from the submitter while preserving prior generation inputs, including transform inputs.",
    "openTakeEquivalent": "crates/opentake-project::GenerationLog; generation rerun UI none",
    "rationale": "Generation log exists, but rerun execution/UI is not wired.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:generation-rerun"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Edit/EditSubmitter+Seeds.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Splits seed derivation/reuse helpers out of the submitter during transform refactor.",
    "openTakeEquivalent": "crates/opentake-gen/src/params.rs seed fields",
    "rationale": "OpenTake supports provider parameters; exact helper organization is not portable.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:generation-seed-reuse"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Edit/EditSubmitter+Upscale.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Splits upscale submission from the common submitter during transform refactor.",
    "openTakeEquivalent": "crates/opentake-agent/src/mcp/dispatch.rs UpscaleMedia stub; generation provider paths",
    "rationale": "OpenTake advertises upscale but the agent body is explicitly unwired.",
    "linkedRequirementIds": [
      "requirement-bffafcccb447be75"
    ],
    "linkedControlIds": [],
    "requirementGapIds": []
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Edit/EditSubmitter.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Refactors common submit flow to dispatch rerun/upscale/audio-transform specializations and persist richer inputs.",
    "openTakeEquivalent": "crates/opentake-gen/src/client.rs; crates/opentake-project::GenerationLog",
    "rationale": "Building blocks exist without the full editor submission orchestrator.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation",
      "requirement-needed:generation-rerun"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/GenerationBackend.swift",
    "destination": null,
    "disposition": "cloud-specific",
    "behavior": "Adds backend procedures/arguments for cleanup and dubbing source uploads, including target language.",
    "openTakeEquivalent": "crates/opentake-gen/src/provider/elevenlabs.rs::ElevenLabsAdapter",
    "rationale": "Palmier Convex/backend calls cannot be copied; direct ElevenLabs BYOK endpoints need separate implementation.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/GenerationService.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Persists completed-generation metadata through batched manifest updates.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs generation log/media APIs",
    "rationale": "OpenTake generation integration is incomplete and manifest writes are direct.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching",
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Preprocessing/AudioTrackExtractor.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Extracts upload-ready audio from video sources for isolation/dubbing.",
    "openTakeEquivalent": "src-tauri/src/playback/audio.rs decode helpers; none for provider upload extraction",
    "rationale": "Decode infrastructure exists, but a bounded temporary-file/upload contract is missing.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Preprocessing/TrimmedSource.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Represents exact selected clip/media ranges and preserves source duration across preprocessing.",
    "openTakeEquivalent": "crates/opentake-domain/src/clip.rs trim fields; media resolution helpers",
    "rationale": "Timeline trim semantics exist; generation preprocessing does not consume them end-to-end.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "R100",
    "path": "Sources/PalmierPro/Generation/VideoCompressor.swift",
    "destination": "Sources/PalmierPro/Generation/Preprocessing/VideoCompressor.swift",
    "disposition": "equivalent",
    "behavior": "Moves unchanged video compression under the shared preprocessing namespace.",
    "openTakeEquivalent": "OpenTake generation/media preprocessing paths",
    "rationale": "Pure organization change; no framework-structure port needed.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:generation-preprocessing-layout"
    ]
  },
  {
    "status": "R077",
    "path": "Sources/PalmierPro/Generation/Edit/VideoTrimExtractor.swift",
    "destination": "Sources/PalmierPro/Generation/Preprocessing/VideoTrimExtractor.swift",
    "disposition": "requires-reconciliation",
    "behavior": "Moves trim extraction to shared preprocessing and adapts it to generic trimmed source/audio-transform use.",
    "openTakeEquivalent": "OpenTake trim/decode helpers",
    "rationale": "Behavior is reusable, but OpenTake needs cross-platform ffmpeg-backed extraction.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/Submission/AudioGenerationSubmission.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Supports source-backed audio generation categories, target language, and source-length defaults.",
    "openTakeEquivalent": "crates/opentake-gen/src/params.rs; crates/opentake-gen/src/provider/elevenlabs.rs",
    "rationale": "Existing request types do not model these transform inputs.",
    "linkedRequirementIds": [
      "requirement-d4cf765d956089f4"
    ],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "A",
    "path": "Sources/PalmierPro/Generation/Submission/GenerationInput+AudioSource.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Persists source-media and target-language metadata for rerun/audit of audio transforms.",
    "openTakeEquivalent": "crates/opentake-domain::MediaManifestEntry::generation_input; crates/opentake-project::GenerationLog",
    "rationale": "Persistence containers exist, but schema/round-trip fields need extension.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/GenerationView+ModelState.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Selects/validates cleanup and dubbing models based on source kind and language.",
    "openTakeEquivalent": "web generation UI none; crates/opentake-gen/src/catalog",
    "rationale": "Catalog alone does not provide model-state UI.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/GenerationView+References.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Allows audio/video transform source selection and derives eligible references.",
    "openTakeEquivalent": "web media library/store; generation UI none",
    "rationale": "Missing reference picker behavior.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/GenerationView+Settings.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds target-language settings and hides irrelevant prompt controls for transform models.",
    "openTakeEquivalent": "none",
    "rationale": "Missing conditional settings UI and validation.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/GenerationView+Submit.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Builds source-backed cleanup/dubbing submissions and validates required language/source before spending.",
    "openTakeEquivalent": "crates/opentake-gen/src/client.rs; web generation submit none",
    "rationale": "Cost-bearing validation must be implemented end-to-end.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/GenerationView.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds cleanup/dubbing categories and state wiring to the main generation panel.",
    "openTakeEquivalent": "OpenTake generation panel none",
    "rationale": "No equivalent integrated generation panel is present.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Generation/UI/ReferenceControls.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Extends reference controls to accept and label audio/video transform sources.",
    "openTakeEquivalent": "none",
    "rationale": "Missing source UI.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Inspector/Tabs/AIEditTab.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Presents audio cleanup/dubbing actions for eligible timeline selections and target language.",
    "openTakeEquivalent": "none",
    "rationale": "Missing inspector entry point.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Models/MediaManifest.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Extends stored generation input with target language/source transform metadata.",
    "openTakeEquivalent": "crates/opentake-domain/src/media.rs::{MediaManifest,MediaManifestEntry}",
    "rationale": "OpenTake schema has generation input support but lacks these new fields.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Timeline/TimelineView+AIEditMenu.swift",
    "destination": null,
    "disposition": "portable",
    "behavior": "Adds cleanup/dub context-menu actions for standalone audio and linked A/V clips.",
    "openTakeEquivalent": "web/src/components/timeline/TimelineContainer.tsx context menus",
    "rationale": "Timeline menu exists but has no audio transform actions.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/UI/CapsuleButton.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Generalizes capsule-button presentation for new audio transform actions.",
    "openTakeEquivalent": "shared web button styles/components",
    "rationale": "Styling implementation is not a behavior gap by itself.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/UI/HoverHighlight.swift",
    "destination": null,
    "disposition": "equivalent",
    "behavior": "Generalizes hover highlight used by new generation/reference controls.",
    "openTakeEquivalent": "CSS hover styles/components",
    "rationale": "Styling implementation is already idiomatic in web UI.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:audio-transform-generation"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Folders.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Applies folder-derived metadata updates through the batch manifest path.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs media folder mutations",
    "rationale": "Folder mutation exists, but batching/flush semantics do not.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/ViewModel/EditorViewModel+MediaLibrary.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Queues metadata updates, batches after 20 ms or 64 items, avoids stale/deleted assets, and lowers production logging.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs::{set_media_favorite,restore_media,save_media_manifest}",
    "rationale": "Direct session mutations are safe but can cause repeated copies/writes and lack stale-batch guards.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Editor/ViewModel/EditorViewModel.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Owns chroma/audio/export state and flushes pending manifest metadata before undo snapshots and saves.",
    "openTakeEquivalent": "web stores; crates/opentake-core/src/session.rs::EditorSession",
    "rationale": "Multiple new state machines cross OpenTake's web/Rust boundary; only base edit/session state exists.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching",
      "requirement-needed:export-queue",
      "requirement-needed:audible-scrub"
    ]
  },
  {
    "status": "M",
    "path": "Sources/PalmierPro/Project/VideoProject.swift",
    "destination": null,
    "disposition": "requires-reconciliation",
    "behavior": "Ensures close/save snapshots flush pending manifest metadata and repeats save until no concurrent unsaved changes remain.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs::{save_project,save_media_manifest}; src-tauri/src/commands.rs project close/save",
    "rationale": "Atomic save exists, but no documented loop/barrier against concurrent post-snapshot mutation.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:project-close-save-barrier",
      "requirement-needed:manifest-metadata-batching"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Media/ManifestMetadataTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Tests batch coalescing, deletion/replacement staleness, threshold flush, undo/save flush, and restore updates.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs tests",
    "rationale": "OpenTake tests direct manifest operations, not this concurrency/performance contract.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:manifest-metadata-batching"
    ]
  },
  {
    "status": "A",
    "path": "Tests/PalmierProTests/Project/ProjectClosePersistenceTests.swift",
    "destination": null,
    "disposition": "test-doc-only",
    "behavior": "Verifies final save before close and catches edits arriving during an in-flight save.",
    "openTakeEquivalent": "crates/opentake-core/src/session.rs save/reopen tests",
    "rationale": "Current tests prove round-trip/atomic write but not concurrent close persistence.",
    "linkedRequirementIds": [],
    "linkedControlIds": [],
    "requirementGapIds": [
      "requirement-needed:project-close-save-barrier"
    ]
  }
].map((entry) => Object.freeze({
    ...entry,
    linkedRequirementIds: Object.freeze(entry.linkedRequirementIds),
    linkedControlIds: Object.freeze(entry.linkedControlIds),
    requirementGapIds: Object.freeze(entry.requirementGapIds),
  })),
);

function reviewedPalmierKey({ status, path, destination }) {
  return status + "\\0" + path + "\\0" + (destination ?? "");
}

const PALMIER_REVIEWED_PATH_BY_KEY = new Map(
  PALMIER_REVIEWED_PATH_LEDGER.map((entry) => [reviewedPalmierKey(entry), entry]),
);

export function reviewPalmierChangedPaths(changedPaths) {
  if (!Array.isArray(changedPaths)) {
    throw new Error("Palmier changed paths must be an array");
  }
  if (changedPaths.length !== PALMIER_REVIEWED_PATH_LEDGER.length) {
    throw new Error("Palmier reviewed ledger expected " + PALMIER_REVIEWED_PATH_LEDGER.length + " paths, found " + changedPaths.length);
  }
  const seen = new Set();
  const reviewed = changedPaths.map((record) => {
    const key = reviewedPalmierKey(record);
    const entry = PALMIER_REVIEWED_PATH_BY_KEY.get(key);
    if (!entry) {
      throw new Error("Palmier changed path is not in reviewed ledger: " + record.status + " " + record.path);
    }
    if (seen.has(key)) {
      throw new Error("Palmier changed path is duplicated: " + record.path);
    }
    seen.add(key);
    return { ...record, ...entry };
  });
  if (seen.size !== PALMIER_REVIEWED_PATH_LEDGER.length) {
    const missing = PALMIER_REVIEWED_PATH_LEDGER.find((entry) => !seen.has(reviewedPalmierKey(entry)));
    throw new Error("Palmier reviewed path is missing from Git diff: " + missing.path);
  }
  return reviewed;
}

const CANONICAL_INTEGRATED_PATHS = new Set([
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
]);

const CANONICAL_EQUIVALENT_PATHS = new Set([
  "README.md",
  "web/package.json",
  "web/src/store/editActions.ts",
  "web/src/store/uiStore.ts",
]);

const CANONICAL_STALE_PATHS = new Set([
  ".github/workflows/ci.yml",
  "src-tauri/Cargo.toml",
]);

function assertPinnedRef(name, path, ref, expectedSha, expectedTree) {
  const actualSha = assertFullSha(name, ref, gitText(name, path, ["rev-parse", ref]));
  const actualTree = assertFullSha(
    name,
    `${ref} tree`,
    gitText(name, path, ["rev-parse", `${ref}^{tree}`]),
  );
  if (actualSha !== expectedSha || actualTree !== expectedTree) {
    throw new Error(`${name}: ${ref} moved from pinned SHA/tree`);
  }
}

function readLiveOpenPullRequestItems() {
  const args = [
    "pr", "list", "--repo", CAPTURED_OPEN_PULL_REQUESTS.repository, "--state", "open",
    "--json", "number,title,headRefName,baseRefName,url",
  ];
  let items;
  try {
    items = JSON.parse(execFileSync("gh", args, {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }));
  } catch (error) {
    const detail = error.stderr?.toString("utf8").trim();
    throw new Error(`live gh open-PR readback failed${detail ? `: ${detail}` : ""}`);
  }
  if (!Array.isArray(items)) {
    throw new Error("live gh open-PR readback did not return an array");
  }
  return items;
}

export function verifyOpenPullRequests({ readLive = readLiveOpenPullRequestItems } = {}) {
  if (typeof readLive !== "function") {
    throw new Error("open-PR verification requires a live reader function");
  }
  const items = readLive();
  if (!Array.isArray(items)) {
    throw new Error("live open-PR reader did not return an array");
  }
  const sortedItems = items
    .map((item) => ({ ...item }))
    .sort((left, right) => left.number - right.number);
  if (JSON.stringify(sortedItems) !== JSON.stringify(CAPTURED_OPEN_PULL_REQUESTS.items)) {
    throw new Error("live open-PR state differs from immutable capture");
  }
  return {
    status: "match",
    capturedAt: CAPTURED_OPEN_PULL_REQUESTS.capturedAt,
    command: CAPTURED_OPEN_PULL_REQUESTS.command,
    count: sortedItems.length,
  };
}

function branchIndexForRemote(root, remote) {
  const targetMain = SOURCE_PINS.targetMain;
  const forkMain = SOURCE_PINS.forkMains[remote].sha;
  const refs = gitText("fork branch index", root, [
    "for-each-ref",
    "--format=%(refname:short)",
    `refs/remotes/${remote}`,
  ])
    .split("\n")
    .filter((ref) => ref && ref !== remote && ref !== `${remote}/main`)
    .sort(compareAuditText);
  return refs.map((ref) => {
    const tip = assertFullSha(ref, "tip", gitText(ref, root, ["rev-parse", ref]));
    const tree = assertFullSha(ref, "tree", gitText(ref, root, ["rev-parse", `${ref}^{tree}`]));
    const targetMergeBase = assertFullSha(
      ref,
      "target merge base",
      gitText(ref, root, ["merge-base", tip, targetMain]),
    );
    const forkMainMergeBase = assertFullSha(
      ref,
      "fork merge base",
      gitText(ref, root, ["merge-base", tip, forkMain]),
    );
    const targetAncestor = targetMergeBase === tip;
    const forkMainAncestor = forkMainMergeBase === tip;
    const targetUniqueCommitCount = parseCount(
      ref,
      "target unique count",
      gitText(ref, root, ["rev-list", "--count", `${targetMain}..${tip}`]),
    );
    const forkMainUniqueCommitCount = parseCount(
      ref,
      "fork unique count",
      gitText(ref, root, ["rev-list", "--count", `${forkMain}..${tip}`]),
    );
    const forkDeltaRaw = gitRead(
      ref,
      root,
      [
        "diff", "--name-status", "-z", "--find-renames", "--find-copies-harder",
        `${forkMain}..${tip}`,
      ],
      null,
    );
    const forkMainChangedPathCount = parseNameStatus(ref, forkDeltaRaw).length;
    const hasUniqueCommits = targetUniqueCommitCount > 0 || forkMainUniqueCommitCount > 0;
    const status = targetAncestor
      ? "integrated-target"
      : forkMainAncestor
        ? "integrated-fork-main"
        : hasUniqueCommits
          ? "relevant-unmerged"
          : "empty";
    return {
      ref,
      remote,
      tip,
      tree,
      targetMain,
      forkMain,
      targetMergeBase,
      forkMainMergeBase,
      targetAncestor,
      forkMainAncestor,
      targetUniqueCommitCount,
      forkMainUniqueCommitCount,
      forkMainChangedPathCount,
      forkMainDelta: forkMainChangedPathCount === 0 ? "empty" : "changes",
      status,
      separatelyRelevant: status === "relevant-unmerged",
    };
  });
}

function integratedForkMainSource(root, remote) {
  const comparison = captureGitSource({
    name: `${remote} main ancestry`,
    path: root,
    base: SOURCE_PINS.forkMains[remote].sha,
    head: SOURCE_PINS.targetMain,
    remote,
    expectChanges: true,
  });
  if (comparison.mergeBase !== comparison.base || comparison.behindCount !== 0) {
    throw new Error(`${remote}/main is not an integrated ancestor of target main`);
  }
  return {
    ...comparison,
    status: "integrated-ancestor",
    comparisonMode: "ancestry-only",
    targetRepository: gitText("target main", root, ["remote", "get-url", "origin"]),
    targetAheadCount: comparison.commitCount,
    forkUniqueCount: comparison.behindCount,
    integratedPathDeltaCount: comparison.changedPaths.length,
    changedPaths: [],
  };
}

function classifyCanonicalPath(entry) {
  const { path, status } = entry;
  const relationship = CANONICAL_INTEGRATED_PATHS.has(path)
    ? "integrated"
    : CANONICAL_EQUIVALENT_PATHS.has(path)
      ? "equivalent"
      : CANONICAL_STALE_PATHS.has(path)
        ? "stale"
        : status === "??"
          ? "requires-reconciliation"
          : "conflict";
  const playback = path.includes("playback")
    || path.includes("preview")
    || path.includes("mpv")
    || path === "src-tauri/src/lib.rs"
    || path === "src-tauri/src/media.rs"
    || path === "web/src/App.tsx"
    || path === "web/src/lib/api.ts"
    || path.startsWith("web/src/store/")
    || path === "web/src/i18n/dict.ts"
    || path === "docs/architecture/PLAYBACK-ENGINE.md";
  const documentation = path.endsWith(".md");
  const build = path === "Cargo.lock" || path.endsWith("Cargo.toml")
    || path === "web/package.json" || path === "web/pnpm-lock.yaml"
    || path.startsWith(".github/") || path === ".gitignore";
  const behavior = playback
    ? "Canonical native-playback/session delta relative to the later audited convergence implementation."
    : documentation
      ? "Canonical handoff or QA evidence delta relative to the audited documentation set."
      : build
        ? "Canonical build/dependency delta associated with the earlier playback implementation."
        : "Canonical UI or state delta associated with the earlier playback convergence work.";
  const linkedRequirementIds = playback
    ? [REQUIREMENT.playback, REQUIREMENT.playbackLifecycle]
    : [];
  const linkedControlIds = path === "web/src/components/media/MediaSearch.tsx"
    ? CONTROL.searchIndex
    : [];
  let disposition;
  let requirementGap = null;
  if (relationship === "integrated") {
    disposition = "integrated";
  } else if (relationship === "equivalent") {
    disposition = "equivalent";
  } else if (relationship === "stale") {
    disposition = "obsolete";
  } else if (linkedRequirementIds.length > 0 || linkedControlIds.length > 0) {
    disposition = "requires-reconciliation";
  } else {
    disposition = "requirement-needed:canonical-three-way-reconciliation";
    requirementGap = disposition;
  }
  return {
    relationship,
    disposition,
    requirementGap,
    behavior,
    openTakeEquivalent: status.includes("D")
      ? `Audited target intentionally deletes or supersedes ${path}`
      : `Audited target path ${path} at tree ${SOURCE_PINS.targetMainTree}`,
    linkedRequirementIds,
    linkedControlIds,
  };
}

function evidenceClassificationRows(evidence) {
  return [
    ...evidence.sources.flatMap(({ changedPaths }) => changedPaths),
    ...evidence.canonicalDirtyCheckout.paths,
  ];
}

export function validateSourceEvidenceShape(evidence) {
  if (!evidence || typeof evidence !== "object") {
    throw new Error("source evidence must be an object");
  }
  if (!Array.isArray(evidence.sources) || evidence.sources.length === 0) {
    throw new Error("source evidence must contain sources");
  }
  if (!Array.isArray(evidence.branchIndex)) {
    throw new Error("source evidence branchIndex must be an array");
  }
  if (!evidence.canonicalDirtyCheckout
      || !Array.isArray(evidence.canonicalDirtyCheckout.paths)) {
    throw new Error("source evidence canonical dirty paths must be an array");
  }
  if (!evidence.openPullRequests
      || !Array.isArray(evidence.openPullRequests.items)
      || evidence.openPullRequests.count !== evidence.openPullRequests.items.length
      || typeof evidence.openPullRequests.capturedAt !== "string") {
    throw new Error("source evidence open-PR capture is malformed");
  }
  for (const source of evidence.sources) {
    if (!Array.isArray(source.changedPaths)) {
      throw new Error(`source ${source.name ?? "unknown"} changedPaths must be an array`);
    }
  }
  for (const row of evidenceClassificationRows(evidence)) {
    if (![row.disposition, row.behavior, row.openTakeEquivalent]
      .every((value) => typeof value === "string" && value.length > 0)) {
      throw new Error(`source classification is incomplete for ${row.path ?? "unknown path"}`);
    }
    if (row.disposition === "unverified") {
      throw new Error(`unverified source change: ${row.path ?? "unknown path"}`);
    }
    if (!Array.isArray(row.linkedRequirementIds) || !Array.isArray(row.linkedControlIds)) {
      throw new Error(`source classification links are malformed for ${row.path ?? "unknown path"}`);
    }
  }
  return evidence;
}

function catalogIds(name, catalog) {
  if (!catalog || !Array.isArray(catalog.records)) {
    throw new Error(`${name} catalog must contain a records array`);
  }
  const ids = catalog.records.map(({ id }) => id);
  if (ids.some((id) => typeof id !== "string" || id.length === 0)) {
    throw new Error(`${name} catalog contains an invalid id`);
  }
  if (new Set(ids).size !== ids.length) {
    throw new Error(`${name} catalog contains duplicate ids`);
  }
  return new Set(ids);
}

export function validateSourceEvidenceCatalogs(evidence, { requirements, controls }) {
  validateSourceEvidenceShape(evidence);
  const requirementIds = catalogIds("requirement", requirements);
  const controlIds = catalogIds("control", controls);
  for (const row of evidenceClassificationRows(evidence)) {
    for (const id of row.linkedRequirementIds) {
      if (!requirementIds.has(id)) {
        throw new Error(`stale requirement id ${id} on ${row.path}`);
      }
    }
    for (const id of row.linkedControlIds) {
      if (!controlIds.has(id)) {
        throw new Error(`stale control id ${id} on ${row.path}`);
      }
    }
  }
  return evidence;
}

function readSourceEvidenceCatalogs(root) {
  return {
    requirements: JSON.parse(readFileSync(
      resolve(root, "docs/audit/2026-07-14/requirements.json"),
      "utf8",
    )),
    controls: JSON.parse(readFileSync(
      resolve(root, "docs/audit/2026-07-14/controls.json"),
      "utf8",
    )),
  };
}

export function buildSourceEvidence({
  root,
  palmierPath,
  canonicalPath,
  operations = {},
}) {
  const sourceOperations = {
    assertPinnedRef,
    captureGitSource,
    branchIndexForRemote,
    integratedForkMainSource,
    captureDirtyCheckout,
    readSourceEvidenceCatalogs,
    ...operations,
  };
  sourceOperations.assertPinnedRef("target", root, SOURCE_PINS.targetStart, SOURCE_PINS.targetStart, SOURCE_PINS.targetStartTree);
  sourceOperations.assertPinnedRef("target", root, "origin/main", SOURCE_PINS.targetMain, SOURCE_PINS.targetMainTree);
  sourceOperations.assertPinnedRef(
    "Palmier Pro",
    palmierPath,
    SOURCE_PINS.palmierPrevious,
    SOURCE_PINS.palmierPrevious,
    SOURCE_PINS.palmierPreviousTree,
  );
  sourceOperations.assertPinnedRef("Palmier Pro", palmierPath, "origin/main", SOURCE_PINS.palmierMain, SOURCE_PINS.palmierMainTree);
  for (const [remote, pin] of Object.entries(SOURCE_PINS.forkMains)) {
    sourceOperations.assertPinnedRef(remote, root, `${remote}/main`, pin.sha, pin.tree);
  }
  sourceOperations.assertPinnedRef("canonical dirty checkout", canonicalPath, "HEAD", SOURCE_PINS.canonicalHead, SOURCE_PINS.canonicalTree);

  const target = sourceOperations.captureGitSource({
    name: "target cloud main vs local start",
    path: root,
    base: SOURCE_PINS.targetStart,
    head: SOURCE_PINS.targetMain,
    remote: "origin",
    expectChanges: true,
  });
  if (target.status !== "equivalent-tree" || target.changedPaths.length !== 0) {
    throw new Error("target cloud main no longer matches the pinned local starting tree");
  }

  const palmier = sourceOperations.captureGitSource({
    name: "Palmier Pro refreshed main",
    path: palmierPath,
    base: SOURCE_PINS.palmierPrevious,
    head: SOURCE_PINS.palmierMain,
    remote: "origin",
    requireClean: true,
    expectChanges: true,
  });
  palmier.changedPaths = reviewPalmierChangedPaths(palmier.changedPaths);

  const branches = Object.keys(SOURCE_PINS.forkMains)
    .sort(compareAuditText)
    .flatMap((remote) => sourceOperations.branchIndexForRemote(root, remote));
  if (branches.length !== 21) {
    throw new Error(`fork branch index expected 21 non-main heads, found ${branches.length}`);
  }
  const relevant = branches.filter(({ separatelyRelevant }) => separatelyRelevant);
  const relevantSources = relevant.map((branch) => {
    const source = sourceOperations.captureGitSource({
      name: branch.ref,
      path: root,
      base: branch.forkMain,
      head: branch.tip,
      remote: branch.remote,
      expectChanges: true,
    });
    source.changedPaths = source.changedPaths.map((record) => ({
      ...record,
      disposition: `requirement-needed:downstream-${branch.ref.replace(/[^a-z0-9]+/gi, "-").toLowerCase()}`,
      behavior: `Unique downstream delta from ${branch.ref}.`,
      openTakeEquivalent: "No integrated target equivalent; independent review required",
      linkedRequirementIds: [],
      linkedControlIds: [],
    }));
    return source;
  });

  const canonical = sourceOperations.captureDirtyCheckout({
    name: "canonical dirty OpenTake checkout",
    path: canonicalPath,
  });
  if (canonical.head !== SOURCE_PINS.canonicalHead || canonical.tree !== SOURCE_PINS.canonicalTree) {
    throw new Error("canonical dirty checkout moved from its pinned HEAD/tree");
  }
  if (canonical.statusSha256 !== FETCH_PORCELAIN_SHA256.canonical) {
    throw new Error("canonical dirty checkout porcelain bytes changed after fetch");
  }
  canonical.captureManifestSha256 = canonical.manifestSha256;
  canonical.paths = canonical.paths.map((entry) => ({
    ...entry,
    ...classifyCanonicalPath(entry),
  }));
  canonical.manifestSha256 = createHash("sha256")
    .update(JSON.stringify(canonical.paths))
    .digest("hex");
  canonical.relationshipCounts = canonical.paths.reduce((counts, entry) => {
    counts[entry.relationship] = (counts[entry.relationship] ?? 0) + 1;
    return counts;
  }, {});
  const expectedCanonicalCounts = {
    conflict: 36,
    equivalent: 4,
    integrated: 10,
    "requires-reconciliation": 4,
    stale: 2,
  };
  if (Object.keys(canonical.relationshipCounts).length !== Object.keys(expectedCanonicalCounts).length
      || Object.entries(expectedCanonicalCounts).some(
        ([relationship, count]) => canonical.relationshipCounts[relationship] !== count,
      )) {
    throw new Error("canonical dirty checkout relationship coverage changed");
  }

  const evidence = {
    schema: 1,
    auditDate: "2026-07-14",
    generation: {
      deterministicOrdering: "UTF-8 byte ordering after slash normalization; filenames are not Unicode-normalized",
      hashAlgorithm: "SHA-256",
      sourceRefs: "full immutable 40-character commit SHAs",
    },
    fetchEvidence: {
      serializedOrder: [
        "git -C ../palmier-pro-upstream fetch --prune origin",
        "git fetch --prune origin",
        "git fetch --prune H-Chris233",
        "git fetch --prune cuic19053-hue",
      ],
      target: { pre: FETCH_PORCELAIN_SHA256.target, post: FETCH_PORCELAIN_SHA256.target },
      canonical: { pre: FETCH_PORCELAIN_SHA256.canonical, post: FETCH_PORCELAIN_SHA256.canonical },
      palmier: { pre: FETCH_PORCELAIN_SHA256.palmier, post: FETCH_PORCELAIN_SHA256.palmier },
      workingFilesUnchanged: true,
    },
    openPullRequests: capturedOpenPullRequests(),
    sources: [
      target,
      palmier,
      ...Object.keys(SOURCE_PINS.forkMains)
        .sort(compareAuditText)
        .map((remote) => sourceOperations.integratedForkMainSource(root, remote)),
      ...relevantSources,
    ],
    branchIndex: branches,
    branchSummary: {
      totalNonMainHeads: branches.length,
      integratedTarget: branches.filter(({ status }) => status === "integrated-target").length,
      integratedForkMain: branches.filter(({ status }) => status === "integrated-fork-main").length,
      empty: branches.filter(({ status }) => status === "empty").length,
      emptyForkMainDeltas: branches.filter(({ forkMainDelta }) => forkMainDelta === "empty").length,
      relevantUnmerged: relevant.length,
    },
    canonicalDirtyCheckout: canonical,
  };
  validateSourceEvidenceCatalogs(evidence, sourceOperations.readSourceEvidenceCatalogs(root));
  return evidence;
}

function markdown(value) {
  return String(value ?? "—")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("\\", "&#92;")
    .replaceAll("|", "&#124;")
    .replaceAll("`", "&#96;")
    .replace(/\r\n?|\n/g, "&#10;");
}

const REQUIREMENT_STATUS_MEANINGS = {
  complete: "Current implementation and exact verification evidence support the statement.",
  contradicted: "Current evidence disproves the source statement.",
  duplicate: "The candidate is context or repeats another adjudicated requirement.",
  incomplete: "A current gap remains with an exact closure contract.",
  obsolete: "A newer decision or implementation supersedes the statement.",
  unverified: "The candidate has not received a final disposition.",
};

export function renderDocumentReconciliation(requirementsBytes) {
  if (!Buffer.isBuffer(requirementsBytes) && !(requirementsBytes instanceof Uint8Array)) {
    throw new Error("document reconciliation needs the exact requirements.json bytes");
  }
  const bytes = Buffer.from(requirementsBytes);
  let requirements;
  try {
    requirements = JSON.parse(bytes.toString("utf8"));
  } catch (error) {
    throw new Error(`document reconciliation requirements JSON is invalid: ${error.message}`);
  }
  if (requirements?.schema !== 1 || !Array.isArray(requirements.records)) {
    throw new Error("document reconciliation needs schema 1 requirements records[]");
  }
  const candidateIds = new Set();
  const recordIds = new Set();
  const statusCounts = Object.fromEntries(Object.keys(REQUIREMENT_STATUS_MEANINGS).map((status) => [status, 0]));
  const gapCounts = new Map();
  const active = [];
  for (const record of requirements.records) {
    if (!record || typeof record !== "object") throw new Error("document reconciliation record must be an object");
    if (!nonEmptyString(record.id) || recordIds.has(record.id)) {
      throw new Error(`document reconciliation record ID is missing or duplicate: ${String(record.id)}`);
    }
    if (!nonEmptyString(record.candidateId) || candidateIds.has(record.candidateId)) {
      throw new Error(`document reconciliation candidate ID is missing or duplicate: ${String(record.candidateId)}`);
    }
    if (!Object.hasOwn(REQUIREMENT_STATUS_MEANINGS, record.status)) {
      throw new Error(`document reconciliation status is unsupported: ${String(record.status)}`);
    }
    if (
      !record.source || typeof record.source !== "object" || Array.isArray(record.source)
      || !nonEmptyString(record.source.path)
      || !Number.isInteger(record.source?.line) || record.source.line < 1
      || !nonEmptyString(record.targetBehavior)
    ) {
      throw new Error(`document reconciliation source or targetBehavior shape is invalid: ${record.id}`);
    }
    for (const field of ["storeApi", "automatedTests", "runtimeEvidence", "acceptanceCriteria"]) {
      if (!Array.isArray(record[field]) || record[field].some((value) => !nonEmptyString(value))) {
        throw new Error(`document reconciliation ${field} must be an array of non-empty strings: ${record.id}`);
      }
    }
    if (record.finalDisposition != null && !nonEmptyString(record.finalDisposition)) {
      throw new Error(`document reconciliation finalDisposition must be null or a non-empty string: ${record.id}`);
    }
    candidateIds.add(record.candidateId);
    recordIds.add(record.id);
    statusCounts[record.status] += 1;
    if (record.status !== "incomplete") continue;
    if (
      !nonEmptyString(record.gapGroup)
      || record.acceptanceCriteria.length === 0
    ) {
      throw new Error(`incomplete requirement lacks a gap group or exact acceptance criteria: ${record.id}`);
    }
    gapCounts.set(record.gapGroup, (gapCounts.get(record.gapGroup) ?? 0) + 1);
    active.push(record);
  }
  active.sort((left, right) => (
    compareAuditText(left.gapGroup, right.gapGroup)
    || compareAuditText(left.candidateId, right.candidateId)
  ));
  const lines = [
    "# OpenTake planning-document reconciliation — 2026-07-14",
    "",
    "This report is the deterministic human-readable index for `requirements.json`. The JSON ledger is normative; this file is rebuilt byte-for-byte from that ledger and contains no independently maintained dispositions.",
    "",
    "## Coverage and integrity",
    "",
    `- Candidate records: ${requirements.records.length}`,
    `- Unique candidate IDs: ${candidateIds.size}`,
    `- Unverified dispositions: ${statusCounts.unverified}`,
    `- Requirements ledger SHA-256: \`${sha256(bytes)}\``,
    "- Complete dispositions require exact tracked implementation and automated-test evidence; incomplete dispositions require a subsystem gap group and exact acceptance criteria.",
    "",
    "| Status | Records | Meaning |",
    "|---|---:|---|",
    ...Object.keys(REQUIREMENT_STATUS_MEANINGS)
      .filter((status) => statusCounts[status] > 0 || status !== "unverified")
      .map((status) => `| ${status} | ${statusCounts[status]} | ${REQUIREMENT_STATUS_MEANINGS[status]} |`),
    "",
    "## Active gap groups",
    "",
    "| Gap group | Requirements |",
    "|---|---:|",
    ...[...gapCounts.entries()]
      .sort(([left], [right]) => compareAuditText(left, right))
      .map(([group, count]) => `| ${markdown(group)} | ${count} |`),
    "",
    "## Exact active requirements",
    "",
    "Every row below is one current `incomplete` requirement from the normative ledger. The acceptance column is its exact closure contract.",
    "",
    "| Gap group | Candidate ID | Source | Target behavior | Acceptance criteria | Current evidence |",
    "|---|---|---|---|---|---|",
  ];
  for (const record of active) {
    const evidence = [
      ...record.storeApi,
      ...record.automatedTests,
      ...record.runtimeEvidence,
    ];
    lines.push(`| ${markdown(record.gapGroup)} | \`${markdown(record.candidateId)}\` | ${markdown(`${record.source.path}:${record.source.line}`)} | ${markdown(record.targetBehavior)} | ${markdown(record.acceptanceCriteria.join("; "))} | ${markdown(evidence.join("; ") || record.finalDisposition || "No current completion evidence; closure contract remains active.")} |`);
  }
  return `${lines.join("\n")}\n`;
}

export function renderSourceReport(evidence) {
  validateSourceEvidenceShape(evidence);
  const lines = [
    "# Immutable upstream and downstream source audit",
    "",
    `Audit date: ${evidence.auditDate}`,
    "",
    "## Preservation and immutable captures",
    "",
    `- Serialized fetches: ${evidence.fetchEvidence.serializedOrder.map((command) => `\`${command}\``).join("; ")}.`,
    `- Target porcelain SHA-256: \`${evidence.fetchEvidence.target.pre}\` before and after fetch.`,
    `- Canonical porcelain SHA-256: \`${evidence.fetchEvidence.canonical.pre}\` before and after fetch.`,
    `- Palmier porcelain SHA-256: \`${evidence.fetchEvidence.palmier.pre}\` before and after fetch.`,
    `- Target open-PR capture: **${evidence.openPullRequests.count}** at \`${evidence.openPullRequests.capturedAt}\` from \`${evidence.openPullRequests.command}\`. Live verification is a separate verify-only check and does not alter this evidence.`,
    "",
    "## Immutable source snapshots",
    "",
    "| Source | Repository | Base | Head | Base tree | Head tree | Merge base | Ahead | Behind | Status | Changed paths |",
    "|---|---|---|---|---|---|---|---:|---:|---|---:|",
  ];
  for (const source of evidence.sources) {
    lines.push(`| ${markdown(source.name)} | ${markdown(source.repository)} | \`${source.base}\` | \`${source.head}\` | \`${source.baseTree}\` | \`${source.headTree}\` | \`${source.mergeBase}\` | ${source.aheadCount} | ${source.behindCount} | ${source.status} | ${source.changedPaths.length} |`);
  }
  lines.push(
    "",
    "Fork-main rows use ancestry-only mode: both fork mains are already ancestors of target main, so reverse target deltas are not downstream candidates and are intentionally not emitted as changed-path rows.",
    "",
    "## Fork branch index",
    "",
    `All ${evidence.branchSummary.totalNonMainHeads} non-main heads are indexed; symbolic remote HEAD aliases and main are excluded. Relevant unmerged heads: **${evidence.branchSummary.relevantUnmerged}**.`,
    `Integrated heads with an empty fork-main tree delta: **${evidence.branchSummary.emptyForkMainDeltas}**.`,
    "",
    "| Ref | Tip | Tree | Status | Target ancestor | Fork-main ancestor | Target unique commits | Fork-main unique commits | Fork-main changed paths | Fork delta |",
    "|---|---|---|---|---|---|---:|---:|---:|---|",
  );
  for (const branch of evidence.branchIndex) {
    lines.push(`| ${markdown(branch.ref)} | \`${branch.tip}\` | \`${branch.tree}\` | ${branch.status} | ${branch.targetAncestor} | ${branch.forkMainAncestor} | ${branch.targetUniqueCommitCount} | ${branch.forkMainUniqueCommitCount} | ${branch.forkMainChangedPathCount} | ${branch.forkMainDelta} |`);
  }
  lines.push("", "## Changed-path dispositions", "");
  for (const source of evidence.sources.filter(({ changedPaths }) => changedPaths.length > 0)) {
    lines.push(
      `### ${source.name}`,
      "",
      "| Git status | Path | Disposition | Behavior | OpenTake equivalent | Requirement IDs | Control IDs | Requirement gaps | Review rationale |",
      "|---|---|---|---|---|---|---|---|---|",
    );
    for (const change of source.changedPaths) {
      const path = change.destination ? `${change.path} → ${change.destination}` : change.path;
      lines.push(`| ${change.status} | ${markdown(path)} | ${markdown(change.disposition)} | ${markdown(change.behavior)} | ${markdown(change.openTakeEquivalent)} | ${markdown(change.linkedRequirementIds.join(", ") || "none")} | ${markdown(change.linkedControlIds.join(", ") || "none")} | ${markdown(change.requirementGapIds?.join(", ") || "none")} | ${markdown(change.rationale || "none")} |`);
    }
    lines.push("");
  }
  const dirty = evidence.canonicalDirtyCheckout;
  lines.push(
    "## Canonical dirty checkout manifest",
    "",
    `- Repository: ${dirty.repository}`,
    `- HEAD/tree: \`${dirty.head}\` / \`${dirty.tree}\``,
    `- Raw porcelain SHA-256: \`${dirty.statusSha256}\``,
    `- Sorted capture manifest SHA-256: \`${dirty.captureManifestSha256}\``,
    `- Sorted classified manifest SHA-256: \`${dirty.manifestSha256}\``,
    `- Entries: ${dirty.paths.length}`,
    `- Relationships: ${Object.entries(dirty.relationshipCounts).map(([name, count]) => `${name}=${count}`).join(", ")}.`,
    "",
    "| Status | Path | Relationship | Disposition | Behavior | OpenTake equivalent | Requirement IDs | Control IDs | Tracked | Patch SHA-256 | File type | Bytes | Content/link SHA-256 |",
    "|---|---|---|---|---|---|---|---|---|---|---|---:|---|",
  );
  for (const entry of dirty.paths) {
    lines.push(`| ${markdown(entry.status)} | ${markdown(entry.path)} | ${entry.relationship} | ${markdown(entry.disposition)} | ${markdown(entry.behavior)} | ${markdown(entry.openTakeEquivalent)} | ${markdown(entry.linkedRequirementIds.join(", ") || "none")} | ${markdown(entry.linkedControlIds.join(", ") || "none")} | ${entry.tracked} | ${entry.patchSha256 ? `\`${entry.patchSha256}\`` : "—"} | ${entry.fileType} | ${entry.bytes ?? "—"} | ${entry.contentSha256 ? `\`${entry.contentSha256}\`` : entry.linkTargetSha256 ? `link \`${entry.linkTargetSha256}\`` : "—"} |`);
  }
  lines.push(
    "",
    "Tracked patch hashes are computed from binary-safe `git diff HEAD -- <path>` bytes. Regular worktree files receive content hashes and byte counts; missing files and non-regular entries do not expose content. Symlinks, if present, expose only a hash of the link target bytes.",
  );
  return `${lines.join("\n")}\n`;
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
        path: normalizedPath,
        line,
        heading,
        text: text.trim(),
        signal,
      });
    }
  });
  const ordinals = new Map();
  return records.map((record) => {
    const semanticKey = JSON.stringify([
      record.path,
      record.signal,
      normalizeSemanticText(record.heading),
      normalizeSemanticText(record.text),
    ]);
    const semanticOrdinal = (ordinals.get(semanticKey) ?? 0) + 1;
    ordinals.set(semanticKey, semanticOrdinal);
    return {
      id: stableId("doc", JSON.stringify([semanticKey, semanticOrdinal])),
      semanticFingerprint: createHash("sha256").update(semanticKey).digest("hex"),
      semanticOrdinal,
      ...record,
    };
  });
}

function documentSemanticKey(candidate) {
  return JSON.stringify([
    normalizePath(candidate.path),
    candidate.signal,
    normalizeSemanticText(candidate.heading),
    normalizeSemanticText(candidate.text),
  ]);
}

function controlSemanticKey(candidate) {
  return JSON.stringify([
    normalizePath(candidate.path),
    normalizeSemanticText(candidate.ownerSymbol),
    candidate.element,
    normalizeSemanticText(candidate.label),
    normalizeSemanticText(candidate.role),
    normalizeSemanticText(candidate.handler),
  ]);
}

function semanticGroups(candidates, semanticKey) {
  const groups = new Map();
  for (const candidate of candidates) {
    const key = semanticKey(candidate);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(candidate);
  }
  return groups;
}

function buildIdentityMappings(kind, oldCandidates, newCandidates, semanticKey) {
  const oldGroups = semanticGroups(oldCandidates, semanticKey);
  const newGroups = semanticGroups(newCandidates, semanticKey);
  const keys = [...new Set([...oldGroups.keys(), ...newGroups.keys()])].sort(compareAuditText);
  const mappings = [];
  let duplicateSignatures = 0;
  let duplicateCandidates = 0;
  for (const key of keys) {
    const oldGroup = oldGroups.get(key) ?? [];
    const newGroup = newGroups.get(key) ?? [];
    if (Math.max(oldGroup.length, newGroup.length) > 1) {
      duplicateSignatures += 1;
      duplicateCandidates += newGroup.length;
    }
    if (oldGroup.length !== newGroup.length) {
      const path = oldGroup[0]?.path ?? newGroup[0]?.path ?? "unknown";
      const duplicate = Math.max(oldGroup.length, newGroup.length) > 1;
      throw new Error(
        `ambiguous semantic identity for ${kind} ${path}: ${duplicate ? "duplicate count changed" : "candidate count changed"} (${oldGroup.length} -> ${newGroup.length})`,
      );
    }
    for (let index = 0; index < oldGroup.length; index += 1) {
      const oldCandidate = oldGroup[index];
      const newCandidate = newGroup[index];
      mappings.push({
        oldCandidateId: oldCandidate.id,
        newCandidateId: newCandidate.id,
        oldRecordId: stableId(kind === "document" ? "requirement" : "control-record", oldCandidate.id),
        newRecordId: stableId(kind === "document" ? "requirement" : "control-record", newCandidate.id),
        semanticFingerprint: createHash("sha256").update(key).digest("hex"),
        semanticOrdinal: index + 1,
        oldSource: {
          path: oldCandidate.path,
          line: oldCandidate.line,
          ...(kind === "control" ? { column: oldCandidate.column } : {}),
        },
        newSource: {
          path: newCandidate.path,
          line: newCandidate.line,
          ...(kind === "control" ? { column: newCandidate.column } : {}),
        },
      });
    }
  }
  mappings.sort((left, right) => compareAuditText(left.oldCandidateId, right.oldCandidateId));
  return { mappings, duplicateSignatures, duplicateCandidates };
}

export function buildIdentityMigration({
  oldDocuments,
  newDocuments,
  oldControls,
  newControls,
}) {
  for (const [name, value] of Object.entries({ oldDocuments, newDocuments, oldControls, newControls })) {
    if (!Array.isArray(value)) throw new Error(`${name} must be an array`);
  }
  const documents = buildIdentityMappings(
    "document",
    oldDocuments,
    newDocuments,
    documentSemanticKey,
  );
  const controls = buildIdentityMappings(
    "control",
    oldControls,
    newControls,
    controlSemanticKey,
  );
  const oldIds = [...documents.mappings, ...controls.mappings].map(({ oldCandidateId }) => oldCandidateId);
  const newIds = [...documents.mappings, ...controls.mappings].map(({ newCandidateId }) => newCandidateId);
  if (new Set(oldIds).size !== oldIds.length || new Set(newIds).size !== newIds.length) {
    throw new Error("identity migration is not one-to-one");
  }
  return {
    schema: 1,
    identityVersion: 2,
    counts: {
      oldDocuments: oldDocuments.length,
      newDocuments: newDocuments.length,
      mappedDocuments: documents.mappings.length,
      oldControls: oldControls.length,
      newControls: newControls.length,
      mappedControls: controls.mappings.length,
      ambiguousSignatures: 0,
      unmappedOld: 0,
      unmappedNew: 0,
    },
    duplicateOrdinals: {
      documentSignatures: documents.duplicateSignatures,
      documentCandidates: documents.duplicateCandidates,
      controlSignatures: controls.duplicateSignatures,
      controlCandidates: controls.duplicateCandidates,
    },
    documentMappings: documents.mappings,
    controlMappings: controls.mappings,
  };
}

export function findMigratedIdentityReferenceLeaks(migration, referencesByPath) {
  if (!Array.isArray(migration?.documentMappings) || !Array.isArray(migration?.controlMappings)) {
    throw new Error("identity migration reference scan needs documentMappings[] and controlMappings[]");
  }
  if (!referencesByPath || typeof referencesByPath !== "object" || Array.isArray(referencesByPath)) {
    throw new Error("identity migration reference scan needs references keyed by path");
  }
  const replacements = new Map();
  for (const mapping of [...migration.documentMappings, ...migration.controlMappings]) {
    for (const [oldKey, newKey] of [
      ["oldCandidateId", "newCandidateId"],
      ["oldRecordId", "newRecordId"],
    ]) {
      const oldId = mapping?.[oldKey];
      const newId = mapping?.[newKey];
      if (typeof oldId !== "string" || typeof newId !== "string" || oldId.length === 0 || newId.length === 0) {
        throw new Error(`identity migration reference scan found an invalid ${oldKey}/${newKey} pair`);
      }
      if (oldId === newId) continue;
      const existing = replacements.get(oldId);
      if (existing && existing !== newId) {
        throw new Error(`identity migration reference scan found conflicting replacements for ${oldId}`);
      }
      replacements.set(oldId, newId);
    }
  }
  const leaks = [];
  const pattern = replacements.size > 0
    ? new RegExp(
      [...replacements.keys()]
        .sort((left, right) => right.length - left.length || compareAuditText(left, right))
        .map(escapeRegExp)
        .join("|"),
      "g",
    )
    : null;
  for (const [path, text] of Object.entries(referencesByPath)) {
    if (typeof text !== "string") throw new Error(`identity migration reference source is not text: ${path}`);
    if (!pattern) continue;
    pattern.lastIndex = 0;
    for (const oldId of new Set(text.match(pattern) ?? [])) {
      leaks.push({ path, oldId, newId: replacements.get(oldId) });
    }
  }
  return leaks.sort((left, right) => (
    compareAuditText(left.path, right.path) || compareAuditText(left.oldId, right.oldId)
  ));
}

function replaceIdentityStrings(value, replacements) {
  if (typeof value === "string") {
    let result = value;
    for (const [oldId, newId] of replacements) result = result.replaceAll(oldId, newId);
    return result;
  }
  if (Array.isArray(value)) return value.map((item) => replaceIdentityStrings(item, replacements));
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.entries(value).map(
    ([key, item]) => [key, replaceIdentityStrings(item, replacements)],
  ));
}

function countExactReferences(value, ids) {
  if (typeof value === "string") return ids.has(value) ? 1 : 0;
  if (Array.isArray(value)) return value.reduce((total, item) => total + countExactReferences(item, ids), 0);
  if (!value || typeof value !== "object") return 0;
  return Object.values(value).reduce((total, item) => total + countExactReferences(item, ids), 0);
}

export function migrateAuditIdentityReferences({
  migration,
  requirements,
  controls,
  runtimeEvidence,
  sources,
}) {
  const documentMappings = migration?.documentMappings ?? [];
  const controlMappings = migration?.controlMappings ?? [];
  const documentByOld = new Map(documentMappings.map((mapping) => [mapping.oldCandidateId, mapping]));
  const controlByOld = new Map(controlMappings.map((mapping) => [mapping.oldCandidateId, mapping]));
  const candidateReplacements = [
    ...documentMappings.map(({ oldCandidateId, newCandidateId }) => [oldCandidateId, newCandidateId]),
    ...controlMappings.map(({ oldCandidateId, newCandidateId }) => [oldCandidateId, newCandidateId]),
  ].filter(([oldId, newId]) => oldId !== newId);
  const recordReplacements = [
    ...documentMappings.map(({ oldRecordId, newRecordId }) => [oldRecordId, newRecordId]),
    ...controlMappings.map(({ oldRecordId, newRecordId }) => [oldRecordId, newRecordId]),
  ].filter(([oldId, newId]) => oldId !== newId);
  const embeddedCandidateSuffixReplacements = candidateReplacements.map(([oldId, newId]) => [
    oldId.replace(/^(?:doc|control)-/, ""),
    newId.replace(/^(?:doc|control)-/, ""),
  ]);
  const replacements = [...recordReplacements, ...candidateReplacements, ...embeddedCandidateSuffixReplacements]
    .sort((left, right) => right[0].length - left[0].length || compareAuditText(left[0], right[0]));

  const migratedRequirements = structuredClone(requirements);
  const migratedControls = structuredClone(controls);
  const migratedRuntimeEvidence = structuredClone(runtimeEvidence);
  const migratedSources = structuredClone(sources);
  const requirementRecords = migratedRequirements?.records ?? [];
  const controlRecords = migratedControls?.records ?? [];
  if (!Array.isArray(requirementRecords) || !Array.isArray(controlRecords)) {
    throw new Error("identity migration needs requirement and control records arrays");
  }
  let requirementCandidateIds = 0;
  let requirementRecordIds = 0;
  for (const record of requirementRecords) {
    const mapping = documentByOld.get(record.candidateId);
    if (!mapping) throw new Error(`requirement has no identity mapping: ${String(record.candidateId)}`);
    if (record.candidateId !== mapping.newCandidateId) requirementCandidateIds += 1;
    if (record.id !== mapping.newRecordId) requirementRecordIds += 1;
    record.candidateId = mapping.newCandidateId;
    record.id = mapping.newRecordId;
    record.source = { ...mapping.newSource };
  }
  let controlCandidateIds = 0;
  let controlRecordIds = 0;
  const oldControlCandidateIds = new Set(controlMappings.map(({ oldCandidateId }) => oldCandidateId));
  const duplicateTargets = controlRecords.reduce(
    (total, record) => total + (record.duplicateOf ?? []).filter((id) => oldControlCandidateIds.has(id)).length,
    0,
  );
  for (const record of controlRecords) {
    const mapping = controlByOld.get(record.candidateId);
    if (!mapping) throw new Error(`control has no identity mapping: ${String(record.candidateId)}`);
    if (record.candidateId !== mapping.newCandidateId) controlCandidateIds += 1;
    if (record.id !== mapping.newRecordId) controlRecordIds += 1;
    record.candidateId = mapping.newCandidateId;
    record.id = mapping.newRecordId;
    record.source = { ...mapping.newSource };
  }
  const runtimeCandidateRefs = countExactReferences(migratedRuntimeEvidence, oldControlCandidateIds);
  const oldRequirementRecordIds = new Set(documentMappings.map(({ oldRecordId }) => oldRecordId));
  const oldControlRecordIds = new Set(controlMappings.map(({ oldRecordId }) => oldRecordId));
  const sourceRequirementLinks = countExactReferences(migratedSources, oldRequirementRecordIds);
  const sourceControlLinks = countExactReferences(migratedSources, oldControlRecordIds);

  return {
    requirements: replaceIdentityStrings(migratedRequirements, replacements),
    controls: replaceIdentityStrings(migratedControls, replacements),
    runtimeEvidence: replaceIdentityStrings(migratedRuntimeEvidence, replacements),
    sources: replaceIdentityStrings(migratedSources, replacements),
    rewriteCounts: {
      requirementCandidateIds,
      requirementRecordIds,
      controlCandidateIds,
      controlRecordIds,
      duplicateTargets,
      runtimeCandidateRefs,
      sourceRequirementLinks,
      sourceControlLinks,
    },
  };
}

const AUDIT_VERIFIER_FILES = [
  "tools/completion-audit.mjs",
  "tools/completion-audit-controls.mjs",
  "tools/completion-audit-plan-map.json",
  "tools/completion-audit.test.mjs",
];

function currentAuditPublicationPaths(root, auditRelative) {
  const prefix = `${normalizePath(auditRelative).replace(/\/$/, "")}/`;
  const migrationPath = `${prefix}identity-migration.json`;
  const historicalEvidencePrefix = `${prefix}runtime-artifacts/`;
  return readTrackedFiles(root).filter((path) => (
    path.startsWith(prefix)
    && path !== migrationPath
    && !path.startsWith(historicalEvidencePrefix)
  ));
}

function jsonBytes(value) {
  return Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function controlCandidateSourceAggregate(root, candidates) {
  const paths = [...new Set(candidates.map(({ path }) => path))].sort(compareAuditText);
  return sha256(paths.map((path) => (
    `${path}\0${sha256(readFileSync(resolve(root, path)))}\n`
  )).join(""));
}

export function retireHistoricalAutomatedReceipts(runtimeEvidence, previousVerifierRevision, nextVerifierRevision) {
  const receipts = runtimeEvidence?.receipts;
  if (!Array.isArray(receipts)) throw new Error("runtime evidence needs receipts[] before verifier rebinding");
  const sameRevision = (left, right) => (
    left?.commit === right?.commit && left?.tree === right?.tree
  );
  for (const receipt of receipts) {
    if (
      receipt?.kind !== "automated"
      || !["passed", "partial"].includes(receipt.status)
      || sameRevision(receipt.executedCheckoutRevision, nextVerifierRevision)
    ) continue;
    if (!sameRevision(receipt.executedCheckoutRevision, previousVerifierRevision)) {
      throw new Error(`historical automated receipt is not bound to the prior verifier revision: ${String(receipt.id)}`);
    }
    if (receipt.evidenceLevel !== "supporting") {
      throw new Error(`historical direct automated receipt cannot be silently retired: ${String(receipt.id)}`);
    }
    const oldCommit = receipt.executedCheckoutRevision.commit;
    receipt.status = "not-run";
    receipt.exitCode = null;
    receipt.result = {
      summary: `Historical supporting execution was captured at ${oldCommit}; this receipt does not claim execution at verifier revision ${nextVerifierRevision.commit}.`,
      exitCode: null,
    };
    receipt.limitations = [
      ...(Array.isArray(receipt.limitations) ? receipt.limitations : []),
      `Historical supporting execution at ${oldCommit} was not re-executed by this receipt after verifier rebinding.`,
    ];
  }
  runtimeEvidence.summary = {
    receipts: receipts.length,
    direct: receipts.filter((receipt) => receipt?.evidenceLevel === "direct").length,
    supporting: receipts.filter((receipt) => receipt?.evidenceLevel === "supporting").length,
    passed: receipts.filter((receipt) => receipt?.status === "passed").length,
    failed: receipts.filter((receipt) => receipt?.status === "failed").length,
    partial: receipts.filter((receipt) => receipt?.status === "partial").length,
    notRun: receipts.filter((receipt) => receipt?.status === "not-run").length,
    blocked: receipts.filter((receipt) => receipt?.status === "blocked").length,
  };
  return runtimeEvidence;
}

export function reviewProcessGapCorrections(requirements) {
  const records = requirements?.records;
  if (!Array.isArray(records)) throw new Error("requirements ledger needs records[] for gap corrections");
  const processPlan = "docs/superpowers/plans/2026-07-14-opentake-completion-audit.md";
  const completedTasks = [
    { start: 24, end: 137, task: 1, symbol: "stableId", test: "stableId is deterministic and prefix scoped" },
    { start: 137, end: 234, task: 2, symbol: "buildFileInventory", test: "files CLI writes tracked paths, hashes, and one self-reference" },
    { start: 234, end: 346, task: 3, symbol: "extractDocumentCandidates", test: "docs CLI excludes generated audit Markdown while file inventory retains it" },
    { start: 346, end: 467, task: 4, symbol: "extractControls", test: "controls CLI scans tracked web/src TSX files through a real subprocess" },
    { start: 467, end: 559, task: 5, symbol: "captureGitSource", test: "captureGitSource records exact immutable Git evidence" },
    { start: 559, end: 602, task: 6, symbol: "verifyAudit", test: "review demotions are unique and semantically bound to frozen candidates" },
    { start: 602, end: 649, task: 7, symbol: "verifyAudit", test: "verifyAudit controls accepts an exact incomplete schema-2 ledger" },
    { start: 649, end: 773, task: 8, symbol: "verifyAudit", test: "verifyAudit all validates the exact published completion program" },
  ];
  const reviewedGroupCorrections = new Map([
    ...[
      "requirement-cbdc477af446a4ea", "requirement-45fa0c2b840442cd", "requirement-83d62f78aa13e484",
      "requirement-2f2d03d0f0d8b62a", "requirement-bd5095a19a955167", "requirement-43fbc8e32bd126c4",
      "requirement-ae88b5f4d3f80eb6", "requirement-7b2d77c2b48b0238", "requirement-8d06ffbfdd515f03",
      "requirement-980f6325823ecf64", "requirement-f917123fb8d790f1",
    ].map((id) => [id, { from: "inspector-text-keyframes", to: "media-library", family: "media-search" }]),
    ...[
      "requirement-5ee9b7ba12472dd2", "requirement-182ee5a96ef748eb", "requirement-033385c9654f4781",
      "requirement-1cfe5d8575e5cc93", "requirement-291a7211a4ee4b55", "requirement-94aebecde1fc0542",
    ].map((id) => [id, { from: "accessibility-polish", to: "media-library", family: "media-transcription" }]),
    ...[
      "requirement-bb4951d728a9bcff", "requirement-c531ff6b55858bc0", "requirement-30c0b150880b10ef",
      "requirement-7fed82947ea8910f", "requirement-798893a778f1813e", "requirement-eeba721f50c7b42d",
      "requirement-e36316e0aa8cd79f", "requirement-6bb9a832824dd470", "requirement-2d93bc046644f4d8",
    ].map((id) => [id, { from: "command-contracts", to: "home-shell", family: "frontend-layout" }]),
  ]);
  const corrections = [];
  for (const record of records) {
    if (record.id === "requirement-b248c29fb16de9d1") {
      corrections.push({
        recordId: record.id,
        oldGroup: record.gapGroup,
        newGroup: null,
        oldStatus: record.status,
        newStatus: "complete",
        reason: "CoreDeps and UnsupportedBackends already expose recoverable injected capability seams with an exact unit test",
      });
      record.status = "complete";
      record.gapGroup = null;
      record.rust = ["code:crates/opentake-core/src/deps.rs#CoreDeps"];
      record.automatedTests = ["test:crates/opentake-core/src/deps.rs#default_deps_report_unsupported_not_panic"];
      record.finalDisposition = "complete: injected capability seams return recoverable unsupported errors and are covered by the exact tracked unit test";
      record.provenance = [
        ...(Array.isArray(record.provenance) ? record.provenance : []),
        "review-correction:product-status:incomplete->complete:core-deps-injection-evidence",
      ];
      continue;
    }
    const groupCorrection = reviewedGroupCorrections.get(record.id);
    if (groupCorrection) {
      if (record.status !== "incomplete" || record.gapGroup !== groupCorrection.from) {
        throw new Error(`reviewed ${groupCorrection.family} correction precondition drifted for ${record.id}`);
      }
      corrections.push({
        recordId: record.id,
        oldGroup: groupCorrection.from,
        newGroup: groupCorrection.to,
        oldStatus: record.status,
        newStatus: record.status,
        reason: `reviewed ${groupCorrection.family} ownership belongs to media-library, not the source-path-derived UI group`,
      });
      record.gapGroup = groupCorrection.to;
      record.finalDisposition = String(record.finalDisposition).replace(
        `active-gap:${groupCorrection.from}:`,
        `active-gap:${groupCorrection.to}:`,
      );
      record.provenance = [
        ...(Array.isArray(record.provenance) ? record.provenance : []),
        `review-correction:gap-group:${groupCorrection.from}->${groupCorrection.to}:${groupCorrection.family}-ownership`,
      ];
      continue;
    }
    if (record.status !== "incomplete" || record.source?.path !== processPlan) continue;
    const processText = [record.targetBehavior, record.visibleResult, ...(record.acceptanceCriteria ?? [])].join(" ");
    if (!/(?:Step\s+\d+|audit|verifier|inventory|candidate|source capture|review|commit|run (?:the )?(?:focused )?test)/i.test(processText)) {
      throw new Error(`completion-audit plan record is not recognizably process-only: ${String(record.id)}`);
    }
    const oldGroup = record.gapGroup;
    const oldStatus = record.status;
    const completedTask = completedTasks.find(({ start, end }) => (
      record.source.line >= start && record.source.line < end
    ));
    const newGroup = completedTask ? null : "documentation";
    const newStatus = completedTask ? "complete" : "incomplete";
    if (oldGroup === newGroup && oldStatus === newStatus) continue;
    corrections.push({
      recordId: record.id,
      oldGroup,
      newGroup,
      oldStatus,
      newStatus,
      reason: completedTask
        ? `completion-audit Task ${completedTask.task} is implemented by the current tracked tool and exact named regression test`
        : "completion-audit Task 9 is future audit-process work, not product subsystem work",
    });
    record.gapGroup = newGroup;
    record.status = newStatus;
    if (completedTask) {
      record.storeApi = [`code:tools/completion-audit.mjs#${completedTask.symbol}`];
      record.automatedTests = [`test:tools/completion-audit.test.mjs#${completedTask.test}`];
      record.finalDisposition = `complete: completion-audit Task ${completedTask.task} is implemented and covered by the exact named tracked regression test`;
      if (completedTask.task === 7) {
        const artifact = record.source.line >= 625 && record.source.line < 629
          ? "docs/audit/2026-07-14/runtime-artifacts/native/exact-app-stdout.log"
          : record.source.line >= 621 && record.source.line < 625
            ? "docs/audit/2026-07-14/runtime-artifacts/browser/page-2026-07-14T11-30-22-654Z.yml"
            : "docs/audit/2026-07-14/control-verification.json";
        record.provenance = [
          ...(Array.isArray(record.provenance) ? record.provenance : []),
          `historical-report:${artifact}`,
        ];
      }
    } else {
      record.finalDisposition = String(record.finalDisposition).replace(
        `active-gap:${oldGroup}:`,
        "active-gap:documentation:",
      );
    }
    record.provenance = [
      ...(Array.isArray(record.provenance) ? record.provenance : []),
      completedTask
        ? `review-correction:process-status:${oldStatus}->complete:task-${completedTask.task}-current-tool-and-test-evidence`
        : `review-correction:gap-group:${oldGroup}->documentation:completion-audit-process-only`,
    ];
  }
  return corrections;
}

function gitJson(root, revision, path) {
  return JSON.parse(gitRead("identity migration", root, ["show", `${revision}:${path}`], "utf8"));
}

function bindMigratedControlProvenance(root, auditRelative, migrated, newControlCandidates) {
  const commit = gitText("identity migration", root, ["rev-parse", "HEAD"]);
  const tree = gitText("identity migration", root, ["rev-parse", "HEAD^{tree}"]);
  const candidateLedger = { schema: 1, candidates: newControlCandidates };
  const provenance = {
    auditedProductRevision: structuredClone(migrated.controls.provenance.auditedProductRevision),
    verifierRevision: { commit, tree },
    candidateLedgerSha256: sha256(jsonBytes(candidateLedger)),
    candidateSourceAggregateSha256: controlCandidateSourceAggregate(root, newControlCandidates),
    verifierFilesSha256: Object.fromEntries(AUDIT_VERIFIER_FILES.map((path) => [
      path,
      sha256(readFileSync(resolve(root, path))),
    ])),
  };
  retireHistoricalAutomatedReceipts(
    migrated.runtimeEvidence,
    migrated.runtimeEvidence.provenance?.verifierRevision,
    provenance.verifierRevision,
  );
  migrated.controls.provenance = structuredClone(provenance);
  migrated.runtimeEvidence.provenance = structuredClone(provenance);
  migrated.controls.scope.sourceLedger = normalizePath(join(auditRelative, "control-candidates.json"));
  migrated.controls.scope.candidateCount = newControlCandidates.length;
  migrated.controls.scope.candidateIdsUnique = new Set(newControlCandidates.map(({ id }) => id)).size === newControlCandidates.length;
  migrated.runtimeEvidence.candidateLedger = normalizePath(join(auditRelative, "control-candidates.json"));
  migrated.runtimeEvidence.controlsLedger = normalizePath(join(auditRelative, "controls.json"));
  return provenance;
}

export function buildIdentityMigrationPublication(root, auditDir, sourceRevision = "HEAD") {
  const repositoryRoot = realpathSync(resolve(root));
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const auditRelative = normalizePath(relative(repositoryRoot, auditDirectory));
  const sourceCommit = gitText("identity migration", repositoryRoot, ["rev-parse", `${sourceRevision}^{commit}`]);
  const sourceTree = gitText("identity migration", repositoryRoot, ["rev-parse", `${sourceCommit}^{tree}`]);
  const paths = readTrackedFiles(repositoryRoot);
  const oldDocumentLedger = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/document-candidates.json`);
  const oldControlLedger = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/control-candidates.json`);
  const oldRequirements = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/requirements.json`);
  const oldControls = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/controls.json`);
  const oldRuntimeEvidence = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/runtime-evidence.json`);
  const oldSources = gitJson(repositoryRoot, sourceCommit, `${auditRelative}/sources.json`);
  const newDocuments = paths
    .filter((path) => path.endsWith(".md") && !path.startsWith("docs/audit/"))
    .flatMap((path) => extractDocumentCandidates(path, readFileSync(resolve(repositoryRoot, path), "utf8")));
  const newControls = paths
    .filter((path) => path.startsWith("web/src/") && path.endsWith(".tsx"))
    .flatMap((path) => extractControls(path, readFileSync(resolve(repositoryRoot, path), "utf8"), typeScriptCompiler()));
  const migration = buildIdentityMigration({
    oldDocuments: oldDocumentLedger.candidates,
    newDocuments,
    oldControls: oldControlLedger.candidates,
    newControls,
  });
  const migrated = migrateAuditIdentityReferences({
    migration,
    requirements: oldRequirements,
    controls: oldControls,
    runtimeEvidence: oldRuntimeEvidence,
    sources: oldSources,
  });
  const classificationCorrections = reviewProcessGapCorrections(migrated.requirements);
  bindMigratedControlProvenance(repositoryRoot, auditRelative, migrated, newControls);
  const outputs = {
    "document-candidates.json": { schema: 1, candidates: newDocuments },
    "requirements.json": migrated.requirements,
    "control-candidates.json": { schema: 1, candidates: newControls },
    "controls.json": migrated.controls,
    "runtime-evidence.json": migrated.runtimeEvidence,
    "sources.json": migrated.sources,
  };
  const reports = {
    "document-reconciliation.md": renderDocumentReconciliation(jsonBytes(migrated.requirements)),
    "upstream-downstream.md": renderSourceReport(migrated.sources),
  };
  const oldLedgers = {
    "document-candidates.json": oldDocumentLedger,
    "requirements.json": oldRequirements,
    "control-candidates.json": oldControlLedger,
    "controls.json": oldControls,
    "runtime-evidence.json": oldRuntimeEvidence,
    "sources.json": oldSources,
  };
  const migrationCommit = gitText("identity migration", repositoryRoot, ["rev-parse", "HEAD"]);
  const migrationTree = gitText("identity migration", repositoryRoot, ["rev-parse", "HEAD^{tree}"]);
  const artifact = {
    schema: 1,
    metadata: {
      schemaName: "completion-audit-identity-migration",
      schemaVersion: 1,
      identityVersion: 2,
      policy: "Semantic signature plus same-signature ordinal; duplicate count changes fail closed.",
    },
    provenance: {
      sourceRevision: { commit: sourceCommit, tree: sourceTree },
      migrationRevision: { commit: migrationCommit, tree: migrationTree },
      oldLedgerSha256: Object.fromEntries(Object.entries(oldLedgers).map(([name, value]) => [name, sha256(jsonBytes(value))])),
      newLedgerSha256: Object.fromEntries(Object.entries(outputs).map(([name, value]) => [name, sha256(jsonBytes(value))])),
    },
    counts: migration.counts,
    duplicateOrdinals: migration.duplicateOrdinals,
    rewriteCounts: migrated.rewriteCounts,
    classificationCorrections,
    hashes: {
      documentMappingsSha256: sha256(JSON.stringify(migration.documentMappings)),
      controlMappingsSha256: sha256(JSON.stringify(migration.controlMappings)),
    },
    documentMappings: migration.documentMappings,
    controlMappings: migration.controlMappings,
  };
  return { artifact, outputs, reports };
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

export function buildFileInventory(root, selfPath = DEFAULT_FILE_INVENTORY_PATH, deferredPaths = []) {
  const normalizedSelfPath = normalizePath(selfPath);
  const deferred = new Set(deferredPaths.map(normalizePath));
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
    if (deferred.has(path)) {
      return {
        ...record,
        bytes: null,
        sha256: null,
        hashStatus: "deterministic-generated-output",
        reason: "content is regenerated and compared byte-for-byte by the all-scope verifier after inventory generation",
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

const DOCUMENT_REQUIREMENT_STATUSES = new Set([
  "complete",
  "incomplete",
  "contradicted",
  "obsolete",
  "duplicate",
]);

export const DOCUMENT_GAP_GROUPS = Object.freeze([
  "data-safety",
  "command-contracts",
  "media-render-playback-export",
  "home-shell",
  "media-library",
  "preview-timeline",
  "inspector-text-keyframes",
  "agent-settings-generation",
  "accessibility-polish",
  "documentation",
]);

const DOCUMENT_GAP_GROUP_SET = new Set(DOCUMENT_GAP_GROUPS);
const DOCUMENT_ARRAY_FIELDS = [
  "uiEntry",
  "react",
  "storeApi",
  "tauri",
  "rust",
  "sideEffects",
  "returnPath",
  "automatedTests",
  "runtimeEvidence",
  "provenance",
  "acceptanceCriteria",
];
const DOCUMENT_IMPLEMENTATION_FIELDS = [
  "react",
  "storeApi",
  "tauri",
  "rust",
];
const DOCUMENT_NON_REQUIREMENT_STATUSES = new Set([
  "contradicted",
  "obsolete",
  "duplicate",
]);

function nonEmptyString(value) {
  return typeof value === "string" && value.trim() !== "";
}

function nonEmptyStringArray(value) {
  return Array.isArray(value) && value.some(nonEmptyString);
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

function pushVerificationError(errors, code, message, candidateId = null) {
  errors.push({ code, candidateId, message });
}

function realPathWithinRoot(root, absolute) {
  try {
    const real = realpathSync(absolute);
    const inside = relative(root, real);
    if (inside === "" || inside === ".." || inside.startsWith(`..${sep}`) || isAbsolute(inside)) {
      return null;
    }
    return real;
  } catch {
    return null;
  }
}

function readDocumentAuditFile(root, auditDirectory, name, collectionName, errors) {
  const path = resolve(auditDirectory, name);
  if (!existsSync(path)) {
    pushVerificationError(errors, "missing-audit-file", `${name} is missing`);
    return [];
  }
  const real = realPathWithinRoot(root, path);
  if (!lstatSync(path).isFile() || !real) {
    pushVerificationError(
      errors,
      "audit-file-symlink-escape",
      `${name} must be a real regular file confined to the repository`,
    );
    return [];
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(real, "utf8"));
  } catch (error) {
    pushVerificationError(
      errors,
      "invalid-audit-file",
      `${name}: ${error.message}`,
    );
    return [];
  }
  if (parsed?.schema !== 1) {
    pushVerificationError(
      errors,
      "invalid-audit-schema",
      `${name}: schema must equal 1`,
    );
  }
  if (!hasExactObjectKeys(parsed, ["schema", collectionName])) {
    pushVerificationError(
      errors,
      "invalid-audit-schema",
      `${name}: top-level keys must be exactly schema/${collectionName}`,
    );
  }
  if (!Array.isArray(parsed?.[collectionName])) {
    pushVerificationError(
      errors,
      "invalid-audit-file",
      `${name}: ${collectionName} must be an array`,
    );
    return [];
  }
  return parsed[collectionName];
}

function pathInsideRoot(root, value) {
  if (typeof value !== "string" || value === "" || isAbsolute(value)) return null;
  const normalized = normalizePath(value);
  const absolute = resolve(root, normalized);
  const inside = relative(root, absolute);
  if (inside === "" || inside === ".." || inside.startsWith(`..${sep}`) || isAbsolute(inside)) {
    return null;
  }
  return { normalized, absolute };
}

function trackedRegularFile(root, trackedPaths, value) {
  const resolved = pathInsideRoot(root, value);
  if (!resolved) return { status: "invalid", ...resolved };
  if (!existsSync(resolved.absolute)) return { status: "missing", ...resolved };
  const stat = lstatSync(resolved.absolute);
  if (stat.isSymbolicLink()) return { status: "symlink", ...resolved };
  if (!stat.isFile()) return { status: "not-file", ...resolved };
  const real = realPathWithinRoot(root, resolved.absolute);
  if (!real) return { status: "realpath-escape", ...resolved };
  if (!trackedPaths.has(resolved.normalized)) return { status: "untracked", ...resolved };
  return { status: "valid", real, ...resolved };
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

function typeScriptNameMatches(ts, name, expected) {
  return Boolean(name) && (
    (ts.isIdentifier(name) && name.text === expected)
    || (ts.isStringLiteral(name) && name.text === expected)
  );
}

function typeScriptDeclaresSymbol(path, source, symbol) {
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  let found = false;
  const visit = (node) => {
    if (found) return;
    if (
      (
        ts.isFunctionDeclaration(node)
        || ts.isClassDeclaration(node)
        || ts.isInterfaceDeclaration(node)
        || ts.isTypeAliasDeclaration(node)
        || ts.isEnumDeclaration(node)
        || ts.isMethodDeclaration(node)
        || ts.isGetAccessorDeclaration(node)
        || ts.isSetAccessorDeclaration(node)
        || ts.isPropertyDeclaration(node)
        || ts.isMethodSignature(node)
        || ts.isPropertySignature(node)
      )
      && typeScriptNameMatches(ts, node.name, symbol)
    ) {
      found = true;
      return;
    }
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === symbol) {
      found = true;
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(sourceFile);
  return found;
}

function stripRustCommentsAndLiterals(source) {
  const output = [...source];
  const blank = (start, end) => {
    for (let cursor = start; cursor < end; cursor += 1) {
      if (output[cursor] !== "\n" && output[cursor] !== "\r") output[cursor] = " ";
    }
  };
  let index = 0;
  while (index < source.length) {
    if (source.startsWith("//", index)) {
      const lineEnd = source.indexOf("\n", index + 2);
      const end = lineEnd === -1 ? source.length : lineEnd;
      blank(index, end);
      index = end;
      continue;
    }
    if (source.startsWith("/*", index)) {
      const start = index;
      index += 2;
      let depth = 1;
      while (index < source.length && depth > 0) {
        if (source.startsWith("/*", index)) {
          depth += 1;
          index += 2;
        } else if (source.startsWith("*/", index)) {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      blank(start, index);
      continue;
    }

    const raw = /^(?:br|rb|r)(#{0,255})"/.exec(source.slice(index));
    if (raw) {
      const start = index;
      const closing = `"${raw[1]}`;
      index += raw[0].length;
      const closingIndex = source.indexOf(closing, index);
      index = closingIndex === -1 ? source.length : closingIndex + closing.length;
      blank(start, index);
      continue;
    }

    const stringPrefixLength = source.startsWith('b"', index) || source.startsWith('c"', index)
      ? 2
      : source[index] === '"'
        ? 1
        : 0;
    if (stringPrefixLength > 0) {
      const start = index;
      index += stringPrefixLength;
      while (index < source.length) {
        if (source[index] === "\\") index += 2;
        else if (source[index] === '"') {
          index += 1;
          break;
        } else index += 1;
      }
      blank(start, index);
      continue;
    }

    const character = /^(?:b)?'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]{1,6}\}|.)|[^\\'\r\n])'/u.exec(source.slice(index));
    if (character) {
      blank(index, index + character[0].length);
      index += character[0].length;
      continue;
    }
    index += 1;
  }
  return output.join("");
}

function rustDelimitedEnd(source, open) {
  const closing = { "(": ")", "{": "}", "[": "]" };
  const stack = [closing[source[open]]];
  let index = open + 1;
  while (index < source.length && stack.length > 0) {
    const character = source[index];
    if (closing[character]) stack.push(closing[character]);
    else if (character === stack.at(-1)) stack.pop();
    index += 1;
  }
  return index;
}

function blankRustRange(output, start, end) {
  for (let cursor = start; cursor < end; cursor += 1) {
    if (output[cursor] !== "\n" && output[cursor] !== "\r") output[cursor] = " ";
  }
}

function stripRustMacroInvocations(source) {
  const output = [...source];
  const definition = /\bmacro_rules\s*!\s*[A-Za-z_][A-Za-z0-9_]*\s*([({[])/g;
  let match;
  while ((match = definition.exec(source)) !== null) {
    const open = match.index + match[0].lastIndexOf(match[1]);
    const end = rustDelimitedEnd(source, open);
    // Preserve `macro_rules! name` as a declaration, but hide every token the
    // macro may emit so generated-looking declarations cannot count as code.
    blankRustRange(output, open, end);
    definition.lastIndex = end;
  }

  const withoutDefinitionBodies = output.join("");
  const macro = /(?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)*[A-Za-z_][A-Za-z0-9_]*\s*!\s*([({[])/g;
  while ((match = macro.exec(withoutDefinitionBodies)) !== null) {
    const open = match.index + match[0].lastIndexOf(match[1]);
    const end = rustDelimitedEnd(withoutDefinitionBodies, open);
    for (let cursor = match.index; cursor < end; cursor += 1) {
      if (output[cursor] !== "\n" && output[cursor] !== "\r") output[cursor] = " ";
    }
    macro.lastIndex = end;
  }
  return output.join("");
}

function rustCodeTokens(source) {
  return stripRustMacroInvocations(stripRustCommentsAndLiterals(source));
}

export function sourceDeclaresSymbol(path, source, symbol) {
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(symbol)) return false;
  if (path.endsWith(".rs")) {
    const escaped = escapeRegExp(symbol);
    const tokens = rustCodeTokens(source);
    return new RegExp(
      `\\b(?:fn|struct|enum|trait|type|const|static|mod)\\s+${escaped}\\b|\\bmacro_rules\\s*!\\s*${escaped}\\b`,
    ).test(tokens);
  }
  if (/\.(?:[cm]?[jt]sx?)$/.test(path)) {
    return typeScriptDeclaresSymbol(path, source, symbol);
  }
  return false;
}

export function sourceDeclaresProductAnchor(path, source, symbol) {
  if (symbol == null) return /\.(?:json|toml|wgsl)$/.test(path);
  if (path.endsWith(".css")) {
    return /^--[A-Za-z0-9_-]+$/.test(symbol)
      && new RegExp(`(?:^|[;{])\\s*${escapeRegExp(symbol)}\\s*:`, "m").test(source);
  }
  return sourceDeclaresSymbol(path, source, symbol);
}

function rustDeclarationBlocks(tokens, headerPattern) {
  const pattern = new RegExp(headerPattern.source, headerPattern.flags.includes("g") ? headerPattern.flags : `${headerPattern.flags}g`);
  const blocks = [];
  for (const match of tokens.matchAll(pattern)) {
    const start = tokens.indexOf("{", match.index);
    if (start === -1) continue;
    let depth = 0;
    for (let index = start; index < tokens.length; index += 1) {
      if (tokens[index] === "{") depth += 1;
      if (tokens[index] === "}") {
        depth -= 1;
        if (depth === 0) {
          blocks.push(tokens.slice(start + 1, index));
          break;
        }
      }
    }
  }
  return blocks;
}

function rustDeclaresQualifiedMember(source, owner, member) {
  const tokens = rustCodeTokens(source);
  if (!sourceDeclaresSymbol("owner.rs", source, owner)) return false;
  const escapedOwner = escapeRegExp(owner);
  const escapedMember = escapeRegExp(member);
  const declarations = rustDeclarationBlocks(
    tokens,
    new RegExp(`\\b(?:enum|struct|trait)\\s+${escapedOwner}\\b[^;{]*\\{`),
  );
  if (declarations.some((block) => new RegExp(
    `(?:\\b(?:fn|const|type|static)\\s+${escapedMember}\\b|\\b${escapedMember}\\b\\s*(?::|\\(|\\{|,|=))`,
  ).test(block))) return true;
  const implementations = rustDeclarationBlocks(
    tokens,
    new RegExp(`\\bimpl\\b[^;{]*\\b${escapedOwner}\\b[^;{]*\\{`),
  );
  return implementations.some((block) => new RegExp(
    `\\b(?:fn|const|type|static)\\s+${escapedMember}\\b`,
  ).test(block));
}

export function sourceDeclaresProductSpec(path, source, symbol) {
  if (symbol == null) return true;
  const parts = String(symbol).split("::");
  if (parts.length === 1) return sourceDeclaresProductAnchor(path, source, symbol);
  if (!path.endsWith(".rs") || parts.length !== 2) return false;
  return rustDeclaresQualifiedMember(source, parts[0], parts[1]);
}

export function sourceDeclaresTest(path, source, name) {
  if (path.endsWith(".rs")) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return false;
    const tokens = rustCodeTokens(source);
    const attribute = String.raw`#\s*\[\s*(?:test|tokio\s*::\s*test|async_std\s*::\s*test)(?:\s*\([^\]]*\))?\s*\]`;
    const declaration = String.raw`\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+${escapeRegExp(name)}\s*\(`;
    return new RegExp(`${attribute}${declaration}`).test(tokens);
  }
  if (!/\.(?:[cm]?[jt]sx?)$/.test(path)) return false;
  const ts = typeScriptCompiler();
  const sourceFile = typeScriptSourceFile(path, source);
  let found = false;
  const testCallKind = (expression) => {
    if (ts.isIdentifier(expression)) {
      return ["test", "it", "describe"].includes(expression.text) ? expression.text : null;
    }
    if (ts.isPropertyAccessExpression(expression)) {
      const owner = testCallKind(expression.expression);
      return owner && ["each", "skip", "only", "concurrent", "todo", "fails"].includes(expression.name.text)
        ? owner
        : null;
    }
    if (ts.isCallExpression(expression)) return testCallKind(expression.expression);
    return null;
  };
  const visit = (node) => {
    if (found) return;
    if (ts.isCallExpression(node) && testCallKind(node.expression)) {
      const [first] = node.arguments;
      if (
        first
        && (ts.isStringLiteral(first) || ts.isNoSubstitutionTemplateLiteral(first))
        && first.text === name
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

function validateImplementationEvidence(
  root,
  trackedPaths,
  evidence,
  candidateId,
  errors,
) {
  const match = /^code:([^#\n]+)#([A-Za-z_$][A-Za-z0-9_$]*)$/.exec(evidence);
  if (!match) {
    pushVerificationError(
      errors,
      "invalid-implementation-evidence",
      `implementation evidence must match code:<tracked-file>#<exact-symbol>: ${evidence}`,
      candidateId,
    );
    return false;
  }
  const [, path, symbol] = match;
  const file = trackedRegularFile(root, trackedPaths, path);
  if (file.status === "missing") {
    pushVerificationError(errors, "implementation-path-missing", `missing implementation path: ${path}`, candidateId);
    return false;
  }
  if (file.status === "untracked") {
    pushVerificationError(errors, "implementation-path-untracked", `untracked implementation path: ${path}`, candidateId);
    return false;
  }
  if (file.status === "realpath-escape") {
    pushVerificationError(errors, "implementation-path-escape", `implementation path escapes repository by symlink: ${path}`, candidateId);
    return false;
  }
  if (file.status === "symlink") {
    pushVerificationError(errors, "implementation-path-symlink", `implementation path is a symlink: ${path}`, candidateId);
    return false;
  }
  if (file.status !== "valid") {
    pushVerificationError(errors, "invalid-implementation-evidence", `implementation path is not a tracked file: ${path}`, candidateId);
    return false;
  }
  const source = readFileSync(file.real, "utf8");
  if (!sourceDeclaresSymbol(path, source, symbol)) {
    pushVerificationError(errors, "implementation-symbol-missing", `implementation symbol is absent: ${path}#${symbol}`, candidateId);
    return false;
  }
  return true;
}

function validateTestEvidence(root, trackedPaths, evidence, candidateId, errors) {
  const match = /^test:([^#\n]+)#([^#\n]+)$/.exec(evidence);
  if (!match) {
    pushVerificationError(
      errors,
      "invalid-test-evidence",
      `test evidence must match test:<tracked-file>#<exact-test-name>: ${evidence}`,
      candidateId,
    );
    return false;
  }
  const [, path, name] = match;
  const file = trackedRegularFile(root, trackedPaths, path);
  if (file.status === "missing") {
    pushVerificationError(errors, "test-path-missing", `missing test path: ${path}`, candidateId);
    return false;
  }
  if (file.status === "untracked") {
    pushVerificationError(errors, "test-path-untracked", `untracked test path: ${path}`, candidateId);
    return false;
  }
  if (file.status === "realpath-escape") {
    pushVerificationError(errors, "test-path-escape", `test path escapes repository by symlink: ${path}`, candidateId);
    return false;
  }
  if (file.status === "symlink") {
    pushVerificationError(errors, "test-path-symlink", `test path is a symlink: ${path}`, candidateId);
    return false;
  }
  if (file.status !== "valid") {
    pushVerificationError(errors, "invalid-test-evidence", `test path is not a tracked file: ${path}`, candidateId);
    return false;
  }
  const source = readFileSync(file.real, "utf8");
  if (!sourceDeclaresTest(path, source, name)) {
    pushVerificationError(errors, "test-name-missing", `test name is absent: ${path}#${name}`, candidateId);
    return false;
  }
  return true;
}

function validateHistoricalReportProvenance(
  root,
  trackedPaths,
  provenance,
  candidateId,
  errors,
) {
  const match = /^historical-report:([^#\r\n]+)$/.exec(provenance);
  const path = match?.[1];
  const file = path ? trackedRegularFile(root, trackedPaths, path) : null;
  if (file?.status === "valid") return true;
  pushVerificationError(
    errors,
    "invalid-historical-report-provenance",
    `historical report provenance must reference one tracked regular file: ${provenance}`,
    candidateId,
  );
  return false;
}

const controlAuditTools = createControlAuditTools({
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
});

export const CONTROL_REVIEW_METADATA = controlAuditTools.CONTROL_REVIEW_METADATA;
export const expectedControlAcceptanceCriteria = controlAuditTools.expectedControlAcceptanceCriteria;
export const extractControls = controlAuditTools.extractControls;
const mergeControlLedger = controlAuditTools.mergeControlLedger;
export const verifyControlAudit = controlAuditTools.verifyControlAudit;

function hasExactObjectKeys(value, expected) {
  return value
    && typeof value === "object"
    && !Array.isArray(value)
    && JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...expected].sort());
}

function verifyFileAudit(root, auditDir) {
  const requestedRepositoryRoot = resolve(root);
  const requestedAuditDirectory = resolve(requestedRepositoryRoot, auditDir);
  const repositoryRoot = realpathSync(requestedRepositoryRoot);
  const requestedAuditRelative = relative(requestedRepositoryRoot, requestedAuditDirectory);
  const auditDirectory = resolve(repositoryRoot, requestedAuditRelative);
  const errors = [];
  const auditRelative = relative(repositoryRoot, auditDirectory);
  if (auditRelative === ".." || auditRelative.startsWith(`..${sep}`) || isAbsolute(auditRelative)) {
    pushVerificationError(errors, "audit-directory-outside-root", "audit directory must remain inside the repository root");
  }
  let ledger = null;
  const ledgerPath = resolve(auditDirectory, "repository-files.json");
  try {
    ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
  } catch (error) {
    pushVerificationError(errors, "invalid-file-ledger", `repository-files.json: ${error.message}`);
  }
  if (!hasExactObjectKeys(ledger, ["schema", "files"]) || ledger?.schema !== 1 || !Array.isArray(ledger?.files)) {
    pushVerificationError(errors, "invalid-file-ledger-schema", "repository-files.json needs exact schema/files keys");
  }
  const files = Array.isArray(ledger?.files) ? ledger.files : [];
  let tracked = [];
  try {
    tracked = readTrackedFiles(repositoryRoot);
  } catch (error) {
    pushVerificationError(errors, "repository-index-unavailable", error.message);
  }
  const recordsByPath = new Map();
  for (const record of files) {
    if (!record || typeof record.path !== "string" || recordsByPath.has(record.path)) {
      pushVerificationError(errors, "invalid-file-record", `invalid or duplicate file record: ${String(record?.path)}`);
      continue;
    }
    recordsByPath.set(record.path, record);
  }
  const trackedSet = new Set(tracked);
  const missing = tracked.filter((path) => !recordsByPath.has(path));
  const orphan = [...recordsByPath.keys()].filter((path) => !trackedSet.has(path));
  for (const path of missing) pushVerificationError(errors, "missing-file", `tracked path is absent from inventory: ${path}`);
  for (const path of orphan) pushVerificationError(errors, "orphan-file", `inventory path is not tracked: ${path}`);
  const selfPath = normalizePath(relative(repositoryRoot, ledgerPath));
  let hashMismatches = 0;
  let deferred = 0;
  for (const path of tracked) {
    const record = recordsByPath.get(path);
    if (!record) continue;
    const classification = classifyFile(path);
    const commonValid = record.id === stableId("file", path)
      && record.path === path
      && record.domain === classification.domain
      && record.kind === classification.kind
      && record.material === classification.material;
    if (!commonValid) {
      pushVerificationError(errors, "file-record-drift", `inventory metadata drifted: ${path}`);
    }
    if (path === selfPath) {
      deferred += 1;
      if (
        !hasExactObjectKeys(record, ["id", "path", "domain", "kind", "material", "bytes", "sha256", "hashStatus", "reason"])
        || record.bytes !== null
        || record.sha256 !== null
        || record.hashStatus !== "self-reference"
        || record.reason !== "inventory cannot hash its own final bytes"
      ) {
        pushVerificationError(errors, "invalid-file-self-reference", `invalid inventory self-reference: ${path}`);
      }
      continue;
    }
    if (record.hashStatus === "deterministic-generated-output") {
      deferred += 1;
      const allowed = new Set([
        normalizePath(join(normalizePath(dirname(selfPath)), "completion-ledger.json")),
        normalizePath(join(normalizePath(dirname(selfPath)), "completion-report.md")),
        normalizePath(join(normalizePath(dirname(selfPath)), "final-verification.json")),
      ]);
      if (
        !allowed.has(path)
        || !hasExactObjectKeys(record, ["id", "path", "domain", "kind", "material", "bytes", "sha256", "hashStatus", "reason"])
        || record.bytes !== null
        || record.sha256 !== null
        || record.reason !== "content is regenerated and compared byte-for-byte by the all-scope verifier after inventory generation"
      ) {
        pushVerificationError(errors, "invalid-deferred-file", `invalid deterministic generated-output exception: ${path}`);
      }
      continue;
    }
    if (!hasExactObjectKeys(record, ["id", "path", "domain", "kind", "material", "bytes", "sha256"])) {
      pushVerificationError(errors, "invalid-file-record-schema", `file record has unexpected keys: ${path}`);
    }
    const bytes = readFileSync(resolve(repositoryRoot, path));
    const digest = createHash("sha256").update(bytes).digest("hex");
    if (record.bytes !== bytes.length || record.sha256 !== digest) {
      hashMismatches += 1;
      pushVerificationError(errors, "file-hash-mismatch", `tracked file bytes/hash drifted: ${path}`);
    }
  }
  return {
    schema: 1,
    scope: "files",
    auditDirectory: normalizePath(relative(repositoryRoot, auditDirectory)) || ".",
    passed: errors.length === 0,
    counts: {
      tracked: tracked.length,
      inventoried: files.length,
      missing: missing.length,
      orphan: orphan.length,
      hashMismatches,
      deferred,
    },
    errors,
  };
}

const SOURCE_LEDGER_KEYS = [
  "schema",
  "auditDate",
  "generation",
  "fetchEvidence",
  "openPullRequests",
  "sources",
  "branchIndex",
  "branchSummary",
  "canonicalDirtyCheckout",
];

function verifySourceAudit(root, auditDir) {
  const requestedRepositoryRoot = resolve(root);
  const requestedAuditDirectory = resolve(requestedRepositoryRoot, auditDir);
  const repositoryRoot = realpathSync(requestedRepositoryRoot);
  const auditRelative = relative(requestedRepositoryRoot, requestedAuditDirectory);
  const auditDirectory = resolve(repositoryRoot, auditRelative);
  const errors = [];
  const load = (name) => {
    try {
      return JSON.parse(readFileSync(resolve(auditDirectory, name), "utf8"));
    } catch (error) {
      pushVerificationError(errors, "invalid-source-audit-file", `${name}: ${error.message}`);
      return null;
    }
  };
  const evidence = load("sources.json");
  const requirements = load("requirements.json");
  const controls = load("controls.json");
  if (!hasExactObjectKeys(evidence, SOURCE_LEDGER_KEYS) || evidence?.schema !== 1) {
    pushVerificationError(errors, "invalid-source-ledger-schema", "sources.json has unexpected top-level keys or schema");
  }
  try {
    validateSourceEvidenceCatalogs(evidence, { requirements, controls });
  } catch (error) {
    pushVerificationError(errors, "invalid-source-evidence", error.message);
  }
  const sources = Array.isArray(evidence?.sources) ? evidence.sources : [];
  const sourceNames = sources.map(({ name }) => name);
  if (sourceNames.some((name) => typeof name !== "string") || new Set(sourceNames).size !== sourceNames.length) {
    pushVerificationError(errors, "duplicate-source-name", "sources must have unique string names");
  }
  let changedPaths = 0;
  let disposedChangedPaths = 0;
  let linkedRequirementIds = 0;
  let linkedControlIds = 0;
  const currentExtractions = [];
  for (const source of sources) {
    const sourceChanges = Array.isArray(source?.changedPaths) ? source.changedPaths : [];
    changedPaths += sourceChanges.length;
    disposedChangedPaths += sourceChanges.filter(
      ({ disposition }) => typeof disposition === "string" && disposition !== "" && disposition !== "unverified",
    ).length;
    linkedRequirementIds += sourceChanges.reduce(
      (total, change) => total + (Array.isArray(change.linkedRequirementIds) ? change.linkedRequirementIds.length : 0),
      0,
    );
    linkedControlIds += sourceChanges.reduce(
      (total, change) => total + (Array.isArray(change.linkedControlIds) ? change.linkedControlIds.length : 0),
      0,
    );
    const sourceRepository = /palmier-pro/i.test(source?.repository ?? "")
      ? resolve(repositoryRoot, "../palmier-pro-upstream")
      : repositoryRoot;
    try {
      const current = captureGitSource({
        name: source.name,
        path: sourceRepository,
        base: source.base,
        head: source.head,
        remote: source.remote,
        expectChanges: source.base !== source.head,
      });
      const fields = [
        "name", "repository", "remote", "base", "head", "baseTree", "headTree",
        "mergeBase", "commitCount", "aheadCount", "behindCount",
      ];
      if (source.comparisonMode !== "ancestry-only") fields.push("status");
      for (const field of fields) {
        if (source[field] !== current[field]) {
          pushVerificationError(errors, "source-provenance-drift", `${source.name}: ${field} differs from current Git extraction`);
        }
      }
      const reviewedPaths = sourceChanges.map(({ status, path, destination }) => ({
        status,
        path,
        destination,
        disposition: "unverified",
      }));
      const currentPathsMatch = source.comparisonMode === "ancestry-only"
        ? sourceChanges.length === 0
          && source.status === "integrated-ancestor"
          && source.mergeBase === source.base
          && source.targetAheadCount === current.commitCount
          && source.forkUniqueCount === current.behindCount
          && source.integratedPathDeltaCount === current.changedPaths.length
        : JSON.stringify(reviewedPaths) === JSON.stringify(current.changedPaths);
      if (!currentPathsMatch) {
        pushVerificationError(errors, "source-changed-paths-drift", `${source.name}: changedPaths differ from current Git extraction`);
      }
      currentExtractions.push({
        name: source.name,
        changedPathsSha256: createHash("sha256").update(JSON.stringify(current.changedPaths)).digest("hex"),
        changedPathCount: current.changedPaths.length,
      });
    } catch (error) {
      pushVerificationError(errors, "source-reextraction-failed", `${source?.name ?? "unknown"}: ${error.message}`);
    }
  }
  const canonicalPaths = Array.isArray(evidence?.canonicalDirtyCheckout?.paths)
    ? evidence.canonicalDirtyCheckout.paths
    : [];
  if (canonicalPaths.length > 0) {
    const canonicalPath = resolve(repositoryRoot, "../OpenTake");
    try {
      const current = captureDirtyCheckout({ name: "canonical dirty OpenTake checkout", path: canonicalPath });
      if (
        current.head !== evidence.canonicalDirtyCheckout.head
        || current.tree !== evidence.canonicalDirtyCheckout.tree
        || current.statusSha256 !== evidence.canonicalDirtyCheckout.statusSha256
        || current.manifestSha256 !== evidence.canonicalDirtyCheckout.captureManifestSha256
        || current.paths.length !== canonicalPaths.length
      ) {
        pushVerificationError(errors, "canonical-source-drift", "canonical dirty checkout differs from captured source provenance");
      }
    } catch (error) {
      pushVerificationError(errors, "canonical-source-reextraction-failed", error.message);
    }
  }
  return {
    schema: 1,
    scope: "sources",
    auditDirectory: normalizePath(relative(repositoryRoot, auditDirectory)) || ".",
    passed: errors.length === 0,
    counts: {
      sources: sources.length,
      changedPaths,
      disposedChangedPaths,
      linkedRequirementIds,
      linkedControlIds,
      canonicalDirtyPaths: canonicalPaths.length,
    },
    hashes: {
      sourceLedgerSha256: existsSync(resolve(auditDirectory, "sources.json"))
        ? createHash("sha256").update(readFileSync(resolve(auditDirectory, "sources.json"))).digest("hex")
        : null,
      currentExtractionsSha256: createHash("sha256").update(JSON.stringify(currentExtractions)).digest("hex"),
    },
    currentExtractions,
    errors,
  };
}

export function verifyAudit(root, auditDir, scope) {
  if (scope === "all") {
    return verifyAllAudit(root, auditDir);
  }
  if (scope === "files") {
    return verifyFileAudit(root, auditDir);
  }
  if (scope === "controls") {
    return verifyControlAudit(root, auditDir);
  }
  if (scope === "sources") {
    return verifySourceAudit(root, auditDir);
  }
  if (scope !== "documents") {
    throw new Error(`unsupported verification scope: ${scope}`);
  }
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
    pushVerificationError(
      errors,
      "audit-directory-outside-root",
      "audit directory must remain inside the repository root",
    );
  }
  let auditSymlinkEscape = false;
  if (!auditOutsideRoot && existsSync(auditDirectory) && !realPathWithinRoot(repositoryRoot, auditDirectory)) {
    auditSymlinkEscape = true;
    pushVerificationError(
      errors,
      "audit-directory-symlink-escape",
      "audit directory resolves outside the repository root",
    );
  }
  const candidates = auditOutsideRoot || auditSymlinkEscape
    ? []
    : readDocumentAuditFile(
      repositoryRoot,
      auditDirectory,
      "document-candidates.json",
      "candidates",
      errors,
    );
  const records = auditOutsideRoot || auditSymlinkEscape
    ? []
    : readDocumentAuditFile(
      repositoryRoot,
      auditDirectory,
      "requirements.json",
      "records",
      errors,
    );
  let trackedPaths = new Set();
  try {
    trackedPaths = new Set(readTrackedFiles(repositoryRoot));
  } catch (error) {
    pushVerificationError(errors, "repository-index-unavailable", error.message);
  }

  const currentCandidates = [...trackedPaths]
    .filter((path) => path.endsWith(".md") && !path.startsWith("docs/audit/"))
    .sort(compareAuditText)
    .flatMap((path) => {
      const absolute = resolve(repositoryRoot, path);
      if (!existsSync(absolute) || !lstatSync(absolute).isFile()) return [];
      return extractDocumentCandidates(path, readFileSync(absolute, "utf8"));
    });
  if (JSON.stringify(candidates) !== JSON.stringify(currentCandidates)) {
    pushVerificationError(
      errors,
      "document-candidate-ledger-drift",
      "document-candidates.json differs from a full current-source re-extraction",
    );
  }

  const candidateIds = candidates
    .map((candidate) => candidate?.id)
    .filter((id) => typeof id === "string");
  const recordIds = records
    .map((record) => record?.id)
    .filter((id) => typeof id === "string");
  const recordCandidateIds = records
    .map((record) => record?.candidateId)
    .filter((id) => typeof id === "string");
  const duplicateCandidateDefinitions = duplicateCount(candidateIds);
  const duplicateRecordIds = duplicateCount(recordIds);
  const duplicateCandidateIds = duplicateCount(recordCandidateIds);
  if (duplicateCandidateDefinitions > 0) {
    pushVerificationError(
      errors,
      "duplicate-candidate-definition-id",
      `document candidates contain ${duplicateCandidateDefinitions} duplicate ID(s)`,
    );
  }
  if (duplicateRecordIds > 0) {
    pushVerificationError(
      errors,
      "duplicate-record-id",
      `requirements contain ${duplicateRecordIds} duplicate record ID(s)`,
    );
  }
  if (duplicateCandidateIds > 0) {
    pushVerificationError(
      errors,
      "duplicate-candidate-id",
      `requirements contain ${duplicateCandidateIds} duplicate candidateId(s)`,
    );
  }

  const candidatesById = new Map();
  for (const candidate of candidates) {
    if (!candidate || typeof candidate.id !== "string") {
      pushVerificationError(errors, "invalid-candidate", "candidate id must be a string");
      continue;
    }
    if (!hasExactObjectKeys(candidate, [
      "id", "semanticFingerprint", "semanticOrdinal", "path", "line", "heading", "text", "signal",
    ])) {
      pushVerificationError(errors, "invalid-candidate", "document candidate keys differ from semantic identity schema", candidate.id);
    }
    if (typeof candidate.path !== "string" || !Number.isInteger(candidate.line)) {
      pushVerificationError(
        errors,
        "invalid-candidate",
        "candidate source must contain a string path and integer line",
        candidate.id,
      );
    } else {
      const sourceFile = trackedRegularFile(repositoryRoot, trackedPaths, candidate.path);
      if (sourceFile.status === "missing") {
        pushVerificationError(errors, "candidate-source-missing", `candidate source is missing: ${candidate.path}`, candidate.id);
      } else if (sourceFile.status === "untracked") {
        pushVerificationError(errors, "candidate-source-untracked", `candidate source is untracked: ${candidate.path}`, candidate.id);
      } else if (sourceFile.status === "realpath-escape") {
        pushVerificationError(errors, "candidate-source-symlink-escape", `candidate source escapes repository by symlink: ${candidate.path}`, candidate.id);
      } else if (sourceFile.status === "symlink") {
        pushVerificationError(errors, "candidate-source-symlink", `candidate source is a symlink: ${candidate.path}`, candidate.id);
      } else if (sourceFile.status !== "valid") {
        pushVerificationError(errors, "invalid-candidate", `candidate source is not a tracked file: ${candidate.path}`, candidate.id);
      } else {
        const derivedCandidates = extractDocumentCandidates(
          candidate.path,
          readFileSync(sourceFile.real, "utf8"),
        );
        const derived = derivedCandidates.find((item) => item.id === candidate.id);
        if (!derived) {
          pushVerificationError(errors, "candidate-source-signal-missing", `candidate signal is absent at ${candidate.path}:${candidate.line}`, candidate.id);
          const relocatedIdentity = derivedCandidates.find((item) => item.line === candidate.line);
          if (relocatedIdentity && relocatedIdentity.id !== candidate.id) {
            pushVerificationError(errors, "candidate-id-mismatch", `candidate ID does not derive from current semantic source: ${candidate.id}`, candidate.id);
          }
        } else {
          if (candidate.text !== derived.text) {
            pushVerificationError(errors, "candidate-text-drift", `candidate text drifted at ${candidate.path}:${candidate.line}`, candidate.id);
          }
          if (candidate.signal !== derived.signal) {
            pushVerificationError(errors, "candidate-signal-drift", `candidate signal drifted at ${candidate.path}:${candidate.line}`, candidate.id);
          }
          if (candidate.heading !== derived.heading) {
            pushVerificationError(errors, "candidate-heading-drift", `candidate heading drifted at ${candidate.path}:${candidate.line}`, candidate.id);
          }
          if (candidate.id !== derived.id) {
            pushVerificationError(errors, "candidate-id-mismatch", `candidate ID does not derive from current source: ${candidate.id}`, candidate.id);
          }
        }
      }
    }
    if (!candidatesById.has(candidate.id)) candidatesById.set(candidate.id, candidate);
  }
  const recordCandidateIdSet = new Set(recordCandidateIds);
  const missingCandidateIds = [...candidatesById.keys()].filter(
    (candidateId) => !recordCandidateIdSet.has(candidateId),
  );
  for (const candidateId of missingCandidateIds) {
    pushVerificationError(
      errors,
      "missing-candidate-id",
      `candidate has no requirement record: ${candidateId}`,
      candidateId,
    );
  }
  const orphanCandidateIds = [...new Set(recordCandidateIds)].filter(
    (candidateId) => !candidatesById.has(candidateId),
  );
  for (const candidateId of orphanCandidateIds) {
    pushVerificationError(
      errors,
      "orphan-candidate-id",
      `requirement record has no document candidate: ${candidateId}`,
      candidateId,
    );
  }

  const statusCounts = {
    unverified: 0,
    complete: 0,
    incomplete: 0,
    contradicted: 0,
    obsolete: 0,
    duplicate: 0,
  };
  for (const record of records) {
    if (!record || typeof record !== "object" || Array.isArray(record)) {
      pushVerificationError(errors, "invalid-record", "requirement record must be an object");
      continue;
    }
    const candidateId = typeof record.candidateId === "string"
      ? record.candidateId
      : null;
    if (!candidateId) {
      pushVerificationError(errors, "invalid-record", "requirement candidateId must be a string");
    }
    if (typeof record.id !== "string") {
      pushVerificationError(
        errors,
        "invalid-record",
        "requirement id must be a string",
        candidateId,
      );
    } else if (
      candidateId
      && record.id !== stableId("requirement", candidateId)
    ) {
      pushVerificationError(
        errors,
        "record-id-mismatch",
        `requirement id must derive from candidateId ${candidateId}`,
        candidateId,
      );
    }
    for (const field of DOCUMENT_ARRAY_FIELDS) {
      if (
        !Array.isArray(record[field])
        || record[field].some((item) => !nonEmptyString(item))
      ) {
        pushVerificationError(
          errors,
          "invalid-field-type",
          `${field} must be an array of non-empty strings`,
          candidateId,
        );
      }
    }
    for (const field of [
      "targetBehavior",
      "priority",
      "visibleResult",
      "gapGroup",
      "finalDisposition",
      "commit",
    ]) {
      if (record[field] != null && typeof record[field] !== "string") {
        pushVerificationError(
          errors,
          "invalid-field-type",
          `${field} must be a string or null`,
          candidateId,
        );
      }
    }

    const candidate = candidateId ? candidatesById.get(candidateId) : null;
    if (candidate) {
      const validSource = record.source
        && typeof record.source === "object"
        && !Array.isArray(record.source)
        && typeof record.source.path === "string"
        && Number.isInteger(record.source.line);
      if (!validSource) {
        pushVerificationError(
          errors,
          "invalid-field-type",
          "source must contain a string path and integer line",
          candidateId,
        );
      } else if (
        typeof candidate.path === "string"
        && Number.isInteger(candidate.line)
        && (
          normalizePath(record.source.path) !== normalizePath(candidate.path)
          || record.source.line !== candidate.line
        )
      ) {
        pushVerificationError(
          errors,
          "candidate-source-drift",
          `source does not match candidate ${candidateId}`,
          candidateId,
        );
      }
    }

    for (const evidence of Array.isArray(record.runtimeEvidence) ? record.runtimeEvidence : []) {
      pushVerificationError(
        errors,
        "unsupported-runtime-evidence",
        `documents scope does not accept self-reported runtime evidence: ${evidence}`,
        candidateId,
      );
    }
    for (const provenance of Array.isArray(record.provenance) ? record.provenance : []) {
      if (provenance.startsWith("historical-report:")) {
        validateHistoricalReportProvenance(
          repositoryRoot,
          trackedPaths,
          provenance,
          candidateId,
          errors,
        );
      }
    }

    if (record.status === "unverified") {
      statusCounts.unverified += 1;
      pushVerificationError(
        errors,
        "unverified-status",
        "unverified status is forbidden",
        candidateId,
      );
      continue;
    }
    if (!DOCUMENT_REQUIREMENT_STATUSES.has(record.status)) {
      pushVerificationError(
        errors,
        "invalid-status",
        `unsupported requirement status: ${String(record.status)}`,
        candidateId,
      );
      continue;
    }
    statusCounts[record.status] += 1;

    if (["complete", "incomplete"].includes(record.status)) {
      if (!nonEmptyString(record.targetBehavior)) {
        pushVerificationError(
          errors,
          "requirement-without-target-behavior",
          `${record.status} requirement needs explicit target behavior`,
          candidateId,
        );
      }
      if (!nonEmptyString(record.finalDisposition)) {
        pushVerificationError(
          errors,
          "requirement-without-final-disposition",
          `${record.status} requirement needs an explicit final disposition`,
          candidateId,
        );
      }
    }

    if (record.status === "complete") {
      const implementationEvidence = DOCUMENT_IMPLEMENTATION_FIELDS
        .flatMap((field) => Array.isArray(record[field]) ? record[field] : []);
      const hasImplementation = implementationEvidence.length > 0;
      for (const evidence of implementationEvidence) {
        validateImplementationEvidence(
          repositoryRoot,
          trackedPaths,
          evidence,
          candidateId,
          errors,
        );
      }
      if (!hasImplementation) {
        pushVerificationError(
          errors,
          "complete-without-implementation",
          "complete requirement needs exact code:<tracked-file>#<symbol> evidence",
          candidateId,
        );
      }
      const automatedTests = Array.isArray(record.automatedTests)
        ? record.automatedTests
        : [];
      for (const evidence of automatedTests) {
        validateTestEvidence(repositoryRoot, trackedPaths, evidence, candidateId, errors);
      }
      if (automatedTests.length === 0) {
        pushVerificationError(
          errors,
          "complete-without-verification",
          "complete requirement needs exact test:<tracked-file>#<test-name> evidence",
          candidateId,
        );
      }
    }

    if (record.status === "incomplete") {
      if (!nonEmptyStringArray(record.acceptanceCriteria)) {
        pushVerificationError(
          errors,
          "incomplete-without-acceptance-criteria",
          "incomplete requirement needs exact acceptance criteria",
          candidateId,
        );
      }
      if (record.gapGroup == null || record.gapGroup === "") {
        pushVerificationError(
          errors,
          "incomplete-without-gap-group",
          "incomplete requirement needs a subsystem gap group",
          candidateId,
        );
      } else if (!DOCUMENT_GAP_GROUP_SET.has(record.gapGroup)) {
        pushVerificationError(
          errors,
          "invalid-gap-group",
          `unsupported gap group: ${record.gapGroup}`,
          candidateId,
        );
      }
    } else if (record.gapGroup != null) {
      pushVerificationError(
        errors,
        "invalid-gap-group",
        `gap group is only valid for incomplete requirements: ${record.gapGroup}`,
        candidateId,
      );
    }

    if (DOCUMENT_NON_REQUIREMENT_STATUSES.has(record.status)) {
      if (!nonEmptyString(record.targetBehavior)) {
        pushVerificationError(
          errors,
          "disposition-without-target-behavior",
          `${record.status} disposition needs explicit target behavior`,
          candidateId,
        );
      }
      if (!nonEmptyString(record.finalDisposition)) {
        pushVerificationError(
          errors,
          "disposition-without-final-disposition",
          `${record.status} disposition needs an explicit final disposition`,
          candidateId,
        );
      }
      if (!nonEmptyStringArray(record.provenance)) {
        pushVerificationError(
          errors,
          "disposition-without-provenance",
          `${record.status} disposition needs provenance`,
          candidateId,
        );
      }
    }
  }

  const counts = {
    candidates: candidates.length,
    records: records.length,
    uniqueCandidateIds: new Set(recordCandidateIds).size,
    uniqueRecordIds: new Set(recordIds).size,
    missingCandidateIds: missingCandidateIds.length,
    orphanCandidateIds: orphanCandidateIds.length,
    duplicateCandidateIds,
    duplicateRecordIds,
    ...statusCounts,
  };
  return {
    schema: 1,
    scope: "documents",
    auditDirectory: normalizePath(relative(repositoryRoot, auditDirectory)) || ".",
    passed: errors.length === 0,
    legalGapGroups: DOCUMENT_GAP_GROUPS,
    counts,
    errors,
  };
}

function loadAuditJson(auditDirectory, name) {
  return JSON.parse(readFileSync(resolve(auditDirectory, name), "utf8"));
}

function auditInputHashes(auditDirectory, names) {
  return Object.fromEntries(names.map((name) => [
    name,
    sha256(readFileSync(resolve(auditDirectory, name))),
  ]));
}

function sourceClassificationRecords(sources) {
  const rows = [];
  for (const source of sources.sources) {
    source.changedPaths.forEach((change, index) => rows.push({
      owner: source.name,
      ownerKind: "git-source",
      index,
      ...change,
    }));
  }
  sources.canonicalDirtyCheckout.paths.forEach((change, index) => rows.push({
    owner: sources.canonicalDirtyCheckout.name,
    ownerKind: "canonical-dirty-checkout",
    index,
    ...change,
  }));
  return rows;
}

export function buildCompletionLedger(root, auditDir) {
  const repositoryRoot = realpathSync(resolve(root));
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const files = loadAuditJson(auditDirectory, "repository-files.json").files;
  const requirements = loadAuditJson(auditDirectory, "requirements.json").records;
  const controls = loadAuditJson(auditDirectory, "controls.json").records;
  const sources = loadAuditJson(auditDirectory, "sources.json");
  const runtime = loadAuditJson(auditDirectory, "runtime-evidence.json");
  const identityMigration = loadAuditJson(auditDirectory, "identity-migration.json");
  const sourceRows = sourceClassificationRecords(sources);
  const records = [
    ...files.map((file) => ({
      id: stableId("completion", `file:${file.id}`),
      kind: "file",
      sourceId: file.id,
      candidateId: null,
      status: "inventoried",
      gapGroup: null,
      source: { path: file.path },
      evidence: {
        implementation: [],
        automatedTests: [],
        runtime: [],
        provenance: [file.sha256 ? `sha256:${file.sha256}` : `${file.hashStatus}:${file.reason}`],
      },
    })),
    ...requirements.map((record) => ({
      id: stableId("completion", `requirement:${record.id}`),
      kind: "requirement",
      sourceId: record.id,
      candidateId: record.candidateId,
      status: record.status,
      gapGroup: record.gapGroup,
      source: structuredClone(record.source),
      evidence: {
        implementation: [...record.react, ...record.storeApi, ...record.tauri, ...record.rust],
        automatedTests: structuredClone(record.automatedTests),
        runtime: structuredClone(record.runtimeEvidence),
        provenance: structuredClone(record.provenance),
      },
    })),
    ...controls.map((record) => ({
      id: stableId("completion", `control:${record.id}`),
      kind: "control",
      sourceId: record.id,
      candidateId: record.candidateId,
      status: record.status,
      gapGroup: record.gapGroup,
      source: structuredClone(record.source),
      evidence: {
        implementation: structuredClone(record.backendTrace),
        automatedTests: structuredClone(record.automatedTests),
        runtime: structuredClone(record.runtimeEvidence),
        provenance: [record.finalDisposition],
      },
    })),
    ...sourceRows.map((record) => ({
      id: stableId("completion", `source:${record.owner}:${record.index}:${record.path}:${record.destination ?? ""}`),
      kind: "source-change",
      sourceId: stableId("source-change", `${record.owner}:${record.index}:${record.path}:${record.destination ?? ""}`),
      candidateId: null,
      status: record.disposition,
      gapGroup: null,
      source: { owner: record.owner, path: record.path, destination: record.destination ?? null },
      evidence: {
        implementation: [record.openTakeEquivalent],
        automatedTests: [],
        runtime: [],
        provenance: [record.behavior, record.rationale ?? record.relationship ?? record.disposition],
      },
    })),
    ...runtime.receipts.map((receipt) => ({
      id: stableId("completion", `runtime:${receipt.id}`),
      kind: "runtime-receipt",
      sourceId: receipt.id,
      candidateId: null,
      status: receipt.status,
      gapGroup: null,
      source: { kind: receipt.kind, command: receipt.command },
      evidence: {
        implementation: [],
        automatedTests: structuredClone(receipt.testEvidence),
        runtime: structuredClone(receipt.artifacts.map(({ path, sha256: digest }) => `${path}#${digest}`)),
        provenance: [receipt.executedCheckoutRevision?.commit ?? "not-executed"],
      },
    })),
  ];
  const incompleteRequirements = requirements.filter(({ status }) => status === "incomplete");
  const incompleteControls = controls.filter(({ status }) => status === "incomplete");
  const gapCounts = Object.fromEntries(DOCUMENT_GAP_GROUPS.map((group) => {
    const requirementCount = incompleteRequirements.filter(({ gapGroup }) => gapGroup === group).length;
    const controlCount = incompleteControls.filter(({ gapGroup }) => gapGroup === group).length;
    return [group, { requirements: requirementCount, controls: controlCount, total: requirementCount + controlCount }];
  }));
  const inputNames = [
    "repository-files.json",
    "document-candidates.json",
    "requirements.json",
    "control-candidates.json",
    "controls.json",
    "runtime-evidence.json",
    "sources.json",
    "identity-migration.json",
    "implementation-plan-index.md",
    "plan-ownership.json",
  ];
  return {
    schema: 1,
    metadata: {
      schemaName: "opentake-completion-ledger",
      schemaVersion: 1,
      recordKinds: ["file", "requirement", "control", "source-change", "runtime-receipt"],
      selfReferencePolicy: "completion-ledger/report/final-verification bytes are deterministically regenerated after repository inventory",
    },
    provenance: {
      verifierRevision: {
        commit: identityMigration.provenance.migrationRevision.commit,
        tree: identityMigration.provenance.migrationRevision.tree,
      },
      inputSha256: auditInputHashes(auditDirectory, inputNames),
    },
    counts: {
      files: files.length,
      requirements: requirements.length,
      controls: controls.length,
      sources: sources.sources.length,
      sourceChanges: sourceRows.length,
      runtimeReceipts: runtime.receipts.length,
      records: records.length,
      incompleteRequirements: incompleteRequirements.length,
      incompleteControls: incompleteControls.length,
      incomplete: incompleteRequirements.length + incompleteControls.length,
      unverified: requirements.filter(({ status }) => status === "unverified").length
        + controls.filter(({ status }) => status === "unverified").length,
    },
    gapCounts,
    records,
  };
}

function markdownCell(value) {
  return String(value ?? "").replaceAll("|", "\\|").replace(/\s+/g, " ").trim();
}

function markdownDocument(lines) {
  const normalized = [...lines];
  while (normalized.at(-1) === "") normalized.pop();
  return `${normalized.join("\n")}\n`;
}

export function renderCompletionReport(ledger) {
  const lines = [
    "# OpenTake completion audit report",
    "",
    "This report is generated from `completion-ledger.json`; completion remains fail-closed until every incomplete record is implemented and directly verified.",
    "",
    "## Coverage",
    "",
    `- Tracked files: ${ledger.counts.files}`,
    `- Planning requirements: ${ledger.counts.requirements}`,
    `- Interactive controls: ${ledger.counts.controls}`,
    `- Source classifications: ${ledger.counts.sourceChanges}`,
    `- Runtime receipts: ${ledger.counts.runtimeReceipts}`,
    `- Incomplete records: ${ledger.counts.incomplete} (${ledger.counts.incompleteRequirements} requirements + ${ledger.counts.incompleteControls} controls)`,
    `- Unverified records: ${ledger.counts.unverified}`,
    "",
    "## Gap groups",
    "",
    "| Group | Requirements | Controls | Total |",
    "|---|---:|---:|---:|",
  ];
  for (const group of DOCUMENT_GAP_GROUPS) {
    const counts = ledger.gapCounts[group];
    lines.push(`| ${group} | ${counts.requirements} | ${counts.controls} | ${counts.total} |`);
  }
  const statusCounts = new Map();
  for (const record of ledger.records.filter(({ kind }) => ["requirement", "control"].includes(kind))) {
    statusCounts.set(record.status, (statusCounts.get(record.status) ?? 0) + 1);
  }
  lines.push("", "## Verified dispositions", "");
  for (const [status, count] of [...statusCounts.entries()].sort(([left], [right]) => compareAuditText(left, right))) {
    lines.push(`- ${status}: ${count}`);
  }
  lines.push(
    "",
    "## Evidence limits",
    "",
    "Browser fallback, static traces, and broad green suites are supporting evidence only. No incomplete record is promoted by this report; Task 9 must close each indexed acceptance contract with candidate-bound tests and the required native/runtime proof.",
    "",
    "## Closure rule",
    "",
    "Every incomplete record appears exactly once in `implementation-plan-index.md`. The all-scope verifier rejects missing, duplicate, illegal-group, source-drift, hash-drift, and unsupported-complete claims.",
  );
  return markdownDocument(lines);
}

function extractPlanFileRefs(record, candidate) {
  const strings = record.kind === "control"
    ? [candidate.path, ...record.backendTrace]
    : [
      record.source.path,
      ...record.react,
      ...record.storeApi,
      ...record.tauri,
      ...record.rust,
      ...record.automatedTests,
      ...record.provenance,
      ...record.acceptanceCriteria,
      record.targetBehavior,
      record.finalDisposition,
    ];
  const refs = [];
  const pattern = /(?:code:|test:|current-evidence:)?((?:\.github|crates|docs|src-tauri|tools|web)\/[^\s|;,\]"'`()]+?\.(?:rs|tsx?|mjs|md|json|toml|ya?ml))(?:#|::)?([A-Za-z_$][A-Za-z0-9_$:]*)?/g;
  for (const value of strings) {
    for (const match of String(value).matchAll(pattern)) {
      refs.push(match[2] ? `${match[1]}#${match[2]}` : match[1]);
    }
  }
  if (refs.length === 0) refs.push(record.source.path);
  return [...new Set(refs)];
}

const PLAN_PRODUCT_SOURCE = /^(?:web\/src\/(?!.*\.(?:test|spec)\.)|src-tauri\/src\/|crates\/[^/]+\/src\/).+\.(?:tsx?|rs)$/;
const PLAN_TYPESCRIPT_TEST = /^web\/src\/.+\.(?:test|spec)\.tsx?$/;
const PLAN_RUST_INTEGRATION_TEST = /^(?:src-tauri|crates\/[^/]+)\/tests\/.+\.rs$/;
function splitPlanRef(ref) {
  const index = String(ref).indexOf("#");
  return index === -1
    ? { path: String(ref), symbol: null }
    : { path: String(ref).slice(0, index), symbol: String(ref).slice(index + 1) };
}

function declaredPlanSymbols(path, content) {
  const symbols = [];
  const patterns = path.endsWith(".rs")
    ? [/(?:^|\n)\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:struct|enum|trait|type|fn|mod|const|static)\s+(?:r#)?([A-Za-z_][A-Za-z0-9_]*)/g]
    : [
      /(?:^|\n)\s*(?:export\s+)?(?:default\s+)?(?:async\s+)?(?:function|class|interface|type|enum|const|let|var)\s+([A-Za-z_$][A-Za-z0-9_$]*)/g,
      /(?:^|\n)\s*(?:public\s+|private\s+|protected\s+|static\s+|async\s+)*([A-Za-z_$][A-Za-z0-9_$]*)\s*\([^\n;{}]*\)\s*\{/g,
    ];
  for (const pattern of patterns) {
    for (const match of content.matchAll(pattern)) {
      if (!["if", "for", "while", "switch", "catch", "constructor"].includes(match[1])) symbols.push(match[1]);
    }
  }
  return [...new Set(symbols)];
}

function planningIndex(root) {
  const trackedPaths = new Set(readTrackedFiles(root));
  const contentsByPath = new Map();
  const sources = [];
  const tests = [];
  for (const path of trackedPaths) {
    if (!/\.(?:tsx?|rs|json|css|toml|wgsl)$/.test(path)) continue;
    const content = readFileSync(resolve(root, path), "utf8");
    const entry = {
      path,
      content,
      symbols: declaredPlanSymbols(path, content),
    };
    contentsByPath.set(path, entry);
    if (!/\.(?:tsx?|rs)$/.test(path)) continue;
    if (PLAN_PRODUCT_SOURCE.test(path) && entry.symbols.length > 0) sources.push(entry);
    if (
      PLAN_TYPESCRIPT_TEST.test(path)
      || PLAN_RUST_INTEGRATION_TEST.test(path)
      || (path.endsWith(".rs") && /#\[(?:tokio::|async_std::)?test\]/.test(content))
    ) tests.push(entry);
  }
  return {
    sources,
    tests,
    trackedPaths,
    contentsByPath,
    sourcesByPath: new Map(sources.map((entry) => [entry.path, entry])),
    testsByPath: new Map(tests.map((entry) => [entry.path, entry])),
  };
}

function rustTestCommand(path, name, features = [], noDefaultFeatures = false) {
  const crate = path.startsWith("crates/") ? path.split("/")[1] : "opentake-tauri";
  const integration = /\/tests\/([^/]+)\.rs$/.exec(path);
  const defaultArgs = noDefaultFeatures ? " --no-default-features" : "";
  const featureArgs = features.length > 0 ? ` --features ${features.join(",")}` : "";
  return integration
    ? `cargo test -p ${crate}${defaultArgs}${featureArgs} --test ${integration[1]} ${name} -- --exact`
    : `cargo test -p ${crate}${defaultArgs}${featureArgs} ${name}`;
}

const REVIEWED_OWNERSHIP_SLICE_OVERRIDES = {
  "MR-capcut-composite": {
    childCapabilities: ["MR-transitions", "MR-text-parity", "MR-media-facade"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "capcut_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" }],
  },
  "MR-hdr-proxy-account-composite": {
    childCapabilities: ["MR-bounded-index-runtime", "MR-packaged-ffmpeg", "MR-media-facade"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "hdr_proxy_account_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" }],
  },
  "MR-renderer-debt-composite": {
    childCapabilities: ["MR-generic-effects", "MR-text-parity", "MR-native-chromium"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "renderer_debt_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" }],
  },
  "MR-release-readiness-composite": {
    childCapabilities: ["MR-packaged-ffmpeg", "MR-playback-route-lifecycle-complete", "MR-ffmpeg-contract-complete"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "release_readiness_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" }],
  },
  "MR-advanced-shader-composite": {
    childCapabilities: ["MR-mask-rendering", "MR-lgg-proof", "MR-hsl-secondary", "MR-lut"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "advanced_shader_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" }],
  },
  "MR-mask-effect-mixed-duplicate": {
    childCapabilities: ["MR-mask-rendering", "MR-generic-effects"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "mask_and_effect_records_have_separate_child_owners", evidenceClass: "reviewed-planned" }],
  },
  "MR-media-principles-headings": {
    childCapabilities: ["MR-media-facade", "MR-ffmpeg-contract-complete", "MR-bounded-audio-streaming"],
    tests: [{ path: "crates/opentake-render/tests/composite_acceptance.rs", name: "media_principles_headings_reference_exact_child_capabilities", evidenceClass: "reviewed-planned" }],
  },
};

function reviewedOwnershipRules(group) {
  const ownershipGroup = REVIEWED_OWNERSHIP_GROUPS[group];
  if (!ownershipGroup) return [];
  return ownershipGroup.slices.map((slice) => {
    const override = REVIEWED_OWNERSHIP_SLICE_OVERRIDES[slice.sliceKey] ?? {};
    const cluster = REVIEWED_CAPABILITY_CLUSTER_BY_RULE_KEY.get(slice.sliceKey);
    const disposition = slice.classification === "implementation"
      ? "implementation-gap"
      : slice.classification === "evidence-closure"
        ? "evidence-closure"
        : "composite-acceptance";
    return {
      ruleId: slice.sliceKey,
      recordIds: slice.recordIds.map(normalizeReviewedRecordId),
      products: override.products ?? cluster?.products ?? slice.products,
      tests: override.tests ?? cluster?.tests ?? slice.tests,
      childCapabilities: override.childCapabilities ?? [],
      capabilityId: cluster?.capabilityId ?? slice.capabilityId,
      primaryGroup: cluster?.primaryGroup ?? slice.primaryGroup,
      disposition,
      classification: slice.classification,
      correctionIds: slice.correctionIds ?? [],
      rationale: cluster?.rationale ?? slice.rationale,
    };
  });
}

const REVIEWED_REPORT_SLICE_DEFINITIONS_RAW = [
  ...reviewedOwnershipRules("media-render-playback-export"),
  {
    ruleId: "DS-legacy-default-matrix",
    disposition: "implementation-gap",
    recordIds: ["requirement-365ac4943b157d3e"],
    products: [
      "crates/opentake-project/src/bundle.rs#Project",
      "crates/opentake-domain/src/clip.rs#Clip",
      "crates/opentake-domain/src/media.rs#MediaManifest",
      "crates/opentake-project/src/gen_log.rs#GenerationLogEntry",
    ],
    tests: [
      { path: "crates/opentake-project/tests/upstream_compat.rs", name: "applies_clip_defaults_for_omitted_fields", evidenceClass: "existing-owned" },
      { path: "crates/opentake-project/tests/upstream_compat.rs", name: "migrates_legacy_transform_xy_to_center", evidenceClass: "existing-owned" },
      { path: "crates/opentake-project/tests/upstream_compat.rs", name: "migrates_generation_log_legacy_cost_and_version", evidenceClass: "existing-owned" },
      { path: "crates/opentake-project/tests/upstream_compat.rs", name: "exhaustive_legacy_default_matrix", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report DS-legacy-default-matrix: existing migration tests do not cover the exhaustive documented field plus save/reopen matrix.",
  },
  {
    ruleId: "DS-mcp-transport",
    disposition: "implementation-gap",
    recordIds: ["requirement-473d4379da3bd4cc"],
    products: ["crates/opentake-agent/src/mcp/server.rs#McpServer", "crates/opentake-agent/src/mcp/server.rs#serve_with_bridge"],
    tests: [
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "non_local_origin_is_rejected", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "oversized_request_body_is_rejected", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "serve_rejects_non_loopback_bind", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "unsupported_protocol_version_is_400", evidenceClass: "reviewed-planned" },
    ],
    capabilityId: "mcp-transport",
    rationale: "Core mapping report DS-mcp-transport: loopback bind and protocol-version rejection remain unproved production constraints.",
  },
  {
    ruleId: "DS-mcp-tool-import",
    disposition: "implementation-gap",
    recordIds: ["requirement-d317ca3e45fba737"],
    products: [
      "crates/opentake-agent/src/tools/errors.rs#decode_tool_args",
      "crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher",
      "src-tauri/src/mcp.rs#TauriMediaBridge",
    ],
    tests: [
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_requires_exactly_one_source", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_rejects_unknown_nested_source_key", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_bytes_rejects_oversized_base64_before_bridge", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/tool_argument_contract.rs", name: "all_tool_schemas_reject_unknown_missing_wrong_type", evidenceClass: "reviewed-planned" },
      { path: "src-tauri/src/mcp.rs", name: "https_url_import_enforces_scheme_mime_and_decoded_limit", evidenceClass: "reviewed-planned" },
    ],
    capabilityId: "mcp-tool-import",
    rationale: "Core mapping report DS-mcp-tool-import: HTTPS-only streaming, typed MIME and decoded-size enforcement are not wired through the bridge.",
  },
  {
    ruleId: "DS-mcp-redaction",
    disposition: "implementation-gap",
    recordIds: ["requirement-2e9e6066655d5846"],
    products: ["src-tauri/src/mcp.rs#TauriMediaBridge", "crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result"],
    tests: [
      { path: "crates/opentake-agent/tests/mcp_error_redaction.rs", name: "llm_errors_redact_paths_credentials_headers_provider_bodies", evidenceClass: "reviewed-planned" },
    ],
    capabilityId: "mcp-error-redaction",
    rationale: "Core mapping report DS-mcp-redaction: bridge errors still need a focused LLM-boundary redaction matrix for paths, credentials, headers and provider bodies.",
  },
  {
    ruleId: "DS-generation-seed",
    disposition: "implementation-gap",
    recordIds: ["requirement-b9010e6717b5d5ea", "requirement-1f35cc4131f8f0b7"],
    products: ["crates/opentake-project/src/bundle.rs#Project", "crates/opentake-core/src/session.rs#EditorSession"],
    tests: [
      { path: "crates/opentake-project/tests/roundtrip.rs", name: "malformed_generation_log_is_ignored", evidenceClass: "existing-owned" },
      { path: "crates/opentake-core/tests/project_open.rs", name: "missing_generation_log_seeds_manifest_provenance_once", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report DS-generation-seed: project open currently defaults an absent generation log instead of seeding manifest provenance once.",
  },
  {
    ruleId: "DS-unix-consuming-tests",
    disposition: "implementation-gap",
    recordIds: ["requirement-67e50cfe6dd7f49a"],
    products: [
      "crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority",
      "crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory",
    ],
    tests: [
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "source_swap_before_quarantine_restores_without_deletion", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "restore_collision_fail_leaks_original_and_quarantine", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "final_unix_name_window_is_explicit_same_account_boundary", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "cleanup_capability_records_identity_before_consuming_delete", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "nested_recursive_quarantine_cleanup_removes_files_symlink_fifo_and_directories", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "destination_collision_preserves_stage_and_every_destination_kind", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report DS-unix-consuming-tests: the six named mutation/cleanup contracts are absent and unix.rs still includes the unsupported backend.",
  },
  {
    ruleId: "DS-windows-safe-fs",
    disposition: "implementation-gap",
    recordIds: [
      "requirement-db25fa5cad389d60", "requirement-03a0d53b4d783fb7", "requirement-e448f9531bbdb638",
    ],
    products: [
      "crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority",
      "crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory",
    ],
    tests: [
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "windows_contract", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/src/safe_fs/tests.rs", name: "synchronous_nt_pending_is_invariant_error", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report DS-windows-safe-fs: windows.rs is still the unsupported backend and needs the complete NT contract plus cross-compile gate.",
  },
  {
    ruleId: "DS-native-receipt-validator",
    disposition: "implementation-gap",
    recordIds: ["requirement-eb7ba5012fb1494f"],
    products: [
      "crates/opentake-project/src/safe_fs/capability.rs#DirectoryAuthority",
      "crates/opentake-project/src/safe_fs/ops.rs#capture_absolute_directory",
    ],
    tests: [
      { path: "scripts/tests/validate-c1b-evidence-test.rb", name: "validates_c1b_receipt_provenance_and_rejects_forgery", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report DS-native-receipt-validator: this process/evidence slice must add the tracked Windows RED harness and receipt validator around the safe-fs boundary.",
  },
  {
    ruleId: "DS-project-open-composite-headings",
    disposition: "composite-acceptance",
    recordIds: [
      "requirement-6748d221ef0d9a4c", "requirement-9335bc98b18f8d8d", "requirement-706e744a85684655",
      "requirement-b38818cf815e0f1e",
    ],
    products: [
      "crates/opentake-project/src/bundle.rs#Project",
      "crates/opentake-core/src/session.rs#EditorSession",
      "crates/opentake-domain/src/media.rs#MediaManifest",
      "crates/opentake-project/src/gen_log.rs#GenerationLogEntry",
    ],
    tests: [
      { path: "crates/opentake-project/tests/upstream_compat.rs", name: "exhaustive_legacy_default_matrix", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-core/tests/project_open.rs", name: "missing_generation_log_seeds_manifest_provenance_once", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-core/tests/project_open.rs", name: "project_open_composite_acceptance", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report: four project-open umbrellas are composite acceptance over the legacy-default and generation-seed child slices, not independent top-one features.",
  },
  {
    ruleId: "DS-cross-cutting-security-headings",
    disposition: "composite-acceptance",
    recordIds: [
      "requirement-37acd430c8e1e82b", "requirement-43bc7b61f5b484f7", "requirement-47a9f1e3f7c11b4c",
      "requirement-debab57411dd52fb",
    ],
    products: [
      "crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher",
      "crates/opentake-project/src/bundle.rs#Project",
      "crates/opentake-core/src/core.rs#AppCore",
      "crates/opentake-ops/src/command.rs#EditCommand",
    ],
    tests: [
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "cross_cutting_mcp_acceptance", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-project/tests/schema_compat.rs", name: "cross_cutting_project_safety_acceptance", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-core/src/core.rs", name: "cross_cutting_runtime_acceptance", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-ops/tests/command_apply.rs", name: "cross_cutting_command_acceptance", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report: these umbrellas cross MCP, safe-fs, playback, export and Agent boundaries and therefore require composite child-slice acceptance.",
  },
  {
    ruleId: "DS-manifest-corruption-conflict",
    disposition: "composite-acceptance",
    recordIds: ["requirement-7908bd026b5a91f6"],
    products: ["crates/opentake-project/src/bundle.rs#Project", "crates/opentake-domain/src/media.rs#MediaManifest"],
    tests: [
      { path: "crates/opentake-project/tests/roundtrip.rs", name: "malformed_manifest_is_an_error", evidenceClass: "existing-owned" },
      { path: "crates/opentake-project/tests/schema_compat.rs", name: "malformed_manifest_contract_matches_authoritative_source", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report: the source contradicts the fail-closed malformed-manifest contract; acceptance text must be reconciled before product work.",
  },
  {
    ruleId: "DS-cache-identity-complete",
    disposition: "evidence-closure",
    recordIds: ["requirement-52da354b46176a99"],
    products: ["crates/opentake-media/src/cache_key.rs#file_identity_key", "crates/opentake-media/src/cache_key.rs#identity_hex"],
    tests: [
      { path: "crates/opentake-media/src/cache_key.rs", name: "identity_hex_is_stable_and_lowercase", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/cache_key.rs", name: "identity_hex_matches_swift_for_whole_second_mtime", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/cache_key.rs", name: "file_identity_key_missing_file_is_none", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report evidence closure: cache identity has direct tracked implementation and focused stability/missing-file tests, pending exact ledger closure.",
  },
  {
    ruleId: "DS-shared-core-command-complete",
    disposition: "evidence-closure",
    recordIds: [
      "requirement-44e32f4f5265f1dc", "requirement-adb4fb8f4521a41f", "requirement-978fe275600796df",
      "requirement-a7fcfeeabb9eca2b", "requirement-2d95c75bfdf43afd", "requirement-90e95608564f8bc4",
      "requirement-479246e3a4dd5891", "requirement-1598d28c23e8cd9b", "requirement-26e566086bff8227",
      "requirement-a2923e1fa84e4a81",
    ],
    products: [
      "crates/opentake-ops/src/editor_state.rs#EditorState",
      "crates/opentake-ops/src/command.rs#EditCommand",
      "crates/opentake-core/src/core.rs#AppCore",
      "crates/opentake-core/src/events.rs#EventBus",
    ],
    tests: [
      { path: "crates/opentake-ops/src/editor_state.rs", name: "commit_undo_redo_cycle_restores_and_versions", evidenceClass: "existing-owned" },
      { path: "crates/opentake-core/src/core.rs", name: "apply_bumps_version_and_emits_once", evidenceClass: "existing-owned" },
      { path: "crates/opentake-core/src/core.rs", name: "unchanged_command_does_not_emit_or_bump", evidenceClass: "existing-owned" },
      { path: "crates/opentake-core/src/core.rs", name: "undo_redo_through_core_bumps_version_and_emits", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report evidence closure: the shared EditorState/EditCommand/AppCore/EventBus path has direct commit, version, no-op and undo/redo evidence.",
  },
  {
    ruleId: "DS-media-resolver-complete",
    disposition: "evidence-closure",
    recordIds: ["requirement-c6caf6aefea9247a"],
    products: ["crates/opentake-domain/src/media.rs#MediaResolver", "src-tauri/src/media.rs#relink_media"],
    tests: [
      { path: "crates/opentake-domain/src/media.rs", name: "resolver_expected_path_external", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/media.rs", name: "dto_reports_file_size_for_present_source", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/media.rs", name: "relink_keeps_same_id_and_clears_missing", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report evidence closure: MediaResolver and relink paths directly prove present/offline recovery semantics, pending exact ledger closure.",
  },
  {
    ruleId: "CC-seconds-truncation",
    disposition: "implementation-gap",
    recordIds: ["requirement-0a3a8376bab5c2b6"],
    products: [
      "web/src/store/editActions.ts#mediaDurationFrames",
      "web/src/store/editActions.ts#momentDurationFrames",
      "web/src/lib/timelineInsert.ts#buildInsertPlan",
    ],
    tests: [
      { path: "web/src/store/editActions.test.ts", name: "seconds_to_frame_truncates_fractional_boundaries", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-seconds-truncation: tracked second-to-frame helpers round fractional boundaries instead of truncating them.",
  },
  {
    ruleId: "CC-first-video-settings",
    disposition: "implementation-gap",
    recordIds: ["requirement-76b4b09bfdb405be"],
    products: [
      "web/src/store/editActions.ts#addMediaToTimelineAt",
      "crates/opentake-ops/src/command.rs#EditCommand",
      "crates/opentake-ops/src/ops/settings.rs#set_timeline_settings",
    ],
    tests: [
      { path: "crates/opentake-ops/src/command.rs", name: "set_timeline_settings_is_undoable", evidenceClass: "existing-owned" },
      { path: "web/src/lib/projectSettings.test.ts", name: "first_video_auto_configures_and_only_configured_empty_mismatch_prompts", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-first-video-settings: the shared settings command exists, but first-import configuration and mismatch prompting are not connected.",
  },
  {
    ruleId: "CC-beat-auto-cut",
    capabilityId: "beat-auto-cut",
    disposition: "implementation-gap",
    recordIds: [
      "requirement-6697c7b7e306b700", "requirement-b96e0201a585b70a", "requirement-408dfad717402883",
      "requirement-1eaff83716e1d766", "requirement-122ba722ed1c9d81",
    ],
    products: [
      "crates/opentake-media/src/analysis/beat.rs#detect_beats",
      "crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher",
      "web/src/store/editActions.ts#applyAutomationCommands",
    ],
    tests: [
      { path: "crates/opentake-media/src/analysis/beat.rs", name: "pulse_audio_detects_beat_frame_with_strength", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "auto_cut_to_beats_write_false_is_read_only", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "auto_cut_to_beats_write_true_is_one_atomic_command_and_preserves_links", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-media/src/analysis/beat.rs", name: "low_energy_speech_is_not_overdetected", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-beat-auto-cut: detection exists, but auto-cut lacks a single atomic write path and the complete audio evidence matrix.",
  },
  {
    ruleId: "CC-mcp-transport",
    capabilityId: "mcp-transport",
    disposition: "implementation-gap",
    recordIds: [
      "requirement-1990a4b0e0df5397", "requirement-a4194cf440c740ca", "requirement-12ca71ff2bf25b39",
    ],
    products: ["crates/opentake-agent/src/mcp/server.rs#McpServer", "crates/opentake-agent/src/mcp/server.rs#serve_with_bridge"],
    tests: [
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "non_local_origin_is_rejected", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "oversized_request_body_is_rejected", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "serve_rejects_non_loopback_bind", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-agent/tests/mcp_http.rs", name: "unsupported_protocol_version_is_400", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-mcp-transport: these records share the data-safety MCP transport capability rather than separate command implementations.",
  },
  {
    ruleId: "CC-mcp-validation-import",
    capabilityId: "mcp-tool-import",
    disposition: "implementation-gap",
    recordIds: ["requirement-729157ed3496b88e", "requirement-4440efda84f45010"],
    products: [
      "crates/opentake-agent/src/tools/errors.rs#decode_tool_args",
      "crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher",
      "src-tauri/src/mcp.rs#TauriMediaBridge",
    ],
    tests: [
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_requires_exactly_one_source", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_rejects_unknown_nested_source_key", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/src/mcp/dispatch.rs", name: "import_media_bytes_rejects_oversized_base64_before_bridge", evidenceClass: "existing-owned" },
      { path: "crates/opentake-agent/tests/tool_argument_contract.rs", name: "all_tool_schemas_reject_unknown_missing_wrong_type", evidenceClass: "reviewed-planned" },
      { path: "src-tauri/src/mcp.rs", name: "https_url_import_enforces_scheme_mime_and_decoded_limit", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-mcp-validation-import: command validation and safe URL import are the shared data-safety MCP import capability.",
  },
  {
    ruleId: "CC-mcp-error-redaction",
    capabilityId: "mcp-error-redaction",
    disposition: "implementation-gap",
    recordIds: ["requirement-72876d064ac1a1c3"],
    products: ["src-tauri/src/mcp.rs#TauriMediaBridge", "crates/opentake-agent/src/mcp/convert.rs#to_call_tool_result"],
    tests: [
      { path: "crates/opentake-agent/tests/mcp_error_redaction.rs", name: "llm_errors_redact_paths_credentials_headers_provider_bodies", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-mcp-error-redaction: this record is the command-contract view of the shared LLM error redaction boundary.",
  },
  {
    ruleId: "CC-readonly-versioned-mirror",
    disposition: "implementation-gap",
    recordIds: ["requirement-cc6851a640c8585d", "requirement-521a6bc7a7b845c6", "requirement-dbce32105bffaabf"],
    products: [
      "src-tauri/src/lib.rs#forward_event",
      "web/src/store/sync.ts#startSync",
      "web/src/store/projectStore.ts#useProjectStore",
    ],
    tests: [
      { path: "web/src/store/sync.test.ts", name: "does not let a late old snapshot replace a newer project", evidenceClass: "existing-owned" },
      { path: "web/src/store/commandRouting.test.ts", name: "project_store_has_no_timeline_mutator_and_refreshes_only_from_native_events", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-readonly-versioned-mirror: the versioned refresh path exists, but whole-store read-only projection ownership lacks a closed proof.",
  },
  {
    ruleId: "CC-edit-gesture-parity",
    disposition: "implementation-gap",
    recordIds: ["requirement-8134b0c5922567c8", "requirement-538b644e2cb07926"],
    products: [
      "web/src/components/timeline/TimelineContainer.tsx#TimelineContainer",
      "web/src/store/editActions.ts#buildMediaInsertPlan",
      "web/src/lib/api.ts#editApply",
      "src-tauri/src/commands.rs#EditRequest",
    ],
    tests: [
      { path: "web/src/store/editActions.test.ts", name: "forwards swapTracks for whole-track reordering", evidenceClass: "existing-owned" },
      { path: "web/src/store/commandRouting.test.ts", name: "every_edit_action_emits_exact_edit_request", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-edit-gesture-parity: focused gesture tests exist, but no closed inventory proves every UI path emits the exact shared request.",
  },
  {
    ruleId: "CC-tauri-command-contract",
    disposition: "implementation-gap",
    recordIds: [
      "requirement-687bc44b4c9243a2", "requirement-5ca0aab886c28dde", "requirement-2c03433376733eb5",
      "requirement-5558df9d0b133739", "requirement-c924246b4d2b7ff9", "requirement-cd691581ab4342b3",
      "requirement-29da2e6af9281076",
    ],
    products: ["web/src/lib/api.ts#editApply", "src-tauri/src/commands.rs#EditRequest", "src-tauri/src/commands.rs#edit_apply"],
    tests: [
      { path: "src-tauri/src/commands.rs", name: "deserializes_camelcase_multiword_commands", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/commands.rs", name: "deserializes_add_captions_camelcase_and_maps_to_command", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/commands.rs", name: "deserializes_effect_commands_and_maps_to_ops_variants", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/commands.rs", name: "deserializes_media_library_commands_and_maps_to_ops_variants", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/commands.rs", name: "every_edit_request_maps_to_exact_edit_command", evidenceClass: "reviewed-planned" },
      { path: "web/src/lib/api.commandContract.test.ts", name: "frontend_command_names_match_invoke_handler", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-tauri-command-contract: individual DTO cases exist, but exhaustive Rust mapping and frontend invoke-handler parity gates remain planned.",
  },
  {
    ruleId: "CC-automation-composite-headings",
    disposition: "composite-acceptance",
    recordIds: [
      "requirement-5c2a4b2f9e5abba6", "requirement-a3187560b02b641a", "requirement-2cf919a757c83ded",
      "requirement-fca4710352ee2ce7", "requirement-45ae268014531007", "requirement-ed0b686740d56057",
      "requirement-c8405288bd463439",
    ],
    products: [
      "crates/opentake-agent/src/mcp/dispatch.rs#Dispatcher",
      "crates/opentake-media/src/analysis/autocrop.rs#detect_autocrop",
      "crates/opentake-ops/src/intent.rs#plan_smart_reframe",
      "crates/opentake-media/src/analysis/beat.rs#detect_beats",
    ],
    tests: [
      { path: "crates/opentake-agent/tests/editing_automation_acceptance.rs", name: "automation_children_are_atomic_reviewable_and_command_routed", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-automation-composite-headings: seven umbrellas aggregate beat, reframe, tighten and shared failure semantics across child capabilities.",
  },
  {
    ruleId: "CC-authority-persistence-mixed",
    disposition: "composite-acceptance",
    recordIds: ["requirement-224726c57b0ebb49", "requirement-d9d443c2efd6fafd", "requirement-12a60880e3a58578"],
    products: ["web/src/store/sync.ts#startSync", "web/src/store/projectStore.ts#useProjectStore", "web/src/store/uiStore.ts#useEditorUiStore"],
    tests: [
      { path: "web/src/store/commandRouting.test.ts", name: "rust_authority_and_ui_persistence_are_independently_owned", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-authority-persistence-mixed: these mixed records require composite acceptance across Rust authority projection and UI-only persistence.",
  },
  {
    ruleId: "CC-event-forwarding-complete",
    capabilityId: "event-forwarding",
    disposition: "evidence-closure",
    recordIds: ["requirement-8c2d9157594b8d92"],
    products: ["crates/opentake-core/src/events.rs#CoreEvent", "src-tauri/src/lib.rs#forward_event"],
    tests: [
      { path: "crates/opentake-core/src/events.rs", name: "core_event_serializes_with_kind_tag", evidenceClass: "existing-owned" },
      { path: "crates/opentake-core/src/events.rs", name: "media_changed_serializes_with_kind_tag", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report CC-event-forwarding-complete: tagged events and intentional nonfatal WebView forwarding are implemented and need ledger evidence closure.",
  },
  {
    ruleId: "HS-project-card-lifecycle",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "src-tauri/src/home.rs", symbol: "ProjectRegistry", planned: true },
      { path: "web/src/store/recentStore.ts", symbol: "useRecentStore", planned: false },
      { path: "web/src/components/home/HomeView.tsx", symbol: "ProjectGridCard", planned: false },
    ],
    tests: [
      { path: "src-tauri/src/home.rs", name: "missing_entry_survives_registry_load_and_safe_trash_removes_only_after_success", evidenceClass: "reviewed-planned" },
      { path: "web/src/components/home/HomeView.test.tsx", name: "missing_card_reveal_remove_and_trash_states", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-project-card-lifecycle: persistent missing, reveal and safe-trash states require one shared registry-backed card lifecycle.",
  },
  {
    ruleId: "HS-new-open-sample",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/components/home/HomeView.tsx", symbol: "HomeView", planned: false },
      { path: "web/src/store/projectActions.ts", symbol: "newProjectAndEnter", planned: false },
      { path: "web/src/store/projectActions.ts", symbol: "openProjectPath", planned: false },
      { path: "web/src/store/projectActions.ts", symbol: "openProjectViaDialog", planned: false },
      { path: "src-tauri/src/samples.rs", symbol: "SampleProjectService", planned: true },
    ],
    tests: [
      { path: "src-tauri/src/samples.rs", name: "failed_materialization_rolls_back_entire_sample_directory", evidenceClass: "reviewed-planned" },
      { path: "web/src/components/home/HomeView.test.tsx", name: "new_open_sample_register_only_after_success_and_route_tutorial", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-new-open-sample: new, open and sample flows must register only after success, roll back atomically and route tutorial state.",
  },
  {
    ruleId: "HS-layout-geometry",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/store/uiStore.ts", symbol: "useEditorUiStore", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "EditorSplit", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "DefaultLayout", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "MediaLayout", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "VerticalLayout", planned: false },
      { path: "web/src/components/ui/PanelShell.tsx", symbol: "PanelShell", planned: false },
    ],
    tests: [
      { path: "web/src/components/shell/EditorSplit.test.tsx", name: "all_presets_match_geometry_visibility_maximize_and_focus_shell", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-layout-geometry: the native home-shell layout capability owns preset geometry, visibility, maximize and focus behavior.",
  },
  {
    ruleId: "CC-layout-misgrouped",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/store/uiStore.ts", symbol: "useEditorUiStore", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "EditorSplit", planned: false },
      { path: "web/src/components/ui/PanelShell.tsx", symbol: "PanelShell", planned: false },
      { path: "web/src/components/shell/ViewMenu.tsx", symbol: "ViewMenu", planned: false },
    ],
    tests: [
      { path: "web/src/components/shell/EditorSplit.test.tsx", name: "all_presets_match_geometry_visibility_maximize_and_focus_shell", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report CC-layout-misgrouped: nine command-classified records are acceptance inputs to the single home-shell layout capability.",
  },
  {
    ruleId: "HS-schema-safe-persistence",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/store/uiStore.ts", symbol: "loadBool", planned: false },
      { path: "web/src/store/uiStore.ts", symbol: "loadPreset", planned: false },
      { path: "web/src/store/uiStore.ts", symbol: "persist", planned: false },
      { path: "web/src/store/uiStore.ts", symbol: "useEditorUiStore", planned: false },
    ],
    tests: [
      { path: "web/src/store/uiStore.persistence.test.ts", name: "schema_safe_layout_panel_and_keyframe_state_survive_restart", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-schema-safe-persistence: layout, panel and keyframe state need one restart-safe persistence contract.",
  },
  {
    ruleId: "HS-menu-contract",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/components/shell/ViewMenu.tsx", symbol: "ViewMenu", planned: false },
      { path: "web/src/store/uiStore.ts", symbol: "useEditorUiStore", planned: false },
    ],
    tests: [
      { path: "web/src/components/shell/ViewMenu.test.tsx", name: "commands_shortcuts_checked_state_and_disabled_rules", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-menu-contract: the complete command, shortcut, checked-state and disabled-rule matrix belongs to ViewMenu.",
  },
  {
    ruleId: "HS-upstream-home-composite",
    disposition: "composite-acceptance",
    recordIds: [],
    products: [
      { path: "web/src/components/home/HomeView.tsx", symbol: "HomeView", planned: false },
      { path: "web/src/store/recentStore.ts", symbol: "useRecentStore", planned: false },
      { path: "web/src/store/projectActions.ts", symbol: "openProjectPath", planned: false },
    ],
    tests: [
      { path: "web/src/components/home/HomeView.test.tsx", name: "upstream_home_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-upstream-home-composite: this umbrella aggregates registry, card, sample, welcome and update child contracts.",
  },
  {
    ruleId: "HS-autosave-metadata-mixed",
    disposition: "composite-acceptance",
    recordIds: [],
    products: [
      { path: "web/src/store/recentStore.ts", symbol: "useRecentStore", planned: false },
      { path: "web/src/store/projectActions.ts", symbol: "saveCurrentProject", planned: false },
    ],
    tests: [
      { path: "web/src/store/recentStore.test.ts", name: "autosave_and_home_metadata_have_separate_owners", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-autosave-metadata-mixed: autosave evidence and Home registry metadata must be accepted as separately owned child contracts.",
  },
  {
    ruleId: "HS-component-mapping-composite",
    disposition: "composite-acceptance",
    recordIds: [],
    products: [
      { path: "web/src/components/home/HomeView.tsx", symbol: "HomeView", planned: false },
      { path: "web/src/components/shell/EditorSplit.tsx", symbol: "EditorSplit", planned: false },
      { path: "web/src/components/shell/ViewMenu.tsx", symbol: "ViewMenu", planned: false },
    ],
    tests: [
      { path: "web/src/components/shell/ShellComponentMapping.test.tsx", name: "every_documented_shell_component_has_exact_owner", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report HS-component-mapping-composite: every-component mapping is a shell acceptance index, not an independent feature.",
  },
  {
    ruleId: "ML-card-state-machine",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "crates/opentake-domain/src/media.rs", symbol: "MediaResolver", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "MediaItemDto::from_entry", planned: false },
      { path: "web/src/components/media/MediaPanel.tsx", symbol: "MediaCard", planned: false },
      { path: "web/src/store/mediaActions.ts", symbol: null, planned: false },
    ],
    tests: [
      { path: "src-tauri/src/media.rs", name: "dto_distinguishes_offline_generating_downloading_failed", evidenceClass: "reviewed-planned" },
      { path: "web/src/components/media/MediaPanel.stateMatrix.test.tsx", name: "state_precedence_actions_and_scoped_relink", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-card-state-machine: the DTO and card need one explicit offline, generating, downloading and failed state machine.",
  },
  {
    ruleId: "ML-import-feedback",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "src-tauri/src/media.rs", symbol: "import_media", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "import_folder", planned: false },
      { path: "web/src/store/mediaActions.ts", symbol: null, planned: false },
    ],
    tests: [
      { path: "src-tauri/src/media.rs", name: "import_media_imports_supported_and_skips_others", evidenceClass: "existing-owned" },
      { path: "web/src/store/mediaActions.test.ts", name: "unsupported_skips_and_folder_failure_remain_visible", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-import-feedback: supported imports work, but unsupported skips and recursive-folder failures need visible feedback.",
  },
  {
    ruleId: "ML-thumbnail-pipeline",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "crates/opentake-media/src/thumbnail/mod.rs", symbol: "video_thumbnail_times", planned: false },
      { path: "crates/opentake-media/src/thumbnail/mod.rs", symbol: "video_thumbnails", planned: false },
      { path: "crates/opentake-media/src/thumbnail/mod.rs", symbol: "image_thumbnail", planned: false },
      { path: "crates/opentake-media/src/thumbnail/sprite.rs", symbol: "load_sprite", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "generate_thumbnail", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/src/thumbnail/sprite.rs", name: "sprite_roundtrip_preserves_times_and_pixels", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/thumbnail/sprite.rs", name: "load_corrupt_sidecar_is_none", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/thumbnail/sprite.rs", name: "load_invalid_zero_or_undersized_geometry_is_none", evidenceClass: "reviewed-planned" },
      { path: "crates/opentake-media/tests/thumbnail_pipeline.rs", name: "video_image_sprite_cache_and_bounded_concurrency_roundtrip", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-thumbnail-pipeline: invalid sprite geometry and the service/cache/concurrency path remain unproved.",
  },
  {
    ruleId: "ML-waveform-pipeline",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "crates/opentake-media/src/waveform/mod.rs", symbol: "waveform_cached_cancellable", planned: false },
      { path: "crates/opentake-media/src/waveform/store.rs", symbol: null, planned: false },
      { path: "src-tauri/src/media.rs", symbol: "get_waveform", planned: false },
      { path: "web/src/components/media/MediaPanel.tsx", symbol: "AudioWaveform", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/src/waveform/store.rs", name: "save_then_load_roundtrips", evidenceClass: "existing-owned" },
      { path: "web/src/components/media/MediaPanel.test.tsx", name: "renders waveform bars when normalized buckets are available", evidenceClass: "existing-owned" },
      {
        path: "crates/opentake-media/tests/waveform_pipeline.rs",
        name: "waveform_sample_count_rms_orientation_and_cache_roundtrip",
        evidenceClass: "reviewed-planned",
        ownershipReason: "The pipeline test must distinguish a valid all-silent cache from poisoned legacy output using format/source provenance; an all == 1.0 value check is not sufficient.",
      },
    ],
    rationale: "Core mapping report ML-waveform-pipeline: a deterministic PCM fixture must jointly prove sample count, RMS normalization, orientation and cache behavior, including format/source discrimination between valid all-silent cache data and poisoned legacy output; all == 1.0 alone cannot classify the cache.",
  },
  {
    ruleId: "ML-panel-workflows",
    disposition: "implementation-gap",
    recordIds: [],
    products: [
      { path: "web/src/components/media/MediaTabBar.tsx", symbol: "MediaTabBar", planned: false },
      { path: "web/src/components/media/MediaPanel.tsx", symbol: "MediaPanel", planned: false },
      { path: "web/src/components/media/LibraryView.tsx", symbol: "LibraryView", planned: false },
      { path: "web/src/components/media/SoundLibraryTab.tsx", symbol: "SoundLibraryTab", planned: false },
      { path: "web/src/store/libraryStore.ts", symbol: null, planned: false },
      { path: "src-tauri/src/library.rs", symbol: null, planned: false },
    ],
    tests: [
      { path: "web/src/components/media/LibraryView.test.tsx", name: "renders global-library entries in the reusable Mine grid", evidenceClass: "existing-owned" },
      { path: "web/src/components/media/MediaLibrary.workflow.test.tsx", name: "tab_folder_breadcrumb_preview_drag_favorite_rename_delete_and_music_flow", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-panel-workflows: the rail, folders, preview, drag, favorite, rename, delete and music paths need one workflow acceptance slice.",
  },
  {
    ruleId: "ML-library-workflow-composite",
    disposition: "composite-acceptance",
    recordIds: [],
    products: [
      { path: "web/src/components/media/MediaPanel.tsx", symbol: "MediaPanel", planned: false },
      { path: "web/src/components/media/LibraryView.tsx", symbol: "LibraryView", planned: false },
      { path: "web/src/store/libraryStore.ts", symbol: null, planned: false },
    ],
    tests: [
      { path: "web/src/components/media/MediaLibrary.workflow.test.tsx", name: "library_workflow_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-library-workflow-composite: this umbrella aggregates import, preview, drag, favorite, rename, delete and persistence child slices.",
  },
  {
    ruleId: "ML-basic-media-composite",
    disposition: "composite-acceptance",
    recordIds: [],
    products: [
      { path: "src-tauri/src/media.rs", symbol: "generate_thumbnail", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "get_waveform", planned: false },
      { path: "crates/opentake-media/src/lib.rs", symbol: "MediaEngine", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/tests/media_pipeline.rs", name: "ffmpeg_thumbnail_and_waveform_children_close_one_composite_acceptance", evidenceClass: "reviewed-planned" },
    ],
    rationale: "Core mapping report ML-basic-media-composite: FFmpeg, thumbnail and waveform contracts are child capabilities, not one implementation owner.",
  },
  {
    ruleId: "ML-manifest-compat-complete",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "crates/opentake-domain/src/media.rs", symbol: "MediaManifest::deserialize", planned: false },
    ],
    tests: [
      { path: "crates/opentake-domain/src/media.rs", name: "manifest_missing_version_falls_back_to_one", evidenceClass: "existing-owned" },
      { path: "crates/opentake-domain/src/media.rs", name: "manifest_empty_object_decodes", evidenceClass: "existing-owned" },
      { path: "crates/opentake-project/tests/roundtrip.rs", name: "save_then_open_is_lossless", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report ML-manifest-compat-complete: legacy version and array defaults plus lossless reopen have direct evidence.",
  },
  {
    ruleId: "ML-cache-identity-duplicate",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "crates/opentake-media/src/cache_key.rs", symbol: "file_identity_key", planned: false },
      { path: "crates/opentake-media/src/cache_key.rs", symbol: "identity_hex", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/src/cache_key.rs", name: "identity_hex_is_stable_and_lowercase", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/cache_key.rs", name: "identity_hex_matches_swift_for_whole_second_mtime", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/cache_key.rs", name: "file_identity_key_missing_file_is_none", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report ML-cache-identity-duplicate: this record references the single data-safety cache identity capability.",
  },
  {
    ruleId: "ML-best-effort-probe-complete",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "src-tauri/src/media.rs", symbol: "import_one", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "probe_media", planned: false },
    ],
    tests: [
      { path: "src-tauri/src/media.rs", name: "import_media_imports_supported_and_skips_others", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report ML-best-effort-probe-complete: supported files remain importable with default metadata after probe failure.",
  },
  {
    ruleId: "ML-relink-in-place-complete",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "crates/opentake-core/src/session.rs", symbol: "EditorSession::relink_media_file", planned: false },
      { path: "src-tauri/src/media.rs", symbol: "relink_media", planned: false },
    ],
    tests: [
      { path: "src-tauri/src/media.rs", name: "relink_keeps_same_id_and_clears_missing", evidenceClass: "existing-owned" },
      { path: "src-tauri/src/media.rs", name: "relink_rejects_type_mismatch", evidenceClass: "existing-owned" },
    ],
    rationale: "Core mapping report ML-relink-in-place-complete: relink preserves id, validates type before mutation and clears missing state.",
  },
  {
    ruleId: "misgrouped-media-search",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "crates/opentake-media/src/search/embedder.rs", symbol: "Embedder", planned: false },
      { path: "crates/opentake-media/src/search/tokenizer.rs", symbol: "SiglipTokenizer", planned: false },
      { path: "crates/opentake-media/src/search/indexer.rs", symbol: "index_video", planned: false },
      { path: "crates/opentake-media/src/search/embed_store.rs", symbol: "AssetIndex", planned: false },
      { path: "crates/opentake-media/src/search/ranker.rs", symbol: "search", planned: false },
      { path: "src-tauri/src/search.rs", symbol: "search_query", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/src/search/embedder.rs", name: "preprocess_squashes_non_square_to_square", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/search/tokenizer.rs", name: "pads_short_sequence_to_context_length", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/search/embed_store.rs", name: "encode_decode_roundtrip_f16_quantized", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/search/ranker.rs", name: "best_per_shot_dedupes_same_shot", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/search/mod.rs", name: "index_then_rank_finds_brightest_match", evidenceClass: "existing-owned" },
    ],
    rationale: "UI mapping report misgrouped-media-search: eleven Inspector-classified headings are evidence references to the single implemented media-search capability.",
  },
  {
    ruleId: "misgrouped-transcription",
    disposition: "evidence-closure",
    recordIds: [],
    products: [
      { path: "crates/opentake-media/src/transcribe/mod.rs", symbol: "Transcriber", planned: false },
      { path: "crates/opentake-media/src/transcribe/mod.rs", symbol: "transcribe_file", planned: false },
      { path: "crates/opentake-media/src/transcribe/cache.rs", symbol: "TranscriptCache", planned: false },
      { path: "crates/opentake-media/src/transcribe/search.rs", symbol: "search", planned: false },
      { path: "crates/opentake-media/src/transcribe/locale.rs", symbol: "match_locale", planned: false },
      { path: "crates/opentake-media/src/transcribe/whisper.rs", symbol: "WhisperTranscriber", planned: false },
      { path: "src-tauri/src/transcribe.rs", symbol: "transcribe_media", planned: false },
    ],
    tests: [
      { path: "crates/opentake-media/src/transcribe/cache.rs", name: "memory_lru_clears_wholesale_at_capacity", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/transcribe/search.rs", name: "search_collects_and_respects_limit", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/transcribe/locale.rs", name: "picks_same_language_same_region", evidenceClass: "existing-owned" },
      { path: "crates/opentake-media/src/transcribe/whisper.rs", name: "centiseconds_convert_to_seconds", evidenceClass: "existing-owned", features: ["whisper-backend"] },
    ],
    rationale: "UI mapping report misgrouped-transcription: six accessibility-classified headings are evidence references to the single implemented media-transcription capability.",
  },
  {
    ruleId: "UI-preview-50-track-performance",
    disposition: "implementation-gap",
    recordIds: ["requirement-9f4b49115b7e6a5b"],
    products: [
      "web/src/components/timeline/TimelineContainer.tsx#TimelineContainer",
      "web/src/components/timeline/timelineCanvas.ts#paintTimeline",
    ],
    tests: ["web/src/components/timeline/TimelineContainer.test.ts"],
    rationale: "UI mapping report slice 1: the existing timeline owner needs a focused 50-track edit/seek/play/save budget test.",
  },
  {
    ruleId: "UI-preview-compound-clips",
    disposition: "implementation-gap",
    recordIds: ["requirement-b49e0f5ed8c2415c"],
    products: [
      "crates/opentake-domain/src/clip.rs#Clip",
      "crates/opentake-render/src/plan/build.rs#build_render_plan",
    ],
    tests: ["crates/opentake-project/tests/upstream_compat.rs", "crates/opentake-render/tests/pixel_diff.rs"],
    rationale: "UI mapping report slice 2: compound persistence and render ownership are absent and require round-trip plus render parity coverage.",
  },
  {
    ruleId: "UI-preview-multicam",
    disposition: "implementation-gap",
    recordIds: ["requirement-d179f69761518eaa"],
    products: [
      "crates/opentake-domain/src/timeline.rs#Timeline",
      "crates/opentake-domain/src/clip.rs#Clip",
    ],
    tests: ["crates/opentake-domain/src/timeline.rs"],
    rationale: "UI mapping report slice 3: no multicam source/sync/switch model exists; the domain runner must establish non-destructive angle switching.",
  },
  {
    ruleId: "UI-preview-state-projection-boundary",
    disposition: "evidence-closure",
    recordIds: ["requirement-0858e3409610b51b", "requirement-63e2699bd9d27494"],
    products: ["web/src/store/uiStore.ts#useEditorUiStore", "web/src/store/sync.ts#startSync"],
    tests: ["web/src/store/uiStore.test.ts", "web/src/store/sync.test.ts"],
    rationale: "UI mapping report slice 4: implementation exists, but derived selectors still need an acceptance test proving they never mutate the Rust timeline projection.",
  },
  {
    ruleId: "UI-preview-geometry-snap-thresholds",
    disposition: "evidence-closure",
    recordIds: [
      "requirement-c8564305ae9fa4bb", "requirement-e8f0a3053bf5ae40", "requirement-daa6ec4af3aeb4a9",
      "requirement-e471db5faf7f4480", "requirement-38807e6a1df065a0",
    ],
    products: ["web/src/lib/snap.ts#findSnap", "web/src/lib/geometry.ts#trackY"],
    tests: ["web/src/lib/snap.test.ts", "web/src/lib/geometry.test.ts", "web/src/components/timeline/TimelineContainer.test.ts"],
    rationale: "UI mapping report slice 5: existing geometry and snap owners need the complete sticky multi-target and dual 1.5x threshold matrix.",
  },
  {
    ruleId: "UI-preview-toolbar-timeline-visual-surface",
    disposition: "composite-acceptance",
    recordIds: [
      "requirement-d82235254f1ad987", "requirement-c83bfe0b365d0d7f", "requirement-a8a43bc740569dc6",
      "requirement-22535d21627595e1", "requirement-a68911b741563de7", "requirement-923f74b3f2c06d84",
      "requirement-32c4ef714d183571", "requirement-1ec5bd08013418ac", "requirement-6cdaf1b0bc759131",
      "requirement-7f8b415cc15cc3a6", "requirement-6f3d4e423997080c", "requirement-01ab68dad9bef233",
    ],
    products: [
      "web/src/components/toolbar/Toolbar.tsx#Toolbar",
      "web/src/components/timeline/timelineCanvas.ts#paintTimeline",
      "web/src/components/timeline/rulerCanvas.ts#paintRuler",
      "web/src/components/timeline/Playhead.tsx#Playhead",
      "web/src/components/timeline/TrackHeaderColumn.tsx#TrackHeaderColumn",
    ],
    tests: [
      "web/src/components/timeline/TimelineContainer.test.ts",
      "web/src/components/timeline/clipRenderer.test.ts",
      "web/src/components/timeline/timelineOverlays.test.ts",
    ],
    rationale: "UI mapping report slice 6: twelve overlapping surface records require one composite toolbar/ruler/track/clip/playhead visual acceptance matrix.",
  },
  {
    ruleId: "UI-preview-timeline-interaction-matrix",
    disposition: "composite-acceptance",
    recordIds: [
      "requirement-d93395512a23997b", "requirement-a35e49bb4f4b3488", "requirement-f53dbd6c1c65375f",
      "requirement-6db3895c5fd3fd75", "requirement-92ec4c8b557a6ee3", "requirement-1779ecc2d9f24651",
      "requirement-64a0fdf590212573",
    ],
    products: [
      "web/src/components/timeline/TimelineContainer.tsx#TimelineContainer",
      "web/src/components/timeline/ClipContextMenu.tsx#clipContextMenuItems",
      "web/src/components/timeline/TimelineRangeContextMenu.tsx#rangeContextMenuItems",
      "web/src/store/editActions.ts#buildMediaInsertPlan",
    ],
    tests: [
      "web/src/components/timeline/TimelineContainer.test.ts",
      "web/src/components/timeline/ClipContextMenu.test.tsx",
      "web/src/components/timeline/TimelineRangeContextMenu.test.tsx",
      "web/src/components/timeline/hitTest.test.ts",
    ],
    rationale: "UI mapping report slice 7: selection, drag/drop, trim, scrub and context menu records share one exhaustive interaction acceptance matrix.",
  },
  {
    ruleId: "UI-preview-shell-playback-overlay",
    disposition: "evidence-closure",
    recordIds: [
      "requirement-dbd3f79b7e0c2f11", "requirement-64629892b7fae65d", "requirement-6a6a5fd7ad27f724",
      "requirement-a81cc5996343e86d", "requirement-aa1d38d1befcc428", "requirement-087ed63292e24e0b",
      "requirement-685b894531563174",
    ],
    products: [
      "web/src/components/preview/Preview.tsx#Preview",
      "web/src/components/preview/TimelinePlaybackLayer.tsx#TimelinePlayback",
      "web/src/components/preview/TransformOverlay.tsx#TransformOverlay",
      "web/src/components/preview/CropOverlay.tsx#CropOverlay",
    ],
    tests: [
      "web/src/components/preview/Preview.test.tsx",
      "web/src/components/preview/TransformOverlay.test.tsx",
      "web/src/components/preview/timelinePlayback.test.ts",
    ],
    rationale: "UI mapping report slice 8: preview implementation exists but needs a complete tabs/transport/scrub/settings/crop control-surface acceptance test.",
  },
  {
    ruleId: "UI-preview-trackpad-wheel-navigation",
    disposition: "evidence-closure",
    recordIds: ["requirement-a5977b149ca8a9ec"],
    products: ["web/src/components/timeline/TimelineContainer.tsx#TimelineContainer"],
    tests: ["web/src/components/timeline/TimelineContainer.test.ts"],
    rationale: "UI mapping report slice 9: the native wheel owner exists but has no focused trackpad pinch/horizontal/vertical navigation test.",
  },
  ...reviewedOwnershipRules("preview-timeline"),
  ...reviewedOwnershipRules("inspector-text-keyframes"),
  ...reviewedOwnershipRules("agent-settings-generation"),
  ...reviewedOwnershipRules("accessibility-polish"),
];

const REVIEWED_REPORT_SLICE_DEFINITIONS = REVIEWED_REPORT_SLICE_DEFINITIONS_RAW.map((rule) => {
  const exact = REVIEWED_EXACT_SLICE_BY_KEY.get(rule.ruleId);
  if (!exact) return rule;
  const defaultDisposition = exact.classification === "implementation"
    ? "implementation-gap"
    : exact.classification === "evidence-closure"
      ? "evidence-closure"
      : "composite-acceptance";
  return {
    ...rule,
    recordIds: exact.recordIds.map(normalizeReviewedRecordId),
    disposition: exact.planDisposition ?? defaultDisposition,
    classification: exact.classification,
    capabilityId: exact.capabilityId ?? rule.capabilityId ?? rule.ruleId,
    primaryGroup: exact.primaryGroup ?? rule.primaryGroup,
    correctionIds: exact.correctionIds ?? rule.correctionIds ?? [],
  };
});

const REVIEWED_REPORT_RULE_BY_RECORD = new Map(REVIEWED_REPORT_SLICE_DEFINITIONS.flatMap((rule) => (
  rule.recordIds.map((recordId) => [recordId, rule])
)));

const REVIEWED_LEDGER_SLICE_DEFINITIONS = REVIEWED_LEDGER_OWNERSHIP.slices.map((slice) => {
  const cluster = REVIEWED_CAPABILITY_CLUSTER_BY_RULE_KEY.get(slice.sliceKey);
  return {
    ruleId: slice.sliceKey,
    recordIds: slice.recordIds,
    products: cluster?.products ?? slice.products,
    tests: cluster?.tests ?? slice.tests,
    strategy: "validated-ledger-evidence",
    disposition: slice.classification === "implementation"
      ? "implementation-gap"
      : slice.classification === "evidence-closure"
        ? "evidence-closure"
        : "composite-acceptance",
    classification: slice.classification,
    capabilityId: cluster?.capabilityId ?? slice.capabilityId ?? slice.sliceKey,
    primaryGroup: cluster?.primaryGroup ?? slice.primaryGroup ?? null,
    childCapabilities: slice.childCapabilities ?? [],
    correctionIds: slice.correctionIds ?? [],
    rationale: cluster?.rationale ?? slice.rationale,
  };
});
const REVIEWED_LEDGER_RULE_BY_RECORD = new Map(REVIEWED_LEDGER_SLICE_DEFINITIONS.flatMap((rule) => (
  rule.recordIds.map((recordId) => [recordId, rule])
)));

function reviewedOwnershipRule(record) {
  if (!REVIEWED_PLAN_RECORD_IDS.has(record.id)) {
    throw new Error(`record is outside the independently reviewed mapping reports: ${record.id}`);
  }
  const exactReportRule = REVIEWED_REPORT_RULE_BY_RECORD.get(record.id);
  if (!exactReportRule) {
    throw new Error(`reviewed report record has no exact ownership slice: ${record.id}`);
  }
  return { ...exactReportRule, strategy: "reviewed-mapping-report" };
}

function reviewedPlanTests(record, tests, index) {
  return tests.map((test) => {
    if (!test || typeof test !== "object" || !test.path || !test.name) {
      throw new Error(`reviewed test spec must name path, test, and evidence class for ${record.id}`);
    }
    const existing = test.evidenceClass === "existing-owned";
    if (!existing && test.evidenceClass !== "reviewed-planned") {
      throw new Error(`reviewed test spec has invalid evidence class for ${record.id}: ${String(test.evidenceClass)}`);
    }
    const { path, name } = test;
    const fileAction = index.trackedPaths.has(path) ? "Modify" : "Create";
    return {
      path,
      name,
      evidenceClass: test.evidenceClass,
      fileAction,
      ownershipReason: test.ownershipReason ?? (existing
        ? "Exact named test already exists in the reviewed owning runner and records current boundary behavior."
        : "Reviewed planned test belongs in this tracked owning runner beside the mapped product boundary."),
      command: path.endsWith(".rs")
        ? rustTestCommand(path, name, test.features ?? [], test.noDefaultFeatures ?? false)
        : path.endsWith(".mjs")
          ? `node --test --test-name-pattern=${JSON.stringify(name)} ${path}`
          : path.endsWith(".rb")
            ? `ruby ${path} --name ${JSON.stringify(name)}`
            : `pnpm -C web test -- --run ${path.replace(/^web\//, "")} -t ${JSON.stringify(name)}`,
    };
  });
}

function reviewedPlanProducts(record, products, index) {
  return products.map((product) => {
    const spec = typeof product === "string"
      ? { ...splitPlanRef(product), planned: false }
      : product;
    if (!spec || typeof spec !== "object" || !spec.path || typeof spec.planned !== "boolean") {
      throw new Error(`reviewed product spec must name path and planned state for ${record.id}`);
    }
    const path = normalizePath(spec.path);
    const symbol = spec.symbol ?? null;
    const tracked = index.trackedPaths.has(path);
    if (spec.planned) {
      if (!/^(?:web\/src\/.+\.(?:tsx?|css)|src-tauri\/(?:src\/.+\.rs|tauri\.conf\.json)|crates\/[^/]+\/(?:src\/.+\.(?:rs|wgsl)|Cargo\.toml)|scripts\/.+|\.github\/workflows\/.+\.ya?ml|docs\/.+\.md)$/.test(path)) {
        throw new Error(`reviewed planned product target is illegal for ${record.id}: ${path}`);
      }
    } else {
      if (!tracked) throw new Error(`reviewed product target is not tracked for ${record.id}: ${path}`);
      if (symbol) {
        const source = index.sourcesByPath.get(path) ?? index.contentsByPath.get(path);
        if (!source || !sourceDeclaresProductSpec(path, source.content, symbol)) {
          throw new Error(`reviewed product target is not a tracked declaration for ${record.id}: ${path}#${symbol}`);
        }
      }
    }
    return {
      path,
      symbol,
      planned: spec.planned,
      fileAction: tracked ? "Modify" : "Create",
      ref: symbol ? `${path}#${symbol}` : path,
    };
  });
}

function resolveRulePlanOwnership(record, candidate, index, rule, resolutionEvidence = {}) {
  const productTargets = reviewedPlanProducts(record, rule.products, index);
  if (productTargets.length === 0 && (rule.childCapabilities ?? []).length === 0) {
    throw new Error(`reviewed ownership has neither product targets nor child capabilities for ${record.id}`);
  }
  const tests = reviewedPlanTests(record, rule.tests, index);
  for (const test of tests) {
    const owningRunner = index.testsByPath.has(test.path)
      || (test.evidenceClass === "reviewed-planned" && (
        index.sourcesByPath.has(test.path)
        || PLAN_PRODUCT_SOURCE.test(test.path)
        || PLAN_TYPESCRIPT_TEST.test(test.path)
        || PLAN_RUST_INTEGRATION_TEST.test(test.path)
        || /^scripts\/tests\/.+-test\.rb$/.test(test.path)
        || /^tools\/.+\.test\.mjs$/.test(test.path)
      ));
    if (!owningRunner) {
      throw new Error(`reviewed test target is not an owning tracked runner for ${record.id}: ${test.path}`);
    }
    if (
      test.evidenceClass === "existing-owned"
      && !sourceDeclaresTest(test.path, index.testsByPath.get(test.path).content, test.name)
    ) {
      throw new Error(`reviewed existing test name is absent for ${record.id}: ${test.path}#${test.name}`);
    }
  }
  const capabilityId = rule.capabilityId ?? rule.ruleId;
  return {
    files: [...productTargets.map(({ ref }) => ref), record.source.path],
    productTargets,
    tests,
    test: tests[0],
    sliceId: stableId("implementation-slice", capabilityId
      ? `capability:${capabilityId}`
      : JSON.stringify({
        gapGroup: record.gapGroup,
        products: rule.products,
        tests: rule.tests.map((test) => typeof test === "object" ? test.path : test),
      })),
    resolution: {
      strategy: rule.strategy,
      ruleId: rule.ruleId,
      capabilityId,
      primaryGroup: rule.primaryGroup ?? record.gapGroup,
      childCapabilities: rule.childCapabilities ?? [],
      correctionIds: rule.correctionIds ?? [],
      classification: rule.classification ?? rule.disposition,
      disposition: rule.disposition,
      matchedSource: `${candidate.path}:${candidate.line}`,
      matchedIdentifiers: resolutionEvidence.matchedIdentifiers
        ?? productTargets.map(({ symbol }) => symbol).filter(Boolean),
      score: resolutionEvidence.score ?? null,
      secondBestScore: resolutionEvidence.secondBestScore ?? null,
      margin: resolutionEvidence.margin ?? null,
      runnerOwnership: tests.map(({ path, evidenceClass, ownershipReason }) => ({ path, evidenceClass, ownershipReason })),
      rationale: resolutionEvidence.rationale ?? rule.rationale,
    },
  };
}

function resolveReviewedPlanOwnership(record, candidate, index) {
  return resolveRulePlanOwnership(record, candidate, index, reviewedOwnershipRule(record));
}

function resolveValidatedLedgerPlanOwnership(record, candidate, index) {
  if (!REVIEWED_LEDGER_RECORD_IDS.has(record.id)) {
    throw new Error(`record is outside validated ledger ownership: ${record.id}`);
  }
  const rule = REVIEWED_LEDGER_RULE_BY_RECORD.get(record.id);
  if (!rule) throw new Error(`validated ledger record has no exact ownership slice: ${record.id}`);
  return resolveRulePlanOwnership(record, candidate, index, rule);
}

function planTestContract(record, candidate, index) {
  if (record.kind === "control") {
    const match = /^Test: ([^#]+)#(.+)\.$/.exec(record.acceptanceCriteria[1] ?? "");
    if (match) {
      return {
        path: match[1],
        name: match[2],
        evidenceClass: "reviewed-planned",
        fileAction: index.trackedPaths.has(match[1]) ? "Modify" : "Create",
        ownershipReason: "The control acceptance contract explicitly names this owning component test runner.",
        command: `pnpm -C web test -- --run ${match[1].replace(/^web\//, "")} -t ${JSON.stringify(match[2])}`,
      };
    }
  }
  const name = `${record.id} closes documented acceptance contract`;
  return {
    path: "tools/completion-audit.test.mjs",
    name,
    evidenceClass: "reviewed-planned",
    fileAction: "Modify",
    ownershipReason: "Documentation/process closure is verified by the tracked completion-audit runner.",
    command: `node --test --test-name-pattern=${JSON.stringify(name)} tools/completion-audit.test.mjs`,
  };
}

const REVIEWED_PLAN_GROUP_OVERRIDES = new Map(Object.values(REVIEWED_EXACT_SLICES.groups).flatMap(({ slices }) => (
  slices.filter(({ targetGroup }) => targetGroup).flatMap(({ recordIds, targetGroup }) => (
    recordIds.map((recordId) => [normalizeReviewedRecordId(recordId), targetGroup])
  ))
)));
const REVIEWED_PLAN_STATUS_OVERRIDES = new Map(Object.values(REVIEWED_EXACT_SLICES.groups).flatMap(({ slices }) => (
  slices.filter(({ targetStatus }) => targetStatus).flatMap(({ recordIds, targetStatus }) => (
    recordIds.map((recordId) => [normalizeReviewedRecordId(recordId), targetStatus])
  ))
)));

function planEntries(root, requirements, controls, documentCandidates, controlCandidates, includedGapGroups = null) {
  const index = planningIndex(root);
  const documentById = new Map(documentCandidates.map((candidate) => [candidate.id, candidate]));
  const controlById = new Map(controlCandidates.map((candidate) => [candidate.id, candidate]));
  return [
    ...requirements.map((record) => {
      const gapGroup = REVIEWED_PLAN_GROUP_OVERRIDES.get(record.id) ?? record.gapGroup;
      const status = REVIEWED_PLAN_STATUS_OVERRIDES.get(record.id) ?? record.status;
      return gapGroup === record.gapGroup && status === record.status ? record : { ...record, gapGroup, status };
    }).filter(({ status, gapGroup }) => (
      status === "incomplete" && (!includedGapGroups || includedGapGroups.has(gapGroup))
    )).map((record) => ({
      kind: "requirement",
      record,
      candidate: documentById.get(record.candidateId),
    })),
    ...controls.filter(({ status, gapGroup }) => (
      status === "incomplete" && (!includedGapGroups || includedGapGroups.has(gapGroup))
    )).map((record) => ({
      kind: "control",
      record,
      candidate: controlById.get(record.candidateId),
    })),
  ].map((entry) => {
    const record = { ...entry.record, kind: entry.kind };
    let ownership;
    if (entry.kind === "requirement" && record.gapGroup !== "documentation") {
      if (REVIEWED_PLAN_RECORD_IDS.has(record.id)) {
        ownership = resolveReviewedPlanOwnership(record, entry.candidate, index);
      } else if (REVIEWED_LEDGER_RECORD_IDS.has(record.id)) {
        ownership = resolveValidatedLedgerPlanOwnership(record, entry.candidate, index);
      } else {
        throw new Error(`non-documentation requirement is outside exact report and ledger ownership: ${record.id}`);
      }
    } else {
      const test = planTestContract(record, entry.candidate, index);
      const files = extractPlanFileRefs(record, entry.candidate);
      ownership = {
        files,
        tests: [test],
        test,
        sliceId: stableId("implementation-slice", JSON.stringify({
          gapGroup: record.gapGroup,
          files,
          testPath: test.path,
        })),
        resolution: {
          strategy: entry.kind === "control" ? "control-acceptance" : "documentation-process",
          ruleId: null,
          disposition: "acceptance-closure",
          matchedSource: `${entry.candidate.path}:${entry.candidate.line}`,
          matchedIdentifiers: [],
          score: null,
          secondBestScore: null,
          margin: null,
          runnerOwnership: [{ path: test.path, evidenceClass: test.evidenceClass, ownershipReason: test.ownershipReason }],
          rationale: test.ownershipReason,
        },
      };
    }
    return {
      ...entry,
      ...ownership,
      expectedBehavior: entry.kind === "requirement"
        ? entry.record.targetBehavior
        : entry.record.outcomes.success,
    };
  });
}

function titleFromGroup(group) {
  return group.split("-").map((part) => part[0].toUpperCase() + part.slice(1)).join(" ");
}

function uniquePlanValues(values, key) {
  const seen = new Set();
  return values.filter((value) => {
    const identity = key(value);
    if (seen.has(identity)) return false;
    seen.add(identity);
    return true;
  });
}

function planSlices(entries) {
  const slices = new Map();
  for (const entry of entries) {
    const primaryGroup = entry.resolution.primaryGroup ?? entry.record.gapGroup;
    const capabilityId = entry.resolution.capabilityId ?? entry.sliceId;
    const existing = slices.get(entry.sliceId) ?? {
      id: entry.sliceId,
      gapGroup: primaryGroup,
      primaryGroup,
      gapGroups: [],
      capabilityId,
      entries: [],
      files: [],
      tests: [],
      ruleIds: [],
    };
    if (existing.primaryGroup !== primaryGroup || existing.capabilityId !== capabilityId) {
      throw new Error(`implementation slice has conflicting shared-capability ownership: ${entry.sliceId}`);
    }
    existing.gapGroups = uniquePlanValues([...existing.gapGroups, entry.record.gapGroup], (value) => value);
    existing.entries.push(entry);
    existing.files = uniquePlanValues([...existing.files, ...entry.files], (value) => value);
    existing.tests = uniquePlanValues(
      [...existing.tests, ...entry.tests],
      ({ path, name, evidenceClass }) => `${path}#${name}:${evidenceClass}`,
    );
    existing.ruleIds = uniquePlanValues(
      [...existing.ruleIds, entry.resolution.ruleId ?? entry.resolution.strategy],
      (value) => value,
    );
    slices.set(entry.sliceId, existing);
  }
  return [...slices.values()].map((slice) => ({ ...slice, sharedCapability: slice.gapGroups.length > 1 }));
}

function renderGapDesign(group, entries) {
  const title = titleFromGroup(group);
  const slices = planSlices(entries);
  const lines = [
    `# ${title} Completion Design`,
    "",
    `**Gap group:** \`${group}\``,
    "",
    `**Records:** ${entries.length}`,
    "",
    `**Implementation slices:** ${slices.length}`,
    "",
    "## Architecture",
    "",
    "Close each record as the smallest end-to-end vertical slice while preserving Rust-authoritative state, command/API parity, transactional safety, and explicit pending/empty/failure UI states. A record changes status only after its exact acceptance contract and strongest relevant runtime path pass.",
    "",
    "## Record contracts",
    "",
  ];
  if (entries.length === 0) {
    lines.push("The verified ledger contains no incomplete records for this group.");
  }
  for (const { kind, record, candidate, files, tests, sliceId, resolution, expectedBehavior } of entries) {
    lines.push(
      `### ${record.id}`,
      "",
      `- Kind: ${kind}`,
      `- Implementation slice: \`${sliceId}\``,
      `- Candidate: \`${record.candidateId}\``,
      `- Source citation: \`${candidate.path}:${candidate.line}${candidate.column ? `:${candidate.column}` : ""}\``,
      `- Exact files/symbols: ${files.map((file) => `\`${file}\``).join(", ")}`,
      `- Target resolution: \`${resolution.strategy}${resolution.ruleId ? `:${resolution.ruleId}` : ""}\`; matched ${resolution.matchedIdentifiers.length > 0 ? resolution.matchedIdentifiers.map((value) => `\`${value}\``).join(", ") : "the exact process/control contract"}.`,
      `- Resolution rationale: ${resolution.rationale}`,
      "- Test ownership:",
      ...tests.map((test) => `  - \`${test.path}#${test.name}\` (${test.evidenceClass}): ${test.ownershipReason}`),
      `- Expected behavior: ${expectedBehavior}`,
      `- Acceptance criteria: ${record.acceptanceCriteria.join(" ")}`,
      "",
    );
  }
  return markdownDocument(lines);
}

function renderGapImplementationPlan(group, entries) {
  const title = titleFromGroup(group);
  const slices = planSlices(entries);
  const implementationSlices = slices.filter(({ primaryGroup }) => primaryGroup === group);
  const sharedReferences = slices.filter(({ primaryGroup }) => primaryGroup !== group);
  const lines = [
    `# ${title} Completion Implementation Plan`,
    "",
    "> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.",
    "",
    `**Goal:** Close all ${entries.length} verified incomplete records in the \`${group}\` gap group.`,
    "",
    `**Architecture:** Implement ${implementationSlices.length} primary evidence-bound slices and reference ${sharedReferences.length} shared capabilities owned by another gap group. Preserve existing OpenTake boundaries and promote a ledger record only after its exact failing test, implementation path, visible behavior, and runtime evidence agree.`,
    "",
    "**Tech Stack:** Rust, Tauri 2, React, TypeScript, Vitest/Node test runner, Cargo.",
    "",
    "---",
    "",
  ];
  if (entries.length === 0) {
    lines.push("The all-scope verifier proves this group is empty; no implementation task is emitted.", "");
  }
  implementationSlices.forEach((slice, index) => {
    const implementationCommand = slice.files.some((file) => file.includes(".rs"))
      ? "cargo fmt --all -- --check && cargo test --workspace --no-fail-fast"
      : slice.files.some((file) => /\.tsx?/.test(file))
        ? "pnpm -C web test -- --run && pnpm -C web build"
        : "node --test tools/completion-audit.test.mjs";
    lines.push(
      `### Task ${index + 1}: ${slice.ruleIds.join(" + ")} (${slice.id})`,
      "",
      "**Covered records:**",
      ...slice.entries.map(({ kind, record }) => `- \`${record.id}\` (${kind})`),
      "",
      "**Files:**",
      ...slice.files.map((file) => `- Modify: \`${file}\``),
      ...slice.tests.map((test) => `- Test (${test.evidenceClass}): \`${test.path}#${test.name}\``),
      "",
      "**Candidate-bound contracts:**",
      "",
      ...slice.entries.flatMap(({ kind, record, candidate, expectedBehavior, resolution }) => [
        `#### ${record.id}`,
        "",
        `- Candidate/source: \`${record.candidateId}\` at \`${candidate.path}:${candidate.line}${candidate.column ? `:${candidate.column}` : ""}\` (${kind})`,
        `- Expected behavior: ${expectedBehavior}`,
        `- Resolution: \`${resolution.strategy}${resolution.ruleId ? `:${resolution.ruleId}` : ""}\` — ${resolution.rationale}`,
        "- Exact acceptance contract:",
        ...record.acceptanceCriteria.map((criterion) => `  - ${criterion}`),
        "",
      ]),
      "- [ ] **Step 1: Write or extend every reviewed owning test**",
      "",
      ...slice.tests.map((test) => `  - \`${test.path}#${test.name}\` (${test.evidenceClass}) — ${test.ownershipReason}`),
      "",
      "  Each assertion must exercise every covered candidate through the mapped product boundary; an existing-owned test may be extended, while a reviewed-planned test must be added at the declared runner path.",
      "",
      "- [ ] **Step 2: Run all focused tests and verify RED**",
      "",
      ...slice.tests.map((test) => `  - Run: \`${test.command}\``),
      "",
      `  Expected: FAIL because one or more of the ${slice.entries.length} candidate-bound contracts are not yet satisfied.`,
      "",
      "- [ ] **Step 3: Implement the minimal vertical slice**",
      "",
      `  Modify only ${slice.files.map((file) => `\`${file}\``).join(", ")} as required to satisfy every listed acceptance criterion, including visible success and explicit failure/recovery behavior.`,
      "",
      "- [ ] **Step 4: Run all focused tests and verify GREEN**",
      "",
      ...slice.tests.map((test) => `  - Run: \`${test.command}\``),
      "",
      "  Expected: PASS with every candidate-bound assertion executed.",
      "",
      "- [ ] **Step 5: Run the subsystem regression gate**",
      "",
      `  Run: \`${implementationCommand}\``,
      "",
      "  Expected: PASS with no new warnings or unrelated changes.",
      "",
    );
  });
  if (sharedReferences.length > 0) {
    lines.push("## Shared capability references", "");
    for (const slice of sharedReferences) {
      lines.push(
        `- \`${slice.capabilityId}\` / \`${slice.id}\`: implemented once in \`${slice.primaryGroup}\`; this group contributes records ${slice.entries.map(({ record }) => `\`${record.id}\``).join(", ")} as acceptance references.`,
      );
    }
    lines.push("");
  }
  return markdownDocument(lines);
}

export function buildImplementationPlanGroup(root, auditDir, group) {
  if (!DOCUMENT_GAP_GROUPS.includes(group)) throw new Error(`unsupported implementation plan group: ${group}`);
  const repositoryRoot = realpathSync(resolve(root));
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const entries = planEntries(
    repositoryRoot,
    loadAuditJson(auditDirectory, "requirements.json").records,
    loadAuditJson(auditDirectory, "controls.json").records,
    loadAuditJson(auditDirectory, "document-candidates.json").candidates,
    loadAuditJson(auditDirectory, "control-candidates.json").candidates,
    new Set([group]),
  );
  return {
    group,
    entries,
    slices: planSlices(entries),
    design: renderGapDesign(group, entries),
    implementation: renderGapImplementationPlan(group, entries),
  };
}

export function buildImplementationPlans(root, auditDir) {
  const repositoryRoot = realpathSync(resolve(root));
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const requirements = loadAuditJson(auditDirectory, "requirements.json").records;
  const controls = loadAuditJson(auditDirectory, "controls.json").records;
  const documentCandidates = loadAuditJson(auditDirectory, "document-candidates.json").candidates;
  const controlCandidates = loadAuditJson(auditDirectory, "control-candidates.json").candidates;
  const entries = planEntries(repositoryRoot, requirements, controls, documentCandidates, controlCandidates);
  const outputs = {};
  const groups = {};
  for (const group of DOCUMENT_GAP_GROUPS) {
    const groupEntries = entries.filter(({ record }) => record.gapGroup === group);
    const directory = normalizePath(join(relative(repositoryRoot, auditDirectory), "implementation-plans"));
    const designPath = normalizePath(join(directory, `${group}-design.md`));
    const planPath = normalizePath(join(directory, `${group}-implementation.md`));
    outputs[designPath] = renderGapDesign(group, groupEntries);
    outputs[planPath] = renderGapImplementationPlan(group, groupEntries);
    groups[group] = {
      designPath,
      implementationPath: planPath,
      entries: groupEntries,
      slices: planSlices(groupEntries),
    };
  }
  const slices = planSlices(entries);
  const testReferences = entries.flatMap(({ tests }) => tests);
  const testIdentity = ({ path, name, evidenceClass }) => `${path}\u0000${name}\u0000${evidenceClass}`;
  const existingOwnedTestReferences = testReferences.filter(({ evidenceClass }) => evidenceClass === "existing-owned");
  const reviewedPlannedTestReferences = testReferences.filter(({ evidenceClass }) => evidenceClass === "reviewed-planned");
  const lines = [
    "# OpenTake implementation plan index",
    "",
    "Every incomplete requirement/control record appears exactly once below. Counts are regenerated from the authoritative ledgers; no count is hard-coded.",
    "",
    `- Incomplete requirements: ${entries.filter(({ kind }) => kind === "requirement").length}`,
    `- Incomplete controls: ${entries.filter(({ kind }) => kind === "control").length}`,
    `- Total planned records: ${entries.length}`,
    `- Total implementation slices: ${slices.length}`,
    "",
  ];
  for (const group of DOCUMENT_GAP_GROUPS) {
    const groupInfo = groups[group];
    lines.push(
      `## ${group}`,
      "",
      `- Design: \`${groupInfo.designPath}\``,
      `- Implementation: \`${groupInfo.implementationPath}\``,
      `- Record count: ${groupInfo.entries.length}`,
      `- Implementation slice count: ${groupInfo.slices.length}`,
      "",
      "| Record | Slice | Kind | Candidate | Source | Reviewed tests |",
      "|---|---|---|---|---|---|",
    );
    if (groupInfo.entries.length === 0) {
      lines.push("| — | — | empty | — | Verifier count is zero | — |");
    }
    for (const { kind, record, candidate, tests, sliceId } of groupInfo.entries) {
      const testSummary = tests.map(({ path, name, evidenceClass }) => `${path}#${name} (${evidenceClass})`).join("; ");
      lines.push(`| ${record.id} | ${sliceId} | ${kind} | ${record.candidateId} | ${markdownCell(`${candidate.path}:${candidate.line}${candidate.column ? `:${candidate.column}` : ""}`)} | ${markdownCell(testSummary)} |`);
    }
    lines.push("");
  }
  const ownership = {
    schema: 1,
    metadata: {
      schemaName: "opentake-reviewed-plan-ownership",
      schemaVersion: 1,
      policy: "Every incomplete record is assigned to an explicit reviewed product/process boundary, owning runner, and clustered implementation slice; heuristic top-one selection is prohibited.",
      mappingReports: structuredClone(REVIEWED_PLAN_MAP.provenance),
    },
    counts: {
      records: entries.length,
      slices: slices.length,
      reviewedMappingReport: entries.filter(({ resolution }) => resolution.strategy === "reviewed-mapping-report").length,
      validatedLedgerEvidence: entries.filter(({ resolution }) => resolution.strategy === "validated-ledger-evidence").length,
      controlAcceptance: entries.filter(({ resolution }) => resolution.strategy === "control-acceptance").length,
      documentationProcess: entries.filter(({ resolution }) => resolution.strategy === "documentation-process").length,
      implementationGap: entries.filter(({ resolution }) => resolution.disposition === "implementation-gap").length,
      compositeAcceptance: entries.filter(({ resolution }) => resolution.disposition === "composite-acceptance").length,
      evidenceClosure: entries.filter(({ resolution }) => resolution.disposition === "evidence-closure").length,
      acceptanceClosure: entries.filter(({ resolution }) => resolution.disposition === "acceptance-closure").length,
      existingOwnedTestReferences: existingOwnedTestReferences.length,
      reviewedPlannedTestReferences: reviewedPlannedTestReferences.length,
      uniqueExistingOwnedTests: new Set(existingOwnedTestReferences.map(testIdentity)).size,
      uniqueReviewedPlannedTests: new Set(reviewedPlannedTestReferences.map(testIdentity)).size,
    },
    records: entries.map(({ kind, record, candidate, files, tests, sliceId, resolution }) => ({
      recordId: record.id,
      kind,
      gapGroup: record.gapGroup,
      candidateId: record.candidateId,
      source: {
        path: candidate.path,
        line: candidate.line,
        column: candidate.column ?? null,
      },
      sliceId,
      files,
      tests,
      resolution,
    })),
    slices: slices.map((slice) => ({
      id: slice.id,
      gapGroup: slice.gapGroup,
      primaryGroup: slice.primaryGroup,
      gapGroups: slice.gapGroups,
      capabilityId: slice.capabilityId,
      sharedCapability: slice.sharedCapability,
      ruleIds: slice.ruleIds,
      recordIds: slice.entries.map(({ record }) => record.id),
      files: slice.files,
      tests: slice.tests,
    })),
  };
  return {
    entries,
    slices,
    groups,
    outputs,
    index: markdownDocument(lines),
    ownership,
  };
}

function validateIdentityMigrationArtifact(root, auditDirectory, errors) {
  let artifact;
  try {
    artifact = loadAuditJson(auditDirectory, "identity-migration.json");
  } catch (error) {
    pushVerificationError(errors, "missing-identity-migration", error.message);
    return null;
  }
  if (!hasExactObjectKeys(artifact, [
    "schema", "metadata", "provenance", "counts", "duplicateOrdinals", "rewriteCounts", "hashes",
    "classificationCorrections", "documentMappings", "controlMappings",
  ]) || artifact.schema !== 1) {
    pushVerificationError(errors, "invalid-identity-migration-schema", "identity-migration.json has unexpected keys or schema");
    return artifact;
  }
  if (!hasExactObjectKeys(artifact.metadata, ["schemaName", "schemaVersion", "identityVersion", "policy"])) {
    pushVerificationError(errors, "invalid-identity-migration-schema", "identity migration metadata keys drifted");
  }
  const sourceRevision = artifact.provenance?.sourceRevision;
  const migrationRevision = artifact.provenance?.migrationRevision;
  for (const [label, revision] of [["source", sourceRevision], ["migration", migrationRevision]]) {
    try {
      const commit = gitText("identity migration verifier", root, ["rev-parse", `${revision.commit}^{commit}`]);
      const tree = gitText("identity migration verifier", root, ["rev-parse", `${revision.commit}^{tree}`]);
      execFileSync("git", ["-C", root, "merge-base", "--is-ancestor", revision.commit, "HEAD"], { stdio: "ignore" });
      if (commit !== revision.commit || tree !== revision.tree) {
        pushVerificationError(errors, "identity-migration-revision-drift", `${label} revision commit/tree mismatch`);
      }
    } catch {
      pushVerificationError(errors, "identity-migration-revision-drift", `${label} revision is not an immutable ancestor`);
    }
  }
  try {
    const head = gitText("identity migration verifier", root, ["rev-parse", "HEAD"]);
    const parent = head === migrationRevision.commit
      ? head
      : gitText("identity migration verifier", root, ["rev-parse", "HEAD^"]);
    if (parent !== migrationRevision.commit) {
      pushVerificationError(errors, "identity-migration-not-two-stage-parent", "migration revision must be current HEAD or its direct parent");
    }
  } catch (error) {
    pushVerificationError(errors, "identity-migration-not-two-stage-parent", error.message);
  }
  const oldNames = [
    "document-candidates.json", "requirements.json", "control-candidates.json", "controls.json",
    "runtime-evidence.json", "sources.json",
  ];
  const auditRelative = normalizePath(relative(root, auditDirectory));
  const oldLedgers = {};
  for (const name of oldNames) {
    try {
      const bytes = gitRead("identity migration verifier", root, ["show", `${sourceRevision.commit}:${auditRelative}/${name}`], null);
      oldLedgers[name] = JSON.parse(bytes.toString("utf8"));
      if (artifact.provenance.oldLedgerSha256?.[name] !== sha256(bytes)) {
        pushVerificationError(errors, "identity-old-ledger-hash-mismatch", `${name} old ledger hash drifted`);
      }
      const currentBytes = readFileSync(resolve(auditDirectory, name));
      if (artifact.provenance.newLedgerSha256?.[name] !== sha256(currentBytes)) {
        pushVerificationError(errors, "identity-new-ledger-hash-mismatch", `${name} current ledger hash drifted`);
      }
    } catch (error) {
      pushVerificationError(errors, "identity-ledger-unavailable", `${name}: ${error.message}`);
    }
  }
  try {
    const newDocuments = loadAuditJson(auditDirectory, "document-candidates.json").candidates;
    const newControls = loadAuditJson(auditDirectory, "control-candidates.json").candidates;
    const expected = buildIdentityMigration({
      oldDocuments: oldLedgers["document-candidates.json"].candidates,
      newDocuments,
      oldControls: oldLedgers["control-candidates.json"].candidates,
      newControls,
    });
    for (const field of ["counts", "duplicateOrdinals", "documentMappings", "controlMappings"]) {
      if (JSON.stringify(artifact[field]) !== JSON.stringify(expected[field])) {
        pushVerificationError(errors, "identity-mapping-drift", `${field} differs from deterministic migration`);
      }
    }
    if (
      artifact.hashes.documentMappingsSha256 !== sha256(JSON.stringify(expected.documentMappings))
      || artifact.hashes.controlMappingsSha256 !== sha256(JSON.stringify(expected.controlMappings))
    ) {
      pushVerificationError(errors, "identity-mapping-hash-mismatch", "identity mapping hashes drifted");
    }
    const migrated = migrateAuditIdentityReferences({
      migration: expected,
      requirements: oldLedgers["requirements.json"],
      controls: oldLedgers["controls.json"],
      runtimeEvidence: oldLedgers["runtime-evidence.json"],
      sources: oldLedgers["sources.json"],
    });
    const classificationCorrections = reviewProcessGapCorrections(migrated.requirements);
    if (JSON.stringify(artifact.rewriteCounts) !== JSON.stringify(migrated.rewriteCounts)) {
      pushVerificationError(errors, "identity-rewrite-counts-drift", "identity rewrite counts differ from deterministic migration");
    }
    if (JSON.stringify(artifact.classificationCorrections) !== JSON.stringify(classificationCorrections)) {
      pushVerificationError(errors, "identity-classification-corrections-drift", "reviewed process-only gap corrections differ from deterministic replay");
    }
    const currentRequirements = loadAuditJson(auditDirectory, "requirements.json");
    if (JSON.stringify(currentRequirements) !== JSON.stringify(migrated.requirements)) {
      pushVerificationError(errors, "identity-classification-corrections-drift", "requirements ledger differs from identity rewrite plus reviewed gap corrections");
    }
    const reportExpectations = {
      "document-reconciliation.md": renderDocumentReconciliation(
        readFileSync(resolve(auditDirectory, "requirements.json")),
      ),
      "upstream-downstream.md": renderSourceReport(loadAuditJson(auditDirectory, "sources.json")),
    };
    for (const [name, expectedReport] of Object.entries(reportExpectations)) {
      const reportPath = resolve(auditDirectory, name);
      if (!existsSync(reportPath) || readFileSync(reportPath, "utf8") !== expectedReport) {
        pushVerificationError(
          errors,
          "identity-human-report-drift",
          `${name} differs from its normative JSON renderer`,
        );
      }
    }
    const currentPublicationPaths = currentAuditPublicationPaths(root, auditRelative);
    const referenceSources = Object.fromEntries([
      ...AUDIT_VERIFIER_FILES.map((path) => [path, readFileSync(resolve(root, path), "utf8")]),
      ...currentPublicationPaths.map((path) => [path, readFileSync(resolve(root, path), "utf8")]),
    ]);
    const staleReferences = findMigratedIdentityReferenceLeaks(expected, referenceSources);
    if (staleReferences.length > 0) {
      const first = staleReferences[0];
      pushVerificationError(
        errors,
        "identity-stale-reference",
        `legacy candidate/record IDs remain after migration (${staleReferences.length}): ${first.path} contains ${first.oldId}`,
      );
    }
  } catch (error) {
    pushVerificationError(errors, "identity-migration-replay-failed", error.message);
  }
  return artifact;
}

export function hasValidProductPlanOwnership(entry) {
  const strategyValid = ["reviewed-mapping-report", "validated-ledger-evidence"]
    .includes(entry?.resolution?.strategy);
  const productTargets = Array.isArray(entry?.productTargets) ? entry.productTargets : [];
  const childCapabilities = Array.isArray(entry?.resolution?.childCapabilities)
    ? entry.resolution.childCapabilities
    : [];
  const productBoundaryValid = productTargets.some((target) => (
    target && typeof target === "object" && typeof target.path === "string" && target.path.length > 0
    && typeof target.planned === "boolean" && Object.hasOwn(target, "symbol")
  )) || childCapabilities.some((capabilityId) => typeof capabilityId === "string" && capabilityId.length > 0);
  const runnersValid = Array.isArray(entry?.tests) && entry.tests.length > 0 && entry.tests.every((test) => (
    test && typeof test.path === "string" && test.path !== "tools/completion-audit.test.mjs"
    && typeof test.name === "string" && test.name.length > 0
    && ["existing-owned", "reviewed-planned"].includes(test.evidenceClass)
  ));
  return strategyValid && productBoundaryValid && runnersValid;
}

function validateCompletionArtifacts(root, auditDirectory, errors) {
  const plans = buildImplementationPlans(root, auditDirectory);
  for (const [path, expected] of Object.entries(plans.outputs)) {
    const absolute = resolve(root, path);
    if (!existsSync(absolute) || readFileSync(absolute, "utf8") !== expected) {
      pushVerificationError(errors, "implementation-plan-drift", `implementation plan differs from authoritative records: ${path}`);
    }
  }
  const indexPath = resolve(auditDirectory, "implementation-plan-index.md");
  if (!existsSync(indexPath) || readFileSync(indexPath, "utf8") !== plans.index) {
    pushVerificationError(errors, "implementation-plan-index-drift", "implementation-plan-index.md differs from exact record coverage");
  }
  const ownershipPath = resolve(auditDirectory, "plan-ownership.json");
  try {
    const ownership = JSON.parse(readFileSync(ownershipPath, "utf8"));
    if (JSON.stringify(ownership) !== JSON.stringify(plans.ownership)) {
      pushVerificationError(errors, "plan-ownership-drift", "plan-ownership.json differs from reviewed record ownership and clustered slices");
    }
    if (!hasExactObjectKeys(ownership, ["schema", "metadata", "counts", "records", "slices"]) || ownership.schema !== 1) {
      pushVerificationError(errors, "invalid-plan-ownership-schema", "plan-ownership.json has unexpected keys or schema");
    }
  } catch (error) {
    pushVerificationError(errors, "plan-ownership-unavailable", error.message);
  }
  const plannedIds = plans.entries.map(({ record }) => record.id);
  if (new Set(plannedIds).size !== plannedIds.length) {
    pushVerificationError(errors, "duplicate-planned-record", "an incomplete record is assigned more than once");
  }
  const slicedIds = plans.slices.flatMap(({ entries }) => entries.map(({ record }) => record.id));
  if (
    slicedIds.length !== plannedIds.length
    || new Set(slicedIds).size !== slicedIds.length
    || plannedIds.some((id) => !slicedIds.includes(id))
  ) {
    pushVerificationError(errors, "invalid-plan-slice-coverage", "clustered implementation slices do not cover every incomplete record exactly once");
  }
  if (plannedIds.length > 1 && plans.slices.length >= plannedIds.length) {
    pushVerificationError(errors, "unclustered-implementation-plan", "shared product/test ownership was not clustered into implementation slices");
  }
  for (const entry of plans.entries) {
    if (entry.tests.length === 0 || entry.tests.some(({ evidenceClass }) => (
      !["existing-owned", "reviewed-planned"].includes(evidenceClass)
    ))) {
      pushVerificationError(errors, "invalid-plan-test-ownership", `${entry.record.id} lacks classified owning tests`);
    }
    if (entry.kind === "requirement" && entry.record.gapGroup !== "documentation") {
      if (!hasValidProductPlanOwnership(entry)) {
        pushVerificationError(errors, "invalid-product-plan-ownership", `${entry.record.id} is not bound to reviewed product declarations and owning runners`);
      }
    }
  }
  const ledgerPath = resolve(auditDirectory, "completion-ledger.json");
  let ledger = null;
  try {
    ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
    const expectedLedger = buildCompletionLedger(root, auditDirectory);
    if (JSON.stringify(ledger) !== JSON.stringify(expectedLedger)) {
      pushVerificationError(errors, "completion-ledger-drift", "completion-ledger.json differs from its current hashed inputs");
    }
    if (!hasExactObjectKeys(ledger, ["schema", "metadata", "provenance", "counts", "gapCounts", "records"])) {
      pushVerificationError(errors, "invalid-completion-ledger-schema", "completion ledger top-level keys drifted");
    }
    const reportPath = resolve(auditDirectory, "completion-report.md");
    const expectedReport = renderCompletionReport(expectedLedger);
    if (!existsSync(reportPath) || readFileSync(reportPath, "utf8") !== expectedReport) {
      pushVerificationError(errors, "completion-report-drift", "completion-report.md differs from completion ledger");
    }
  } catch (error) {
    pushVerificationError(errors, "completion-artifact-unavailable", error.message);
  }
  return { plans, ledger };
}

function verifyAllAudit(root, auditDir) {
  const repositoryRoot = realpathSync(resolve(root));
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const scopeResults = Object.fromEntries(["files", "documents", "controls", "sources"].map((scope) => [
    scope,
    verifyAudit(repositoryRoot, auditDirectory, scope),
  ]));
  const errors = Object.entries(scopeResults).flatMap(([scope, result]) => (
    result.errors.map((error) => ({ ...error, scope }))
  ));
  const identity = validateIdentityMigrationArtifact(repositoryRoot, auditDirectory, errors);
  const { plans, ledger } = validateCompletionArtifacts(repositoryRoot, auditDirectory, errors);
  const gapCounts = ledger?.gapCounts ?? Object.fromEntries(DOCUMENT_GAP_GROUPS.map((group) => [group, {
    requirements: plans.entries.filter(({ kind, record }) => kind === "requirement" && record.gapGroup === group).length,
    controls: plans.entries.filter(({ kind, record }) => kind === "control" && record.gapGroup === group).length,
    total: plans.entries.filter(({ record }) => record.gapGroup === group).length,
  }]));
  const counts = {
    tracked: scopeResults.files.counts.tracked,
    files: scopeResults.files.counts.inventoried,
    documents: scopeResults.documents.counts.candidates,
    requirements: scopeResults.documents.counts.records,
    controls: scopeResults.controls.counts.candidates,
    controlRecords: scopeResults.controls.counts.records,
    sources: scopeResults.sources.counts.sources,
    sourceChanges: scopeResults.sources.counts.changedPaths,
    incompleteRequirements: scopeResults.documents.counts.incomplete,
    incompleteControls: scopeResults.controls.counts.incomplete,
    plannedRecords: plans.entries.length,
    unverified: scopeResults.documents.counts.unverified + scopeResults.controls.counts.unverified,
    identityMappedDocuments: identity?.counts?.mappedDocuments ?? 0,
    identityMappedControls: identity?.counts?.mappedControls ?? 0,
  };
  return {
    schema: 1,
    scope: "all",
    auditDirectory: normalizePath(relative(repositoryRoot, auditDirectory)) || ".",
    ok: errors.length === 0,
    passed: errors.length === 0,
    counts,
    gapCounts,
    scopeHashes: {
      files: sha256(readFileSync(resolve(auditDirectory, "repository-files.json"))),
      documents: sha256(readFileSync(resolve(auditDirectory, "document-candidates.json"))),
      controls: sha256(readFileSync(resolve(auditDirectory, "control-candidates.json"))),
      sources: scopeResults.sources.hashes.sourceLedgerSha256,
      completionLedger: existsSync(resolve(auditDirectory, "completion-ledger.json"))
        ? sha256(readFileSync(resolve(auditDirectory, "completion-ledger.json")))
        : null,
      planOwnership: existsSync(resolve(auditDirectory, "plan-ownership.json"))
        ? sha256(readFileSync(resolve(auditDirectory, "plan-ownership.json")))
        : null,
    },
    scopes: Object.fromEntries(Object.entries(scopeResults).map(([scope, result]) => [scope, {
      passed: result.passed,
      counts: result.counts,
    }])),
    errors,
  };
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
  if (command === "verify-open-prs") {
    return verifyOpenPullRequests();
  }
  if (command === "sources") {
    if (!args.palmier || !args.canonical || !args.report) {
      throw new Error("sources command needs --palmier <path> --canonical <path> --report <path>");
    }
    const evidence = buildSourceEvidence({
      root,
      palmierPath: resolve(args.palmier),
      canonicalPath: resolve(args.canonical),
    });
    const report = resolve(args.report);
    mkdirSync(dirname(report), { recursive: true });
    writeFileSync(report, renderSourceReport(evidence));
    return evidence;
  }
  if (command === "migrate-identities") {
    if (!args.audit) {
      throw new Error("migrate-identities command needs --audit <path>");
    }
    const publication = buildIdentityMigrationPublication(
      root,
      args.audit,
      args.from ?? "HEAD",
    );
    const auditDirectory = resolve(root, args.audit);
    for (const [name, value] of Object.entries(publication.outputs)) {
      writeJson(resolve(auditDirectory, name), value);
    }
    for (const [name, value] of Object.entries(publication.reports)) {
      writeFileSync(resolve(auditDirectory, name), value);
    }
    return publication.artifact;
  }
  if (command === "publish") {
    if (!args.audit || !args.report || !args.index) {
      throw new Error("publish command needs --audit <path> --report <path> --index <path>");
    }
    const auditDirectory = resolve(root, args.audit);
    const requirementsPath = resolve(auditDirectory, "requirements.json");
    writeFileSync(
      resolve(auditDirectory, "document-reconciliation.md"),
      renderDocumentReconciliation(readFileSync(requirementsPath)),
    );
    writeFileSync(
      resolve(auditDirectory, "upstream-downstream.md"),
      renderSourceReport(loadAuditJson(auditDirectory, "sources.json")),
    );
    const plans = buildImplementationPlans(root, auditDirectory);
    for (const [path, content] of Object.entries(plans.outputs)) {
      const absolute = resolve(root, path);
      mkdirSync(dirname(absolute), { recursive: true });
      writeFileSync(absolute, content);
    }
    const indexPath = resolve(args.index);
    mkdirSync(dirname(indexPath), { recursive: true });
    writeFileSync(indexPath, plans.index);
    writeJson(resolve(auditDirectory, "plan-ownership.json"), plans.ownership);
    const inventoryPath = resolve(auditDirectory, "repository-files.json");
    const deferredPaths = [
      normalizePath(relative(root, resolve(args.out))),
      normalizePath(relative(root, resolve(args.report))),
      normalizePath(relative(root, resolve(auditDirectory, "final-verification.json"))),
    ];
    writeJson(inventoryPath, {
      schema: 1,
      files: buildFileInventory(root, normalizePath(relative(root, inventoryPath)), deferredPaths),
    });
    const ledger = buildCompletionLedger(root, auditDirectory);
    const reportPath = resolve(args.report);
    mkdirSync(dirname(reportPath), { recursive: true });
    writeFileSync(reportPath, renderCompletionReport(ledger));
    return ledger;
  }
  if (command === "files") {
    const selfPath = normalizePath(relative(root, resolve(args.out)));
    return { schema: 1, files: buildFileInventory(root, selfPath) };
  }
  if (command === "docs") {
    const candidates = readTrackedFiles(root)
      .filter((path) => path.endsWith(".md") && !path.startsWith("docs/audit/"))
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
  if (command === "verify") {
    if (!args.audit || !args.scope) {
      throw new Error("verify command needs --audit <path> --scope <scope>");
    }
    return verifyAudit(root, args.audit, args.scope);
  }
  throw new Error(`unsupported command: ${command}`);
}

async function main(argv) {
  const args = parseArgs(argv);
  const verifyOnly = args.command === "verify-open-prs";
  if (!args.command || !args.root || (!verifyOnly && !args.out)) {
    throw new Error("usage: completion-audit <command> --root <repo> [--out <path>]");
  }
  const root = resolve(args.root);
  const result = await runCommand(args.command, root, args);
  if (verifyOnly) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  const output = resolve(args.out);
  writeJson(output, result);
  if (args.command === "verify" && !result.passed) {
    console.error(`audit verification failed with ${result.errors.length} error(s)`);
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
}
