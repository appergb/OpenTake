import { describe, expect, it } from "vitest";
import {
  handleAgentPanelKeyDown,
  handleProjectSaveKeyDown,
  handleTransportSpaceKeyDown,
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
