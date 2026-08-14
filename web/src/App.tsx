import { useEffect, useRef } from "react";
import { TitleBar } from "./components/shell/TitleBar";
import { ApplicationMenuBridge } from "./components/shell/ViewMenu";
import { ExportDialog } from "./components/shell/ExportDialog";
import { SaveAsProgress } from "./components/shell/SaveAsProgress";
import { ProjectSettingsMismatchDialog } from "./components/shell/ProjectSettingsMismatchDialog";
import { EditorSplit } from "./components/shell/EditorSplit";
import { CompatibilityBanner } from "./components/shell/CompatibilityBanner";
import { HomeView } from "./components/home/HomeView";
import { SettingsView } from "./components/settings/SettingsView";
import { UpdateCenter } from "./components/settings/UpdateDialog";
import { LibraryView } from "./components/media/LibraryView";
import { MotionStudio } from "./components/motion/MotionStudio";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useTimelinePlaybackEngine } from "./components/preview/previewEngine";
import { useAutosave } from "./hooks/useAutosave";
import { startSync, stopSync } from "./store/sync";
import { startMediaSync, stopMediaSync } from "./store/mediaStore";
import { startLibrarySync, stopLibrarySync } from "./store/libraryStore";
import { useEditorUiStore } from "./store/uiStore";
import { initI18n } from "./i18n";
import { initProxyPlayback, initWindowSize } from "./store/settingsStore";
import { isTauri, onGoHome } from "./lib/api";
import { stopNativePlaybackForProjectBoundary } from "./components/preview/nativePlaybackSession";
import { useUpdateStore } from "./store/updateStore";
import { startUpdateScheduler } from "./lib/updateScheduler";

const LIFECYCLE_RETRY_DELAYS_MS = [100, 500, 2_000] as const;

function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

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
      role="status"
      aria-live="polite"
      aria-atomic="true"
      className="app-toast"
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

const PRIMARY_VIEWS = ["home", "library", "editor", "motion"] as const;
type PrimaryView = (typeof PRIMARY_VIEWS)[number];

function PrimaryViewContent({ view }: { view: PrimaryView }) {
  if (view === "home") return <HomeView />;
  if (view === "library") return <LibraryView />;
  if (view === "motion") {
    return (
      <>
        <TitleBar />
        <div style={{ flex: 1, minHeight: 0 }}>
          <MotionStudio />
        </div>
      </>
    );
  }
  return (
    <>
      <TitleBar />
      <div style={{ flex: 1, minHeight: 0 }}>
        <EditorSplit />
      </div>
    </>
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
  const activePrimaryView: PrimaryView =
    view === "home" || view === "library" || view === "motion" ? view : "editor";
  const mountedPrimaryViews = useRef(new Set<PrimaryView>());
  mountedPrimaryViews.current.add(activePrimaryView);

  useEffect(() => {
    initI18n();
    initWindowSize();
    initProxyPlayback();
    const stopUpdateScheduler = isTauri
      ? startUpdateScheduler(() => useUpdateStore.getState().check("background"))
      : undefined;
    let disposed = false;
    const retryTimers = new Set<ReturnType<typeof setTimeout>>();
    let unlisten: (() => void) | undefined;

    const reportLifecycleFailure = (label: string, error: unknown, retrying: boolean) => {
      const suffix = retrying
        ? "；正在重试 / retrying"
        : "；已停止重试 / retry limit reached";
      useEditorUiStore
        .getState()
        .pushToast(`${label}: ${errorMessage(error)}${suffix}`);
    };

    function launchWithRetry<T>(
      label: string,
      operation: () => Promise<T>,
      onSuccess?: (value: T) => void,
      onDisposedSuccess?: (value: T) => void,
    ): void {
      let failureCount = 0;
      const run = async () => {
        try {
          const value = await operation();
          if (disposed) {
            onDisposedSuccess?.(value);
            return;
          }
          onSuccess?.(value);
        } catch (error) {
          if (disposed) return;
          const retryDelay = LIFECYCLE_RETRY_DELAYS_MS[failureCount];
          failureCount += 1;
          reportLifecycleFailure(label, error, retryDelay !== undefined);
          if (retryDelay === undefined) return;
          const timer = setTimeout(() => {
            retryTimers.delete(timer);
            void run();
          }, retryDelay);
          retryTimers.add(timer);
        }
      };
      void run();
    }

    const stopHiddenPlayback = () => {
      const ui = useEditorUiStore.getState();
      if (ui.isPlaying || ui.isScrubbing) ui.setPlaying(false);
      for (const media of document.querySelectorAll<HTMLMediaElement>("audio, video")) {
        media.pause();
      }
      void stopNativePlaybackForProjectBoundary().catch((error) => {
        if (!disposed) {
          useEditorUiStore
            .getState()
            .pushToast(`停止预览失败 / Failed to stop preview: ${errorMessage(error)}`);
        }
      });
    };

    const unsubscribeView = useEditorUiStore.subscribe((state, previous) => {
      if (state.view === previous.view) return;
      if (previous.view === "library" && state.view !== "library") stopLibrarySync();
      if (state.view === "library" && previous.view !== "library") {
        launchWithRetry("素材库同步失败 / Library sync failed", startLibrarySync);
      }
      if (state.view !== "editor") stopHiddenPlayback();
    });

    launchWithRetry("时间线同步失败 / Timeline sync failed", startSync);
    launchWithRetry("媒体同步失败 / Media sync failed", startMediaSync);
    // Window closed → app stays resident; return to the launcher (so a
    // Dock-reopen shows Home), mirroring upstream "close window → Home".
    launchWithRetry(
      "窗口监听失败 / Window listener failed",
      () =>
        onGoHome(() => {
          if (disposed) return;
          const ui = useEditorUiStore.getState();
          if (ui.view === "home") stopHiddenPlayback();
          else ui.setView("home");
        }),
      (registeredUnlisten) => {
        unlisten?.();
        unlisten = registeredUnlisten;
      },
      (registeredUnlisten) => registeredUnlisten(),
    );
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
      disposed = true;
      stopUpdateScheduler?.();
      for (const timer of retryTimers) clearTimeout(timer);
      retryTimers.clear();
      unsubscribeView();
      unlisten?.();
      stopSync();
      stopMediaSync();
      stopLibrarySync();
      stopHiddenPlayback();
      document.removeEventListener("contextmenu", onContextMenu);
    };
  }, []);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        height: "100%",
        width: "100%",
        position: "relative",
        background: "var(--bg-base)",
      }}
    >
      <CompatibilityBanner />
      <ApplicationMenuBridge />
      {PRIMARY_VIEWS.filter((candidate) => mountedPrimaryViews.current.has(candidate)).map(
        (candidate) => {
          const active = candidate === activePrimaryView;
          return (
            <div
              key={candidate}
              data-app-view={candidate}
              className={active ? "app-view-enter" : undefined}
              hidden={!active}
              aria-hidden={active ? undefined : true}
              style={{
                flex: 1,
                minHeight: 0,
                minWidth: 0,
                display: active ? "flex" : "none",
                flexDirection: "column",
                overflow: "hidden",
              }}
            >
              <PrimaryViewContent view={candidate} />
            </div>
          );
        },
      )}
      {settingsOpen && <SettingsView />}
      <UpdateCenter />
      <ExportDialog />
      <SaveAsProgress />
      <ProjectSettingsMismatchDialog />
      <Toast />
    </div>
  );
}
