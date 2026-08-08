/**
 * Shared front-end admission control for derived media work that still enters
 * Rust through synchronous Tauri commands (waveforms, thumbnails and preview
 * posters). It bounds request fan-out before IPC, coalesces identical work and
 * makes queued/stale consumers cancellable.
 *
 * An AbortSignal is supplied to each task so future cancellable transports can
 * stop active work. The current Tauri commands ignore it; active work therefore
 * keeps its slot until Rust resolves, while its stale result is never delivered.
 */

export type DerivedResourcePriority = "background" | "visible" | "interactive";

export interface DerivedResourceSchedulerOptions {
  maxActive: number;
  maxPending: number;
}

const RESOURCE_KIND_ID: unique symbol = Symbol("derived-resource-kind-id");
const RESOURCE_KIND_RESULT: unique symbol = Symbol("derived-resource-kind-result");

/**
 * An invariant, scheduler-owned result token. Callers cannot accidentally use
 * one raw string identity for two incompatible promise result types.
 */
export interface DerivedResourceKind<T> {
  readonly [RESOURCE_KIND_ID]: string;
  readonly [RESOURCE_KIND_RESULT]: (value: T) => T;
}

let nextResourceKindId = 0;

function defineResourceKind<T>(name: string): DerivedResourceKind<T> {
  return {
    [RESOURCE_KIND_ID]: `${name}:${nextResourceKindId++}`,
    [RESOURCE_KIND_RESULT]: (value: T) => value,
  };
}

export const derivedResourceKinds = {
  thumbnail: defineResourceKind<string | null>("thumbnail"),
  searchThumbnail: defineResourceKind<string | null>("search-thumbnail"),
  previewPoster: defineResourceKind<string | null>("preview-poster"),
  waveform: defineResourceKind<number[] | null>("waveform"),
} as const;

export interface DerivedResourceRequest<T> {
  projectEpoch: number;
  kind: DerivedResourceKind<T>;
  key: string;
  run: (signal: AbortSignal) => Promise<T>;
  priority?: DerivedResourcePriority;
  /**
   * At most one logical request in this group remains publishable. A new key
   * cancels the previous group's consumers (used by the selected preview poster).
   */
  latestGroup?: string;
}

export interface DerivedResourceRequestHandle<T> {
  readonly admitted: boolean;
  readonly promise: Promise<T | null>;
  cancel: () => void;
}

export interface DerivedResourceSchedulerStats {
  active: number;
  pending: number;
  /** Current-project entries only; abandoned active work is excluded. */
  inFlight: number;
  projectEpoch: number | null;
}

type EntryState = "queued" | "active" | "discarded" | "done";

interface Subscriber<T> {
  settled: boolean;
  resolve: (value: T | null) => void;
  reject: (reason?: unknown) => void;
}

interface Entry<T> {
  identity: string;
  projectEpoch: number;
  key: string;
  latestGroup?: string;
  priority: DerivedResourcePriority;
  sequence: number;
  state: EntryState;
  controller: AbortController;
  run: (signal: AbortSignal) => Promise<T>;
  subscribers: Set<Subscriber<T>>;
}

const PRIORITY_RANK: Record<DerivedResourcePriority, number> = {
  background: 0,
  visible: 1,
  interactive: 2,
};

function positiveInteger(value: number, label: string): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${label} must be a positive integer`);
  }
  return value;
}

function identityFor<T>(
  projectEpoch: number,
  kind: DerivedResourceKind<T>,
  key: string,
): string {
  return `${projectEpoch}\u0000${kind[RESOURCE_KIND_ID]}\u0000${key}`;
}

function settledHandle<T>(): DerivedResourceRequestHandle<T> {
  return {
    admitted: false,
    promise: Promise.resolve(null),
    cancel: () => undefined,
  };
}

export class DerivedResourceScheduler {
  readonly #maxActive: number;
  readonly #maxPending: number;
  #active = 0;
  #projectEpoch: number | null = null;
  #sequence = 0;
  #queue: Array<Entry<unknown>> = [];
  #entries = new Map<string, Entry<unknown>>();

  constructor(options: DerivedResourceSchedulerOptions) {
    this.#maxActive = positiveInteger(options.maxActive, "maxActive");
    this.#maxPending = positiveInteger(options.maxPending, "maxPending");
  }

  /**
   * Atomically invalidates every queued/current subscriber from the previous
   * project. Already-running Rust commands cannot be killed by today's IPC, so
   * they retain an active slot until settlement but are removed from inFlight.
   */
  activateProject(projectEpoch: number): void {
    if (this.#projectEpoch === projectEpoch) return;
    this.#projectEpoch = projectEpoch;
    for (const entry of [...this.#entries.values()]) {
      if (entry.projectEpoch !== projectEpoch) this.#discardEntry(entry, false);
    }
    this.#pump();
  }

  request<T>(request: DerivedResourceRequest<T>): DerivedResourceRequestHandle<T> {
    if (this.#projectEpoch === null) this.activateProject(request.projectEpoch);
    if (request.projectEpoch !== this.#projectEpoch) return settledHandle<T>();

    const identity = identityFor(request.projectEpoch, request.kind, request.key);
    const existing = this.#entries.get(identity) as Entry<T> | undefined;

    if (request.latestGroup) {
      for (const entry of [...this.#entries.values()]) {
        if (entry !== existing && entry.latestGroup === request.latestGroup) {
          this.#discardEntry(entry, false);
        }
      }
    }
    if (existing) return this.#subscribe(existing);

    if (this.#active >= this.#maxActive && this.#queue.length >= this.#maxPending) {
      const incomingRank = PRIORITY_RANK[request.priority ?? "visible"];
      let candidate: Entry<unknown> | null = null;
      for (const queued of this.#queue) {
        const queuedRank = PRIORITY_RANK[queued.priority];
        // A direct user selection must remain latest-wins even when the whole
        // bounded queue is already interactive. Ordinary visible/background
        // work may only displace strictly lower-priority work.
        if (
          queuedRank > incomingRank ||
          (queuedRank === incomingRank && request.priority !== "interactive")
        ) {
          continue;
        }
        if (!candidate || queuedRank < PRIORITY_RANK[candidate.priority]) candidate = queued;
      }
      if (!candidate) return settledHandle<T>();
      this.#discardEntry(candidate, false);
    }

    const entry: Entry<T> = {
      identity,
      projectEpoch: request.projectEpoch,
      key: request.key,
      latestGroup: request.latestGroup,
      priority: request.priority ?? "visible",
      sequence: this.#sequence++,
      state: "queued",
      controller: new AbortController(),
      run: request.run,
      subscribers: new Set(),
    };
    this.#entries.set(identity, entry as Entry<unknown>);
    const handle = this.#subscribe(entry);
    if (this.#active < this.#maxActive) this.#start(entry as Entry<unknown>);
    else this.#enqueue(entry as Entry<unknown>);
    return handle;
  }

  stats(): DerivedResourceSchedulerStats {
    return {
      active: this.#active,
      pending: this.#queue.length,
      inFlight: [...this.#entries.values()].filter((entry) => entry.subscribers.size > 0).length,
      projectEpoch: this.#projectEpoch,
    };
  }

  #subscribe<T>(entry: Entry<T>): DerivedResourceRequestHandle<T> {
    let subscriber!: Subscriber<T>;
    const promise = new Promise<T | null>((resolve, reject) => {
      subscriber = { settled: false, resolve, reject };
      entry.subscribers.add(subscriber);
    });
    return {
      admitted: true,
      promise,
      cancel: () => {
        if (subscriber.settled) return;
        subscriber.settled = true;
        entry.subscribers.delete(subscriber);
        subscriber.resolve(null);
        if (entry.subscribers.size === 0 && entry.state === "queued") {
          this.#discardEntry(entry as Entry<unknown>);
        }
      },
    };
  }

  #enqueue(entry: Entry<unknown>): void {
    const rank = PRIORITY_RANK[entry.priority];
    const before = this.#queue.findIndex((candidate) => {
      const candidateRank = PRIORITY_RANK[candidate.priority];
      return candidateRank < rank ||
        (candidateRank === rank && candidate.sequence > entry.sequence);
    });
    if (before === -1) this.#queue.push(entry);
    else this.#queue.splice(before, 0, entry);
  }

  #start(entry: Entry<unknown>): void {
    if (entry.state !== "queued" || entry.subscribers.size === 0) {
      this.#discardEntry(entry);
      return;
    }
    entry.state = "active";
    this.#active += 1;
    let work: Promise<unknown>;
    try {
      work = entry.run(entry.controller.signal);
    } catch (error) {
      work = Promise.reject(error);
    }
    void Promise.resolve(work)
      .then(
        (value) => this.#settleEntry(entry, value),
        (error) => this.#rejectEntry(entry, error),
      )
      .finally(() => {
        this.#active = Math.max(0, this.#active - 1);
        entry.state = "done";
        if (this.#entries.get(entry.identity) === entry) this.#entries.delete(entry.identity);
        this.#pump();
      });
  }

  #settleEntry(entry: Entry<unknown>, value: unknown): void {
    if (entry.state !== "active") return;
    entry.state = "done";
    if (this.#entries.get(entry.identity) === entry) this.#entries.delete(entry.identity);
    for (const subscriber of entry.subscribers) {
      if (subscriber.settled) continue;
      subscriber.settled = true;
      subscriber.resolve(value);
    }
    entry.subscribers.clear();
  }

  #rejectEntry(entry: Entry<unknown>, error: unknown): void {
    if (entry.state !== "active") return;
    entry.state = "done";
    if (this.#entries.get(entry.identity) === entry) this.#entries.delete(entry.identity);
    for (const subscriber of entry.subscribers) {
      if (subscriber.settled) continue;
      subscriber.settled = true;
      subscriber.reject(error);
    }
    entry.subscribers.clear();
  }

  #discardEntry(entry: Entry<unknown>, pump = true): void {
    if (entry.state === "discarded" || entry.state === "done") return;
    const wasQueued = entry.state === "queued";
    entry.controller.abort();
    if (wasQueued) {
      entry.state = "discarded";
      const index = this.#queue.indexOf(entry);
      if (index >= 0) this.#queue.splice(index, 1);
      if (this.#entries.get(entry.identity) === entry) this.#entries.delete(entry.identity);
    }
    // An active Tauri command cannot currently be physically cancelled. Keep
    // its keyed entry until settlement so an A→B→A switch can re-subscribe
    // to that one physical flight instead of consuming a duplicate FFmpeg slot.
    for (const subscriber of entry.subscribers) {
      if (subscriber.settled) continue;
      subscriber.settled = true;
      subscriber.resolve(null);
    }
    entry.subscribers.clear();
    if (pump) this.#pump();
  }

  #pump(): void {
    while (this.#active < this.#maxActive) {
      const next = this.#queue.shift();
      if (!next) return;
      if (next.state !== "queued" || next.subscribers.size === 0) {
        this.#discardEntry(next, false);
        continue;
      }
      this.#start(next);
    }
  }
}

export const DERIVED_RESOURCE_MAX_ACTIVE = 4;
export const DERIVED_RESOURCE_MAX_PENDING = 64;

/** One admission domain prevents waveform/search/poster paths from each using
 * their own independent limit and multiplying the actual FFmpeg fan-out. */
export const derivedResourceScheduler = new DerivedResourceScheduler({
  maxActive: DERIVED_RESOURCE_MAX_ACTIVE,
  maxPending: DERIVED_RESOURCE_MAX_PENDING,
});
