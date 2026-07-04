/**
 * rustEngineEnabled flag logic (default-OFF opt-in). The proven legacy <video>
 * stack is the shipped preview path; only an explicit "1" opts INTO the Rust
 * streaming engine. Every other state ("0", a missing key, a stray value, an
 * unreadable / undefined localStorage) resolves to the legacy default.
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

describe("rustEngineEnabled (default-off, opt-in)", () => {
  it("defaults OFF (legacy) when the flag key is absent", () => {
    vi.stubGlobal("localStorage", makeLocalStorage());
    expect(rustEngineEnabled()).toBe(false);
  });

  it('opts INTO the Rust engine only for the exact string "1"', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "1" }));
    expect(rustEngineEnabled()).toBe(true);
  });

  it('keeps "0" as legacy', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "0" }));
    expect(rustEngineEnabled()).toBe(false);
  });

  it('treats any other stray value as legacy (only "1" enables)', () => {
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "rust" }));
    expect(rustEngineEnabled()).toBe(false);
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "true" }));
    expect(rustEngineEnabled()).toBe(false);
    vi.stubGlobal("localStorage", makeLocalStorage({ [KEY]: "" }));
    expect(rustEngineEnabled()).toBe(false);
  });

  it("defaults OFF when localStorage is undefined (non-DOM context)", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(rustEngineEnabled()).toBe(false);
  });

  it("defaults OFF when localStorage.getItem throws (locked-down context)", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("access denied");
      },
    } as unknown as Storage);
    expect(rustEngineEnabled()).toBe(false);
  });
});
