import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_FILE_INVENTORY_PATH = "docs/audit/2026-07-14/repository-files.json";

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

function gitRead(name, path, args, encoding = "utf8") {
  try {
    return execFileSync("git", ["-C", path, ...args], {
      encoding,
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
  chromaKey: "requirement-5698f215032c0f0e",
  lutImport: "requirement-308f50408571c49c",
  voiceIsolation: "requirement-f686c7eff856124f",
  generationTools: "requirement-ae3876aa43f16739",
  generationContract: "requirement-dd0fe05957bd7e1e",
  elevenLabs: "requirement-9f64c383bdf1c254",
  generationError: "requirement-1f36e817d0ba77ea",
  manifest: "requirement-6a74e3f1924c4e22",
  projectBundle: "requirement-9a5fece84fd52934",
  agentUndo: "requirement-d8779330e21693ad",
  toolCalls: "requirement-34e3026d3b05e9ee",
  telemetry: "requirement-923fa228087ad4f3",
  telemetryDecision: "requirement-681025f8d7d4facf",
  playback: "requirement-10e720a4f5ddd734",
  playbackLifecycle: "requirement-9bceb67f73cd51d4",
});

const CONTROL = Object.freeze({
  chromaKey: Object.freeze([
    "control-record-f8fda6ae2f426fe7",
    "control-record-46a4a8652371f465",
    "control-record-1dda462442994c48",
    "control-record-15ec45b9eaa7d580",
  ]),
  exportCancel: Object.freeze(["control-record-8592a780adc50cb8"]),
  generation: Object.freeze(["control-record-d53ae6c2dec481d0"]),
  searchIndex: Object.freeze(["control-record-64640989bd95e214"]),
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
      "requirement-681025f8d7d4facf"
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
      "requirement-681025f8d7d4facf"
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
      "requirement-681025f8d7d4facf"
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
      "requirement-681025f8d7d4facf"
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
      "control-record-8592a780adc50cb8"
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
      "requirement-9f64c383bdf1c254"
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
      "requirement-03ec8ed1077fbfd6"
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
      "requirement-d8779330e21693ad",
      "requirement-681025f8d7d4facf"
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
      "requirement-681025f8d7d4facf"
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
      "requirement-681025f8d7d4facf"
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
      "requirement-03ec8ed1077fbfd6",
      "requirement-d8779330e21693ad"
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
      "requirement-5698f215032c0f0e"
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
      "requirement-5698f215032c0f0e"
    ],
    "linkedControlIds": [
      "control-record-1dda462442994c48"
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
      "requirement-5698f215032c0f0e"
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
      "requirement-308f50408571c49c"
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
      "requirement-5698f215032c0f0e"
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
      "control-record-f8fda6ae2f426fe7"
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
      "control-record-7b6a6c4db4520595",
      "control-record-46a4a8652371f465",
      "control-record-1dda462442994c48"
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
      "requirement-5698f215032c0f0e"
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
      "requirement-308f50408571c49c"
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
      "requirement-5698f215032c0f0e"
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
      "control-record-558afc9b67a29ef9"
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
      "control-record-5808716793cc1f0f"
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
      "control-record-51cd2ee5dd1aecc9"
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
      "control-record-8592a780adc50cb8"
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
      "control-record-f62dd7b0a91d2321",
      "control-record-8592a780adc50cb8"
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
      "control-record-f62dd7b0a91d2321",
      "control-record-8592a780adc50cb8"
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
      "requirement-9f64c383bdf1c254"
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
      "requirement-1f36e817d0ba77ea"
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
      "requirement-9f64c383bdf1c254"
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
      "requirement-9f64c383bdf1c254"
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
      "requirement-cce7bd16986c8437"
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
      "requirement-9f64c383bdf1c254"
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
      "requirement-9f64c383bdf1c254"
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
    .replaceAll("|", "\\|")
    .replaceAll("\n", "\\n");
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

const DOCUMENT_REQUIREMENT_STATUSES = new Set([
  "complete",
  "incomplete",
  "contradicted",
  "obsolete",
  "duplicate",
]);

export const DOCUMENT_GAP_GROUPS = Object.freeze([
  "accessibility-polish",
  "agent-settings-generation",
  "command-contracts",
  "data-safety",
  "documentation",
  "home-shell",
  "inspector-text-keyframes",
  "media-library",
  "media-render-playback-export",
  "preview-timeline",
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

function readDocumentAuditFile(auditDirectory, name, collectionName, errors) {
  const path = resolve(auditDirectory, name);
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
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
  if (!lstatSync(resolved.absolute).isFile()) return { status: "not-file", ...resolved };
  if (!trackedPaths.has(resolved.normalized)) return { status: "untracked", ...resolved };
  return { status: "valid", ...resolved };
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function sourceDeclaresSymbol(path, source, symbol) {
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(symbol)) return false;
  const escaped = escapeRegExp(symbol);
  if (path.endsWith(".rs")) {
    return new RegExp(
      `\\b(?:fn|struct|enum|trait|type|const|static|mod)\\s+${escaped}\\b|\\bmacro_rules!\\s*${escaped}\\b`,
    ).test(source);
  }
  if (/\\.(?:[cm]?[jt]sx?)$/.test(path)) {
    const declaration = new RegExp(
      `\\b(?:function|class|interface|type|enum|const|let|var)\\s+${escaped}\\b`,
    );
    const method = new RegExp(
      `(?:^|\\n)\\s*(?:(?:public|private|protected|static|async|get|set|readonly|abstract|override)\\s+)*${escaped}\\s*(?:<[^>\\n]*>)?\\s*\\([^;]{0,2000}?\\)\\s*(?::[^={;\\n]+)?\\s*\\{`,
    );
    return declaration.test(source) || method.test(source);
  }
  if (path.endsWith(".py")) {
    return new RegExp(`(?:^|\\n)\\s*(?:async\\s+)?(?:def|class)\\s+${escaped}\\b`).test(source);
  }
  return new RegExp(
    `\\b(?:fn|function|struct|enum|class|interface|type|const|let|var|trait|mod)\\s+${escaped}\\b`,
  ).test(source);
}

function sourceDeclaresTest(path, source, name) {
  if (path.endsWith(".rs")) {
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) return false;
    return new RegExp(`\\bfn\\s+${escapeRegExp(name)}\\s*\\(`).test(source);
  }
  const escaped = escapeRegExp(name);
  return new RegExp(`\\b(?:test|it)\\s*\\(\\s*["'\`]${escaped}["'\`]`).test(source);
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
  if (file.status !== "valid") {
    pushVerificationError(errors, "invalid-implementation-evidence", `implementation path is not a tracked file: ${path}`, candidateId);
    return false;
  }
  const source = readFileSync(file.absolute, "utf8");
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
  if (file.status !== "valid") {
    pushVerificationError(errors, "invalid-test-evidence", `test path is not a tracked file: ${path}`, candidateId);
    return false;
  }
  const source = readFileSync(file.absolute, "utf8");
  if (!sourceDeclaresTest(path, source, name)) {
    pushVerificationError(errors, "test-name-missing", `test name is absent: ${path}#${name}`, candidateId);
    return false;
  }
  return true;
}

function validateRuntimeEvidence(root, trackedPaths, evidence, candidateId, errors) {
  const commit = /^commit:([0-9a-f]{40})$/.exec(evidence);
  if (commit) {
    try {
      execFileSync("git", ["-C", root, "cat-file", "-e", `${commit[1]}^{commit}`], {
        stdio: "ignore",
      });
      execFileSync("git", ["-C", root, "merge-base", "--is-ancestor", commit[1], "HEAD"], {
        stdio: "ignore",
      });
      return true;
    } catch {
      pushVerificationError(errors, "runtime-commit-invalid", `runtime commit is absent or not an ancestor: ${commit[1]}`, candidateId);
      return false;
    }
  }

  const hash = /^hash:([^#\n]+)#sha256=([0-9a-f]{64})$/.exec(evidence);
  if (hash) {
    const file = trackedRegularFile(root, trackedPaths, hash[1]);
    const actual = file.status === "valid"
      ? createHash("sha256").update(readFileSync(file.absolute)).digest("hex")
      : null;
    if (actual === hash[2]) return true;
    pushVerificationError(errors, "runtime-hash-invalid", `runtime hash cannot be verified: ${hash[1]}`, candidateId);
    return false;
  }

  const receipt = /^receipt:([^#\n]+)#sha256=([0-9a-f]{64})#exit=0$/.exec(evidence);
  if (receipt) {
    const file = trackedRegularFile(root, trackedPaths, receipt[1]);
    if (file.status === "valid") {
      const bytes = readFileSync(file.absolute);
      const digest = createHash("sha256").update(bytes).digest("hex");
      try {
        const parsed = JSON.parse(bytes.toString("utf8"));
        if (digest === receipt[2] && parsed?.exitCode === 0) return true;
      } catch {
        // The common error below is deliberately stable for malformed receipts.
      }
    }
    pushVerificationError(errors, "runtime-receipt-invalid", `runtime receipt cannot be verified: ${receipt[1]}`, candidateId);
    return false;
  }

  pushVerificationError(
    errors,
    "invalid-runtime-evidence",
    `runtime evidence must be a verifiable commit, hash, or exit receipt: ${evidence}`,
    candidateId,
  );
  return false;
}

export function verifyAudit(root, auditDir, scope) {
  if (scope !== "documents") {
    throw new Error(`unsupported verification scope: ${scope}`);
  }
  const repositoryRoot = resolve(root);
  const auditDirectory = resolve(repositoryRoot, auditDir);
  const errors = [];
  const auditRelative = relative(repositoryRoot, auditDirectory);
  const auditOutsideRoot = auditRelative === ".."
    || auditRelative.startsWith(`..${sep}`)
    || isAbsolute(auditRelative);
  if (auditOutsideRoot) {
    pushVerificationError(
      errors,
      "audit-directory-outside-root",
      "audit directory must remain inside the repository root",
    );
  }
  const candidates = auditOutsideRoot
    ? []
    : readDocumentAuditFile(
      auditDirectory,
      "document-candidates.json",
      "candidates",
      errors,
    );
  const records = auditOutsideRoot
    ? []
    : readDocumentAuditFile(
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
      } else if (sourceFile.status !== "valid") {
        pushVerificationError(errors, "invalid-candidate", `candidate source is not a tracked file: ${candidate.path}`, candidate.id);
      } else {
        const derived = extractDocumentCandidates(
          candidate.path,
          readFileSync(sourceFile.absolute, "utf8"),
        ).find((item) => item.line === candidate.line);
        if (!derived) {
          pushVerificationError(errors, "candidate-source-signal-missing", `candidate signal is absent at ${candidate.path}:${candidate.line}`, candidate.id);
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
      for (const evidence of Array.isArray(record.runtimeEvidence) ? record.runtimeEvidence : []) {
        validateRuntimeEvidence(repositoryRoot, trackedPaths, evidence, candidateId, errors);
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
