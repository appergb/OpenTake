#!/usr/bin/env ruby
# frozen_string_literal: true

require "base64"
require "digest"
require "json"
require "open3"
require "pathname"
require "time"
require_relative "validate-c1b-ci"

SHA = /\A[0-9a-f]{40}\z/
POSITIVE_INTEGER = /\A[1-9][0-9]*\z/
REVIEW_TASK = "evidence-bootstrap"
RECEIPT_SCHEMA = "opentake-c1b-native-receipt-v1"
POLICY_PATH = File.expand_path("c1b-evidence-policy.json", __dir__)

def fail!(message)
  raise message
end

def exact_hash_keys!(value, keys, label)
  fail!("#{label} must be an object") unless value.is_a?(Hash)
  actual = value.keys.sort
  expected = keys.sort
  fail!("#{label} fields mismatch") unless actual == expected
end

def confined_file!(root, relative, label)
  fail!("#{label} path must be a nonempty relative string") unless
    relative.is_a?(String) && !relative.empty? && !relative.include?("\0")
  fail!("#{label} path must be relative") if Pathname.new(relative).absolute?

  parts = relative.split(File::SEPARATOR, -1)
  fail!("#{label} path is not canonical") if
    parts.any? { |part| part.empty? || part == "." || part == ".." }

  current = root
  parts.each_with_index do |part, index|
    current = File.join(current, part)
    stat = File.lstat(current)
    fail!("#{label} path contains a symlink") if stat.symlink?
    if index == parts.length - 1
      fail!("#{label} is not a regular file") unless stat.file?
    else
      fail!("#{label} parent is not a directory") unless stat.directory?
    end
  rescue Errno::ENOENT, Errno::ENOTDIR
    fail!("#{label} missing")
  end

  prefix = root.end_with?(File::SEPARATOR) ? root : "#{root}#{File::SEPARATOR}"
  real = File.realpath(current)
  fail!("#{label} resolves outside gate") unless real.start_with?(prefix)
  real
end

def json_file!(root, relative, label)
  path = confined_file!(root, relative, label)
  [path, JSON.parse(File.binread(path))]
rescue JSON::ParserError
  fail!("#{label} is not valid JSON")
end

def git!(repo, *arguments)
  stdout, stderr, status = Open3.capture3("git", "-C", repo, *arguments)
  fail!("git #{arguments.first} failed: #{stderr.strip}") unless status.success?
  stdout
rescue Errno::ENOENT
  fail!("git is required")
end

def git_success?(repo, *arguments)
  _stdout, _stderr, status = Open3.capture3("git", "-C", repo, *arguments)
  status.success?
rescue Errno::ENOENT
  false
end

def allowed_bootstrap_path?(path, allowlist)
  allowlist.any? do |allowed|
    allowed.end_with?("/") ? path.start_with?(allowed) && path.length > allowed.length : path == allowed
  end
end

def timestamp!(value, label)
  fail!("#{label} must be a string") unless value.is_a?(String)
  parsed = Time.iso8601(value)
  fail!("#{label} must be UTC") unless parsed.utc_offset.zero?
  parsed
rescue ArgumentError
  fail!("#{label} is not RFC3339")
end

def field!(body, name, expected, label, case_sensitive: true)
  values = body.lines.map do |line|
    match = line.match(/\A#{Regexp.escape(name)}:\s*(.*?)\s*\z/i)
    match && match[1]
  end.compact
  fail!("#{label} #{name} field mismatch") unless values.length == 1
  actual = values.first.gsub(/\A(?:\*\*|`)|(?:\*\*|`)\z/, "")
  matches = case_sensitive ? actual == expected : actual.casecmp?(expected)
  fail!("#{label} #{name} mismatch") unless matches
end

def approved_report!(path, role, expected_sha)
  body = File.binread(path)
  field!(body, "Role", role, role, case_sensitive: false)
  field!(body, "Task", REVIEW_TASK, role, case_sensitive: false)
  field!(body, "Commit", expected_sha, role, case_sensitive: false)
  field!(body, "Verdict", "APPROVE", role, case_sensitive: false)
  %w[Critical Important Minor].each { |severity| field!(body, severity, "0", role) }
end

def gh_authenticated!
  _stdout, stderr, status = Open3.capture3("gh", "auth", "status", "--hostname", "github.com")
  fail!("gh is not authenticated for github.com: #{stderr.strip}") unless status.success?
rescue Errno::ENOENT
  fail!("authenticated gh CLI is required")
end

def gh_json!(endpoint, api_version, label)
  stdout, stderr, status = Open3.capture3("gh", "api", "--hostname", "github.com",
    "-H", "X-GitHub-Api-Version: #{api_version}", endpoint)
  fail!("#{label} GitHub API failed: #{stderr.strip}") unless status.success?
  [stdout, JSON.parse(stdout)]
rescue Errno::ENOENT
  fail!("authenticated gh CLI is required")
rescue JSON::ParserError
  fail!("#{label} GitHub API returned invalid JSON")
end

def positive_integer_string!(value, label)
  text = value.to_s
  fail!("#{label} must be a positive integer") unless POSITIVE_INTEGER.match?(text)
  text
end

def page!(value, collection_key, label)
  fail!("#{label} response must be an object") unless value.is_a?(Hash)
  rows = value.fetch(collection_key)
  total = value.fetch("total_count")
  fail!("#{label} rows must be an array") unless rows.is_a?(Array)
  fail!("#{label} total_count is invalid") unless total.is_a?(Integer) && total >= 0
  fail!("#{label} response is paginated or ambiguous") unless total == rows.length && total <= 100
  rows
rescue KeyError
  fail!("#{label} response fields missing")
end

def unzip_entries!(archive, label)
  stdout, stderr, status = Open3.capture3("unzip", "-Z1", archive)
  fail!("#{label} cannot be listed: #{stderr.strip}") unless status.success?
  entries = stdout.lines.map(&:chomp)
  fail!("#{label} contains duplicate entries") unless entries.uniq.length == entries.length
  entries
rescue Errno::ENOENT
  fail!("unzip is required to validate artifact archives")
end

def unzip_entry!(archive, entry, label)
  stdout, stderr, status = Open3.capture3("unzip", "-p", archive, entry)
  fail!("#{label} cannot be read from archive: #{stderr.strip}") unless status.success?
  stdout
rescue Errno::ENOENT
  fail!("unzip is required to validate artifact archives")
end

begin
gate_argument, expected_sha, anchor_sha, spec_relative, implementation_relative, repo_argument = ARGV
abort "usage: validate-c1b-evidence.rb GATE_DIR EXPECTED_SHA ANCHOR_SHA SPEC_REPORT_REL IMPLEMENTATION_REPORT_REL REPO" unless
  ARGV.length == 6

fail!("expected SHA must be lowercase 40-hex") unless SHA.match?(expected_sha)
fail!("anchor SHA must be lowercase 40-hex") unless SHA.match?(anchor_sha)
fail!("expected SHA must advance beyond anchor") if expected_sha == anchor_sha

gate_stat = File.lstat(gate_argument)
fail!("gate root must not be a symlink") if gate_stat.symlink?
fail!("gate root must be a directory") unless gate_stat.directory?
gate = File.realpath(gate_argument)
fail!("gate root does not bind expected SHA and nonce") unless
  File.basename(gate).match?(/\Ac1b-bootstrap-#{Regexp.escape(expected_sha)}-[0-9a-f]{16}\z/)
repo = File.realpath(repo_argument)
top = File.realpath(git!(repo, "rev-parse", "--show-toplevel").strip)
fail!("repository root mismatch") unless top == repo
fail!("validator policy does not belong to repository") unless
  File.realpath(POLICY_PATH) == File.join(repo, "scripts", "c1b-evidence-policy.json")

policy = JSON.parse(File.binread(POLICY_PATH))
exact_hash_keys!(policy,
  %w[schema api_version repository workflow workflow_file dispatcher_ref job_id receipts native_commands local_commands bootstrap_paths],
  "evidence policy")
fail!("evidence policy schema mismatch") unless policy.fetch("schema") == "opentake-c1b-evidence-policy-v1"
fail!("GitHub API version mismatch") unless policy.fetch("api_version") == "2026-03-10"
fail!("trusted dispatcher ref mismatch") unless policy.fetch("dispatcher_ref") == "refs/heads/main"
fail!("bootstrap allowlist is empty") unless
  policy.fetch("bootstrap_paths").is_a?(Array) && !policy.fetch("bootstrap_paths").empty?
repository = policy.fetch("repository")
workflow_path = confined_file!(repo, policy.fetch("workflow_file"), "repository workflow")
C1bCiValidator.validate(workflow_path)

head = git!(repo, "rev-parse", "HEAD").strip
fail!("repository HEAD mismatch") unless head == expected_sha
[anchor_sha, expected_sha].each do |sha|
  fail!("required bootstrap commit is missing: #{sha}") unless git_success?(repo, "cat-file", "-e", "#{sha}^{commit}")
end
fail!("expected SHA does not descend from anchor") unless
  git_success?(repo, "merge-base", "--is-ancestor", anchor_sha, expected_sha)
commits = git!(repo, "rev-list", "#{anchor_sha}..#{expected_sha}").lines.map(&:strip).reject(&:empty?)
fail!("bootstrap range is empty") if commits.empty?
touched = commits.flat_map do |commit|
  git!(repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "-m", "-z", commit).split("\0")
end.uniq
fail!("bootstrap range contains no changed paths") if touched.empty?
disallowed = touched.reject { |path| allowed_bootstrap_path?(path, policy.fetch("bootstrap_paths")) }
fail!("bootstrap range touches disallowed paths: #{disallowed.join(', ')}") unless disallowed.empty?

status = git!(repo, "status", "--porcelain=v1")
fail!("repository is not clean") unless status.empty?

run_id_path = confined_file!(gate, "run-id.txt", "run id")
run_id = positive_integer_string!(File.binread(run_id_path).strip, "run id")
fail!("run id file must contain one canonical line") unless File.binread(run_id_path) == "#{run_id}\n"

ledger_path, ledger = json_file!(gate, "command-ledger.json", "command ledger")
local_commands = policy.fetch("local_commands")
fail!("local command policy must be an array") unless local_commands.is_a?(Array)
fail!("command ledger row count mismatch") unless ledger.is_a?(Array) && ledger.length == local_commands.length
expected_local_ids = local_commands.map { |row| row.fetch("id") }
fail!("local command policy IDs are not unique") unless expected_local_ids.uniq.length == expected_local_ids.length
fail!("local command id/order mismatch") unless ledger.map { |row| row.fetch("id") } == expected_local_ids
ledger.each_with_index do |row, index|
  id = expected_local_ids.fetch(index)
  expected_command = local_commands.fetch(index).fetch("command")
  exact_hash_keys!(row,
    %w[id command cwd started_at_utc finished_at_utc exit_code log raw_exit], "local command #{id}")
  fail!("local command substituted: #{id}") unless row.fetch("command") == expected_command
  fail!("local command cwd mismatch: #{id}") unless row.fetch("cwd") == repo
  fail!("local command exit is nonzero: #{id}") unless row.fetch("exit_code") == 0
  started = timestamp!(row.fetch("started_at_utc"), "#{id} started_at_utc")
  finished = timestamp!(row.fetch("finished_at_utc"), "#{id} finished_at_utc")
  fail!("local command timestamps reversed: #{id}") if finished < started
  fail!("local log name mismatch: #{id}") unless row.fetch("log") == "#{id}.log"
  fail!("local raw-exit name mismatch: #{id}") unless row.fetch("raw_exit") == "#{id}.raw-exit"
  log = confined_file!(gate, row.fetch("log"), "#{id} log")
  fail!("local log is empty: #{id}") if File.zero?(log)
  raw = confined_file!(gate, row.fetch("raw_exit"), "#{id} raw exit")
  fail!("local raw exit mismatch: #{id}") unless File.binread(raw) == "0\n"
end

%w[pre-status.txt post-status.txt].each do |name|
  path = confined_file!(gate, name, name)
  fail!("#{name} is not clean") unless File.binread(path).empty?
end

fail!("review report paths must be distinct") if spec_relative == implementation_relative
spec_path = confined_file!(gate, spec_relative, "spec-security report")
implementation_path = confined_file!(gate, implementation_relative, "implementation report")
approved_report!(spec_path, "spec-security", expected_sha)
approved_report!(implementation_path, "implementation", expected_sha)

receipt_policies = policy.fetch("receipts")
native_command_policies = policy.fetch("native_commands")
fail!("receipt policy must contain exactly three rows") unless
  receipt_policies.is_a?(Array) && receipt_policies.length == 3
expected_receipt_ids = receipt_policies.map { |row| row.fetch("id") }
fail!("receipt policy IDs are not unique") unless expected_receipt_ids.uniq.length == 3

receipt_glob = File.join(gate, "native-receipts", "*", "*", "receipt.json")
receipt_paths = Dir.glob(receipt_glob, File::FNM_DOTMATCH)
fail!("expected exactly three native receipts") unless receipt_paths.length == 3
receipts = receipt_paths.map do |path|
  relative = Pathname.new(path).relative_path_from(Pathname.new(gate)).to_s
  confined, value = json_file!(gate, relative, "native receipt")
  [confined, value]
end
ids = receipts.map { |_path, receipt| receipt.fetch("receipt_id") }
fail!("duplicate or missing native receipt IDs") unless ids.sort == expected_receipt_ids.sort && ids.uniq.length == 3

run_ids = []
run_attempts = []
receipt_events = []
dispatcher_shas = []
dispatcher_refs = []
receipts.each do |path, receipt|
  exact_hash_keys!(receipt,
    %w[schema receipt_id repository workflow workflow_file job_id event_name run_id run_attempt runner_label runner_os runner_arch requested_sha checked_out_sha dispatcher_sha dispatcher_ref commands aggregate_exit],
    "native receipt")
  receipt_id = receipt.fetch("receipt_id")
  receipt_policy = receipt_policies.find { |row| row.fetch("id") == receipt_id }
  fail!("receipt schema mismatch: #{receipt_id}") unless receipt.fetch("schema") == RECEIPT_SCHEMA
  fail!("receipt repository mismatch: #{receipt_id}") unless receipt.fetch("repository") == policy.fetch("repository")
  fail!("receipt workflow mismatch: #{receipt_id}") unless receipt.fetch("workflow") == policy.fetch("workflow")
  fail!("receipt workflow file mismatch: #{receipt_id}") unless receipt.fetch("workflow_file") == policy.fetch("workflow_file")
  fail!("receipt job id mismatch: #{receipt_id}") unless receipt.fetch("job_id") == policy.fetch("job_id")
  fail!("receipt event mismatch: #{receipt_id}") unless receipt.fetch("event_name") == "workflow_dispatch"
  receipt_events << receipt.fetch("event_name")
  {
    "runner_label" => receipt_policy.fetch("runner"),
    "runner_os" => receipt_policy.fetch("os"),
    "runner_arch" => receipt_policy.fetch("arch"),
  }.each do |field, expected|
    fail!("receipt #{field} mismatch: #{receipt_id}") unless receipt.fetch(field) == expected
  end
  fail!("receipt requested SHA mismatch: #{receipt_id}") unless receipt.fetch("requested_sha") == expected_sha
  fail!("receipt checked-out SHA mismatch: #{receipt_id}") unless receipt.fetch("checked_out_sha") == expected_sha
  current_dispatcher_sha = receipt.fetch("dispatcher_sha")
  fail!("receipt dispatcher SHA malformed: #{receipt_id}") unless
    current_dispatcher_sha.is_a?(String) && SHA.match?(current_dispatcher_sha)
  expected_dispatcher_ref = policy.fetch("dispatcher_ref")
  fail!("receipt dispatcher ref mismatch: #{receipt_id}") unless
    receipt.fetch("dispatcher_ref") == expected_dispatcher_ref
  dispatcher_shas << current_dispatcher_sha
  dispatcher_refs << receipt.fetch("dispatcher_ref")
  current_run_id = positive_integer_string!(receipt.fetch("run_id"), "receipt run id")
  current_attempt = positive_integer_string!(receipt.fetch("run_attempt"), "receipt run attempt")
  run_ids << current_run_id
  run_attempts << current_attempt
  expected_path = File.join(gate, "native-receipts", current_run_id, receipt_id, "receipt.json")
  fail!("receipt path identity mismatch: #{receipt_id}") unless path == expected_path

  commands = receipt.fetch("commands")
  fail!("native command count mismatch: #{receipt_id}") unless
    commands.is_a?(Array) && commands.length == native_command_policies.length
  fail!("native command id/order mismatch: #{receipt_id}") unless
    commands.map { |row| row.fetch("id") } == native_command_policies.map { |row| row.fetch("id") }
  commands.each_with_index do |command, index|
    command_policy = native_command_policies.fetch(index)
    id = command_policy.fetch("id")
    exact_hash_keys!(command, %w[id command exit_code log raw_exit], "native command #{receipt_id}/#{id}")
    fail!("native command substituted: #{receipt_id}/#{id}") unless
      command.fetch("command") == command_policy.fetch("command")
    fail!("native command exit is nonzero: #{receipt_id}/#{id}") unless command.fetch("exit_code") == 0
    fail!("native log name mismatch: #{receipt_id}/#{id}") unless command.fetch("log") == "#{id}.log"
    fail!("native raw-exit name mismatch: #{receipt_id}/#{id}") unless command.fetch("raw_exit") == "#{id}.raw-exit"
  end
  fail!("native aggregate failed: #{receipt_id}") unless receipt.fetch("aggregate_exit") == 0
end
fail!("native receipts do not belong to one run") unless run_ids.uniq == [run_id]
fail!("native receipts do not belong to one run attempt") unless run_attempts.uniq.length == 1
fail!("native receipts do not belong to one event") unless receipt_events.uniq.length == 1
fail!("native receipts do not bind one dispatcher SHA") unless dispatcher_shas.uniq.length == 1
fail!("native receipts do not bind one dispatcher ref") unless dispatcher_refs.uniq.length == 1
run_attempt = run_attempts.first
dispatcher_sha = dispatcher_shas.first

gh_authenticated!
api_version = policy.fetch("api_version")
run_raw, live_run = gh_json!("/repos/#{repository}/actions/runs/#{run_id}", api_version, "workflow run")
jobs_raw, live_jobs = gh_json!(
  "/repos/#{repository}/actions/runs/#{run_id}/jobs?per_page=100", api_version, "workflow jobs")
artifacts_raw, live_artifacts = gh_json!(
  "/repos/#{repository}/actions/runs/#{run_id}/artifacts?per_page=100", api_version, "workflow artifacts")
_workflow_raw, live_workflow = gh_json!(
  "/repos/#{repository}/contents/#{policy.fetch('workflow_file')}?ref=#{dispatcher_sha}", api_version,
  "workflow content")

fail!("live run is not an object") unless live_run.is_a?(Hash)
fail!("live run id mismatch") unless live_run.fetch("id").to_s == run_id
fail!("live run attempt mismatch") unless live_run.fetch("run_attempt").to_s == run_attempt
fail!("live run repository mismatch") unless live_run.dig("repository", "full_name") == repository
fail!("live workflow name mismatch") unless live_run.fetch("name") == policy.fetch("workflow")
fail!("live workflow path mismatch") unless
  live_run.fetch("path") == policy.fetch("workflow_file")
fail!("live dispatcher branch mismatch") unless live_run.fetch("head_branch") == "main"
fail!("live run did not complete successfully") unless
  live_run.fetch("status") == "completed" && live_run.fetch("conclusion") == "success"
event = live_run.fetch("event")
fail!("live run is not a trusted default-branch dispatch") unless event == "workflow_dispatch"
fail!("receipt event does not match live run") unless receipt_events == [event, event, event]
run_head = live_run.fetch("head_sha")
fail!("live run head SHA malformed") unless run_head.is_a?(String) && SHA.match?(run_head)
fail!("live dispatcher SHA mismatch") unless run_head == dispatcher_sha

jobs = page!(live_jobs, "jobs", "workflow jobs")
artifacts = page!(live_artifacts, "artifacts", "workflow artifacts")

fail!("workflow content response must be an object") unless live_workflow.is_a?(Hash)
fail!("workflow content encoding mismatch") unless live_workflow.fetch("encoding") == "base64"
encoded_workflow = live_workflow.fetch("content")
fail!("workflow content is not base64") unless encoded_workflow.is_a?(String)
decoded_workflow = Base64.strict_decode64(encoded_workflow.gsub(/[\r\n]/, ""))
fail!("live workflow content mismatch") unless decoded_workflow == File.binread(workflow_path)

seen_job_ids = []
seen_artifact_ids = []
receipts.each do |receipt_path, receipt|
  receipt_id = receipt.fetch("receipt_id")
  receipt_root_relative = File.join("native-receipts", run_id, receipt_id)
  saved_run_path = confined_file!(gate, File.join(receipt_root_relative, "run.json"), "#{receipt_id} saved run")
  saved_jobs_path = confined_file!(gate, File.join(receipt_root_relative, "jobs.json"), "#{receipt_id} saved jobs")
  saved_artifacts_path = confined_file!(gate,
    File.join(receipt_root_relative, "artifacts.json"), "#{receipt_id} saved artifacts")
  saved_artifact_path, saved_artifact = json_file!(gate,
    File.join(receipt_root_relative, "artifact.json"), "#{receipt_id} saved artifact")
  fail!("saved run JSON differs from live API: #{receipt_id}") unless File.binread(saved_run_path) == run_raw
  fail!("saved jobs JSON differs from live API: #{receipt_id}") unless File.binread(saved_jobs_path) == jobs_raw
  fail!("saved artifacts JSON differs byte-for-byte from live API: #{receipt_id}") unless
    File.binread(saved_artifacts_path) == artifacts_raw

  expected_job_name = "Safe filesystem (#{receipt_id})"
  matched_jobs = jobs.select { |job| job["name"] == expected_job_name }
  fail!("missing or duplicate live job: #{receipt_id}") unless matched_jobs.length == 1
  job = matched_jobs.first
  job_id = positive_integer_string!(job.fetch("id"), "live job id")
  fail!("live job run mismatch: #{receipt_id}") unless job.fetch("run_id").to_s == run_id
  fail!("live job attempt mismatch: #{receipt_id}") unless job.fetch("run_attempt").to_s == run_attempt
  fail!("live job head mismatch: #{receipt_id}") unless job.fetch("head_sha") == run_head
  fail!("live job did not complete successfully: #{receipt_id}") unless
    job.fetch("status") == "completed" && job.fetch("conclusion") == "success"
  seen_job_ids << job_id

  expected_artifact_name = "c1b-native-#{receipt_id}-#{expected_sha}"
  matched_artifacts = artifacts.select { |artifact| artifact["name"] == expected_artifact_name }
  fail!("missing or duplicate live artifact: #{receipt_id}") unless matched_artifacts.length == 1
  artifact = matched_artifacts.first
  artifact_id = positive_integer_string!(artifact.fetch("id"), "live artifact id")
  fail!("saved artifact row differs from live artifact list: #{receipt_id}") unless saved_artifact == artifact
  fail!("live artifact expired: #{receipt_id}") unless artifact.fetch("expired") == false
  fail!("live artifact run mismatch: #{receipt_id}") unless artifact.dig("workflow_run", "id").to_s == run_id
  fail!("live artifact head mismatch: #{receipt_id}") unless
    artifact.dig("workflow_run", "head_sha") == run_head
  digest = artifact.fetch("digest")
  fail!("live artifact digest malformed: #{receipt_id}") unless
    digest.is_a?(String) && digest.match?(/\Asha256:[0-9a-f]{64}\z/)
  seen_artifact_ids << artifact_id

  archive = confined_file!(gate, File.join(receipt_root_relative, "artifact.zip"), "#{receipt_id} archive")
  actual_digest = "sha256:#{Digest::SHA256.file(archive).hexdigest}"
  fail!("artifact archive digest mismatch: #{receipt_id}") unless actual_digest == digest

  expected_entries = native_command_policies.flat_map do |command|
    ["#{command.fetch('id')}.log", "#{command.fetch('id')}.raw-exit"]
  end + %w[final-aggregate.raw-exit receipt.json]
  entries = unzip_entries!(archive, "#{receipt_id} archive")
  fail!("artifact archive entry mismatch: #{receipt_id}") unless entries.sort == expected_entries.sort
  expected_entries.each do |entry|
    local = confined_file!(gate, File.join(receipt_root_relative, entry), "#{receipt_id} #{entry}")
    fail!("artifact archive content mismatch: #{receipt_id}/#{entry}") unless
      unzip_entry!(archive, entry, "#{receipt_id} #{entry}") == File.binread(local)
  end
  native_command_policies.each do |command|
    id = command.fetch("id")
    log = confined_file!(gate, File.join(receipt_root_relative, "#{id}.log"), "#{receipt_id} #{id} log")
    fail!("native log is empty: #{receipt_id}/#{id}") if File.zero?(log)
    raw = confined_file!(gate, File.join(receipt_root_relative, "#{id}.raw-exit"), "#{receipt_id} #{id} exit")
    fail!("native raw exit mismatch: #{receipt_id}/#{id}") unless File.binread(raw) == "0\n"
  end
  aggregate = confined_file!(gate, File.join(receipt_root_relative, "final-aggregate.raw-exit"),
    "#{receipt_id} aggregate")
  fail!("native aggregate raw exit mismatch: #{receipt_id}") unless File.binread(aggregate) == "0\n"
  fail!("receipt file identity changed during validation") unless receipt_path ==
    File.join(gate, receipt_root_relative, "receipt.json")
  fail!("saved artifact path identity changed during validation") unless saved_artifact_path ==
    File.join(gate, receipt_root_relative, "artifact.json")
end
fail!("duplicate workflow job IDs") unless seen_job_ids.uniq.length == 3
fail!("duplicate artifact IDs") unless seen_artifact_ids.uniq.length == 3

results_path = confined_file!(gate, "results.md", "results")
results = File.binread(results_path)
field!(results, "Task", REVIEW_TASK, "results", case_sensitive: false)
field!(results, "Anchor SHA", anchor_sha, "results", case_sensitive: false)
field!(results, "Final SHA", expected_sha, "results", case_sensitive: false)
field!(results, "Run ID", run_id, "results")
field!(results, "Pre-status", "clean", "results", case_sensitive: false)
field!(results, "Post-status", "clean", "results", case_sensitive: false)
field!(results, "Spec report", spec_relative, "results")
field!(results, "Implementation report", implementation_relative, "results")
field!(results, "Aggregate", "0", "results")

puts "c1b-evidence-validation=ok anchor=#{anchor_sha} sha=#{expected_sha} run=#{run_id} attempt=#{run_attempt}"
rescue KeyError => error
  abort "evidence field missing: #{error.message}"
rescue JSON::ParserError => error
  abort "invalid evidence policy JSON: #{error.message}"
rescue ArgumentError, Errno::ENOENT, Errno::ENOTDIR => error
  abort error.message
end
