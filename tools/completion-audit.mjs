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
import { dirname, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const DEFAULT_FILE_INVENTORY_PATH = "docs/audit/2026-07-14/repository-files.json";

export function normalizePath(value) {
  return value.replaceAll("\\", "/").replace(/^\.\//, "");
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

export function captureDirtyCheckout({ name, path, remote = "origin" }) {
  if (![name, path, remote].every((value) => typeof value === "string" && value)) {
    throw new Error("captureDirtyCheckout requires non-empty name, path, and remote strings");
  }
  const rawStatus = gitRead(
    name,
    path,
    ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    null,
  );
  const paths = parsePorcelain(name, rawStatus)
    .sort((left, right) => (
      left.path.localeCompare(right.path, "en")
      || (left.source ?? "").localeCompare(right.source ?? "", "en")
      || left.status.localeCompare(right.status, "en")
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
  return {
    name,
    repository: gitText(name, path, ["remote", "get-url", remote]),
    remote,
    status: paths.length === 0 ? "clean" : "dirty",
    head: assertFullSha(name, "HEAD", gitText(name, path, ["rev-parse", "HEAD"])),
    tree: assertFullSha(name, "HEAD tree", gitText(name, path, ["rev-parse", "HEAD^{tree}"])),
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

const CHROMA_PATHS = new Set([
  "Metal/ChromaKey.metal",
  "Sources/PalmierPro/Compositing/EffectRegistry.swift",
  "Sources/PalmierPro/Compositing/FrameRenderer.swift",
  "Sources/PalmierPro/Editor/EditorWindowController.swift",
  "Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ChromaKey.swift",
  "Sources/PalmierPro/Inspector/InspectorView.swift",
  "Sources/PalmierPro/Inspector/Tabs/AdjustTab.swift",
  "Sources/PalmierPro/Preview/ChromaKeySamplerOverlayView.swift",
  "Sources/PalmierPro/Preview/PreviewContainerView.swift",
  "Sources/PalmierPro/Preview/PreviewHitTester.swift",
]);

const AUDIO_SCRUB_PATHS = new Set([
  "Sources/PalmierPro/Audio/AudioMeter.swift",
  "Sources/PalmierPro/Audio/AudioMeterView.swift",
  "Sources/PalmierPro/Editor/EditorView.swift",
  "Sources/PalmierPro/Preview/ScrubAudioEngine.swift",
  "Sources/PalmierPro/Preview/ScrubAudioOutput.swift",
]);

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

function topic({
  disposition,
  behavior,
  equivalent,
  requirements = [],
  controls = [],
}) {
  return {
    disposition,
    behavior,
    openTakeEquivalent: equivalent,
    linkedRequirementIds: requirements,
    linkedControlIds: controls,
  };
}

function classifyPalmierProductPath(path) {
  if (path.endsWith("AgentService.swift")) {
    return topic({
      disposition: "cloud-specific",
      behavior: "Changes hosted-chat model selection and in-app activation accounting.",
      equivalent: "crates/opentake-agent/src/chat (BYOK architecture); no hosted activation sink",
      requirements: [REQUIREMENT.telemetryDecision],
    });
  }
  if (path.endsWith("BackendConfig.swift")) {
    return topic({
      disposition: "equivalent",
      behavior: "Moves shared hosted backend configuration without changing its content.",
      equivalent: "crates/opentake-gen/src/keys.rs and provider configuration",
      requirements: [REQUIREMENT.elevenLabs],
    });
  }
  if (path.endsWith("BackendError.swift")) {
    return topic({
      disposition: "equivalent",
      behavior: "Adds provider-neutral backend error decoding for storage and generation requests.",
      equivalent: "crates/opentake-gen/src/error.rs",
      requirements: [REQUIREMENT.generationError],
    });
  }
  if (path.endsWith("ToolExecutor+Generate.swift")) {
    return topic({
      disposition: "requires-reconciliation",
      behavior: "Adds sourceMediaRef and targetLanguage validation for voice isolation and dubbing tool calls.",
      equivalent: "crates/opentake-agent/src/mcp/dispatch.rs; crates/opentake-gen/src/provider/elevenlabs.rs",
      requirements: [REQUIREMENT.elevenLabs, REQUIREMENT.generationTools],
      controls: CONTROL.generation,
    });
  }
  if (path.endsWith("ToolExecutor+Import.swift")) {
    return topic({
      disposition: "requirement-needed:manifest-metadata-batching",
      behavior: "Routes imported asset metadata through a batched manifest updater.",
      equivalent: "crates/opentake-core/src/session.rs::import_media_file_checked (direct manifest mutation)",
      requirements: [REQUIREMENT.manifest],
    });
  }
  if (path.endsWith("TitleBarView.swift")) {
    return topic({
      disposition: "requirement-needed:cancellable-export-queue",
      behavior: "Adds export queue activity/history status to the title bar.",
      equivalent: "web/src/components/shell/TitleBar.tsx (export entry without queue history)",
      controls: ["control-record-51cd2ee5dd1aecc9"],
    });
  }
  if (path.endsWith("MediaResolver.swift")) {
    return topic({
      disposition: "equivalent",
      behavior: "Makes media resolution reads observe queued-export cancellation.",
      equivalent: "src-tauri/src/export.rs operation-scoped media cancellation",
      controls: CONTROL.exportCancel,
    });
  }
  if (CHROMA_PATHS.has(path) || /ChromaKey/.test(path)) {
    return topic({
      disposition: "portable",
      behavior: "Corrects chroma-key compositing and adds preview eyedropper sampling.",
      equivalent: "crates/opentake-domain/src/grade.rs; crates/opentake-render/src/gpu/shader.wgsl; src-tauri/src/commands.rs; web/src/components/inspector/Inspector.tsx",
      requirements: [REQUIREMENT.chromaKey],
      controls: CONTROL.chromaKey,
    });
  }
  if (/LUTLoader/.test(path)) {
    return topic({
      disposition: "portable",
      behavior: "Extends .cube LUT parsing to 65-point tables.",
      equivalent: "crates/opentake-domain/src/grade.rs; crates/opentake-render/src/gpu/shader.wgsl",
      requirements: [REQUIREMENT.lutImport],
    });
  }
  if (AUDIO_SCRUB_PATHS.has(path) || /AudioMeter|ScrubAudio/.test(path)) {
    return topic({
      disposition: "requirement-needed:audio-scrub-meter",
      behavior: "Adds non-blocking timeline audio scrubbing and a low-invalidation stereo meter.",
      equivalent: "src-tauri/src/playback/audio.rs; web/src/components/preview/Preview.tsx (no confirmed meter/scrub-audio parity)",
    });
  }
  if (path.includes("/Export/") || path.endsWith("ToolExecutor+Export.swift")) {
    return topic({
      disposition: "requirement-needed:cancellable-export-queue",
      behavior: "Replaces single export coordination with a cancellable queued export lifecycle and UI.",
      equivalent: "src-tauri/src/export.rs; web/src/components/shell/ExportDialog.tsx (single in-flight export with cancellation)",
      controls: CONTROL.exportCancel,
    });
  }
  if (path.includes("/Generation/") || path.endsWith("AIEditTab.swift")
      || path.endsWith("TimelineView+AIEditMenu.swift")
      || path.endsWith("EditorViewModel+AIEdit.swift")
      || path.endsWith("TranscriptionBackend.swift")
      || path.endsWith("CapsuleButton.swift")
      || path.endsWith("HoverHighlight.swift")) {
    return topic({
      disposition: "portable",
      behavior: "Adds voice isolation/dubbing inputs and splits rerun, seed, upscale, and audio-transform submission flows.",
      equivalent: "crates/opentake-gen; crates/opentake-agent/src/mcp/dispatch.rs; web/src/components/media/MediaPanel.tsx",
      requirements: [
        REQUIREMENT.voiceIsolation,
        REQUIREMENT.elevenLabs,
        REQUIREMENT.generationTools,
        REQUIREMENT.generationContract,
      ],
      controls: CONTROL.generation,
    });
  }
  if (path.includes("/Backend/") || path.endsWith("BackendConfig.swift")
      || path.endsWith("BackendStorage.swift")) {
    return topic({
      disposition: "cloud-specific",
      behavior: "Centralizes hosted-service configuration, storage, and backend error handling.",
      equivalent: "crates/opentake-gen/src/provider; platform key/config storage (BYOK architecture differs)",
    });
  }
  if (path.includes("/Agent/MCP/")) {
    return topic({
      disposition: "requirement-needed:agent-session-telemetry",
      behavior: "Tracks activated MCP sessions and consolidates project-tool registration.",
      equivalent: "crates/opentake-agent/src/mcp/server.rs; src-tauri/src/mcp.rs",
      requirements: [REQUIREMENT.telemetry],
    });
  }
  if (path.endsWith("ToolExecutor+Timeline.swift") || path.endsWith("UndoToolTests.swift")) {
    return topic({
      disposition: "portable",
      behavior: "Tightens Agent edit transaction boundaries so undo groups match tool calls.",
      equivalent: "crates/opentake-agent/src/mcp/dispatch.rs; crates/opentake-core/src/session.rs",
      requirements: [REQUIREMENT.agentUndo],
    });
  }
  if (path.includes("/Agent/Tools/") || path.endsWith("AgentService.swift")) {
    return topic({
      disposition: "requirement-needed:agent-project-tool-consolidation",
      behavior: "Consolidates project management tools, records tool calls, and updates hosted chat instructions/model selection.",
      equivalent: "crates/opentake-agent/src/mcp/dispatch.rs; crates/opentake-agent/src/chat; web/src/store/chatStore.ts",
      requirements: [REQUIREMENT.toolCalls],
    });
  }
  if (path.includes("/Search/")) {
    return topic({
      disposition: "requires-reconciliation",
      behavior: "Moves visual-search preflight off the UI actor and makes index work more deterministic.",
      equivalent: "crates/opentake-media/src/search; src-tauri/src/search.rs; web/src/components/media/MediaSearch.tsx",
      controls: CONTROL.searchIndex,
    });
  }
  if (path.endsWith("MediaManifest.swift") || path.endsWith("MediaResolver.swift")
      || path.endsWith("EditorViewModel+Folders.swift")
      || path.endsWith("EditorViewModel+MediaLibrary.swift")) {
    return topic({
      disposition: "portable",
      behavior: "Batches manifest metadata updates and restores persisted star/favorite history.",
      equivalent: "crates/opentake-domain/src/media.rs; crates/opentake-project/src/bundle.rs; src-tauri/src/library.rs",
      requirements: [REQUIREMENT.manifest],
    });
  }
  if (path.endsWith("VideoProject.swift") || path.includes("/App/")
      || path.endsWith("EditorWindowController.swift")) {
    return topic({
      disposition: "requires-reconciliation",
      behavior: "Consolidates project open/manage/close persistence and application lifecycle handling.",
      equivalent: "crates/opentake-core/src/session.rs; crates/opentake-project/src/bundle.rs; web/src/store/projectActions.ts",
      requirements: [REQUIREMENT.projectBundle],
    });
  }
  if (path.includes("/Telemetry/") || path.endsWith("Log.swift")) {
    return topic({
      disposition: "platform-specific",
      behavior: "Reduces production logging overhead and adds hosted telemetry counters.",
      equivalent: "Rust tracing/logging and optional platform telemetry; no Swift runtime port",
      requirements: [REQUIREMENT.telemetry],
    });
  }
  if (path.endsWith("MLXRuntime.swift") || path.includes("/Audio/Analysis/")
      || path.endsWith("AudioEnhancer.swift")) {
    return topic({
      disposition: "platform-specific",
      behavior: "Moves model preflight off the main actor and drains Apple MLX work before termination.",
      equivalent: "crates/opentake-media/src/ort_worker (cross-platform ONNX worker architecture)",
    });
  }
  if (path.endsWith("EditorViewModel.swift") || path.endsWith("VideoEngine.swift")
      || path.endsWith("AppTheme.swift")) {
    return topic({
      disposition: "requires-reconciliation",
      behavior: "Carries shared state/UI plumbing for chroma sampling, audio scrub/meter, and manifest updates.",
      equivalent: "src-tauri/src/playback; web/src/components/preview/Preview.tsx; web/src/components/inspector/Inspector.tsx",
      requirements: [REQUIREMENT.chromaKey],
      controls: CONTROL.chromaKey,
    });
  }
  if (path.includes("/UI/") || path.includes("/Inspector/") || path.includes("/Editor/")) {
    return topic({
      disposition: "requirement-needed:upstream-editor-ui-delta",
      behavior: "Adjusts editor presentation and interaction state for the refreshed upstream workflows.",
      equivalent: "web/src/components/inspector; web/src/components/preview; web/src/components/shell",
    });
  }
  return topic({
    disposition: "requirement-needed:upstream-behavior-delta",
    behavior: "Changes an upstream product behavior that has no exact current completion-ledger record.",
    equivalent: "No exact OpenTake equivalent confirmed during source capture",
  });
}

function classifyPalmierPath(path) {
  if (path.startsWith("Tests/PalmierProTests/")) {
    let productPath = path
      .replace(/^Tests\/PalmierProTests\/Rendering\//, "Sources/PalmierPro/Compositing/")
      .replace(/^Tests\/PalmierProTests\/Audio\//, "Sources/PalmierPro/Audio/")
      .replace(/^Tests\/PalmierProTests\/Export\//, "Sources/PalmierPro/Export/")
      .replace(/^Tests\/PalmierProTests\/Search\//, "Sources/PalmierPro/Search/")
      .replace(/^Tests\/PalmierProTests\/Media\//, "Sources/PalmierPro/Models/")
      .replace(/^Tests\/PalmierProTests\/Project\//, "Sources/PalmierPro/Project/")
      .replace(/^Tests\/PalmierProTests\/Agent\//, "Sources/PalmierPro/Agent/Tools/")
      .replace(/^Tests\/PalmierProTests\/Timeline\//, "Sources/PalmierPro/Editor/")
      .replace(/^Tests\/PalmierProTests\/Utilities\//, "Sources/PalmierPro/Utilities/");
    if (/ManifestMetadataTests/.test(path)) {
      productPath = "Sources/PalmierPro/Models/MediaManifest.swift";
    } else if (/ProjectClosePersistenceTests/.test(path)) {
      productPath = "Sources/PalmierPro/Project/VideoProject.swift";
    } else if (/CompositorRenderTests|HistogramTests|ClipMutationsTests/.test(path)) {
      productPath = "Metal/ChromaKey.metal";
    } else if (/MLXOperationGateTests/.test(path)) {
      productPath = "Sources/PalmierPro/Utilities/MLXRuntime.swift";
    } else if (/ExportProjectToolTests/.test(path)) {
      productPath = "Sources/PalmierPro/Agent/Tools/ToolExecutor+Export.swift";
    } else if (/ManageTracksTests/.test(path)) {
      productPath = "Sources/PalmierPro/Agent/Tools/ToolExecutor+Clips.swift";
    } else if (/ManageProjectToolTests/.test(path)) {
      productPath = "Sources/PalmierPro/Agent/Tools/ToolExecutor+Projects.swift";
    } else if (/AnalyticsSessionActivationTests/.test(path)) {
      productPath = "Sources/PalmierPro/Agent/MCP/MCPService.swift";
    }
    const product = classifyPalmierProductPath(productPath);
    return {
      ...product,
      disposition: "test-doc-only",
      behavior: `Regression coverage: ${product.behavior}`,
      openTakeEquivalent: `${product.openTakeEquivalent}; corresponding Rust/TypeScript tests`,
    };
  }
  if (path.startsWith("docs/readme/")) {
    return topic({
      disposition: "test-doc-only",
      behavior: "Updates localized upstream repository analytics artwork.",
      equivalent: "No OpenTake runtime equivalent; upstream documentation only",
    });
  }
  if (path === "README.md" || path === "appcast.xml"
      || path === "Sources/PalmierPro/Resources/Info.plist") {
    return topic({
      disposition: "obsolete",
      behavior: "Updates Palmier-only release, appcast, version, or repository analytics metadata.",
      equivalent: "OpenTake release metadata is independently versioned and signed",
    });
  }
  if (path === "Package.swift" || path === "scripts/bundle.sh") {
    return topic({
      disposition: "requires-reconciliation",
      behavior: "Adds Swift package traits and release-time speech/telemetry/signing feature composition.",
      equivalent: "Cargo feature tables; src-tauri packaging; OpenTake release workflows",
      requirements: [REQUIREMENT.telemetryDecision],
    });
  }
  if (path === ".swift-version" || path === "Package.resolved"
      || path === ".github/workflows/ci.yml") {
    return topic({
      disposition: "platform-specific",
      behavior: "Updates Swift toolchain, dependency, CI, version, or macOS bundle metadata.",
      equivalent: "Cargo.toml/Cargo.lock; .github/workflows/ci.yml; src-tauri/tauri.conf.json",
    });
  }
  return classifyPalmierProductPath(path);
}

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

function readOpenPullRequests() {
  const repository = "appergb/OpenTake";
  const args = [
    "pr", "list", "--repo", repository, "--state", "open",
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
  items.sort((left, right) => left.number - right.number);
  return {
    repository,
    state: "open",
    transport: "gh-live-readback",
    command: `gh ${args.join(" ")}`,
    count: items.length,
    items,
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
    .sort((left, right) => left.localeCompare(right, "en"));
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

export function buildSourceEvidence({ root, palmierPath, canonicalPath }) {
  assertPinnedRef("target", root, SOURCE_PINS.targetStart, SOURCE_PINS.targetStart, SOURCE_PINS.targetStartTree);
  assertPinnedRef("target", root, "origin/main", SOURCE_PINS.targetMain, SOURCE_PINS.targetMainTree);
  assertPinnedRef(
    "Palmier Pro",
    palmierPath,
    SOURCE_PINS.palmierPrevious,
    SOURCE_PINS.palmierPrevious,
    SOURCE_PINS.palmierPreviousTree,
  );
  assertPinnedRef("Palmier Pro", palmierPath, "origin/main", SOURCE_PINS.palmierMain, SOURCE_PINS.palmierMainTree);
  for (const [remote, pin] of Object.entries(SOURCE_PINS.forkMains)) {
    assertPinnedRef(remote, root, `${remote}/main`, pin.sha, pin.tree);
  }
  assertPinnedRef("canonical dirty checkout", canonicalPath, "HEAD", SOURCE_PINS.canonicalHead, SOURCE_PINS.canonicalTree);

  const target = captureGitSource({
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

  const palmier = captureGitSource({
    name: "Palmier Pro refreshed main",
    path: palmierPath,
    base: SOURCE_PINS.palmierPrevious,
    head: SOURCE_PINS.palmierMain,
    remote: "origin",
    requireClean: true,
    expectChanges: true,
  });
  palmier.changedPaths = palmier.changedPaths.map((record) => ({
    ...record,
    ...classifyPalmierPath(record.path),
  }));

  const branches = Object.keys(SOURCE_PINS.forkMains)
    .sort((left, right) => left.localeCompare(right, "en"))
    .flatMap((remote) => branchIndexForRemote(root, remote));
  if (branches.length !== 21) {
    throw new Error(`fork branch index expected 21 non-main heads, found ${branches.length}`);
  }
  const relevant = branches.filter(({ separatelyRelevant }) => separatelyRelevant);
  const relevantSources = relevant.map((branch) => {
    const source = captureGitSource({
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

  const canonical = captureDirtyCheckout({
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

  return {
    schema: 1,
    auditDate: "2026-07-14",
    generation: {
      deterministicOrdering: "UTF-8 normalized path/ref ordering",
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
    openPullRequests: readOpenPullRequests(),
    sources: [
      target,
      palmier,
      ...Object.keys(SOURCE_PINS.forkMains)
        .sort((left, right) => left.localeCompare(right, "en"))
        .map((remote) => integratedForkMainSource(root, remote)),
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
}

function markdown(value) {
  return String(value ?? "—")
    .replaceAll("|", "\\|")
    .replaceAll("\n", "\\n");
}

export function renderSourceReport(evidence) {
  const lines = [
    "# Immutable upstream and downstream source audit",
    "",
    `Audit date: ${evidence.auditDate}`,
    "",
    "## Preservation and live readback",
    "",
    `- Serialized fetches: ${evidence.fetchEvidence.serializedOrder.map((command) => `\`${command}\``).join("; ")}.`,
    `- Target porcelain SHA-256: \`${evidence.fetchEvidence.target.pre}\` before and after fetch.`,
    `- Canonical porcelain SHA-256: \`${evidence.fetchEvidence.canonical.pre}\` before and after fetch.`,
    `- Palmier porcelain SHA-256: \`${evidence.fetchEvidence.palmier.pre}\` before and after fetch.`,
    `- Live target open-PR readback: **${evidence.openPullRequests.count}** from \`${evidence.openPullRequests.command}\`.`,
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
      "| Git status | Path | Disposition | Behavior | OpenTake equivalent | Requirement IDs | Control IDs |",
      "|---|---|---|---|---|---|---|",
    );
    for (const change of source.changedPaths) {
      const path = change.destination ? `${change.path} → ${change.destination}` : change.path;
      lines.push(`| ${change.status} | ${markdown(path)} | ${markdown(change.disposition)} | ${markdown(change.behavior)} | ${markdown(change.openTakeEquivalent)} | ${markdown(change.linkedRequirementIds.join(", ") || "none")} | ${markdown(change.linkedControlIds.join(", ") || "none")} |`);
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
