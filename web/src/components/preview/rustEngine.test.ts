/**
 * rustEngineEnabled flag logic (default-ON escape hatch). The Rust streaming
 * engine is the shipped preview path; only an explicit "0" opts back out to the
 * legacy <video> stack, "1" force-enables, and every other state (missing key,
 * unreadable / undefined localStorage) resolves to ON.
 *
 * vitest's default node environment has no localStorage, so each case injects a
 * fresh in-memory stub (or removes it) — the same pattern favorites.test.ts uses.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { rustEngineEnabled } from "./rustEngine";

function makeLocalStorage(seed?: Record<string, string>): Storage {
  const map = new Map<string, string>(seed ? Object.entries(seed) : []);
  return {
    getItem: (k) => (map.has(k) ? (map.get(k) as string) : null),
    setItem: (k, v) => void map.set(k, String(v)),
    removeItem: (k) => void map.delete(k),
    clear: () => map.clear(),
    key: (i) => [...map.keys()][i] ?? null,
    get length() {
      return map.size;
    },
  } as Storage;
}

const KEY = "opentake.rustEngine";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("rustEngineEnabled (default-on)", () => {
  it("defaults ON when the flag key is absent", () => {
    vi.stubGlobal("localStorage", makeLocalStorage());
    expect(rustEngineEnabled()).toBe(true);
  });

  it('opts OUT to legacy only for the exact string "0"', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "0" }));
    expect(rustEngineEnabled()).toBe(false);
  });

  it('keeps "1" as a force-ON', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "1" }));
    expect(rustEngineEnabled()).toBe(true);
  });

  it('treats any other stray value as ON (only "0" disables)', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "legacy" }));
    expect(rustEngineEnabled()).toBe(true);
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "false" }));
    expect(rustEngineEnabled()).toBe(true);
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "" }));
    expect(rustEngineEnabled()).toBe(true);
  });

  it("defaults ON when localStorage is undefined (non-DOM context)", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(rustEngineEnabled()).toBe(true);
  });

  it("defaults ON when localStorage.getItem throws (locked-down context)", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("access denied");
      },
    } as unknown as Storage);
    expect(rustEngineEnabled()).toBe(true);
  });
});
