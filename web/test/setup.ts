import { Storage } from "happy-dom";

// Node's native Web Storage is shared across files/workers. Give each test
// environment its own browser storage before persisted stores are imported.
for (const name of ["localStorage", "sessionStorage"] as const) {
  const storage = new Storage();
  Object.defineProperty(globalThis, name, { configurable: true, value: storage });
  if (typeof window !== "undefined" && window !== globalThis) {
    Object.defineProperty(window, name, { configurable: true, value: storage });
  }
}
