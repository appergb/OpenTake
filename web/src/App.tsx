import { useEffect } from "react";
import { TitleBar } from "./components/shell/TitleBar";
import { EditorSplit } from "./components/shell/EditorSplit";
import { HomeView } from "./components/home/HomeView";
import { SettingsView } from "./components/settings/SettingsView";
import { LibraryView } from "./components/media/LibraryView";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { usePlaybackTicker } from "./hooks/usePlaybackTicker";
import { useAutosave } from "./hooks/useAutosave";
import { startSync } from "./store/sync";
import { startMediaSync } from "./store/mediaStore";
import { useEditorUiStore } from "./store/uiStore";
import { initI18n } from "./i18n";
import { initTheme } from "./store/settingsStore";
import { onGoHome } from "./lib/api";

export default function App() {
  // Editor-only hooks are safe to keep mounted across views: they only act on
  // editor state/events and the keyboard handler is a no-op until the editor is
  // shown (no selection / no focus). Keeping them unconditional preserves hook
  // order across navigation.
  useKeyboardShortcuts();
  usePlaybackTicker();
  useAutosave();

  const view = useEditorUiStore((s) => s.view);

  useEffect(() => {
    initI18n();
    initTheme();
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

  if (view === "home") return <HomeView />;
  if (view === "settings") return <SettingsView />;
  if (view === "library") return <LibraryView />;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", width: "100%" }}>
      <TitleBar />
      <div style={{ flex: 1, minHeight: 0 }}>
        <EditorSplit />
      </div>
    </div>
  );
}
