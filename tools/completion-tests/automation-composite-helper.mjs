import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const owner = "automation_children_are_atomic_reviewable_and_command_routed";

export function runAutomationCompositeOwner() {
  const result = spawnSync(
    "cargo",
    ["test", "-p", "opentake-agent", "--test", "editing_automation_acceptance", owner, "--", "--exact"],
    { cwd: root, encoding: "utf8", timeout: 10 * 60 * 1000 },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  assert.equal(result.status, 0, `${owner} failed\n${output}`);
  assert.match(
    output,
    /test result: ok\.[^\n]*1 passed; 0 failed/,
    `${owner} did not execute exactly once\n${output}`,
  );
}
