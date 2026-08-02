import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));

function run(command, args, expected, label) {
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
    timeout: 10 * 60 * 1000,
  });
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  assert.equal(result.status, 0, `${label} failed\n${output}`);
  assert.match(output, expected, `${label} did not execute its owning tests\n${output}`);
}

export function runBeatAutoCutOwners() {
  run(
    "cargo",
    ["test", "-p", "opentake-media", "analysis::beat::tests::"],
    /test result: ok\.[^\n]*2 passed; 0 failed/,
    "beat detector owners",
  );
  run(
    "cargo",
    ["test", "-p", "opentake-agent", "auto_cut_to_beats_write_"],
    /test result: ok\.[^\n]*2 passed; 0 failed/,
    "auto-cut dispatcher owners",
  );
  run(
    "pnpm",
    ["-C", "web", "exec", "vitest", "run", "src/store/editActions.test.ts", "-t", "accepts one atomic request"],
    /Tests\s+1 passed/,
    "frontend atomic automation owner",
  );
}
