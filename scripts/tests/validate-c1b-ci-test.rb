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

def assert(condition, message)
  raise message unless condition
end

def run_validator(path)
  Open3.capture3(RbConfig.ruby, VALIDATOR, path)
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
mutations = {
  "pr-uses-merge" => ["github.event.pull_request.head.sha", "github.sha"],
  "checkout-not-bound" => ["ref: ${{ env.TARGET_SHA }}", "ref: main"],
  "checkout-persists-credentials" => ["persist-credentials: false", "persist-credentials: true"],
  "wrong-linux-runner" => ["runner: ubuntu-24.04", "runner: windows-2022"],
  "wrong-macos-arch" => ["expected_arch: ARM64", "expected_arch: X64"],
  "receipt-not-always" => ["name: Build exclusive JSON receipt\n        if: always()",
    "name: Build exclusive JSON receipt\n        if: success()"],
  "artifact-not-sha-bound" => ["c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}",
    "c1b-native-${{ matrix.receipt_id }}-mutable"],
  "aggregate-not-enforced" => ["name: Enforce native aggregate", "name: Ignore native aggregate"],
  "red-wrong-runner" => ["name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: windows-2022",
    "name: Windows expected RED (${{ inputs.red_task }})\n    if: github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'\n    runs-on: ubuntu-24.04"],
  "red-arbitrary-harness" => ["./scripts/run-c1b-windows-red.ps1", "./scripts/arbitrary.ps1"],
  "red-path-guard-removed" => ["git diff-tree --no-commit-id --name-only -r HEAD", "Write-Output windows.rs"],
  "red-artifact-not-nonce-bound" => ["-${{ inputs.red_nonce }}", "-fixed"],
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
end

puts "c1b-ci-validator-tests=ok"
