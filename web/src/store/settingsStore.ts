/**
 * App-level settings (UI preferences only — never editing truth). Persisted to
 * localStorage so they survive restarts: theme, the default folder the import
 * dialog opens to, the BYOK provider choice, model selection, telemetry toggles,
 * and notification preferences. Only the *provider choice* is stored here —
 * the API key itself never touches this store or localStorage; it lives in the
 * OS keychain via the `secret_*` Tauri commands (see `lib/api.ts` /
 * `src-tauri/src/secret.rs`).
 */

import { create } from "zustand";

export type Theme = "dark" | "light";
export type ByokProvider = "anthropic" | "openai" | "google";

const LS = {
  theme: "theme",
  defaultImportFolder: "defaultImportFolder",
  byokProvider: "byokProvider",
  byokModel: "byokModel",
  telemetryUsage: "telemetryUsage",
  telemetryCrash: "telemetryCrash",
  notificationsGeneration: "notificationsGeneration",
} as const;

function loadTheme(): Theme {
  if (typeof localStorage === "undefined") return "dark";
  return localStorage.getItem(LS.theme) === "light" ? "light" : "dark";
}
function loadString(key: string): string | null {
  if (typeof localStorage === "undefined") return null;
  return localStorage.getItem(key);
}
function loadBool(key: string, fallback: boolean): boolean {
  if (typeof localStorage === "undefined") return fallback;
  const v = localStorage.getItem(key);
  if (v === null) return fallback;
  return v === "true";
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
function persistBool(key: string, value: boolean) {
  persist(key, value ? "true" : "false");
}

interface SettingsState {
  theme: Theme;
  defaultImportFolder: string | null;
  byokProvider: ByokProvider;
  byokModel: string;
  telemetryUsage: boolean;
  telemetryCrash: boolean;
  notificationsGeneration: boolean;
  setTheme: (theme: Theme) => void;
  setDefaultImportFolder: (path: string | null) => void;
  setByokProvider: (provider: ByokProvider) => void;
  setByokModel: (model: string) => void;
  setTelemetryUsage: (v: boolean) => void;
  setTelemetryCrash: (v: boolean) => void;
  setNotificationsGeneration: (v: boolean) => void;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  theme: loadTheme(),
  defaultImportFolder: loadString(LS.defaultImportFolder),
  byokProvider: loadProvider(),
  byokModel: loadString(LS.byokModel) ?? "claude-sonnet-4-20250514",
  telemetryUsage: loadBool(LS.telemetryUsage, false),
  telemetryCrash: loadBool(LS.telemetryCrash, false),
  notificationsGeneration: loadBool(LS.notificationsGeneration, true),
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
  setByokModel: (byokModel) => {
    persist(LS.byokModel, byokModel);
    set({ byokModel });
  },
  setTelemetryUsage: (telemetryUsage) => {
    persistBool(LS.telemetryUsage, telemetryUsage);
    set({ telemetryUsage });
  },
  setTelemetryCrash: (telemetryCrash) => {
    persistBool(LS.telemetryCrash, telemetryCrash);
    set({ telemetryCrash });
  },
  setNotificationsGeneration: (notificationsGeneration) => {
    persistBool(LS.notificationsGeneration, notificationsGeneration);
    set({ notificationsGeneration });
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