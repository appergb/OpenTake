#!/usr/bin/env ruby
# frozen_string_literal: true

require "digest"
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
  ALL_JOBS = (NORMAL_JOBS + ["windows-red-evidence"]).freeze
  RUN_DIGESTS = {
    "Validate immutable SHA input" => "9047dcb191ffbcc36e39e563fed9d1f52d0ba4e4dd67052b5303405343f947ac",
    "Assert exact checked-out SHA" => "956656d8a9f0f6e8931d089ba78fb3626470819672243af21f9cc33ed89110e0",
    "Install Rust components" => "6a9466ea5252ead01f047aec0f5cc1105b496c48df6fed06c754abbdc3c42299",
    "Parse Windows expected-RED harness" => "ce782516548a5fc4e2a5aa9434ed4d8d5840d5371b11d672de8ff8c27039c6ac",
    "Re-assert immutable target before native gates" => "f6a5747011e6bdf7295690c9eb8ef73980d3e85cfe12e10f4a5bcdeba5779adf",
    "Run all native gates and retain every exit" => "3c879ba220178567594d02a9a44c17e2243e89906c76db838a74e8e00a1576f3",
    "Build exclusive JSON receipt" => "21922e9ce85e91bc80ec9163c889666ddc44f68297c03273a898e208d65b7684",
    "Enforce native aggregate" => "2e108004e97ad29c453f8d2e9ee84d93c11e8ef899b22b6855e03c5fbd4f2430",
    "Validate immutable RED inputs" => "d52e1b5c9dc160e2af0c2853da884834bc524a84806818c7fdd7cd849a2e62e9",
    "Assert trusted RED dispatcher" => "89c29db6dbd47a621e44748ac3f3f55c6a90fc6aae3387530e7f8af33370919f",
    "Assert exact RED commit and parent" => "a21065725f8dd93a4336999d4118aeb542d72926afb7684e4ff84ae4ede633d8",
    "Run focused expected-RED contract" => "6b9172b0c06722861d8d0aba5f059a406a7a667c1fbdb4a9a069856924ef9447",
  }.freeze
  SAFE_RUN_NAMES = RUN_DIGESTS.keys.first(8).freeze
  RED_RUN_NAMES = [
    "Validate immutable RED inputs",
    "Assert trusted RED dispatcher",
    "Assert exact RED commit and parent",
    "Run focused expected-RED contract",
  ].freeze
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

  def exact_keys!(value, expected, label)
    raise "#{label} fields mismatch" unless value.is_a?(Hash) && value.keys.sort == expected.sort
  end

  def require_step_sequence!(steps, expected, label)
    actual = steps.map do |step|
      step.key?("uses") ? ["uses", step.fetch("uses")] : ["name", step.fetch("name", nil)]
    end
    raise "#{label} step sequence mismatch" unless actual == expected
  end

  def require_run_digests!(steps, names)
    names.each do |name|
      step = steps.find { |candidate| candidate["name"] == name }
      raise "missing digest-bound step #{name}" unless step
      actual = Digest::SHA256.hexdigest(step.fetch("run"))
      raise "#{name} body differs from reviewed workflow" unless actual == RUN_DIGESTS.fetch(name)
    end
  end

  def validate(path)
    raw = File.read(path)
    document = YAML.safe_load(raw, aliases: true)
    raise "workflow root must be a mapping" unless document.is_a?(Hash)
    raise "workflow root contains unknown or missing fields" unless
      document.keys.sort_by(&:to_s) == ["name", true, "permissions", "concurrency", "jobs"].sort_by(&:to_s)

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
    raise "workflow contains unknown or missing jobs" unless jobs.keys.sort == ALL_JOBS.sort
    NORMAL_JOBS.each do |job_name|
      raise "#{job_name} must be disabled only during expected-RED dispatch" unless
        jobs.dig(job_name, "if") == NORMAL_CONDITION
    end

    job = jobs["safe-filesystem"]
    raise "missing safe-filesystem job and immutable SHA binding" unless job.is_a?(Hash)
    exact_keys!(job, %w[name if strategy runs-on timeout-minutes env steps], "safe-filesystem job")
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
    raise "safe-filesystem environment contains unreviewed values" unless
      job["env"] == { "TARGET_SHA" => TARGET_EXPRESSION, "RECEIPT_DIR" => "c1b-native-receipt" }
    steps = job.fetch("steps")
    raise "safe-filesystem steps must be a sequence" unless steps.is_a?(Array)
    require_step_sequence!(steps, [
      ["name", "Validate immutable SHA input"],
      ["uses", "actions/checkout@v4"],
      ["name", "Assert exact checked-out SHA"],
      ["name", "Install Rust components"],
      ["uses", "actions/cache@v4"],
      ["name", "Parse Windows expected-RED harness"],
      ["name", "Re-assert immutable target before native gates"],
      ["name", "Run all native gates and retain every exit"],
      ["name", "Build exclusive JSON receipt"],
      ["uses", "actions/upload-artifact@v4"],
      ["name", "Enforce native aggregate"],
    ], "safe-filesystem")
    require_run_digests!(steps, SAFE_RUN_NAMES)

    immutable_input = require_exact_step!(steps, "Validate immutable SHA input", shell: "bash")
    exact_keys!(immutable_input, %w[name shell run], "immutable SHA input step")
    raise "normal target immutable SHA guard missing" unless
      immutable_input["run"].to_s.include?("[[ \"$TARGET_SHA\" =~ ^[0-9a-fA-F]{40}$ ]]")
    checkouts = steps.select { |step| step["uses"] == "actions/checkout@v4" }
    raise "safe-filesystem must contain one checkout@v4 step" unless checkouts.length == 1
    checkout = checkouts.first
    exact_keys!(checkout, %w[uses with], "safe-filesystem checkout step")
    raise "safe-filesystem checkout fields mismatch" unless checkout["with"] == {
      "ref" => "${{ env.TARGET_SHA }}", "fetch-depth" => 0, "persist-credentials" => false,
    }
    raise "checkout ref is not TARGET_SHA" unless checkout.dig("with", "ref") == "${{ env.TARGET_SHA }}"
    raise "checkout must fetch immutable object history" unless checkout.dig("with", "fetch-depth") == 0
    raise "checkout credentials must not persist" unless checkout.dig("with", "persist-credentials") == false

    bind = require_exact_step!(steps, "Assert exact checked-out SHA", shell: "bash")
    exact_keys!(bind, %w[name id shell run], "exact checkout binding step")
    bind_text = bind["run"].to_s
    raise "exact checkout binding output id missing" unless bind["id"] == "bind"
    raise "missing exact git rev-parse assertion" unless
      bind_text.include?("git rev-parse HEAD") && bind_text.include?('test "$actual" = "$expected"') &&
      bind_text.include?('git cat-file -e "${expected}^{commit}"') && bind_text.include?("GITHUB_OUTPUT")

    install = require_exact_step!(steps, "Install Rust components", shell: "bash")
    exact_keys!(install, %w[name shell run], "Rust component install step")
    raise "Rust component install command mismatch" unless install["run"] == "rustup component add rustfmt clippy"
    caches = steps.select { |step| step["uses"] == "actions/cache@v4" }
    raise "safe-filesystem must contain one cache step" unless caches.length == 1
    exact_keys!(caches.first, %w[name uses with], "safe-filesystem cache step")
    raise "safe-filesystem cache contract mismatch" unless caches.first["with"] == {
      "path" => "~/.cargo/registry\n~/.cargo/git\ntarget\n",
      "key" => "safe-fs-${{ matrix.receipt_id }}-${{ hashFiles('**/Cargo.toml', 'Cargo.lock') }}",
      "restore-keys" => "safe-fs-${{ matrix.receipt_id }}-",
    }

    parser = require_exact_step!(steps, "Parse Windows expected-RED harness", shell: "pwsh")
    exact_keys!(parser, %w[name if shell run], "Windows RED parser step")
    reassert = require_exact_step!(steps, "Re-assert immutable target before native gates", shell: "bash")
    exact_keys!(reassert, %w[name shell run], "immutable target reassertion step")
    reassert_text = reassert["run"].to_s
    ["git rev-parse HEAD", "HEAD^{tree}", '${expected}^{tree}',
     "git status --porcelain=v1 --untracked-files=all", 'test "$actual" = "$expected"'].each do |token|
      raise "immutable target reassertion missing #{token}" unless reassert_text.include?(token)
    end

    gates = require_exact_step!(steps, "Run all native gates and retain every exit", shell: "bash")
    exact_keys!(gates, %w[name shell run], "native gate step")
    gate_text = gates["run"].to_s
    gate_rows = gate_text.lines.each_with_object([]) do |line, rows_accumulator|
      match = line.match(/^\s*run_gate\s+(\S+)\s+(.+?)\s*$/)
      rows_accumulator << [match[1], match[2]] if match
    end
    raise "native gate commands differ from policy" unless gate_rows == NATIVE_COMMANDS.to_a
    %w[set\ -u set\ +e code=\$? final-aggregate.raw-exit].each do |token|
      raise "native raw-exit aggregation missing #{token.tr('\\', '')}" unless gate_text.include?(token.tr("\\", ""))
    end
    [%q{printf ' %q' "$@"}, %q{>>"$RECEIPT_DIR/$id.log" 2>&1}].each do |token|
      raise "native gate log capture missing #{token}" unless gate_text.include?(token)
    end

    receipt = require_exact_step!(steps, "Build exclusive JSON receipt", shell: "pwsh", condition: "always()")
    exact_keys!(receipt, %w[name if shell env run], "native receipt step")
    expected_env = {
      "RECEIPT_SHA" => "${{ steps.bind.outputs.sha }}",
      "RECEIPT_ID" => "${{ matrix.receipt_id }}",
      "RUNNER_LABEL" => "${{ matrix.runner }}",
      "EXPECTED_RUNNER_OS" => "${{ matrix.expected_os }}",
      "EXPECTED_RUNNER_ARCH" => "${{ matrix.expected_arch }}",
    }
    raise "receipt environment is not checkout/matrix-bound" unless
      receipt["env"] == expected_env
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
      "dispatcher_sha" => "dispatcher_sha = '${{ github.workflow_sha }}'",
      "dispatcher_ref" => "dispatcher_ref = '${{ github.workflow_ref }}'",
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
    exact_keys!(upload, %w[name if uses with], "native receipt upload step")
    expected_name = "c1b-native-${{ matrix.receipt_id }}-${{ steps.bind.outputs.sha }}"
    expected_upload = {
      "name" => expected_name, "path" => "c1b-native-receipt/",
      "if-no-files-found" => "error", "retention-days" => 30,
    }
    raise "receipt artifact upload is not always and SHA-bound" unless
      upload["if"] == "always()" && upload["with"] == expected_upload
    enforce = require_exact_step!(steps, "Enforce native aggregate", shell: "bash", condition: "always()")
    exact_keys!(enforce, %w[name if shell run], "native aggregate enforcement step")
    raise "aggregate enforce step does not consume raw aggregate" unless
      enforce["run"].to_s.include?("final-aggregate.raw-exit") &&
      enforce["run"].to_s.include?('test "$(cat "$RECEIPT_DIR/final-aggregate.raw-exit")" = 0')

    parser_text = parser["run"].to_s
    raise "Windows RED harness parser missing" unless parser["if"] == "runner.os == 'Windows'" &&
      parser_text.include?("scripts/run-c1b-windows-red.ps1") && parser_text.include?("ParseFile") &&
      parser_text.include?("$errors.Count -ne 0")

    red_job = jobs["windows-red-evidence"]
    raise "missing dispatch-only Windows RED job" unless red_job.is_a?(Hash) &&
      red_job["name"] == "Windows expected RED (${{ inputs.red_task }})" &&
      red_job["if"] == "github.event_name == 'workflow_dispatch' && inputs.red_task != 'none'" &&
      red_job["runs-on"] == "windows-2022" && red_job["timeout-minutes"] == 35
    exact_keys!(red_job, %w[name if runs-on timeout-minutes env steps], "Windows RED job")
    expected_red_env = {
      "TARGET_SHA" => "${{ inputs.commit_sha }}",
      "PARENT_SHA" => "${{ inputs.red_parent_sha }}",
      "RED_TASK" => "${{ inputs.red_task }}",
      "RED_NONCE" => "${{ inputs.red_nonce }}",
      "DISPATCHER_SHA" => "${{ github.workflow_sha }}",
      "DISPATCHER_REF" => "${{ github.workflow_ref }}",
    }
    raise "Windows RED job environment is not context-bound" unless
      red_job["env"] == expected_red_env
    red_steps = red_job.fetch("steps")
    raise "Windows RED steps must be a sequence" unless red_steps.is_a?(Array)
    require_step_sequence!(red_steps, [
      ["name", "Validate immutable RED inputs"],
      ["uses", "actions/checkout@v4"],
      ["name", "Assert trusted RED dispatcher"],
      ["uses", "actions/checkout@v4"],
      ["name", "Assert exact RED commit and parent"],
      ["name", "Run focused expected-RED contract"],
      ["uses", "actions/upload-artifact@v4"],
    ], "Windows RED")
    require_run_digests!(red_steps, RED_RUN_NAMES)
    red_input = require_exact_step!(red_steps, "Validate immutable RED inputs", shell: "pwsh")
    exact_keys!(red_input, %w[name shell run], "Windows RED input step")
    red_input_text = red_input["run"].to_s
    raise "Windows RED immutable input guards missing" unless
      %w[TARGET_SHA PARENT_SHA RED_NONCE].all? { |token| red_input_text.include?(token) } &&
      red_input_text.scan("^[0-9a-f]{40}$").length == 2 && red_input_text.include?("^[0-9a-f]{16}$") &&
      red_input_text.scan("-cnotmatch").length == 3

    red_checkouts = red_steps.select { |step| step["uses"] == "actions/checkout@v4" }
    raise "Windows RED job must contain trusted dispatcher and target checkouts" unless red_checkouts.length == 2
    dispatcher_checkout, red_checkout = red_checkouts
    exact_keys!(dispatcher_checkout, %w[name uses with], "Windows RED dispatcher checkout step")
    raise "Windows RED dispatcher checkout fields mismatch" unless dispatcher_checkout["with"] == {
      "ref" => "${{ env.DISPATCHER_SHA }}", "fetch-depth" => 1,
      "persist-credentials" => false, "path" => "c1b-dispatcher",
    }
    dispatcher_bind = require_exact_step!(red_steps, "Assert trusted RED dispatcher", shell: "pwsh")
    exact_keys!(dispatcher_bind, %w[name id shell run], "Windows RED dispatcher binding step")
    raise "Windows RED dispatcher output id missing" unless dispatcher_bind["id"] == "bind-dispatcher"
    dispatcher_bind_text = dispatcher_bind["run"].to_s
    ["github.repository", ".github/workflows/ci.yml@refs/heads/main", "DISPATCHER_REF",
     "git -C c1b-dispatcher rev-parse HEAD", "DISPATCHER_SHA", "GITHUB_OUTPUT"].each do |token|
      raise "Windows RED dispatcher assertion missing #{token}" unless dispatcher_bind_text.include?(token)
    end
    exact_keys!(red_checkout, %w[name uses with], "Windows RED target checkout step")
    raise "Windows RED target checkout fields mismatch" unless red_checkout["with"] == {
      "ref" => "${{ env.TARGET_SHA }}", "fetch-depth" => 2,
      "persist-credentials" => false, "path" => "c1b-target",
    }
    red_bind = require_exact_step!(red_steps, "Assert exact RED commit and parent", shell: "pwsh")
    exact_keys!(red_bind, %w[name id shell run], "Windows RED binding step")
    raise "Windows RED binding output id missing" unless red_bind["id"] == "bind-red"
    red_bind_text = red_bind["run"].to_s
    [
      "git rev-parse HEAD", "git rev-parse 'HEAD^'", "git rev-list --parents -n 1 HEAD",
      "git diff-tree --no-commit-id --name-only -r HEAD", "$commitRow.Count -ne 2",
      "$changedPaths.Count -ne 1", "crates/opentake-project/src/safe_fs/windows.rs",
      "Set-Location c1b-target", "checked-out RED SHA mismatch", "RED parent SHA mismatch", "GITHUB_OUTPUT",
    ].each do |token|
      raise "Windows RED identity assertion missing #{token}" unless red_bind_text.include?(token)
    end

    red_run = require_exact_step!(red_steps, "Run focused expected-RED contract", shell: "pwsh")
    exact_keys!(red_run, %w[name shell run], "Windows RED harness step")
    red_run_text = red_run["run"].to_s
    raise "repository Windows RED harness is not invoked with fixed inputs" unless
      red_run_text.include?("Set-Location c1b-target") &&
      red_run_text.include?("../c1b-dispatcher/scripts/run-c1b-windows-red.ps1") &&
      %w[-Task\ $env:RED_TASK -TestSha\ $env:TARGET_SHA -ParentSha\ $env:PARENT_SHA
         -Nonce\ $env:RED_NONCE RUNNER_TEMP].all? { |token| red_run_text.include?(token.tr("\\", "")) }
    raise "Windows RED job permits arbitrary command input" if red_run_text.match?(/Invoke-Expression|iex\b|Start-Process/)

    red_uploads = red_steps.select { |step| step["uses"] == "actions/upload-artifact@v4" }
    raise "Windows RED job must contain one artifact upload" unless red_uploads.length == 1
    red_upload = red_uploads.first
    exact_keys!(red_upload, %w[name if uses with], "Windows RED artifact step")
    expected_red_name = "c1b-red-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}"
    expected_red_path = "${{ runner.temp }}/c1b-red/c1b-task-${{ inputs.red_task }}-${{ steps.bind-red.outputs.sha }}-${{ inputs.red_nonce }}/"
    expected_red_upload = {
      "name" => expected_red_name, "path" => expected_red_path,
      "if-no-files-found" => "error", "retention-days" => 30,
    }
    raise "Windows RED artifact is not immutable and nonce-bound" unless
      red_upload["if"] == "always()" && red_upload["with"] == expected_red_upload

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
