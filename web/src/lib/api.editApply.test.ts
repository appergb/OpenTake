import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("edit_apply production IPC envelope", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({
      changed: true,
      actionName: "Move Clips",
      affectedClipIds: ["clip-a"],
      timelineVersion: 9,
      summary: "Moved 1 clip",
    });
  });

  afterEach(() => vi.unstubAllGlobals());

  it("edit_apply_forwards_exact_command_envelope", async () => {
    const { editApply } = await import("./api");
    const command = {
      type: "moveClips" as const,
      moves: [{ clipId: "clip-a", toTrack: 2, toFrame: 48 }],
    };

    const result = await editApply(command);

    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith("edit_apply", { command });
    expect(result).toEqual({
      changed: true,
      actionName: "Move Clips",
      affectedClipIds: ["clip-a"],
      timelineVersion: 9,
      summary: "Moved 1 clip",
    });
  });
});
