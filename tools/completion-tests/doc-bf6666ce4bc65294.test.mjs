import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));

function runFocusedRustTest(packageName, testName) {
  const result = spawnSync(
    "cargo",
    ["test", "-p", packageName, testName, "--", "--exact", "--test-threads=1"],
    {
      cwd: root,
      encoding: "utf8",
      timeout: 10 * 60 * 1000,
    },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  assert.equal(
    result.status,
    0,
    `${packageName}#${testName} failed\n${output}`,
  );
  assert.match(
    output,
    /test result: ok\.[^\n]*1 passed; 0 failed/,
    `${packageName}#${testName} did not execute exactly one owning test\n${output}`,
  );
}

test(
  "completion_bf6666ce4bc65294_resolve_media_ref_to_an_expected_path_while_dist",
  { timeout: 30 * 60 * 1000 },
  () => {
    runFocusedRustTest("opentake-domain", "media::tests::resolver_expected_path_external");
    runFocusedRustTest("opentake-tauri", "media::tests::dto_reports_file_size_for_present_source");
    runFocusedRustTest("opentake-tauri", "media::tests::relink_keeps_same_id_and_clears_missing");
  },
);
