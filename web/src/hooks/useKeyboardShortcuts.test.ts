import { describe, expect, it } from "vitest";
import {
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
