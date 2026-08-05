import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

describe("edit_apply production IPC envelope", () => {
  const expected = {
    projectEpoch: 7,
    projectPath: "/tmp/project-a.opentake",
    timelineVersion: 8,
  };

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

    const result = await editApply(command, expected);

    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    expect(mocks.invoke).toHaveBeenCalledWith("edit_apply", {
      command,
      expectedProjectEpoch: 7,
      expectedProjectPath: "/tmp/project-a.opentake",
      expectedTimelineVersion: 8,
    });
    expect(result).toEqual({
      changed: true,
      actionName: "Move Clips",
      affectedClipIds: ["clip-a"],
      timelineVersion: 9,
      summary: "Moved 1 clip",
    });
  });

  it("binds undo and redo to the same complete project identity", async () => {
    const { undo, redo } = await import("./api");

    await undo(expected);
    await redo(expected);

    const identity = {
      expectedProjectEpoch: 7,
      expectedProjectPath: "/tmp/project-a.opentake",
      expectedTimelineVersion: 8,
    };
    expect(mocks.invoke).toHaveBeenNthCalledWith(1, "undo", identity);
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, "redo", identity);
  });

  it("binds project save and save-as to the initiating project identity", async () => {
    const { projectSave } = await import("./api");

    await projectSave(null, expected.projectEpoch, expected.projectPath);
    await projectSave(
      "/tmp/project-a-copy.opentake",
      expected.projectEpoch,
      expected.projectPath,
    );

    expect(mocks.invoke).toHaveBeenNthCalledWith(1, "project_save", {
      path: null,
      expectedProjectEpoch: 7,
      expectedProjectPath: "/tmp/project-a.opentake",
    });
    expect(mocks.invoke).toHaveBeenNthCalledWith(2, "project_save", {
      path: "/tmp/project-a-copy.opentake",
      expectedProjectEpoch: 7,
      expectedProjectPath: "/tmp/project-a.opentake",
    });
  });

  it("preserves the stable typed command error", async () => {
    mocks.invoke.mockRejectedValueOnce({
      code: "validation",
      message: "clipIds[0]: unknown clip",
    });
    const { editApply, TauriCommandError } = await import("./api");

    await expect(
      editApply({ type: "removeClips", clipIds: ["missing"] }, expected),
    ).rejects.toEqual(
      expect.objectContaining({
        name: "TauriCommandError",
        code: "validation",
        message: "clipIds[0]: unknown clip",
      }),
    );
    await expect(
      editApply({ type: "removeClips", clipIds: ["missing"] }, expected),
    ).resolves.toBeDefined();
    expect(TauriCommandError.prototype).toBeInstanceOf(Error);
  });
});
