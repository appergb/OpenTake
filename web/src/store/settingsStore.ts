/**
 * App-level settings (UI preferences only — never editing truth). Persisted to
 * localStorage so they survive restarts: theme, the default folder the import
 * dialog opens to, and the BYOK provider choice. Only the *provider choice* is
 * stored here — the API key itself never touches this store or localStorage; it
 * lives in the OS keychain via the `secret_*` Tauri commands (see
 * `lib/api.ts` / `src-tauri/src/secret.rs`).
 */

import { create } from "zustand";
import { isTauri } from "../lib/api";

export type Theme = "dark" | "light";
export type ByokProvider = "anthropic" | "openai" | "google";
export type WindowSizeOpt = "standard" | "compact";

const LS = {
  theme: "theme",
  defaultImportFolder: "defaultImportFolder",
  byokProvider: "byokProvider",
  windowSize: "windowSize",
} as const;

function loadTheme(): Theme {
  if (typeof localStorage === "undefined") return "dark";
  return localStorage.getItem(LS.theme) === "light" ? "light" : "dark";
}
function loadString(key: string): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(key);
}
function loadProvider(): ByokProvider {
  const v = loadString(LS.byokProvider);
  return v === "openai" || v === "google" ? v : "anthropic";
}
function loadWindowSize(): WindowSizeOpt {
  if (typeof localStorage === "undefined") return "standard";
  return localStorage.getItem(LS.windowSize) === "compact" ? "compact" : "standard";
}
function persist(key: string, value: string | null) {
  if (typeof localStorage === "undefined") return;
  if (value === null) localStorage.removeItem(key);
  else localStorage.setItem(key, value);
}

interface SettingsState {
  theme: Theme;
  defaultImportFolder: string | null;
  byokProvider: ByokProvider;
  windowSize: WindowSizeOpt;
  setTheme: (theme: Theme) => void;
  setDefaultImportFolder: (path: string | null) => void;
  setByokProvider: (provider: ByokProvider) => void;
  setWindowSize: (size: WindowSizeOpt) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  theme: loadTheme(),
  defaultImportFolder: loadString(LS.defaultImportFolder),
  byokProvider: loadProvider(),
  windowSize: loadWindowSize(),
  setTheme: (theme) => {
    persist(LS.theme, theme);
    applyTheme(theme);
    set({ theme });
  },
  setDefaultImportFolder: (defaultImportFolder) => {
    persist(LS.defaultImportFolder, defaultImportFolder);
    set({ defaultImportFolder });
  },
  setByokProvider: (byokProvider) => {
    persist(LS.byokProvider, byokProvider);
    set({ byokProvider });
  },
  setWindowSize: (windowSize) => {
    persist(LS.windowSize, windowSize);
    void applyWindowSize(windowSize);
    set({ windowSize });
  },
}));

/** Reflect the theme onto the document root so tokens can switch on it. */
export function applyTheme(theme: Theme): void {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }
}

/** Apply the window size (width: 1600x1000 or 1066x666 centered) dynamically in Tauri. */
export async function applyWindowSize(size: WindowSizeOpt): Promise<void> {
  if (!isTauri) return;
  try {
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
    
    await win.setPosition(new LogicalPosition(newX, newY));
    await win.setSize(new LogicalSize(targetWidth, targetHeight));
  } catch (e) {
    console.error("Failed to apply window size:", e);
  }
}

/** Apply the persisted theme and window size at startup. */
export function initTheme(): void {
  applyTheme(useSettingsStore.getState().theme);
}

export function initWindowSize(): void {
  void applyWindowSize(useSettingsStore.getState().windowSize);
}
