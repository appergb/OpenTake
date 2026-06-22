/**
 * App-level settings (UI preferences only — never editing truth). Persisted to
 * localStorage so they survive restarts: theme, the default folder the import
 * dialog opens to, and the BYOK provider choice. Only the *provider choice* is
 * stored here — the API key itself never touches this store or localStorage; it
 * lives in the OS keychain via the `secret_*` Tauri commands (see
 * `lib/api.ts` / `src-tauri/src/secret.rs`).
 */

import { create } from "zustand";

export type Theme = "dark" | "light";
export type ByokProvider = "anthropic" | "openai" | "google";

const LS = {
  theme: "theme",
  defaultImportFolder: "defaultImportFolder",
  byokProvider: "byokProvider",
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
function persist(key: string, value: string | null) {
  if (typeof localStorage === "undefined") return;
  if (value === null) localStorage.removeItem(key);
  else localStorage.setItem(key, value);
}

interface SettingsState {
  theme: Theme;
  defaultImportFolder: string | null;
  byokProvider: ByokProvider;
  setTheme: (theme: Theme) => void;
  setDefaultImportFolder: (path: string | null) => void;
  setByokProvider: (provider: ByokProvider) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  theme: loadTheme(),
  defaultImportFolder: loadString(LS.defaultImportFolder),
  byokProvider: loadProvider(),
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
}));

/** Reflect the theme onto the document root so tokens can switch on it. */
export function applyTheme(theme: Theme): void {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.theme = theme;
  }
}

/** Apply the persisted theme at startup. */
export function initTheme(): void {
  applyTheme(useSettingsStore.getState().theme);
}
