/**
 * Tiny bounded LRU cache. Map iteration order is insertion order, so the oldest
 * live key is always `keys().next()`; a `get`/`set` on an existing key refreshes
 * its recency by re-inserting it at the end. Used for the media-panel thumbnail
 * path cache so scrolling a long library can't grow memory without limit
 * (evicted entries simply re-request a disk-cached path later).
 */
export class BoundedCache<V> {
  private readonly store = new Map<string, V>();

  constructor(private readonly max: number) {}

  has(key: string): boolean {
    return this.store.has(key);
  }

  /** Get a value, refreshing its recency. `undefined` when absent. */
  get(key: string): V | undefined {
    if (!this.store.has(key)) return undefined;
    const value = this.store.get(key) as V;
    this.store.delete(key);
    this.store.set(key, value);
    return value;
  }

  /** Insert/update, evicting the least-recently-used entry when over capacity. */
  set(key: string, value: V): void {
    this.store.delete(key);
    this.store.set(key, value);
    if (this.store.size > this.max) {
      const oldest = this.store.keys().next().value;
      if (oldest !== undefined) this.store.delete(oldest);
    }
  }

  get size(): number {
    return this.store.size;
  }
}
