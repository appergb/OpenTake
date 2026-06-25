import { useEffect } from "react";
import { TitleBar } from "./components/shell/TitleBar";
import { EditorSplit } from "./components/shell/EditorSplit";
import { HomeView } from "./components/home/HomeView";
import { SettingsView } from "./components/settings/SettingsView";
import { LibraryView } from "./components/media/LibraryView";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useTimelinePlaybackEngine } from "./components/preview/previewEngine";
import { useAutosave } from "./hooks/useAutosave";
import { startSync } from "./store/sync";
import { startMediaSync } from "./store/mediaStore";
import { useEditorUiStore } from "./store/uiStore";
import { initI18n } from "./i18n";
import { initTheme, initWindowSize } from "./store/settingsStore";
import { onGoHome } from "./lib/api";

function Toast() {
  const toast = useEditorUiStore((s) => s.toast);
  const clearToast = useEditorUiStore((s) => s.clearToast);
  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(clearToast, 2000);
    return () => clearTimeout(timer);
  }, [toast, clearToast]);
  if (!toast) return null;
  return (
    <div
      style={{
        position: "fixed",
        bottom: 24,
        left: "50%",
        transform: "translateX(-50%)",
        padding: "8px 16px",
        background: "var(--bg-elevated)",
        border: "var(--bw-thin) solid var(--border-primary)",
        borderRadius: 6,
        boxShadow: "0 4px 12px rgba(0,0,0,0.3)",
        fontSize: "var(--fs-sm)",
        color: "var(--text-primary)",
        zIndex: 9999,
        pointerEvents: "none",
      }}
    >
      {toast.message}
    </div>
  );
}

export default function App() {
  // Editor-only hooks are safe to keep mounted across views: they only act on
  // editor state/events and the keyboard handler is a no-op until the editor is
  // shown (no selection / no focus). Keeping them unconditional preserves hook
  // order across navigation.
  useKeyboardShortcuts();
  useTimelinePlaybackEngine();
  useAutosave();

  const view = useEditorUiStore((s) => s.view);
  const settingsOpen = useEditorUiStore((s) => s.settingsOpen);

  useEffect(() => {
    initI18n();
    initTheme();
    initWindowSize();
    void startSync();
    void startMediaSync();
    // Window closed → app stays resident; return to the launcher (so a
    // Dock-reopen shows Home), mirroring upstream "close window → Home".
    let unlisten: (() => void) | undefined;
    void onGoHome(() => useEditorUiStore.getState().setView("home")).then((un) => {
      unlisten = un;
    });
    // Suppress the WebView's native context menu (the stray "Reload" item) so
    // app-native menus can own right-click; allow it in text fields.
    const onContextMenu = (e: MouseEvent) => {
      const el = e.target as HTMLElement | null;
      if (
        el &&
        (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable)
      ) {
        return;
      }
      e.preventDefault();
    };
    document.addEventListener("contextmenu", onContextMenu);
    return () => {
      unlisten?.();
      document.removeEventListener("contextmenu", onContextMenu);
    };
  }, []);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%", position: "relative" }}>
      {view === "home" ? (
        <HomeView />
      ) : view === "library" ? (
        <LibraryView />
      ) : (
        <>
          <TitleBar />
          <div style={{ flex: 1, minHeight: 0 }}>
            <EditorSplit />
          </div>
        </>
      )}
      {settingsOpen && <SettingsView />}
      <Toast />
    </div>
  );
}
