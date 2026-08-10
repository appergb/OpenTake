// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { UpdateDialog } from "./UpdateDialog";

const updateApi = vi.hoisted(() => ({ openUpdateReleases: vi.fn() }));

vi.mock("../../i18n", () => ({ useT: () => (key: string) => key }));
vi.mock("../../lib/api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../../lib/api")>()),
  openUpdateReleases: updateApi.openUpdateReleases,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let root: Root;
let container: HTMLDivElement;

beforeEach(() => {
  updateApi.openUpdateReleases.mockReset().mockResolvedValue(undefined);
  container = document.createElement("div");
  document.body.append(container);
  root = createRoot(container);
});

afterEach(async () => {
  await act(async () => root.unmount());
  container.remove();
});

describe("UpdateDialog", () => {
  it("requires explicit confirmation before installing an available version", async () => {
    const install = vi.fn();
    await act(async () =>
      root.render(
        <UpdateDialog
          phase="available"
          version="1.0.0-beta.4"
          notes="Playback fixes"
          progress={null}
          error={null}
          onInstall={install}
          onClose={vi.fn()}
        />,
      ),
    );

    expect(container.textContent).toContain("1.0.0-beta.4");
    expect(container.textContent).toContain("Playback fixes");
    expect(install).not.toHaveBeenCalled();
    const button = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === "update.install",
    );
    await act(async () => button?.click());
    expect(install).toHaveBeenCalledOnce();
  });

  it("shows progress and prevents dismissal while downloading", async () => {
    const close = vi.fn();
    await act(async () =>
      root.render(
        <UpdateDialog
          phase="downloading"
          version="1.0.0-beta.4"
          notes={null}
          progress={37}
          error={null}
          onInstall={vi.fn()}
          onClose={close}
        />,
      ),
    );
    expect(container.querySelector('[role="progressbar"]')?.getAttribute("aria-valuenow")).toBe(
      "37",
    );
    expect(container.querySelector("button")?.hasAttribute("disabled")).toBe(true);

    const editorShortcut = vi.fn();
    window.addEventListener("keydown", editorShortcut);
    const space = new KeyboardEvent("keydown", {
      key: " ",
      code: "Space",
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(space);
    window.removeEventListener("keydown", editorShortcut);
    expect(space.defaultPrevented).toBe(true);
    expect(editorShortcut).not.toHaveBeenCalled();
  });

  it("disables dismissal while the native RID close is pending", async () => {
    const close = vi.fn();
    await act(async () =>
      root.render(
        <UpdateDialog
          phase="closing"
          version="1.0.0-beta.4"
          notes={null}
          progress={null}
          error={null}
          onInstall={vi.fn()}
          onClose={close}
        />,
      ),
    );

    const button = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === "update.close",
    );
    expect(button?.hasAttribute("disabled")).toBe(true);
    await act(async () => button?.click());
    expect(close).not.toHaveBeenCalled();
  });

  it("opens the fixed GitHub Releases fallback through the native command", async () => {
    await act(async () =>
      root.render(
        <UpdateDialog
          phase="error"
          version=""
          notes={null}
          progress={null}
          error="signed update check failed"
          onInstall={vi.fn()}
          onClose={vi.fn()}
        />,
      ),
    );
    const button = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === "update.releases",
    );
    expect(container.querySelector("a")).toBeNull();
    await act(async () => button?.click());
    expect(updateApi.openUpdateReleases).toHaveBeenCalledOnce();
  });

  it("reports a native browser launch failure instead of creating a dead link", async () => {
    updateApi.openUpdateReleases.mockRejectedValueOnce(new Error("no default browser"));
    await act(async () =>
      root.render(
        <UpdateDialog
          phase="error"
          version=""
          notes={null}
          progress={null}
          error="signed update check failed"
          onInstall={vi.fn()}
          onClose={vi.fn()}
        />,
      ),
    );
    const button = [...container.querySelectorAll("button")].find(
      (item) => item.textContent === "update.releases",
    );
    await act(async () => button?.click());
    expect(container.textContent).toContain("update.releasesOpenFailed");
  });
});
