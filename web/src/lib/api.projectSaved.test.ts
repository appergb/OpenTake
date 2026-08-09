import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("project_saved event subscription", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.stubGlobal("window", { __TAURI_INTERNALS__: {} });
    mocks.invoke.mockReset().mockResolvedValue({});
    mocks.listen.mockReset().mockResolvedValue(vi.fn());
  });

  afterEach(() => vi.unstubAllGlobals());

  function lastListener(): ((event: { payload: unknown }) => void) | undefined {
    const call = mocks.listen.mock.calls.at(-1);
    return call?.[1] as ((event: { payload: unknown }) => void) | undefined;
  }

  it("subscribes to the exact `project_saved` event name", async () => {
    const { onProjectSaved } = await import("./api");
    await onProjectSaved(() => {});
    expect(mocks.listen).toHaveBeenCalledWith(
      "project_saved",
      expect.any(Function),
    );
  });

  it("decodes the tagged payload into path + project epoch", async () => {
    const { onProjectSaved } = await import("./api");
    const handler = vi.fn();
    await onProjectSaved(handler);
    lastListener()?.({ payload: { kind: "project_saved", path: "/p.opentake", projectEpoch: 3 } });
    expect(handler).toHaveBeenCalledWith("/p.opentake", 3);
  });

  it("ignores malformed or unrelated payloads instead of inferring defaults", async () => {
    const { onProjectSaved } = await import("./api");
    const handler = vi.fn();
    await onProjectSaved(handler);
    const listener = lastListener()!;
    listener({ payload: {} });
    listener({ payload: { kind: "project_saved", path: "/p.opentake" } });
    listener({ payload: { kind: "project_saved", projectEpoch: 3 } });
    listener({ payload: { kind: "project_saved", path: 42, projectEpoch: 3 } });
    listener({ payload: { kind: "project_saved", path: "/p.opentake", projectEpoch: "3" } });
    listener({ payload: undefined });
    expect(handler).not.toHaveBeenCalled();
  });

  it("is a no-op (and returns a no-op unlisten) outside Tauri", async () => {
    vi.unstubAllGlobals();
    delete (globalThis as Record<string, unknown>).window;
    const { onProjectSaved } = await import("./api");
    const unlisten = await onProjectSaved(() => {});
    expect(() => unlisten()).not.toThrow();
    expect(mocks.listen).not.toHaveBeenCalled();
  });
});
