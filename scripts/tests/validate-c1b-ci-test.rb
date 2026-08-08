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
windows_product = raw[/^  windows-product:\n.*?(?=^  windows-security:)/m]
assert(windows_product, "canonical workflow lacks the Windows product job")
windows_security = raw[/^  windows-security:\n.*?(?=^  web:)/m]
assert(windows_security, "canonical workflow lacks the Windows security job")
safe_filesystem = raw[/^  safe-filesystem:\n.*?(?=^  windows-red-evidence:)/m]
assert(safe_filesystem, "canonical workflow lacks the safe-filesystem job")
red_harness_raw = File.read(RED_HARNESS)
assert(
  red_harness_raw.lines.map(&:strip).reject(&:empty?).last == "exit 0",
  "Windows expected-RED harness does not clear expected native failures"
)
mutations = {
  "checkout-action-floating" => [
    "actions/checkout@11d5960a326750d5838078e36cf38b85af677262",
    "actions/checkout@v4",
  ],
  "cache-action-floating" => [
    "actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830",
    "actions/cache@v4",
  ],
  "upload-action-floating" => [
    "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02",
    "actions/upload-artifact@v4",
  ],
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
    safe_labels = %w[
      checkout-action-floating
      cache-action-floating
      upload-action-floating
      receipt-target-not-bound
      receipt-not-always
    ]
    source = safe_labels.include?(label) ? safe_filesystem : raw
    assert(source.include?(before), "canonical mutation token missing: #{label}")
    path = File.join(directory, "#{label}.yml")
    File.write(path, raw.sub(source, source.sub(before, after)))
    _out, _err, result = run_validator(path)
    assert(!result.success?, "validator accepted mutation #{label}")
  end

  structural_mutations = {
    "duplicate-windows-security-key" => raw.sub(
      "\n  web:\n", "\n#{windows_security}\n  web:\n"
    ),
    "missing-windows-product" => raw.sub("  windows-product:\n", "  disabled-windows-product:\n"),
    "windows-product-target-not-bound" => raw.sub(
      windows_product,
      windows_product.sub(
        "TARGET_SHA: ${{ github.event_name == 'workflow_dispatch' && inputs.commit_sha || github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}",
        "TARGET_SHA: ${{ github.sha }}"
      )
    ),
    "windows-product-checkout-not-bound" => raw.sub(
      windows_product,
      windows_product.sub("ref: ${{ env.TARGET_SHA }}", "ref: main")
    ),
    "windows-product-checkout-persists-credentials" => raw.sub(
      windows_product,
      windows_product.sub("persist-credentials: false", "persist-credentials: true")
    ),
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

  receipt_block = <<~'POWERSHELL'.chomp
    } | ConvertTo-Json -Depth 8 | Set-Content -Encoding utf8NoBOM `
      (Join-Path $Evidence 'red-receipt.json')
  POWERSHELL
  harness_mutations = {
    "missing-success-exit" => red_harness_raw.sub(/\nexit 0\s*\z/, "\n"),
    "only-success-exit" => "exit 0\n",
    "early-success-exit" => red_harness_raw.sub(
      "Set-StrictMode -Version Latest",
      "exit 0\nSet-StrictMode -Version Latest"
    ),
    "receipt-removed" => red_harness_raw.sub(receipt_block, "Write-Output 'receipt omitted'"),
    "red-success-check-removed" => red_harness_raw.sub(
      'if ($code -eq 0) { throw "RED unexpectedly passed: $Name" }',
      'if ($code -eq 0) { return 0 }'
    ),
  }
  harness_mutations.each do |label, mutated|
    assert(mutated != red_harness_raw, "canonical harness mutation token missing: #{label}")
    mutated_harness = File.join(directory, "#{label}.ps1")
    File.write(mutated_harness, mutated)
    _out, _err, result = run_validator(WORKFLOW, mutated_harness)
    assert(!result.success?, "validator accepted RED harness mutation #{label}")
  end
end

puts "c1b-ci-validator-tests=ok"
