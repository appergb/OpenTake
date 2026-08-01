// @vitest-environment happy-dom

import { describe, expect, it } from "vitest";
import {
  resolveDocumentedShortcut,
  type DocumentedShortcutContext,
  type ResolvedShortcut,
} from "./useKeyboardShortcuts";

function event(overrides: Partial<KeyboardEvent>): KeyboardEvent {
  return {
    code: "Space",
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    repeat: false,
    target: null,
    ...overrides,
  } as KeyboardEvent;
}

const base: DocumentedShortcutContext = {
  view: "editor",
  blocked: false,
  focusedPanel: "timeline",
  compatibilityReadOnly: false,
  cropEditingActive: false,
};

type MatrixRow = [string, Partial<KeyboardEvent>, Partial<DocumentedShortcutContext>, ResolvedShortcut];

describe("documented shortcut resolver", () => {
  it("all_shortcuts_conflicts_editable_suppression_and_platform_modifiers", () => {
    const rows: MatrixRow[] = [
      ["space", { code: "Space" }, {}, { type: "transport" }],
      ["frame left", { code: "ArrowLeft" }, {}, { type: "stepFrame", delta: -1 }],
      ["frame right 5", { code: "ArrowRight", shiftKey: true }, {}, { type: "stepFrame", delta: 5 }],
      ["media left", { code: "ArrowLeft" }, { focusedPanel: "media" }, { type: "moveMediaSelection", delta: -1 }],
      ["media down", { code: "ArrowDown" }, { focusedPanel: "media" }, { type: "moveMediaSelection", delta: 1 }],
      ["delete", { code: "Backspace" }, {}, { type: "delete", ripple: false }],
      ["ripple delete", { code: "Delete", shiftKey: true }, {}, { type: "delete", ripple: true }],
      ["razor", { code: "KeyC" }, {}, { type: "setTool", tool: "razor" }],
      ["pointer", { code: "KeyV" }, {}, { type: "setTool", tool: "pointer" }],
      ["mark in", { code: "KeyI" }, {}, { type: "markRange", edge: "start" }],
      ["mark out", { code: "KeyO" }, {}, { type: "markRange", edge: "end" }],
      ["trim bracket left", { code: "BracketLeft" }, {}, { type: "trimStart" }],
      ["trim bracket right", { code: "BracketRight" }, {}, { type: "trimEnd" }],
      ["trim Q", { code: "KeyQ" }, {}, { type: "trimStart" }],
      ["trim W", { code: "KeyW" }, {}, { type: "trimEnd" }],
      ["maximize", { code: "Backquote" }, {}, { type: "maximize" }],
      ["media return", { code: "Enter" }, { focusedPanel: "media" }, { type: "mediaEnter" }],
      ["crop return", { code: "Enter" }, { cropEditingActive: true }, { type: "exitCrop" }],
      ["escape", { code: "Escape" }, {}, { type: "escape" }],
      ["undo mac", { code: "KeyZ", metaKey: true }, {}, { type: "history", redo: false }],
      ["redo windows", { code: "KeyZ", ctrlKey: true, shiftKey: true }, {}, { type: "history", redo: true }],
      ["copy", { code: "KeyC", metaKey: true }, {}, { type: "clipboard", action: "copy" }],
      ["cut", { code: "KeyX", ctrlKey: true }, {}, { type: "clipboard", action: "cut" }],
      ["paste", { code: "KeyV", metaKey: true }, {}, { type: "clipboard", action: "paste" }],
      ["split", { code: "KeyK", metaKey: true }, {}, { type: "split" }],
      ["new", { code: "KeyN", metaKey: true }, {}, { type: "application", id: "new" }],
      ["open", { code: "KeyO", ctrlKey: true }, {}, { type: "application", id: "open" }],
      ["save", { code: "KeyS", metaKey: true }, {}, { type: "application", id: "save" }],
      ["save as", { code: "KeyS", ctrlKey: true, shiftKey: true }, {}, { type: "application", id: "saveAs" }],
      ["import", { code: "KeyI", metaKey: true }, {}, { type: "application", id: "importMedia" }],
      ["export", { code: "KeyE", ctrlKey: true }, {}, { type: "application", id: "export" }],
      ["media panel", { code: "Digit0", metaKey: true }, {}, { type: "application", id: "mediaPanel" }],
      ["inspector", { code: "Digit0", ctrlKey: true, altKey: true }, {}, { type: "application", id: "inspector" }],
      ["agent", { code: "KeyA", metaKey: true, altKey: true }, {}, { type: "application", id: "agentPanel" }],
      ["layout", { code: "Digit2", ctrlKey: true }, {}, { type: "application", id: "layoutMedia" }],
      ["fullscreen", { code: "KeyF", metaKey: true }, {}, { type: "application", id: "fullscreen" }],
      ["help", { code: "Slash", ctrlKey: true, shiftKey: true }, {}, { type: "application", id: "shortcuts" }],
      ["settings", { code: "Comma", metaKey: true }, {}, { type: "application", id: "settings" }],
    ];

    for (const [name, keys, context, expected] of rows) {
      expect(resolveDocumentedShortcut(event(keys), { ...base, ...context }), name).toEqual(expected);
    }

    for (const context of [
      { ...base, view: "home" as const },
      { ...base, blocked: true },
    ]) {
      expect(resolveDocumentedShortcut(event({ code: "KeyS", metaKey: true }), context)).toBeNull();
    }
    expect(
      resolveDocumentedShortcut(
        event({ code: "KeyS", metaKey: true, target: document.createElement("input") }),
        base,
      ),
    ).toBeNull();
    expect(resolveDocumentedShortcut(event({ code: "Space", shiftKey: true }), base)).toBeNull();
    expect(resolveDocumentedShortcut(event({ code: "KeyS", metaKey: true, altKey: true }), base)).toBeNull();
    expect(
      resolveDocumentedShortcut(event({ code: "BracketLeft" }), {
        ...base,
        compatibilityReadOnly: true,
      }),
    ).toBeNull();
  });
});
