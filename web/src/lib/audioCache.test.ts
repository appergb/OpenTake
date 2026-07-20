import { describe, expect, it } from "vitest";
import { AudioBufferCache } from "./audioCache";

/**
 * Minimal `AudioBuffer` stub for testing the cache logic. The real `AudioBuffer`
 * (Web Audio API) is unavailable in vitest's default Node environment, but the
 * cache only stores/retrieves opaque references — it never inspects the buffer's
 * contents — so any object cast to `AudioBuffer` suffices.
 */
function fakeBuffer(id: string): AudioBuffer {
  return { id } as unknown as AudioBuffer;
}

describe("AudioBufferCache", () => {
  it("returns null on miss", () => {
    const cache = new AudioBufferCache();
    expect(cache.getCached("asset1", 0)).toBeNull();
    expect(cache.getCached("asset1", 10.5)).toBeNull();
  });

  it("stores and retrieves a buffer by mediaRef + startTime", () => {
    const cache = new AudioBufferCache();
    const buf = fakeBuffer("b1");
    cache.setCached("asset1", 5, buf);
    // Exact start time → hit.
    expect(cache.getCached("asset1", 5)).toBe(buf);
    // Within the same 1s chunk boundary (floor) → same hit.
    expect(cache.getCached("asset1", 5.7)).toBe(buf);
    // Different chunk → miss.
    expect(cache.getCached("asset1", 6)).toBeNull();
  });

  it("separates caches per mediaRef", () => {
    const cache = new AudioBufferCache();
    const a = fakeBuffer("a");
    const b = fakeBuffer("b");
    cache.setCached("asset1", 0, a);
    cache.setCached("asset2", 0, b);
    expect(cache.getCached("asset1", 0)).toBe(a);
    expect(cache.getCached("asset2", 0)).toBe(b);
    expect(cache.getCached("asset1", 0)).not.toBe(b);
  });

  it("evicts the oldest entry when full (LRU)", () => {
    const cache = new AudioBufferCache(3);
    const b0 = fakeBuffer("0");
    const b1 = fakeBuffer("1");
    const b2 = fakeBuffer("2");
    const b3 = fakeBuffer("3");
    cache.setCached("a", 0, b0);
    cache.setCached("a", 1, b1);
    cache.setCached("a", 2, b2);
    // Cache is full (3 entries). Inserting b3 should evict b0 (oldest).
    cache.setCached("a", 3, b3);
    expect(cache.size).toBe(3);
    expect(cache.getCached("a", 0)).toBeNull(); // evicted
    expect(cache.getCached("a", 1)).toBe(b1); // still resident
    expect(cache.getCached("a", 2)).toBe(b2);
    expect(cache.getCached("a", 3)).toBe(b3);
  });

  it("marks an entry as recently used on get (LRU recency)", () => {
    const cache = new AudioBufferCache(3);
    const b0 = fakeBuffer("0");
    const b1 = fakeBuffer("1");
    const b2 = fakeBuffer("2");
    const b3 = fakeBuffer("3");
    cache.setCached("a", 0, b0);
    cache.setCached("a", 1, b1);
    cache.setCached("a", 2, b2);
    // Touch b0 → it becomes most-recently-used, so b1 is now the oldest.
    expect(cache.getCached("a", 0)).toBe(b0);
    // Insert b3 → evicts the oldest (b1, since b0 was just touched).
    cache.setCached("a", 3, b3);
    expect(cache.getCached("a", 0)).toBe(b0); // still resident (touched)
    expect(cache.getCached("a", 1)).toBeNull(); // evicted
    expect(cache.getCached("a", 2)).toBe(b2);
    expect(cache.getCached("a", 3)).toBe(b3);
  });

  it("replaces an existing key in place without growing size", () => {
    const cache = new AudioBufferCache(3);
    const old = fakeBuffer("old");
    const next = fakeBuffer("next");
    cache.setCached("a", 5, old);
    expect(cache.size).toBe(1);
    // Same key, new buffer → replaces, doesn't evict or double-count.
    cache.setCached("a", 5, next);
    expect(cache.size).toBe(1);
    expect(cache.getCached("a", 5)).toBe(next);
    expect(cache.getCached("a", 5)).not.toBe(old);
  });

  it("clear drops all entries", () => {
    const cache = new AudioBufferCache();
    cache.setCached("a", 0, fakeBuffer("0"));
    cache.setCached("a", 1, fakeBuffer("1"));
    expect(cache.size).toBe(2);
    cache.clear();
    expect(cache.size).toBe(0);
    expect(cache.getCached("a", 0)).toBeNull();
  });
});
