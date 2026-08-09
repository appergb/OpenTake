import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const repositoryRoot = new URL("../../../", import.meta.url);
const tauriLib = readFileSync(new URL("src-tauri/src/lib.rs", repositoryRoot), "utf8");
const frontendBridges = ["web/src/lib/api.ts", "web/src/lib/libraryApi.ts", "web/src/lib/haptic.ts"]
  .map((path) => readFileSync(new URL(path, repositoryRoot), "utf8"))
  .join("\n");

function registeredCommandNames(source: string): Set<string> {
  const handlerBody = source.match(
    /\.invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)/,
  )?.[1];
  expect(handlerBody, "Tauri invoke handler must remain statically discoverable").toBeDefined();
  return new Set(
    [...(handlerBody ?? "").matchAll(/(?:[A-Za-z_]\w*::)+(\w+),/g)].map(
      ([, command]) => command,
    ),
  );
}

function frontendCommandNames(source: string): Set<string> {
  const invokeImplNames = [...source.matchAll(/invokeImpl(?:<[\s\S]*?>)?\(\s*"([a-z0-9_]+)"/g)].map(
    ([, command]) => command,
  );
  const directInvokeNames = [...source.matchAll(/core\.invoke[\s\S]{0,120}?\(\s*"([a-z0-9_]+)"/g)].map(
    ([, command]) => command,
  );
  return new Set([...invokeImplNames, ...directInvokeNames]);
}

describe("Tauri command registry contract", () => {
  it("frontend_command_names_match_invoke_handler", () => {
    const registered = registeredCommandNames(tauriLib);
    const invoked = frontendCommandNames(frontendBridges);
    const missingHandlers = [...invoked].filter((command) => !registered.has(command)).sort();

    expect(invoked.size).toBeGreaterThan(60);
    expect(missingHandlers).toEqual([]);
  });
});
