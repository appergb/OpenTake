import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
  assert.match(output, expected, `${label} did not execute its exact owner\n${output}`);
}

export function runTauriCommandContractOwners() {
  run(
    "cargo",
    [
      "test",
      "-p",
      "opentake-tauri",
      "commands::edit_request_serde_tests::every_edit_request_maps_to_exact_edit_command",
      "--",
      "--exact",
    ],
    /1 passed; 0 failed/,
    "Rust edit-request contract",
  );
  run(
    "cargo",
    ["test", "-p", "opentake-core", "dto::tests::edit_apply_handler_maps_validation_error", "--", "--exact"],
    /1 passed; 0 failed/,
    "core typed-error atomicity contract",
  );
  run(
    "cargo",
    [
      "test",
      "-p",
      "opentake-core",
      "dto::tests::internal_command_error_does_not_expose_project_paths",
      "--",
      "--exact",
    ],
    /1 passed; 0 failed/,
    "core internal-error redaction contract",
  );
  run(
    "pnpm",
    ["-C", "web", "exec", "vitest", "run", "src/lib/api.commandContract.test.ts", "src/lib/api.editApply.test.ts"],
    /Tests\s+3 passed/,
    "frontend invoke and typed-error contract",
  );

  const coreSpec = readFileSync(`${root}/docs/specs/core/6-tauri-commands.md`, "utf8");
  const frontendSpec = readFileSync(`${root}/docs/specs/frontend/11-tauri.md`, "utf8");
  assert.match(coreSpec, /41 分支严格联合/);
  assert.match(coreSpec, /TauriCommandError/);
  assert.match(frontendSpec, /frontend_command_names_match_invoke_handler/);
  assert.doesNotMatch(frontendSpec, /`export_start`|`clip_copy`|`project_save_as`/);
}
