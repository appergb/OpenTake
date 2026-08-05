// @vitest-environment happy-dom

import { describe, expect, it } from "vitest";
import {
  DOCUMENTED_SHORTCUT_ROWS,
  handleAgentPanelKeyDown,
  handleProjectSaveKeyDown,
  handleTransportSpaceKeyDown,
  resolveDocumentedShortcut,
  shouldHandleTransportSpaceKey,
} from "./useKeyboardShortcuts";

function event(overrides: Partial<KeyboardEvent> = {}): KeyboardEvent {
  return {
    code: "Space",
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    target: null,
    ...overrides,
  } as KeyboardEvent;
}

describe("keyboard transport Space shortcut", () => {
  it("handles plain Space in the editor", () => {
    expect(shouldHandleTransportSpaceKey(event(), "editor")).toBe(true);
  });

  it("does not suppress Space keyup outside the editor", () => {
    expect(shouldHandleTransportSpaceKey(event(), "home")).toBe(false);
  });

  it("does not suppress modified Space keyup", () => {
    expect(shouldHandleTransportSpaceKey(event({ metaKey: true }), "editor")).toBe(false);
  });

  it("does not claim Space from native controls or media interaction surfaces", () => {
    const button = document.createElement("button");
    const buttonLabel = document.createElement("span");
    button.append(buttonLabel);
    const mediaCard = document.createElement("div");
    mediaCard.dataset.mediaTile = "true";
    const context = {
      view: "editor" as const,
      blocked: false,
      focusedPanel: "timeline" as const,
      compatibilityReadOnly: false,
      cropEditingActive: false,
    };

    expect(shouldHandleTransportSpaceKey(event({ target: buttonLabel }), "editor")).toBe(false);
    expect(shouldHandleTransportSpaceKey(event({ target: mediaCard }), "editor")).toBe(false);
    expect(resolveDocumentedShortcut(event({ target: buttonLabel }), context)).toBeNull();
    expect(
      resolveDocumentedShortcut(event({ code: "Escape", target: buttonLabel }), context),
    ).toEqual({ type: "escape" });
    expect(
      resolveDocumentedShortcut(event({ code: "Backquote", target: buttonLabel }), context),
    ).toEqual({ type: "maximize" });
    expect(
      resolveDocumentedShortcut(
        event({ code: "KeyS", metaKey: true, target: buttonLabel }),
        context,
      ),
    ).toEqual({ type: "application", id: "save" });
  });

  it("toggles playback synchronously on Space keydown", () => {
    let toggles = 0;
    const e = event({
      preventDefault: () => {},
      stopPropagation: () => {},
    } as Partial<KeyboardEvent>);

    const handled = handleTransportSpaceKeyDown(e, {
      view: "editor",
      previewMediaId: null,
      timelinePlaybackAllowed: true,
      requestMediaPreviewToggle: () => {},
      togglePlay: () => {
        toggles += 1;
      },
    });

    expect(handled).toBe(true);
    expect(toggles).toBe(1);
  });

  it("does not toggle repeatedly while Space is held", () => {
    let toggles = 0;
    const e = event({
      repeat: true,
      preventDefault: () => {},
      stopPropagation: () => {},
    } as Partial<KeyboardEvent>);

    const handled = handleTransportSpaceKeyDown(e, {
      view: "editor",
      previewMediaId: null,
      timelinePlaybackAllowed: true,
      requestMediaPreviewToggle: () => {},
      togglePlay: () => {
        toggles += 1;
      },
    });

    expect(handled).toBe(true);
    expect(toggles).toBe(0);
  });

  it("does not start timeline playback from Space when route is Unsupported", () => {
    let toggles = 0;
    const e = event({
      preventDefault: () => {},
      stopPropagation: () => {},
    } as Partial<KeyboardEvent>);

    const handled = handleTransportSpaceKeyDown(e, {
      view: "editor",
      previewMediaId: null,
      timelinePlaybackAllowed: false,
      requestMediaPreviewToggle: () => {},
      togglePlay: () => {
        toggles += 1;
      },
    });

    expect(handled).toBe(true);
    expect(toggles).toBe(0);
  });

  it("does not export stale focus-release or keyup-suppression helpers", async () => {
    const shortcuts = await import("./useKeyboardShortcuts");

    expect("releaseTransportSpaceFocus" in shortcuts).toBe(false);
    expect("suppressTransportSpaceKeyUp" in shortcuts).toBe(false);
  });
});

describe("project save shortcut", () => {
  it("prevents the native shortcut but ignores repeated KeyS events", () => {
    let saves = 0;
    let prevented = 0;
    const handled = handleProjectSaveKeyDown(
      event({
        code: "KeyS",
        metaKey: true,
        repeat: true,
        preventDefault: () => {
          prevented += 1;
        },
      }),
      () => {
        saves += 1;
      },
    );

    expect(handled).toBe(true);
    expect(prevented).toBe(1);
    expect(saves).toBe(0);
  });

  it("complete_documented_shortcut_table", () => {
    expect(DOCUMENTED_SHORTCUT_ROWS).toEqual([
      "transport",
      "timeline-arrows",
      "media-arrows",
      "delete",
      "tools",
      "range",
      "trim",
      "maximize",
      "return-escape",
      "undo-redo",
      "clipboard",
      "split",
      "file",
      "panels",
      "layouts",
      "fullscreen",
      "help",
      "settings",
    ]);

    const context = {
      view: "editor" as const,
      blocked: false,
      focusedPanel: "timeline" as const,
      compatibilityReadOnly: false,
      cropEditingActive: false,
    };
    expect(resolveDocumentedShortcut(event({ code: "BracketLeft" }), context)).toEqual({
      type: "trimStart",
    });
    expect(resolveDocumentedShortcut(event({ code: "BracketRight" }), context)).toEqual({
      type: "trimEnd",
    });
    expect(
      resolveDocumentedShortcut(
        event({ code: "KeyS", metaKey: true, shiftKey: true }),
        context,
      ),
    ).toEqual({ type: "application", id: "saveAs" });
    expect(
      resolveDocumentedShortcut(event({ code: "Slash", ctrlKey: true, shiftKey: true }), context),
    ).toEqual({ type: "application", id: "shortcuts" });
  });
});

describe("Agent panel shortcut", () => {
  it("supports both macOS and Windows modifiers without repeating", () => {
    for (const modifiers of [{ metaKey: true }, { ctrlKey: true }]) {
      let toggles = 0;
      let prevented = 0;
      const handled = handleAgentPanelKeyDown(
        event({
          code: "KeyA",
          altKey: true,
          ...modifiers,
          preventDefault: () => {
            prevented += 1;
          },
        }),
        "editor",
        () => {
          toggles += 1;
        },
      );
      const repeatHandled = handleAgentPanelKeyDown(
        event({
          code: "KeyA",
          altKey: true,
          repeat: true,
          ...modifiers,
          preventDefault: () => {},
        }),
        "editor",
        () => {
          toggles += 1;
        },
      );

      expect(handled).toBe(true);
      expect(repeatHandled).toBe(true);
      expect(prevented).toBe(1);
      expect(toggles).toBe(1);
    }
  });

  it("does not consume the shortcut outside the editor", () => {
    expect(
      handleAgentPanelKeyDown(
        event({ code: "KeyA", metaKey: true, altKey: true }),
        "home",
        () => {
          throw new Error("must not toggle");
        },
      ),
    ).toBe(false);
  });
});
