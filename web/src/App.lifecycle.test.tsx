// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

const srv = vi.hoisted(() => ({
  startSync: vi.fn(async () => {}),
  stopSync: vi.fn(),
  startMediaSync: vi.fn(async () => {}),
  stopMediaSync: vi.fn(),
  startLibrarySync: vi.fn(async () => {}),
  stopLibrarySync: vi.fn(),
  onGoHome: vi.fn(),
  stopNativePlayback: vi.fn(async () => {}),
}));

vi.mock("./store/sync", () => ({ startSync: srv.startSync, stopSync: srv.stopSync }));
vi.mock("./store/mediaStore", () => ({
  startMediaSync: srv.startMediaSync,
  stopMediaSync: srv.stopMediaSync,
}));
vi.mock("./store/libraryStore", () => ({
  startLibrarySync: srv.startLibrarySync,
  stopLibrarySync: srv.stopLibrarySync,
}));
vi.mock("./lib/api", () => ({
  isTauri: false,
  onGoHome: srv.onGoHome,
  checkForAppUpdate: vi.fn().mockResolvedValue(null),
  closeAppUpdate: vi.fn().mockResolvedValue(undefined),
  installAppUpdate: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("./components/preview/nativePlaybackSession", () => ({
  stopNativePlaybackForProjectBoundary: srv.stopNativePlayback,
}));
vi.mock("./i18n", () => ({ initI18n: vi.fn() }));
const settings = vi.hoisted(() => ({
  initProxyPlayback: vi.fn(),
  initWindowSize: vi.fn(),
}));
vi.mock("./store/settingsStore", () => ({
  initProxyPlayback: settings.initProxyPlayback,
  initWindowSize: settings.initWindowSize,
}));
vi.mock("./hooks/useKeyboardShortcuts", () => ({ useKeyboardShortcuts: vi.fn() }));
vi.mock("./components/preview/previewEngine", () => ({ useTimelinePlaybackEngine: vi.fn() }));
vi.mock("./hooks/useAutosave", () => ({ useAutosave: vi.fn() }));

vi.mock("./components/shell/TitleBar", () => ({ TitleBar: () => null }));
vi.mock("./components/shell/ExportDialog", () => ({ ExportDialog: () => null }));
vi.mock("./components/shell/SaveAsProgress", () => ({ SaveAsProgress: () => null }));
vi.mock("./components/shell/ProjectSettingsMismatchDialog", () => ({
  ProjectSettingsMismatchDialog: () => null,
}));
vi.mock("./components/shell/EditorSplit", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  return {
    EditorSplit: () => {
      const [count, setCount] = React.useState(0);
      return (
        <div>
          <button data-testid="editor-state" onClick={() => setCount((value) => value + 1)}>
            {count}
          </button>
          <div data-testid="editor-scroll" />
        </div>
      );
    },
  };
});
vi.mock("./components/shell/CompatibilityBanner", () => ({
  CompatibilityBanner: () => null,
}));
vi.mock("./components/home/HomeView", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  return {
    HomeView: () => {
      const [count, setCount] = React.useState(0);
      return (
        <div>
          <button data-testid="home-state" onClick={() => setCount((value) => value + 1)}>
            {count}
          </button>
          <div data-testid="home-scroll" />
        </div>
      );
    },
  };
});
vi.mock("./components/settings/SettingsView", () => ({ SettingsView: () => null }));
vi.mock("./components/media/LibraryView", async () => {
  const React = await vi.importActual<typeof import("react")>("react");
  return {
    LibraryView: () => {
      const [count, setCount] = React.useState(0);
      return (
        <div>
          <button data-testid="library-state" onClick={() => setCount((value) => value + 1)}>
            {count}
          </button>
          <div data-testid="library-scroll" />
        </div>
      );
    },
  };
});
vi.mock("./components/shell/ViewMenu", () => ({ ApplicationMenuBridge: () => null }));

import App from "./App";
import { useEditorUiStore } from "./store/uiStore";

describe("App lifecycle listeners", () => {
  let container: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    vi.useRealTimers();
    srv.startSync.mockReset().mockResolvedValue(undefined);
    srv.stopSync.mockReset();
    srv.startMediaSync.mockReset().mockResolvedValue(undefined);
    srv.stopMediaSync.mockReset();
    srv.startLibrarySync.mockReset().mockResolvedValue(undefined);
    srv.stopLibrarySync.mockReset();
    srv.onGoHome.mockReset().mockResolvedValue(vi.fn());
    srv.stopNativePlayback.mockReset().mockResolvedValue(undefined);
    settings.initProxyPlayback.mockReset();
    settings.initWindowSize.mockReset();
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    useEditorUiStore.setState({ view: "home", settingsOpen: false, toast: null });
  });

  afterEach(async () => {
    if (root) {
      await act(async () => root?.unmount());
      root = null;
    }
    container.remove();
    vi.useRealTimers();
  });

  it("unsubscribes a listener whose async registration finishes after unmount", async () => {
    const registration = deferred<() => void>();
    const unsubscribe = vi.fn();
    srv.onGoHome.mockReturnValueOnce(registration.promise);

    await act(async () => root?.render(<App />));
    expect(srv.onGoHome).toHaveBeenCalledOnce();
    await act(async () => root?.unmount());
    root = null;

    await act(async () => {
      registration.resolve(unsubscribe);
      await registration.promise;
    });

    expect(unsubscribe).toHaveBeenCalledOnce();
    expect(srv.stopSync).toHaveBeenCalledOnce();
    expect(srv.stopMediaSync).toHaveBeenCalledOnce();
  });

  it("initializes persisted dark-window preferences without a theme initializer", async () => {
    await act(async () => root?.render(<App />));

    expect(settings.initWindowSize).toHaveBeenCalledOnce();
    expect(settings.initProxyPlayback).toHaveBeenCalledOnce();
  });

  it("ignores an old go-home callback after its owning effect is disposed", async () => {
    const registration = deferred<() => void>();
    let goHome: (() => void) | null = null;
    srv.onGoHome.mockImplementationOnce((handler: () => void) => {
      goHome = handler;
      return registration.promise;
    });

    await act(async () => root?.render(<App />));
    await act(async () => root?.unmount());
    root = null;
    useEditorUiStore.setState({ view: "editor" });

    goHome?.();

    expect(useEditorUiStore.getState().view).toBe("editor");
    const unsubscribe = vi.fn();
    registration.resolve(unsubscribe);
    await registration.promise;
    expect(unsubscribe).toHaveBeenCalledOnce();
  });

  it("owns each async listener registration across the StrictMode effect replay", async () => {
    const firstRegistration = deferred<() => void>();
    const secondRegistration = deferred<() => void>();
    const firstUnsubscribe = vi.fn();
    const secondUnsubscribe = vi.fn();
    srv.onGoHome
      .mockReturnValueOnce(firstRegistration.promise)
      .mockReturnValueOnce(secondRegistration.promise);

    await act(async () => root?.render(<StrictMode><App /></StrictMode>));
    expect(srv.onGoHome).toHaveBeenCalledTimes(2);

    await act(async () => {
      firstRegistration.resolve(firstUnsubscribe);
      secondRegistration.resolve(secondUnsubscribe);
      await Promise.all([firstRegistration.promise, secondRegistration.promise]);
    });
    expect(firstUnsubscribe).toHaveBeenCalledOnce();
    expect(secondUnsubscribe).not.toHaveBeenCalled();

    await act(async () => root?.unmount());
    root = null;
    expect(secondUnsubscribe).toHaveBeenCalledOnce();
  });

  it("reports and retries every failed lifecycle registration with a bounded backoff", async () => {
    vi.useFakeTimers();
    srv.startSync.mockRejectedValue(new Error("timeline bootstrap failed"));
    srv.startMediaSync
      .mockRejectedValueOnce(new Error("media bootstrap failed"))
      .mockResolvedValue(undefined);
    srv.onGoHome
      .mockRejectedValueOnce(new Error("go-home listener failed"))
      .mockResolvedValue(vi.fn());
    const messages: string[] = [];
    const unsubscribeToast = useEditorUiStore.subscribe((state) => {
      if (state.toast) messages.push(state.toast.message);
    });

    await act(async () => {
      root?.render(<App />);
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(srv.startSync).toHaveBeenCalledOnce();
    expect(srv.startMediaSync).toHaveBeenCalledOnce();
    expect(srv.onGoHome).toHaveBeenCalledOnce();
    expect(messages.some((message) => message.includes("timeline bootstrap failed"))).toBe(true);
    expect(messages.some((message) => message.includes("media bootstrap failed"))).toBe(true);
    expect(messages.some((message) => message.includes("go-home listener failed"))).toBe(true);

    await act(async () => vi.advanceTimersByTimeAsync(100));
    expect(srv.startSync).toHaveBeenCalledTimes(2);
    expect(srv.startMediaSync).toHaveBeenCalledTimes(2);
    expect(srv.onGoHome).toHaveBeenCalledTimes(2);

    await act(async () => vi.advanceTimersByTimeAsync(500));
    await act(async () => vi.advanceTimersByTimeAsync(2_000));
    expect(srv.startSync).toHaveBeenCalledTimes(4);

    await act(async () => vi.advanceTimersByTimeAsync(30_000));
    expect(srv.startSync).toHaveBeenCalledTimes(4);
    unsubscribeToast();
  });

  it("cancels pending lifecycle retries when the app unmounts", async () => {
    vi.useFakeTimers();
    srv.startSync.mockRejectedValue(new Error("timeline bootstrap failed"));

    await act(async () => {
      root?.render(<App />);
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(srv.startSync).toHaveBeenCalledOnce();

    await act(async () => root?.unmount());
    root = null;
    await act(async () => vi.advanceTimersByTimeAsync(30_000));

    expect(srv.startSync).toHaveBeenCalledOnce();
  });

  it.each(["home", "library"] as const)(
    "stops DOM, transport, and native playback when leaving the editor for %s",
    async (destination) => {
      useEditorUiStore.setState({ view: "editor", isPlaying: true, isScrubbing: true });
      const media = document.createElement("video");
      const pause = vi.fn();
      Object.defineProperty(media, "pause", { configurable: true, value: pause });
      document.body.append(media);

      await act(async () => root?.render(<App />));
      await act(async () => useEditorUiStore.getState().setView(destination));

      expect(pause).toHaveBeenCalledOnce();
      expect(useEditorUiStore.getState().isPlaying).toBe(false);
      expect(useEditorUiStore.getState().isScrubbing).toBe(false);
      expect(srv.stopNativePlayback).toHaveBeenCalledOnce();
      media.remove();
    },
  );

  it("releases Library synchronization when leaving the Library view", async () => {
    useEditorUiStore.setState({ view: "library" });

    await act(async () => root?.render(<App />));
    await act(async () => useEditorUiStore.getState().setView("home"));

    expect(srv.stopLibrarySync).toHaveBeenCalledOnce();
  });

  it("announces toast feedback and marks every primary view for reduced-motion-aware entry", async () => {
    useEditorUiStore.setState({
      view: "home",
      toast: { message: "Saved" },
    });

    await act(async () => root?.render(<App />));

    const view = container.querySelector<HTMLElement>('[data-app-view="home"]');
    expect(view?.classList.contains("app-view-enter")).toBe(true);
    const toast = container.querySelector<HTMLElement>('[role="status"]');
    expect(toast?.getAttribute("aria-live")).toBe("polite");
    expect(toast?.getAttribute("aria-atomic")).toBe("true");
    expect(toast?.classList.contains("app-toast")).toBe(true);
  });

  it("keeps visited views mounted with local state and scroll while animating only the active view", async () => {
    await act(async () => root?.render(<App />));
    const homePanel = container.querySelector<HTMLElement>('[data-app-view="home"]')!;
    const homeState = container.querySelector<HTMLButtonElement>('[data-testid="home-state"]')!;
    const homeScroll = container.querySelector<HTMLElement>('[data-testid="home-scroll"]')!;
    await act(async () => homeState.click());
    homeScroll.scrollTop = 41;

    await act(async () => useEditorUiStore.getState().setView("editor"));
    const editorPanel = container.querySelector<HTMLElement>('[data-app-view="editor"]')!;
    const editorState = container.querySelector<HTMLButtonElement>('[data-testid="editor-state"]')!;
    const editorScroll = container.querySelector<HTMLElement>('[data-testid="editor-scroll"]')!;
    await act(async () => editorState.click());
    editorScroll.scrollTop = 73;
    expect(homePanel.hidden).toBe(true);

    await act(async () => useEditorUiStore.getState().setView("library"));
    const libraryPanel = container.querySelector<HTMLElement>('[data-app-view="library"]')!;
    const libraryState = container.querySelector<HTMLButtonElement>('[data-testid="library-state"]')!;
    const libraryScroll = container.querySelector<HTMLElement>('[data-testid="library-scroll"]')!;
    await act(async () => libraryState.click());
    libraryScroll.scrollTop = 109;
    expect(srv.startLibrarySync).toHaveBeenCalledOnce();

    await act(async () => useEditorUiStore.getState().setView("home"));
    expect(container.querySelector('[data-app-view="home"]')).toBe(homePanel);
    expect(container.querySelector('[data-testid="home-state"]')?.textContent).toBe("1");
    expect(homeScroll.scrollTop).toBe(41);
    expect(homePanel.hidden).toBe(false);
    expect(editorPanel.hidden).toBe(true);
    expect(libraryPanel.hidden).toBe(true);
    expect(container.querySelectorAll(".app-view-enter")).toHaveLength(1);
    expect(container.querySelector(".app-view-enter")?.getAttribute("data-app-view")).toBe(
      "home",
    );

    await act(async () => useEditorUiStore.getState().setView("editor"));
    expect(container.querySelector('[data-app-view="editor"]')).toBe(editorPanel);
    expect(container.querySelector('[data-testid="editor-state"]')?.textContent).toBe("1");
    expect(editorScroll.scrollTop).toBe(73);

    await act(async () => useEditorUiStore.getState().setView("library"));
    expect(container.querySelector('[data-app-view="library"]')).toBe(libraryPanel);
    expect(container.querySelector('[data-testid="library-state"]')?.textContent).toBe("1");
    expect(libraryScroll.scrollTop).toBe(109);
    expect(srv.startLibrarySync).toHaveBeenCalledTimes(2);
  });

  it("stops playback before a go-home callback changes the view", async () => {
    let goHome: (() => void) | null = null;
    srv.onGoHome.mockImplementationOnce(async (handler: () => void) => {
      goHome = handler;
      return vi.fn();
    });
    useEditorUiStore.setState({ view: "editor", isPlaying: true });

    await act(async () => root?.render(<App />));
    await act(async () => goHome?.());

    expect(useEditorUiStore.getState().view).toBe("home");
    expect(useEditorUiStore.getState().isPlaying).toBe(false);
    expect(srv.stopNativePlayback).toHaveBeenCalledOnce();
  });
});
