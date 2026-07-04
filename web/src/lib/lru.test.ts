import { describe, expect, it } from "vitest";
import { BoundedCache } from "./lru";

describe("BoundedCache", () => {
  it("stores and retrieves values", () => {
    const c = new BoundedCache<number>(3);
    c.set("a", 1);
    expect(c.has("a")).toBe(true);
    expect(c.get("a")).toBe(1);
    expect(c.get("missing")).toBeUndefined();
    expect(c.size).toBe(1);
  });

  it("evicts the least-recently-used entry past capacity", () => {
    const c = new BoundedCache<number>(2);
    c.set("a", 1);
    c.set("b", 2);
    c.set("c", 3); // evicts "a" (oldest)
    expect(c.has("a")).toBe(false);
    expect(c.has("b")).toBe(true);
    expect(c.has("c")).toBe(true);
    expect(c.size).toBe(2);
  });

  it("get refreshes recency so the touched key survives eviction", () => {
    const c = new BoundedCache<number>(2);
    c.set("a", 1);
    c.set("b", 2);
    expect(c.get("a")).toBe(1); // touch "a" → "b" is now oldest
    c.set("c", 3); // evicts "b", not "a"
    expect(c.has("a")).toBe(true);
    expect(c.has("b")).toBe(false);
    expect(c.has("c")).toBe(true);
  });

  it("re-setting an existing key refreshes recency without growing size", () => {
    const c = new BoundedCache<number>(2);
    c.set("a", 1);
    c.set("b", 2);
    c.set("a", 11); // update + touch → "b" oldest
    expect(c.size).toBe(2);
    expect(c.get("a")).toBe(11);
    c.set("c", 3); // evicts "b"
    expect(c.has("b")).toBe(false);
    expect(c.has("a")).toBe(true);
  });

  it("preserves a null value distinctly from absence", () => {
    const c = new BoundedCache<string | null>(2);
    c.set("a", null);
    expect(c.has("a")).toBe(true);
    expect(c.get("a")).toBeNull();
  });
});
