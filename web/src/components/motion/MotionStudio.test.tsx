// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));

import { MotionStudio } from "./MotionStudio";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe("MotionStudio semantic shell", () => {
  let cleanup: (() => Promise<void>) | undefined;

  afterEach(async () => cleanup?.());

  it("exposes independent file, editor, preview, inspector, and timeline landmarks", async () => {
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    cleanup = async () => {
      await act(async () => root.unmount());
      container.remove();
    };

    await act(async () => root.render(<MotionStudio />));

    expect(container.querySelector('main[aria-label="motionStudio.workspace"]')).not.toBeNull();
    for (const label of ["files", "editor", "inspector", "timeline"]) {
      expect(
        container.querySelector(`[aria-label="motionStudio.${label}"]`),
        `missing ${label} landmark`,
      ).not.toBeNull();
    }
    expect(
      container.querySelector('figure[role="region"][aria-label="motionStudio.preview"]'),
      "missing preview landmark",
    ).not.toBeNull();
    expect(container.textContent).toContain("motionStudio.visibleStarterTitle");
    expect(container.textContent).toContain("motionStudio.visibleStarterSubtitle");
  });
});
