import { describe, expect, it, vi } from "vitest";
import {
  derivedResourceKinds,
  DerivedResourceScheduler,
  type DerivedResourceRequest,
} from "./derivedResourceScheduler";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function requestText(
  scheduler: DerivedResourceScheduler,
  request: Omit<DerivedResourceRequest<string | null>, "kind">,
) {
  return scheduler.request({ ...request, kind: derivedResourceKinds.previewPoster });
}

describe("DerivedResourceScheduler", () => {
  it("enforces shared active and pending limits across derived resource kinds", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 2, maxPending: 2 });
    scheduler.activateProject(1);
    const jobs = new Map(
      ["waveform:a", "thumbnail:b", "search:c", "poster:d"].map((key) => [
        key,
        deferred<string>(),
      ]),
    );
    const started: string[] = [];
    const request = (key: string) =>
      requestText(scheduler, {
        projectEpoch: 1,
        key,
        run: () => {
          started.push(key);
          return jobs.get(key)!.promise;
        },
      });

    const a = request("waveform:a");
    const b = request("thumbnail:b");
    const c = request("search:c");
    const d = request("poster:d");
    const overflow = request("thumbnail:overflow");

    expect(started).toEqual(["waveform:a", "thumbnail:b"]);
    expect(scheduler.stats()).toEqual({
      active: 2,
      pending: 2,
      inFlight: 4,
      projectEpoch: 1,
    });
    expect(overflow.admitted).toBe(false);
    await expect(overflow.promise).resolves.toBeNull();

    jobs.get("waveform:a")!.resolve("a");
    jobs.get("thumbnail:b")!.resolve("b");
    await flushMicrotasks();
    expect(started).toEqual(["waveform:a", "thumbnail:b", "search:c", "poster:d"]);

    jobs.get("search:c")!.resolve("c");
    jobs.get("poster:d")!.resolve("d");
    await expect(Promise.all([a.promise, b.promise, c.promise, d.promise])).resolves.toEqual([
      "a",
      "b",
      "c",
      "d",
    ]);
    expect(scheduler.stats()).toEqual({
      active: 0,
      pending: 0,
      inFlight: 0,
      projectEpoch: 1,
    });
  });

  it("single-flights identical keys while keeping subscriber cancellation independent", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    scheduler.activateProject(4);
    const work = deferred<string>();
    const firstRun = vi.fn(() => work.promise);
    const duplicateRun = vi.fn(() => Promise.resolve("wrong"));

    const first = requestText(scheduler, { projectEpoch: 4, key: "waveform:same", run: firstRun });
    const duplicate = requestText(scheduler, {
      projectEpoch: 4,
      key: "waveform:same",
      run: duplicateRun,
    });
    first.cancel();

    expect(firstRun).toHaveBeenCalledTimes(1);
    expect(duplicateRun).not.toHaveBeenCalled();
    await expect(first.promise).resolves.toBeNull();
    work.resolve("shared");
    await expect(duplicate.promise).resolves.toBe("shared");
    expect(scheduler.stats().active).toBe(0);
  });

  it("admits fresh work for a key after the previous request settles", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    scheduler.activateProject(5);
    const first = requestText(scheduler, {
      projectEpoch: 5,
      key: "thumbnail:reusable",
      run: () => Promise.resolve("first"),
    });
    await expect(first.promise).resolves.toBe("first");
    const secondRun = vi.fn(() => Promise.resolve("second"));
    const second = requestText(scheduler, {
      projectEpoch: 5,
      key: "thumbnail:reusable",
      run: secondRun,
    });

    await expect(second.promise).resolves.toBe("second");
    expect(secondRun).toHaveBeenCalledTimes(1);
  });

  it("keeps an active physical flight single-keyed across unmount and immediate remount", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 2, maxPending: 2 });
    scheduler.activateProject(6);
    const work = deferred<string>();
    const firstRun = vi.fn(() => work.promise);
    const duplicateRun = vi.fn(() => Promise.resolve("duplicate"));
    const first = requestText(scheduler, {
      projectEpoch: 6,
      key: "waveform:remount",
      run: firstRun,
    });

    first.cancel();
    await expect(first.promise).resolves.toBeNull();
    expect(scheduler.stats().inFlight).toBe(0);
    const remounted = requestText(scheduler, {
      projectEpoch: 6,
      key: "waveform:remount",
      run: duplicateRun,
    });

    expect(firstRun).toHaveBeenCalledTimes(1);
    expect(duplicateRun).not.toHaveBeenCalled();
    work.resolve("reused");
    await expect(remounted.promise).resolves.toBe("reused");
  });

  it("removes a cancelled pending request before it can start", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    scheduler.activateProject(2);
    const active = deferred<string>();
    const queuedRun = vi.fn(() => Promise.resolve("queued"));
    const running = requestText(scheduler, {
      projectEpoch: 2,
      key: "waveform:active",
      run: () => active.promise,
    });
    const queued = requestText(scheduler, {
      projectEpoch: 2,
      key: "thumbnail:queued",
      run: queuedRun,
    });

    queued.cancel();
    await expect(queued.promise).resolves.toBeNull();
    expect(scheduler.stats().pending).toBe(0);
    active.resolve("active");
    await expect(running.promise).resolves.toBe("active");
    expect(queuedRun).not.toHaveBeenCalled();
  });

  it("cancels old-project subscribers and queued work without publishing stale results", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 2 });
    scheduler.activateProject(10);
    const oldActive = deferred<string>();
    const oldQueuedRun = vi.fn(() => Promise.resolve("old queued"));
    const current = deferred<string>();
    const currentRun = vi.fn(() => current.promise);
    const activeHandle = requestText(scheduler, {
      projectEpoch: 10,
      key: "poster:old-active",
      run: () => oldActive.promise,
    });
    const queuedHandle = requestText(scheduler, {
      projectEpoch: 10,
      key: "waveform:old-queued",
      run: oldQueuedRun,
    });

    scheduler.activateProject(11);
    const currentHandle = requestText(scheduler, {
      projectEpoch: 11,
      key: "poster:current",
      run: currentRun,
    });

    await expect(activeHandle.promise).resolves.toBeNull();
    await expect(queuedHandle.promise).resolves.toBeNull();
    expect(oldQueuedRun).not.toHaveBeenCalled();
    expect(currentRun).not.toHaveBeenCalled();
    expect(scheduler.stats()).toEqual({
      active: 1,
      pending: 1,
      inFlight: 1,
      projectEpoch: 11,
    });

    oldActive.resolve("stale");
    await flushMicrotasks();
    expect(currentRun).toHaveBeenCalledTimes(1);
    current.resolve("current");
    await expect(currentHandle.promise).resolves.toBe("current");
  });

  it("makes preview poster requests latest-wins and aborts the superseded task", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 2 });
    scheduler.activateProject(3);
    const oldWork = deferred<string>();
    const newWork = deferred<string>();
    let oldSignal: AbortSignal | null = null;
    const newRun = vi.fn(() => newWork.promise);
    const oldHandle = requestText(scheduler, {
      projectEpoch: 3,
      key: "poster:old",
      latestGroup: "preview-poster",
      priority: "interactive",
      run: (signal) => {
        oldSignal = signal;
        return oldWork.promise;
      },
    });
    const newHandle = requestText(scheduler, {
      projectEpoch: 3,
      key: "poster:new",
      latestGroup: "preview-poster",
      priority: "interactive",
      run: newRun,
    });

    await expect(oldHandle.promise).resolves.toBeNull();
    expect(oldSignal?.aborted).toBe(true);
    expect(newRun).not.toHaveBeenCalled();
    oldWork.resolve("stale");
    await flushMicrotasks();
    expect(newRun).toHaveBeenCalledTimes(1);
    newWork.resolve("fresh");
    await expect(newHandle.promise).resolves.toBe("fresh");
  });

  it("runs interactive preview work before already queued background work", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 2 });
    scheduler.activateProject(8);
    const active = deferred<string>();
    const background = deferred<string>();
    const preview = deferred<string>();
    const started: string[] = [];
    const running = requestText(scheduler, {
      projectEpoch: 8,
      key: "thumbnail:active",
      priority: "visible",
      run: () => {
        started.push("active");
        return active.promise;
      },
    });
    const queuedBackground = requestText(scheduler, {
      projectEpoch: 8,
      key: "waveform:background",
      priority: "background",
      run: () => {
        started.push("background");
        return background.promise;
      },
    });
    const queuedPreview = requestText(scheduler, {
      projectEpoch: 8,
      key: "poster:interactive",
      priority: "interactive",
      run: () => {
        started.push("preview");
        return preview.promise;
      },
    });

    active.resolve("active");
    await flushMicrotasks();
    expect(started).toEqual(["active", "preview"]);
    preview.resolve("preview");
    await flushMicrotasks();
    expect(started).toEqual(["active", "preview", "background"]);
    background.resolve("background");
    await expect(
      Promise.all([running.promise, queuedPreview.promise, queuedBackground.promise]),
    ).resolves.toEqual(["active", "preview", "background"]);
  });

  it("admits interactive work by evicting a lower-priority queued request at capacity", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    scheduler.activateProject(9);
    const active = deferred<string>();
    const preview = deferred<string>();
    const running = requestText(scheduler, {
      projectEpoch: 9,
      key: "thumbnail:active",
      priority: "visible",
      run: () => active.promise,
    });
    const backgroundRun = vi.fn(() => Promise.resolve("background"));
    const background = requestText(scheduler, {
      projectEpoch: 9,
      key: "waveform:queued",
      priority: "background",
      run: backgroundRun,
    });
    const interactiveRun = vi.fn(() => preview.promise);
    const interactive = requestText(scheduler, {
      projectEpoch: 9,
      key: "poster:latest",
      priority: "interactive",
      latestGroup: "preview-poster",
      run: interactiveRun,
    });

    expect(interactive.admitted).toBe(true);
    await expect(background.promise).resolves.toBeNull();
    expect(backgroundRun).not.toHaveBeenCalled();
    expect(scheduler.stats().pending).toBe(1);
    active.resolve("active");
    await flushMicrotasks();
    expect(interactiveRun).toHaveBeenCalledTimes(1);
    preview.resolve("preview");
    await expect(Promise.all([running.promise, interactive.promise])).resolves.toEqual([
      "active",
      "preview",
    ]);
  });

  it("keeps the newest interactive poster admissible when the queue is all interactive", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    scheduler.activateProject(12);
    const oldPoster = deferred<string>();
    const newPoster = deferred<string>();
    const old = requestText(scheduler, {
      projectEpoch: 12,
      key: "poster:old",
      priority: "interactive",
      latestGroup: "preview-poster",
      run: () => oldPoster.promise,
    });
    const queuedRun = vi.fn(() => Promise.resolve("queued"));
    const queued = requestText(scheduler, {
      projectEpoch: 12,
      key: "interactive:queued",
      priority: "interactive",
      run: queuedRun,
    });
    const newRun = vi.fn(() => newPoster.promise);
    const current = requestText(scheduler, {
      projectEpoch: 12,
      key: "poster:current",
      priority: "interactive",
      latestGroup: "preview-poster",
      run: newRun,
    });

    expect(current.admitted).toBe(true);
    await expect(old.promise).resolves.toBeNull();
    await expect(queued.promise).resolves.toBeNull();
    expect(queuedRun).not.toHaveBeenCalled();
    oldPoster.resolve("stale");
    await flushMicrotasks();
    expect(newRun).toHaveBeenCalledTimes(1);
    newPoster.resolve("current");
    await expect(current.promise).resolves.toBe("current");
  });

  it("does not coalesce the same raw key across incompatible result kinds", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 2, maxPending: 2 });
    scheduler.activateProject(13);
    const text = deferred<string>();
    const numbers = deferred<number[]>();
    const textRun = vi.fn(() => text.promise);
    const numberRun = vi.fn(() => numbers.promise);

    const textHandle = requestText(scheduler, {
      projectEpoch: 13,
      key: "same-raw-key",
      run: textRun,
    });
    const numberHandle = scheduler.request({
      projectEpoch: 13,
      kind: derivedResourceKinds.waveform,
      key: "same-raw-key",
      run: numberRun,
    });

    expect(textRun).toHaveBeenCalledTimes(1);
    expect(numberRun).toHaveBeenCalledTimes(1);
    text.resolve("text-result");
    numbers.resolve([1, 2, 3]);
    await expect(textHandle.promise).resolves.toBe("text-result");
    await expect(numberHandle.promise).resolves.toEqual([1, 2, 3]);
  });

  it("binds each scheduler-owned kind token to one invariant result type", () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 1, maxPending: 1 });
    if (false) {
      scheduler.request<string | null>({
        projectEpoch: 13,
        // @ts-expect-error waveform jobs cannot claim an image-path result type.
        kind: derivedResourceKinds.waveform,
        key: "compile-time-contract",
        run: () => Promise.resolve("wrong-kind"),
      });
    }
    expect(true).toBe(true);
  });

  it("reuses an abandoned active A flight across a latest-group A-B-A switch", async () => {
    const scheduler = new DerivedResourceScheduler({ maxActive: 2, maxPending: 2 });
    scheduler.activateProject(14);
    const firstAWork = deferred<string>();
    const bWork = deferred<string>();
    const duplicateAWork = deferred<string>();
    const firstARun = vi.fn(() => firstAWork.promise);
    const bRun = vi.fn(() => bWork.promise);
    const duplicateARun = vi.fn(() => duplicateAWork.promise);

    const firstA = requestText(scheduler, {
      projectEpoch: 14,
      key: "poster:a",
      latestGroup: "preview-poster",
      priority: "interactive",
      run: firstARun,
    });
    const b = requestText(scheduler, {
      projectEpoch: 14,
      key: "poster:b",
      latestGroup: "preview-poster",
      priority: "interactive",
      run: bRun,
    });
    const currentA = requestText(scheduler, {
      projectEpoch: 14,
      key: "poster:a",
      latestGroup: "preview-poster",
      priority: "interactive",
      run: duplicateARun,
    });

    await expect(firstA.promise).resolves.toBeNull();
    await expect(b.promise).resolves.toBeNull();
    firstAWork.resolve("retained-a");
    await flushMicrotasks();
    expect(firstARun).toHaveBeenCalledTimes(1);
    expect(bRun).toHaveBeenCalledTimes(1);
    expect(duplicateARun).not.toHaveBeenCalled();
    await expect(currentA.promise).resolves.toBe("retained-a");
    bWork.resolve("stale-b");
    await flushMicrotasks();
  });
});
