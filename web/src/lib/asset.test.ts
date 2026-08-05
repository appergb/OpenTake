import { expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  convertFileSrc: vi.fn((path: string, protocol?: string) => `${protocol}:${path}`),
}));

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: mocks.convertFileSrc }));
vi.mock("./api", () => ({ isTauri: true }));

import { assetUrl } from "./asset";

it("routes local paths only through the bounded OpenTake asset protocol", () => {
  expect(assetUrl("/tmp/frame.jpg")).toBe("opentake-asset:/tmp/frame.jpg");
  expect(mocks.convertFileSrc).toHaveBeenCalledWith(
    "/tmp/frame.jpg",
    "opentake-asset",
  );
  expect(assetUrl(null)).toBeNull();
});
