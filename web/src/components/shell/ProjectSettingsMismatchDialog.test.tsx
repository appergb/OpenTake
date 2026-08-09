// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));

import { useEditorUiStore } from "../../store/uiStore";
import { ProjectSettingsMismatchDialog } from "./ProjectSettingsMismatchDialog";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let container: HTMLDivElement;
let returnFocus: HTMLButtonElement;

beforeEach(() => {
  useEditorUiStore.getState().resetProjectRuntimeState();
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
  returnFocus = document.createElement("button");
  returnFocus.textContent = "Import media";
  document.body.append(returnFocus);
  returnFocus.focus();
});

afterEach(async () => {
  useEditorUiStore.getState().resolveProjectSettingsPrompt(false);
  await act(async () => root.unmount());
  container.remove();
  returnFocus.remove();
});

async function openPrompt(): Promise<{ choice: Promise<boolean> }> {
  let choice!: Promise<boolean>;
  await act(async () => {
    choice = useEditorUiStore.getState().requestProjectSettingsPrompt({
      current: { fps: 30, width: 1920, height: 1080 },
      suggested: { fps: 24, width: 3840, height: 2160 },
    });
    root.render(<ProjectSettingsMismatchDialog />);
  });
  return { choice };
}

it("renders an accessible mismatch and resolves match or keep choices", async () => {
  const { choice: matchChoice } = await openPrompt();
  const dialog = container.querySelector<HTMLElement>('[role="dialog"]');
  expect(dialog?.getAttribute("aria-modal")).toBe("true");
  expect(dialog?.getAttribute("aria-labelledby")).toBe("project-settings-mismatch-title");
  expect(dialog?.textContent).toContain("1920 × 1080 · 30 fps");
  expect(dialog?.textContent).toContain("3840 × 2160 · 24 fps");

  const match = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => button.textContent === "projectSettingsMismatch.match",
  );
  expect(document.activeElement).toBe(match);
  const keep = [...container.querySelectorAll<HTMLButtonElement>("button")].find(
    (button) => button.textContent === "projectSettingsMismatch.keep",
  );
  expect(match?.style.minHeight).toBe("28px");
  expect(keep?.style.minHeight).toBe("28px");
  await act(async () => {
    match?.focus();
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }),
    );
  });
  expect(document.activeElement).toBe(keep);
  await act(async () => {
    keep?.focus();
    window.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Tab",
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
  });
  expect(document.activeElement).toBe(match);
  await act(async () => match?.click());
  await expect(matchChoice).resolves.toBe(true);
  expect(container.querySelector('[role="dialog"]')).toBeNull();
  expect(document.activeElement).toBe(returnFocus);

  returnFocus.focus();
  const { choice: keepChoice } = await openPrompt();
  await act(async () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
  await expect(keepChoice).resolves.toBe(false);
  expect(container.querySelector('[role="dialog"]')).toBeNull();
  expect(document.activeElement).toBe(returnFocus);
});
