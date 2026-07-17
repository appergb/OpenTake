#!/usr/bin/env ruby
# frozen_string_literal: true

require "json"
require "open3"
require "rbconfig"
require "tmpdir"

ROOT = File.expand_path("../..", __dir__)
POLICY = JSON.parse(File.read(File.join(ROOT, "scripts/c1b-evidence-policy.json")))
WORKFLOW = File.join(ROOT, ".github/workflows/ci.yml")
VALIDATOR = File.join(ROOT, "scripts/validate-c1b-ci.rb")
RED_HARNESS = File.join(ROOT, "scripts/run-c1b-windows-red.ps1")

def assert(condition, message)
  raise message unless condition
end

def run_validator(path, red_harness = RED_HARNESS)
  Open3.capture3(RbConfig.ruby, VALIDATOR, path, red_harness)
end

assert(POLICY.fetch("schema") == "opentake-c1b-evidence-policy-v1", "policy schema")
assert(POLICY.fetch("api_version") == "2026-03-10", "API version")
assert(POLICY.fetch("receipts").map { |row| row.fetch("id") }.sort ==
  %w[linux-x86_64 macos-native windows-x86_64], "receipt ids")
assert(POLICY.fetch("native_commands").length == 4, "native command count")

stdout, stderr, status = run_validator(WORKFLOW)
if !status.success? && stderr.include?("C1B CI validator not implemented")
  abort "C1B CI validator not implemented"
end
assert(status.success?, "canonical workflow rejected: #{stdout}#{stderr}")
assert(stdout.include?("c1b-ci-validation=ok"), "canonical success marker missing")

raw = File.read(WORKFLOW)
assert(raw.include?(%q{printf ' %q' "$@"}), "native gate logs lack an exact command marker")
red_harness_raw = File.read(RED_HARNESS)
assert(
  red_harness_raw.lines.map(&:strip).reject(&:empty?).last == "exit 0",
  "Windows expected-RED harness does not clear expected native failures"
)
mutations = {
  "pr-uses-merge" => ["github.event.pull_request.head.sha", "github.sha"],
  "checkout-not-bound" => ["ref: ${{ env.TARGET_SHA }}", "ref: main"],
  "checkout-shallow" => ["fetch-depth: 0", "fetch-depth: 1"],
  "checkout-persists-credentials" => ["persist-credentials: false", "persist-credentials: true"],
  "wrong-linux-runner" => ["runner: ubuntu-24.04", "runner: windows-2022"],
  "wrong-macos-arch" => ["expected_arch: ARM64", "expected_arch: X64"],
  "native-command-substitution" => [
    "run_gate cargo-clippy cargo clippy -p opentake-project --lib --tests -- -D warnings",
    "run_gate cargo-clippy cargo check -p opentake-project",
  ],
  "native-log-marker-removed" => [%q{printf ' %q' "$@"}, %q{printf '' "$@"}],
  "receipt-target-not-bound" => [
    "checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
    "checked_out_sha = ('0' * 40)",
  ],
  "receipt-dispatcher-sha-not-bound" => [
    "dispatcher_sha = '${{ github.workflow_sha }}'",
    "dispatcher_sha = $env:TARGET_SHA.ToLowerInvariant()",
  ],
  "receipt-dispatcher-ref-not-bound" => [
    "dispatcher_ref = '${{ github.workflow_ref }}'",
    "dispatcher_ref = 'refs/heads/arbitrary'",
  ],
  "receipt-not-always" => ["name: Build exclusive JSON receipt\n        if: always()",
    "name: Build exclusive JSON receipt\n        if: success()"],
  "artifact-not-sha-bound" => ["c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}",
    "c1b-native-${{ matrix.receipt_id }}-mutable"],
  "aggregate-not-enforced" => ["name: Enforce native aggregate", "name: Ignore native aggregate"],
  "red-wrong-runner" => ["name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: windows-2022",
    "name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: ubuntu-24.04"],
  "red-input-not-lowercase" => [
    "if ($env:TARGET_SHA -cnotmatch '^[0-9a-f]{40}$')",
    "if ($env:TARGET_SHA -cnotmatch '^[0-9A-Fa-f]{40}$')",
  ],
  "red-dispatcher-not-workflow-sha" => ["ref: ${{ env.DISPATCHER_SHA }}", "ref: main"],
  "red-dispatcher-not-isolated" => ["path: c1b-dispatcher", "path: c1b-target"],
  "red-target-not-isolated" => ["path: c1b-target", "path: ."],
  "red-arbitrary-harness" => ["../c1b-dispatcher/scripts/run-c1b-windows-red.ps1",
    "../c1b-dispatcher/scripts/arbitrary.ps1"],
  "red-target-controlled-harness" => ["../c1b-dispatcher/scripts/run-c1b-windows-red.ps1",
    "./scripts/run-c1b-windows-red.ps1"],
  "red-path-guard-removed" => ["git diff-tree --no-commit-id --name-only -r HEAD", "Write-Output windows.rs"],
  "red-artifact-not-nonce-bound" => ["-${{ inputs.red_nonce }}", "-fixed"],
  "concurrency-not-nonce-bound" => [
    "-${{ inputs.red_nonce || 'none' }}",
    "-fixed-nonce",
  ],
  "normal-jobs-run-during-red" => ["github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'", "always()"],
}

Dir.mktmpdir("c1b-ci-mutations") do |directory|
  mutations.each do |label, (before, after)|
    assert(raw.include?(before), "canonical mutation token missing: #{label}")
    path = File.join(directory, "#{label}.yml")
    File.write(path, raw.sub(before, after))
    _out, _err, result = run_validator(path)
    assert(!result.success?, "validator accepted mutation #{label}")
  end

  structural_mutations = {
    "extra-normal-job" => raw.sub("jobs:\n", <<~YAML),
      jobs:
        rogue-normal:
          runs-on: ubuntu-24.04
          steps:
            - run: cargo test --workspace
    YAML
    "extra-step-after-checkout-binding" => raw.sub(
      "      - name: Run all native gates and retain every exit\n",
      <<~YAML.gsub(/^/, "      ")
        - name: Shadow native tools
          shell: bash
          run: printf '%s\\n' /tmp/shadow >> "$GITHUB_PATH"
        - name: Run all native gates and retain every exit
      YAML
    ),
    "extra-red-step" => raw.sub(
      "      - name: Run focused expected-RED contract\n",
      <<~YAML.gsub(/^/, "      ")
        - name: Mutate RED checkout
          shell: pwsh
          run: Set-Content crates/opentake-project/src/safe_fs/windows.rs 'mutated'
        - name: Run focused expected-RED contract
      YAML
    ),
  }
  structural_mutations.each do |label, mutated|
    assert(mutated != raw, "canonical structural mutation token missing: #{label}")
    path = File.join(directory, "#{label}.yml")
    File.write(path, mutated)
    _out, _err, result = run_validator(path)
    assert(!result.success?, "validator accepted mutation #{label}")
  end

  mutated_harness = File.join(directory, "red-harness-missing-success-exit.ps1")
  File.write(mutated_harness, red_harness_raw.sub(/\nexit 0\s*\z/, "\n"))
  _out, _err, result = run_validator(WORKFLOW, mutated_harness)
  assert(!result.success?, "validator accepted RED harness without explicit success exit")
end

puts "c1b-ci-validator-tests=ok"
