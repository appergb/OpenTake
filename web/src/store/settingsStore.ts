/**
 * App-level settings (UI preferences only — never editing truth). Persisted to
 * localStorage so they survive restarts: the default folder the import
 * dialog opens to, and the BYOK provider choice. Only the *provider choice* is
 * stored here — the API key itself never touches this store or localStorage; it
 * lives in the OS keychain via the `secret_*` Tauri commands (see
 * `lib/api.ts` / `src-tauri/src/secret.rs`).
 */

import { create } from "zustand";
import { isTauri, setProxyPlaybackEnabled as setProxyPlaybackEnabledNative } from "../lib/api";
import { t } from "../i18n";
import { useEditorUiStore } from "./uiStore";

export type ByokProvider =
  | "codex"
  | "anthropic"
  | "fal"
  | "replicate"
  | "openai"
  | "elevenlabs"
  | "google";
export type WindowSizeOpt = "standard" | "compact";

const LS = {
  defaultImportFolder: "defaultImportFolder",
  byokProvider: "byokProvider",
  windowSize: "windowSize",
  proxyPlaybackEnabled: "proxyPlaybackEnabled",
} as const;

function isLegacyThemeKey(key: string): boolean {
  return /^(?:opentake[._:-])?theme(?:[._:-]*(?:v(?:ersion)?|version)?[._:-]*\d+)?$/i.test(key);
}

/** Remove obsolete theme preferences; the only shipped token set is dark. */
export function migrateLegacyThemePreferences(): void {
  if (typeof localStorage !== "undefined") {
    const legacyKeys = Array.from({ length: localStorage.length }, (_, index) => localStorage.key(index))
      .filter((key): key is string => key !== null && isLegacyThemeKey(key));
    legacyKeys.forEach((key) => localStorage.removeItem(key));
  }
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = "dark";
  }
}
function loadString(key: string): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(key);
}
function loadProvider(): ByokProvider {
  const v = loadString(LS.byokProvider);
  return v === "codex" ||
    v === "fal" ||
    v === "replicate" ||
    v === "openai" ||
    v === "elevenlabs" ||
    v === "google"
    ? v
    : "anthropic";
}
function loadWindowSize(): WindowSizeOpt {
  if (typeof localStorage === "undefined") return "standard";
  return localStorage.getItem(LS.windowSize) === "compact" ? "compact" : "standard";
}
function loadProxyPlaybackEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(LS.proxyPlaybackEnabled) === "true";
}
function persist(key: string, value: string | null) {
  if (typeof localStorage === "undefined") return;
  if (value === null) localStorage.removeItem(key);
  else localStorage.setItem(key, value);
}

interface SettingsState {
  defaultImportFolder: string | null;
  byokProvider: ByokProvider;
  windowSize: WindowSizeOpt;
  proxyPlaybackEnabled: boolean;
  setDefaultImportFolder: (path: string | null) => void;
  setByokProvider: (provider: ByokProvider) => void;
  setWindowSize: (size: WindowSizeOpt) => Promise<void>;
  setProxyPlaybackEnabled: (enabled: boolean) => void;
}

let windowSizeRequest = 0;
let windowSizeQueue = Promise.resolve();

function enqueueWindowResize(size: WindowSizeOpt): Promise<void> {
  const operation = windowSizeQueue.then(() => applyWindowSize(size));
  windowSizeQueue = operation.catch(() => undefined);
  return operation;
}

migrateLegacyThemePreferences();

export const useSettingsStore = create<SettingsState>((set, get) => ({
  defaultImportFolder: loadString(LS.defaultImportFolder),
  byokProvider: loadProvider(),
  windowSize: loadWindowSize(),
  proxyPlaybackEnabled: loadProxyPlaybackEnabled(),
  setDefaultImportFolder: (defaultImportFolder) => {
    persist(LS.defaultImportFolder, defaultImportFolder);
    set({ defaultImportFolder });
  },
  setByokProvider: (byokProvider) => {
    persist(LS.byokProvider, byokProvider);
    set({ byokProvider });
  },
  setWindowSize: async (windowSize) => {
    const previousWindowSize = get().windowSize;
    const request = ++windowSizeRequest;
    set({ windowSize });
    return enqueueWindowResize(windowSize).then(
      () => {
        if (request !== windowSizeRequest) return;
        persist(LS.windowSize, windowSize);
      },
      (error) => {
        if (request !== windowSizeRequest) return;
        set({ windowSize: previousWindowSize });
        persist(LS.windowSize, previousWindowSize);
        const message = error instanceof Error ? error.message : String(error);
        useEditorUiStore.getState().pushToast(t("settings.windowSizeFailed", { error: message }));
      },
    );
  },
  setProxyPlaybackEnabled: (proxyPlaybackEnabled) => {
    persist(LS.proxyPlaybackEnabled, String(proxyPlaybackEnabled));
    void setProxyPlaybackEnabledNative(proxyPlaybackEnabled);
    set({ proxyPlaybackEnabled });
  },
}));

/** Apply the window size (width: 1600x1000 or 1066x666 centered) dynamically in Tauri. */
export async function applyWindowSize(size: WindowSizeOpt): Promise<void> {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const { LogicalSize, LogicalPosition } = await import("@tauri-apps/api/dpi");
  const win = getCurrentWindow();
  const factor = await win.scaleFactor();

  const targetWidth = size === "compact" ? 1066 : 1600;
  const targetHeight = size === "compact" ? 666 : 1000;

  const physicalSize = await win.innerSize();
  const logicalSize = physicalSize.toLogical(factor);

  const physicalPos = await win.outerPosition();
  const logicalPos = physicalPos.toLogical(factor);

  const dw = logicalSize.width - targetWidth;
  const dh = logicalSize.height - targetHeight;

  const newX = logicalPos.x + dw / 2;
  const newY = logicalPos.y + dh / 2;

  await win.setSize(new LogicalSize(targetWidth, targetHeight));
  try {
    await win.setPosition(new LogicalPosition(newX, newY));
  } catch (error) {
    try {
      await win.setSize(new LogicalSize(logicalSize.width, logicalSize.height));
      await win.setPosition(new LogicalPosition(logicalPos.x, logicalPos.y));
    } catch (rollbackError) {
      console.error("Failed to restore window geometry:", rollbackError);
    }
    throw error;
  }
}

export function initWindowSize(): void {
  void enqueueWindowResize(useSettingsStore.getState().windowSize).catch((error) => {
    console.error("Failed to apply window size:", error);
  });
}

export function initProxyPlayback(): void {
  void setProxyPlaybackEnabledNative(useSettingsStore.getState().proxyPlaybackEnabled);
}
