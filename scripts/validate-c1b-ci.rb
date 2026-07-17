#!/usr/bin/env ruby
# frozen_string_literal: true

require "yaml"

module C1bCiValidator
  SHA = /\A[0-9a-f]{40}\z/
  RECEIPTS = %w[linux-x86_64 macos-native windows-x86_64].freeze
  PROVENANCE = {
    "linux-x86_64" => { "runner" => "ubuntu-24.04", "expected_os" => "Linux", "expected_arch" => "X64" },
    "macos-native" => { "runner" => "macos-14", "expected_os" => "macOS", "expected_arch" => "ARM64" },
    "windows-x86_64" => { "runner" => "windows-2022", "expected_os" => "Windows", "expected_arch" => "X64" },
  }.freeze
  NATIVE_COMMANDS = {
    "cargo-fmt" => "cargo fmt --all --check",
    "cargo-clippy" => "cargo clippy -p opentake-project --lib --tests -- -D warnings",
    "safe-fs-unit" => "cargo test -p opentake-project --lib safe_fs -- --test-threads=1",
    "archive-security" => "cargo test -p opentake-project --test archive_security -- --test-threads=1",
  }.freeze
  NORMAL_JOBS = %w[rust windows-security web windows-library-security safe-filesystem].freeze
  NORMAL_CONDITION = "github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'"
  TARGET_EXPRESSION = "${{ github.event_name == 'workflow_dispatch' && inputs.commit_sha || github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}"

  module_function

  def select_target(event_name:, github_sha:, pull_request_head_sha: nil, dispatch_sha: nil)
    value = case event_name
            when "workflow_dispatch" then dispatch_sha
            when "pull_request" then pull_request_head_sha
            when "push" then github_sha
            else raise "unsupported event #{event_name.inspect}"
            end
    normalized = value.to_s.downcase
    raise "selected SHA is not immutable 40-hex" unless SHA.match?(normalized)

    normalized
  end

  def require_exact_step!(steps, name, shell: nil, condition: nil)
    matches = steps.select { |step| step["name"] == name }
    raise "missing or duplicate step #{name}" unless matches.length == 1

    step = matches.first
    raise "#{name} shell mismatch" if shell && step["shell"] != shell
    raise "#{name} condition mismatch" if condition && step["if"] != condition
    step
  end

  def validate(path)
    raw = File.read(path)
    document = YAML.safe_load(raw, aliases: true)
    raise "workflow root must be a mapping" unless document.is_a?(Hash)

    events = document["on"] || document[true]
    raise "missing on mapping" unless events.is_a?(Hash)
    raise "push must be independently bound to main" unless events.dig("push", "branches") == ["main"]
    raise "pull_request trigger missing" unless events.key?("pull_request")

    inputs = events.dig("workflow_dispatch", "inputs")
    raise "workflow_dispatch inputs missing" unless inputs.is_a?(Hash)
    dispatch = inputs["commit_sha"]
    raise "workflow_dispatch.commit_sha must be a required string" unless
      dispatch.is_a?(Hash) && dispatch.values_at("required", "type") == [true, "string"]
    red_task = inputs["red_task"]
    raise "workflow_dispatch.red_task contract mismatch" unless red_task.is_a?(Hash) &&
      red_task.values_at("required", "default", "type") == [true, "none", "choice"] &&
      red_task.fetch("options") == %w[none 6b 7a 7b 7c]
    %w[red_parent_sha red_nonce].each do |name|
      input = inputs[name]
      raise "workflow_dispatch #{name} contract mismatch" unless input.is_a?(Hash) &&
        input.values_at("required", "default", "type") == [false, "", "string"]
    end

    raise "workflow permissions must be read-only" unless document["permissions"] == { "contents" => "read" }
    expected_concurrency = "${{ github.workflow }}-${{ github.event_name }}-${{ github.ref }}-${{ inputs.commit_sha || github.sha }}-${{ inputs.red_task || 'normal' }}-${{ inputs.red_nonce || 'none' }}"
    raise "workflow concurrency is not exact-SHA and RED nonce-bound" unless
      document.dig("concurrency", "group") == expected_concurrency &&
      document.dig("concurrency", "cancel-in-progress") == true

    jobs = document["jobs"]
    raise "jobs mapping missing" unless jobs.is_a?(Hash)
    NORMAL_JOBS.each do |job_name|
      raise "#{job_name} must be disabled only during expected-RED dispatch" unless
        jobs.dig(job_name, "if") == NORMAL_CONDITION
    end

    job = jobs["safe-filesystem"]
    raise "missing safe-filesystem job and immutable SHA binding" unless job.is_a?(Hash)
    raise "safe-filesystem job identity mismatch" unless
      job["name"] == "Safe filesystem (${{ matrix.receipt_id }})" &&
      job["runs-on"] == "${{ matrix.runner }}" && job["timeout-minutes"] == 35
    raise "safe-filesystem matrix must retain all failures" unless job.dig("strategy", "fail-fast") == false
    rows = job.dig("strategy", "matrix", "include")
    raise "safe-filesystem matrix missing" unless rows.is_a?(Array)
    receipt_ids = rows.map { |row| row.fetch("receipt_id") }
    raise "duplicate or missing receipt ids" unless
      receipt_ids.sort == RECEIPTS.sort && receipt_ids.uniq.length == receipt_ids.length
    rows.each do |row|
      expected = PROVENANCE.fetch(row.fetch("receipt_id"))
      raise "native runner provenance does not match receipt id" unless
        row.values_at("runner", "expected_os", "expected_arch") ==
          expected.values_at("runner", "expected_os", "expected_arch")
    end

    raise "TARGET_SHA must bind push, PR head, and dispatch independently" unless
      job.dig("env", "TARGET_SHA") == TARGET_EXPRESSION
    raise "receipt directory must be job-local and fixed" unless job.dig("env", "RECEIPT_DIR") == "c1b-native-receipt"
    steps = job.fetch("steps")
    raise "safe-filesystem steps must be a sequence" unless steps.is_a?(Array)

    immutable_input = require_exact_step!(steps, "Validate immutable SHA input", shell: "bash")
    raise "normal target immutable SHA guard missing" unless
      immutable_input["run"].to_s.include?("[[ \"$TARGET_SHA\" =~ ^[0-9a-fA-F]{40}$ ]]")
    checkouts = steps.select { |step| step["uses"] == "actions/checkout@v4" }
    raise "safe-filesystem must contain one checkout@v4 step" unless checkouts.length == 1
    checkout = checkouts.first
    raise "checkout ref is not TARGET_SHA" unless checkout.dig("with", "ref") == "${{ env.TARGET_SHA }}"
    raise "checkout must fetch immutable object history" unless checkout.dig("with", "fetch-depth") == 0
    raise "checkout credentials must not persist" unless checkout.dig("with", "persist-credentials") == false

    bind = require_exact_step!(steps, "Assert exact checked-out SHA", shell: "bash")
    bind_text = bind["run"].to_s
    raise "exact checkout binding output id missing" unless bind["id"] == "bind"
    raise "missing exact git rev-parse assertion" unless
      bind_text.include?("git rev-parse HEAD") && bind_text.include?('test "$actual" = "$expected"') &&
      bind_text.include?('git cat-file -e "${expected}^{commit}"') && bind_text.include?("GITHUB_OUTPUT")

    gates = require_exact_step!(steps, "Run all native gates and retain every exit", shell: "bash")
    gate_text = gates["run"].to_s
    gate_rows = gate_text.lines.each_with_object([]) do |line, rows_accumulator|
      match = line.match(/^\s*run_gate\s+(\S+)\s+(.+?)\s*$/)
      rows_accumulator << [match[1], match[2]] if match
    end
    raise "native gate commands differ from policy" unless gate_rows == NATIVE_COMMANDS.to_a
    %w[set\ -u set\ +e code=\$? final-aggregate.raw-exit].each do |token|
      raise "native raw-exit aggregation missing #{token.tr('\\', '')}" unless gate_text.include?(token.tr("\\", ""))
    end

    receipt = require_exact_step!(steps, "Build exclusive JSON receipt", shell: "pwsh", condition: "always()")
    expected_env = {
      "RECEIPT_SHA" => "${{ steps.bind.outputs.sha }}",
      "RECEIPT_ID" => "${{ matrix.receipt_id }}",
      "RUNNER_LABEL" => "${{ matrix.runner }}",
      "EXPECTED_RUNNER_OS" => "${{ matrix.expected_os }}",
      "EXPECTED_RUNNER_ARCH" => "${{ matrix.expected_arch }}",
    }
    raise "receipt environment is not checkout/matrix-bound" unless
      expected_env.all? { |key, value| receipt.dig("env", key) == value }
    receipt_text = receipt["run"].to_s
    receipt_commands = receipt_text.scan(/@\{ id = '([^']+)'; command = '([^']+)' \}/)
    raise "receipt command ledger differs from native gates" unless receipt_commands == NATIVE_COMMANDS.to_a
    {
      "schema" => "schema = 'opentake-c1b-native-receipt-v1'",
      "repository" => "repository = '${{ github.repository }}'",
      "workflow" => "workflow = '${{ github.workflow }}'",
      "workflow_file" => "workflow_file = '.github/workflows/ci.yml'",
      "run_id" => "run_id = '${{ github.run_id }}'",
      "run_attempt" => "run_attempt = '${{ github.run_attempt }}'",
      "job_id" => "job_id = '${{ github.job }}'",
      "receipt_id" => "receipt_id = $env:RECEIPT_ID",
      "runner_label" => "runner_label = $env:RUNNER_LABEL",
      "runner_os" => "runner_os = '${{ runner.os }}'",
      "runner_arch" => "runner_arch = '${{ runner.arch }}'",
      "event_name" => "event_name = '${{ github.event_name }}'",
      "requested_sha" => "requested_sha = $env:TARGET_SHA.ToLowerInvariant()",
      "checked_out_sha" => "checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
      "commands" => "commands = @(\$commands)",
      "aggregate_exit" => "aggregate_exit = [int](Get-Content (Join-Path $env:RECEIPT_DIR 'final-aggregate.raw-exit'))",
    }.each do |field, binding|
      binding = binding.delete("\\") if field == "commands"
      raise "receipt #{field} is not context-bound" unless receipt_text.include?(binding)
    end
    [
      "if ('${{ runner.os }}' -ne $env:EXPECTED_RUNNER_OS)",
      "if ('${{ runner.arch }}' -ne $env:EXPECTED_RUNNER_ARCH)",
      "exit_code = [int](Get-Content $exitPath)",
      "log = ($_.id + '.log')",
      "raw_exit = ($_.id + '.raw-exit')",
      "Set-Content -Encoding utf8NoBOM",
    ].each do |guard|
      raise "receipt provenance or raw evidence guard missing: #{guard}" unless receipt_text.include?(guard)
    end

    uploads = steps.select { |step| step["uses"] == "actions/upload-artifact@v4" }
    raise "safe-filesystem must contain one artifact upload" unless uploads.length == 1
    upload = uploads.first
    expected_name = "c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}"
    raise "receipt artifact upload is not always and SHA-bound" unless
      upload["if"] == "always()" && upload.dig("with", "name") == expected_name &&
      upload.dig("with", "path") == "c1b-native-receipt/" &&
      upload.dig("with", "if-no-files-found") == "error" && upload.dig("with", "retention-days") == 30
    enforce = require_exact_step!(steps, "Enforce native aggregate", shell: "bash", condition: "always()")
    raise "aggregate enforce step does not consume raw aggregate" unless
      enforce["run"].to_s.include?("final-aggregate.raw-exit") &&
      enforce["run"].to_s.include?('test "$(cat "$RECEIPT_DIR/final-aggregate.raw-exit")" = 0')

    parser = require_exact_step!(steps, "Parse Windows expected-RED harness", shell: "pwsh")
    parser_text = parser["run"].to_s
    raise "Windows RED harness parser missing" unless parser["if"] == "runner.os == 'Windows'" &&
      parser_text.include?("scripts/run-c1b-windows-red.ps1") && parser_text.include?("ParseFile") &&
      parser_text.include?("$errors.Count -ne 0")

    red_job = jobs["windows-red-evidence"]
    raise "missing dispatch-only Windows RED job" unless red_job.is_a?(Hash) &&
      red_job["name"] == "Windows expected RED (${{ inputs.red_task }})" &&
      red_job["if"] == "github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'" &&
      red_job["runs-on"] == "windows-2022" && red_job["timeout-minutes"] == 35
    expected_red_env = {
      "TARGET_SHA" => "${{ inputs.commit_sha }}",
      "PARENT_SHA" => "${{ inputs.red_parent_sha }}",
      "RED_TASK" => "${{ inputs.red_task }}",
      "RED_NONCE" => "${{ inputs.red_nonce }}",
    }
    raise "Windows RED job environment is not context-bound" unless
      expected_red_env.all? { |key, value| red_job.dig("env", key) == value }
    red_steps = red_job.fetch("steps")
    red_input = require_exact_step!(red_steps, "Validate immutable RED inputs", shell: "pwsh")
    red_input_text = red_input["run"].to_s
    raise "Windows RED immutable input guards missing" unless
      %w[TARGET_SHA PARENT_SHA RED_NONCE].all? { |token| red_input_text.include?(token) } &&
      red_input_text.scan("^[0-9a-f]{40}$").length == 2 && red_input_text.include?("^[0-9a-f]{16}$") &&
      red_input_text.scan("-cnotmatch").length == 3

    red_checkouts = red_steps.select { |step| step["uses"] == "actions/checkout@v4" }
    raise "Windows RED job must contain one checkout" unless red_checkouts.length == 1
    red_checkout = red_checkouts.first
    raise "Windows RED checkout is not immutable" unless
      red_checkout.dig("with", "ref") == "${{ env.TARGET_SHA }}" &&
      red_checkout.dig("with", "fetch-depth") == 2 &&
      red_checkout.dig("with", "persist-credentials") == false
    red_bind = require_exact_step!(red_steps, "Assert exact RED commit and parent", shell: "pwsh")
    raise "Windows RED binding output id missing" unless red_bind["id"] == "bind-red"
    red_bind_text = red_bind["run"].to_s
    [
      "git rev-parse HEAD", "git rev-parse 'HEAD^'", "git rev-list --parents -n 1 HEAD",
      "git diff-tree --no-commit-id --name-only -r HEAD", "$commitRow.Count -ne 2",
      "$changedPaths.Count -ne 1", "crates/opentake-project/src/safe_fs/windows.rs",
      "checked-out RED SHA mismatch", "RED parent SHA mismatch", "GITHUB_OUTPUT",
    ].each do |token|
      raise "Windows RED identity assertion missing #{token}" unless red_bind_text.include?(token)
    end

    red_run = require_exact_step!(red_steps, "Run focused expected-RED contract", shell: "pwsh")
    red_run_text = red_run["run"].to_s
    raise "repository Windows RED harness is not invoked with fixed inputs" unless
      red_run_text.include?("./scripts/run-c1b-windows-red.ps1") &&
      %w[-Task\ $env:RED_TASK -TestSha\ $env:TARGET_SHA -ParentSha\ $env:PARENT_SHA
         -Nonce\ $env:RED_NONCE RUNNER_TEMP].all? { |token| red_run_text.include?(token.tr("\\", "")) }
    raise "Windows RED job permits arbitrary command input" if red_run_text.match?(/Invoke-Expression|iex\b|Start-Process/)

    red_uploads = red_steps.select { |step| step["uses"] == "actions/upload-artifact@v4" }
    raise "Windows RED job must contain one artifact upload" unless red_uploads.length == 1
    red_upload = red_uploads.first
    expected_red_name = "c1b-red-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}"
    expected_red_path = "${{ runner.temp }}/c1b-red/c1b-task-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}/"
    raise "Windows RED artifact is not immutable and nonce-bound" unless
      red_upload["if"] == "always()" && red_upload.dig("with", "name") == expected_red_name &&
      red_upload.dig("with", "path") == expected_red_path &&
      red_upload.dig("with", "if-no-files-found") == "error" && red_upload.dig("with", "retention-days") == 30

    merge = "a" * 40
    head = "b" * 40
    dispatch_sha = "c" * 40
    push = "d" * 40
    raise "push selection failed" unless select_target(event_name: "push", github_sha: push) == push
    selected_pr = select_target(event_name: "pull_request", github_sha: merge, pull_request_head_sha: head)
    raise "PR selected synthetic merge SHA" unless selected_pr == head && selected_pr != merge
    raise "dispatch selection failed" unless select_target(
      event_name: "workflow_dispatch", github_sha: push, dispatch_sha: dispatch_sha
    ) == dispatch_sha

    true
  end
end

if $PROGRAM_NAME == __FILE__
  path = ARGV.fetch(0) { abort "usage: validate-c1b-ci.rb WORKFLOW" }
  C1bCiValidator.validate(path)
  puts "c1b-ci-validation=ok"
end
