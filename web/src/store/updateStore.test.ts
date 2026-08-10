import { describe, expect, it, vi } from "vitest";
import { createUpdateStore, type UpdateBackend } from "./updateStore";

const available = {
  rid: 17,
  currentVersion: "1.0.0-beta.3",
  version: "1.0.0-beta.4",
  notes: "Playback fixes",
  publishedAt: "2026-08-10T00:00:00Z",
};

function backend(overrides: Partial<UpdateBackend> = {}): UpdateBackend {
  return {
    check: vi.fn().mockResolvedValue(null),
    close: vi.fn().mockResolvedValue(undefined),
    install: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("update store", () => {
  it("keeps background no-update and errors silent", async () => {
    const noUpdate = backend();
    const noUpdateStore = createUpdateStore(noUpdate);
    await noUpdateStore.getState().check("background");
    expect(noUpdateStore.getState()).toMatchObject({ phase: "idle", dialogOpen: false });

    const failed = backend({ check: vi.fn().mockRejectedValue(new Error("offline")) });
    const failedStore = createUpdateStore(failed);
    await failedStore.getState().check("background");
    expect(failedStore.getState()).toMatchObject({ phase: "idle", dialogOpen: false });
  });

  it("shows manual up-to-date and error outcomes", async () => {
    const noUpdateStore = createUpdateStore(backend());
    await noUpdateStore.getState().check("manual");
    expect(noUpdateStore.getState()).toMatchObject({
      phase: "upToDate",
      dialogOpen: true,
      source: "manual",
    });

    const failedStore = createUpdateStore(
      backend({ check: vi.fn().mockRejectedValue(new Error("rate limited")) }),
    );
    await failedStore.getState().check("manual");
    expect(failedStore.getState()).toMatchObject({
      phase: "error",
      dialogOpen: true,
      error: "rate limited",
    });
  });

  it("requires an explicit install action after finding an update", async () => {
    const api = backend({ check: vi.fn().mockResolvedValue(available) });
    const store = createUpdateStore(api);
    await store.getState().check("background");

    expect(store.getState()).toMatchObject({
      phase: "available",
      dialogOpen: true,
      update: available,
    });
    expect(api.install).not.toHaveBeenCalled();

    await store.getState().install();
    expect(api.install).toHaveBeenCalledOnce();
    expect(api.install).toHaveBeenCalledWith(17, expect.any(Function));
  });

  it("maps download events to determinate progress and installing state", async () => {
    const api = backend({
      check: vi.fn().mockResolvedValue(available),
      install: vi.fn().mockImplementation(async (_rid, onEvent) => {
        onEvent({ event: "started", data: { contentLength: 100 } });
        onEvent({ event: "progress", data: { downloaded: 25 } });
        expect(store.getState()).toMatchObject({ phase: "downloading", progress: 25 });
        onEvent({ event: "installing" });
        expect(store.getState().phase).toBe("installing");
        onEvent({ event: "restarting" });
      }),
    });
    const store = createUpdateStore(api);
    await store.getState().check("manual");
    await store.getState().install();
    expect(store.getState()).toMatchObject({ phase: "restarting", progress: 100 });
  });

  it("serializes checks and releases every pending Update RID", async () => {
    const pending = deferred<typeof available | null>();
    const api = backend({ check: vi.fn().mockReturnValue(pending.promise) });
    const store = createUpdateStore(api);
    const first = store.getState().check("manual");
    const second = store.getState().check("manual");
    expect(api.check).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({ phase: "checking", dialogOpen: true });
    pending.resolve(available);
    await Promise.all([first, second]);

    await store.getState().dismiss();
    expect(api.close).toHaveBeenCalledWith(17);
    expect(store.getState()).toMatchObject({ phase: "idle", update: null, dialogOpen: false });

    vi.mocked(api.check).mockResolvedValueOnce(available);
    await store.getState().check("manual");
    vi.mocked(api.check).mockResolvedValueOnce(null);
    await store.getState().check("manual");
    expect(api.close).toHaveBeenLastCalledWith(17);
  });

  it("retains a RID when close fails so the same native resource can be retried", async () => {
    const api = backend({ check: vi.fn().mockResolvedValue(available) });
    const store = createUpdateStore(api);
    await store.getState().check("manual");

    vi.mocked(api.close).mockRejectedValueOnce(new Error("close interrupted"));
    await store.getState().check("manual");
    expect(store.getState()).toMatchObject({
      phase: "error",
      dialogOpen: true,
      update: available,
      error: "close interrupted",
    });

    vi.mocked(api.check).mockResolvedValueOnce(null);
    await store.getState().check("manual");
    expect(api.close).toHaveBeenNthCalledWith(2, available.rid);
    expect(store.getState()).toMatchObject({
      phase: "upToDate",
      dialogOpen: true,
      update: null,
      error: null,
    });
  });

  it("shares one native close across double dismiss and claims closing synchronously", async () => {
    const closing = deferred<void>();
    const api = backend({
      check: vi.fn().mockResolvedValue(available),
      close: vi.fn().mockReturnValue(closing.promise),
    });
    const store = createUpdateStore(api);
    await store.getState().check("manual");

    const first = store.getState().dismiss();
    const second = store.getState().dismiss();
    expect(api.close).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({ phase: "closing", update: available });

    closing.resolve();
    await Promise.all([first, second]);
    expect(store.getState()).toMatchObject({ phase: "idle", update: null, dialogOpen: false });
  });

  it("does not interleave a manual check with an in-flight RID close", async () => {
    const closing = deferred<void>();
    const api = backend({
      check: vi.fn().mockResolvedValueOnce(available),
      close: vi.fn().mockReturnValue(closing.promise),
    });
    const store = createUpdateStore(api);
    await store.getState().check("manual");

    const dismiss = store.getState().dismiss();
    await store.getState().check("manual");
    expect(api.check).toHaveBeenCalledOnce();
    expect(api.close).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({ phase: "closing", update: available });

    closing.resolve();
    await dismiss;
    vi.mocked(api.check).mockResolvedValueOnce(null);
    await store.getState().check("manual");
    expect(api.check).toHaveBeenCalledTimes(2);
    expect(store.getState()).toMatchObject({ phase: "upToDate", update: null });
  });

  it("promotes an in-flight background check when the user checks manually", async () => {
    const pending = deferred<typeof available | null>();
    const api = backend({ check: vi.fn().mockReturnValue(pending.promise) });
    const store = createUpdateStore(api);
    const background = store.getState().check("background");

    await store.getState().check("manual");
    expect(api.check).toHaveBeenCalledOnce();
    expect(store.getState()).toMatchObject({
      phase: "checking",
      source: "manual",
      dialogOpen: true,
    });

    pending.resolve(null);
    await background;
    expect(store.getState()).toMatchObject({ phase: "upToDate", dialogOpen: true });
  });

  it("keeps install failure visible with the manual Releases fallback", async () => {
    const api = backend({
      check: vi.fn().mockResolvedValue(available),
      install: vi.fn().mockRejectedValue(new Error("project save failed")),
    });
    const store = createUpdateStore(api);
    await store.getState().check("manual");
    await store.getState().install();
    expect(store.getState()).toMatchObject({
      phase: "error",
      dialogOpen: true,
      update: null,
      error: "project save failed",
    });
  });
});
