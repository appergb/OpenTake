#!/usr/bin/env ruby
# frozen_string_literal: true

require "base64"
require "digest"
require "fileutils"
require "json"
require "open3"
require "rbconfig"
require "tmpdir"

ROOT = File.expand_path("../..", __dir__)
POLICY = JSON.parse(File.read(File.join(ROOT, "scripts/c1b-evidence-policy.json")))
VALIDATOR = File.join(ROOT, "scripts/validate-c1b-evidence.rb")

def assert(condition, message)
  raise message unless condition
end

assert(POLICY.fetch("bootstrap_paths") == [".github/workflows/ci.yml", "scripts/"],
  "bootstrap allowlist")
assert(POLICY.fetch("dispatcher_ref") == "refs/heads/main", "trusted dispatcher ref")

def git(*arguments)
  output, status = Open3.capture2("git", "-C", ROOT, *arguments)
  raise "git #{arguments.join(' ')} failed" unless status.success?
  output.strip
end

def write_json(path, value)
  File.write(path, JSON.pretty_generate(value) + "\n")
end

def rewrite_json(path)
  value = JSON.parse(File.read(path))
  yield value
  write_json(path, value)
end

def install_fake_gh(root)
  bin = File.join(root, "bin")
  FileUtils.mkdir_p(bin)
  File.write(File.join(bin, "gh"), <<~'SH')
    #!/bin/sh
    set -eu
    fixture=${C1B_FAKE_GH_ROOT:?}
    if [ "$1" = auth ] && [ "$2" = status ]; then
      [ "${C1B_FAKE_GH_AUTH_FAIL:-0}" != 1 ]
      exit
    fi
    [ "$1" = api ]
    [ "${C1B_FAKE_GH_API_FAIL:-0}" != 1 ] || exit 69
    shift
    endpoint=
    while [ "$#" -gt 0 ]; do endpoint=$1; shift; done
    case "$endpoint" in
      */actions/runs/*/jobs?per_page=100) cat "$fixture/jobs.json" ;;
      */actions/runs/*/artifacts?per_page=100) cat "$fixture/artifacts.json" ;;
      */actions/runs/*) cat "$fixture/run.json" ;;
      */contents/.github/workflows/ci.yml?ref=*) cat "$fixture/workflow-content.json" ;;
      *) echo "unsupported fake gh endpoint: $endpoint" >&2; exit 64 ;;
    esac
  SH
  File.chmod(0o755, File.join(bin, "gh"))
  bin
end

def build_fixture(root, label)
  expected = git("rev-parse", "HEAD")
  anchor = git("rev-parse", "HEAD^")
  nonce = Digest::SHA256.hexdigest(label)[0, 16]
  gate = File.join(root, "c1b-bootstrap-#{expected}-#{nonce}")
  fixture = File.join(root, "#{label}-live")
  FileUtils.mkdir_p([gate, fixture, File.join(gate, "reviews")])
  run_id = "424242"
  dispatcher_sha = "f" * 40
  dispatcher_ref = POLICY.fetch("dispatcher_ref")
  File.write(File.join(gate, "run-id.txt"), "#{run_id}\n")
  File.write(File.join(gate, "pre-status.txt"), "")
  File.write(File.join(gate, "post-status.txt"), "")
  report = lambda do |role|
    "Role: #{role}\nTask: evidence-bootstrap\nCommit: #{expected}\n" \
      "Verdict: APPROVE\nCritical: 0\nImportant: 0\nMinor: 0\n"
  end
  spec = "reviews/spec-security-review.md"
  implementation = "reviews/implementation-review.md"
  File.write(File.join(gate, spec), report.call("spec-security"))
  File.write(File.join(gate, implementation), report.call("implementation"))

  timestamp = "2026-07-17T00:00:00Z"
  ledger = POLICY.fetch("local_commands").map do |row|
    id = row.fetch("id")
    File.write(File.join(gate, "#{id}.log"), "synthetic #{id}\n")
    File.write(File.join(gate, "#{id}.raw-exit"), "0\n")
    row.merge("cwd" => ROOT, "started_at_utc" => timestamp,
      "finished_at_utc" => timestamp, "exit_code" => 0,
      "log" => "#{id}.log", "raw_exit" => "#{id}.raw-exit")
  end
  write_json(File.join(gate, "command-ledger.json"), ledger)

  run = {
    "id" => run_id.to_i, "run_attempt" => 1, "head_sha" => dispatcher_sha,
    "head_branch" => "main", "event" => "workflow_dispatch",
    "status" => "completed", "conclusion" => "success", "name" => "CI",
    "path" => POLICY.fetch("workflow_file"),
    "pull_requests" => [],
    "repository" => { "full_name" => POLICY.fetch("repository") },
  }
  jobs = POLICY.fetch("receipts").each_with_index.map do |receipt, index|
    {
      "id" => 7000 + index, "run_id" => run_id.to_i, "run_attempt" => 1,
      "head_sha" => dispatcher_sha, "name" => "Safe filesystem (#{receipt.fetch('id')})",
      "status" => "completed", "conclusion" => "success",
    }
  end
  artifacts = []
  POLICY.fetch("receipts").each_with_index do |receipt_policy, index|
    id = receipt_policy.fetch("id")
    directory = File.join(gate, "native-receipts", run_id, id)
    FileUtils.mkdir_p(directory)
    commands = POLICY.fetch("native_commands").map do |row|
      command_id = row.fetch("id")
      File.write(File.join(directory, "#{command_id}.log"), "synthetic #{command_id}\n")
      File.write(File.join(directory, "#{command_id}.raw-exit"), "0\n")
      row.merge("exit_code" => 0, "log" => "#{command_id}.log",
        "raw_exit" => "#{command_id}.raw-exit")
    end
    File.write(File.join(directory, "final-aggregate.raw-exit"), "0\n")
    receipt = {
      "schema" => "opentake-c1b-native-receipt-v1", "receipt_id" => id,
      "repository" => POLICY.fetch("repository"), "workflow" => POLICY.fetch("workflow"),
      "workflow_file" => POLICY.fetch("workflow_file"), "job_id" => POLICY.fetch("job_id"),
      "event_name" => "workflow_dispatch", "run_id" => run_id, "run_attempt" => "1",
      "runner_label" => receipt_policy.fetch("runner"), "runner_os" => receipt_policy.fetch("os"),
      "runner_arch" => receipt_policy.fetch("arch"), "requested_sha" => expected,
      "checked_out_sha" => expected, "dispatcher_sha" => dispatcher_sha,
      "dispatcher_ref" => dispatcher_ref, "commands" => commands, "aggregate_exit" => 0,
    }
    write_json(File.join(directory, "receipt.json"), receipt)
    archive_files = commands.flat_map { |row| [row.fetch("log"), row.fetch("raw_exit")] } +
      %w[final-aggregate.raw-exit receipt.json]
    _out, error, status = Open3.capture3("zip", "-q", "artifact.zip", *archive_files,
      chdir: directory)
    raise "zip failed: #{error}" unless status.success?
    artifact = {
      "id" => 8000 + index, "name" => "c1b-native-#{id}-#{expected}",
      "expired" => false,
      "digest" => "sha256:#{Digest::SHA256.file(File.join(directory, 'artifact.zip')).hexdigest}",
      "workflow_run" => { "id" => run_id.to_i, "head_sha" => dispatcher_sha },
    }
    artifacts << artifact
    write_json(File.join(directory, "run.json"), run)
    write_json(File.join(directory, "jobs.json"), { "total_count" => jobs.length, "jobs" => jobs })
    write_json(File.join(directory, "artifact.json"), artifact)
  end
  write_json(File.join(fixture, "run.json"), run)
  write_json(File.join(fixture, "jobs.json"), { "total_count" => jobs.length, "jobs" => jobs })
  artifact_page = { "total_count" => artifacts.length, "artifacts" => artifacts }
  write_json(File.join(fixture, "artifacts.json"), artifact_page)
  Dir.glob(File.join(gate, "native-receipts", run_id, "*"), File::FNM_DOTMATCH).each do |directory|
    next unless File.directory?(directory) && !%w[. ..].include?(File.basename(directory))

    write_json(File.join(directory, "artifacts.json"), artifact_page)
  end
  write_json(File.join(fixture, "workflow-content.json"),
    { "encoding" => "base64", "content" => Base64.strict_encode64(
      File.binread(File.join(ROOT, POLICY.fetch("workflow_file")))) })
  results = [
    "Task: evidence-bootstrap", "Anchor SHA: #{anchor}", "Final SHA: #{expected}",
    "Run ID: #{run_id}", "Pre-status: clean", "Post-status: clean",
    "Spec report: #{spec}", "Implementation report: #{implementation}", "Aggregate: 0",
  ].join("\n") + "\n"
  File.write(File.join(gate, "results.md"), results)
  [gate, expected, anchor, spec, implementation, fixture]
end

def run_validator(gate, expected, anchor, spec, implementation, fixture, fake_bin, extra_env = {})
  env = { "PATH" => "#{fake_bin}#{File::PATH_SEPARATOR}#{ENV.fetch('PATH', '')}",
    "C1B_FAKE_GH_ROOT" => fixture }.merge(extra_env)
  Open3.capture3(env, RbConfig.ruby, VALIDATOR, gate, expected, anchor,
    spec, implementation, ROOT)
end

Dir.mktmpdir("c1b-evidence-test") do |temporary|
  fake_bin = install_fake_gh(temporary)
  gate, expected, anchor, spec, implementation, fixture = build_fixture(temporary, "canonical")
  stdout, stderr, status = run_validator(gate, expected, anchor, spec, implementation,
    fixture, fake_bin)
  if !status.success? && stderr.include?("C1B evidence validator not implemented")
    abort "C1B evidence validator not implemented"
  end
  assert(status.success?, "canonical synthetic evidence rejected: #{stdout}#{stderr}")
  assert(stdout.include?("c1b-evidence-validation=ok"), "canonical success marker")

  mutations = {
    "dirty-pre-status" => ->(copy, _live) { File.write(File.join(copy, "pre-status.txt"), " M file\n") },
    "invalid-ledger-timestamp" => lambda { |copy, _live|
      rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["started_at_utc"] = "not-a-time" }
    },
    "absolute-ledger-log" => lambda { |copy, _live|
      rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["log"] = "/tmp/outside.log" }
    },
    "review-has-finding" => lambda { |copy, _live|
      path = File.join(copy, "reviews/spec-security-review.md")
      File.write(path, File.read(path).sub("Important: 0", "Important: 1"))
    },
    "review-wrong-commit" => lambda { |copy, _live|
      path = File.join(copy, "reviews/implementation-review.md")
      File.write(path, File.read(path).sub(/Commit: [0-9a-f]{40}/, "Commit: #{'0' * 40}"))
    },
    "escaped-ledger-log" => lambda { |copy, _live|
      rewrite_json(File.join(copy, "command-ledger.json")) { |rows| rows[0]["log"] = "../outside.log" }
    },
    "external-review-symlink" => lambda { |copy, _live|
      path = File.join(copy, "reviews/spec-security-review.md")
      outside = File.join(File.dirname(copy), "outside-review.md")
      File.write(outside, File.read(path)); File.unlink(path); File.symlink(outside, path)
    },
    "wrong-receipt-sha" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).first
      rewrite_json(path) { |value| value["checked_out_sha"] = "0" * 40 }
    },
    "wrong-dispatcher-sha" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).first
      rewrite_json(path) { |value| value["dispatcher_sha"] = "0" * 40 }
    },
    "wrong-receipt-provenance" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).first
      rewrite_json(path) { |value| value["runner_os"] = "WrongOS" }
    },
    "wrong-native-command" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).first
      rewrite_json(path) { |value| value.fetch("commands")[0]["command"] = "cargo test --workspace" }
    },
    "duplicate-receipt-id" => lambda { |copy, _live|
      paths = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).sort
      first_id = JSON.parse(File.read(paths.first)).fetch("receipt_id")
      rewrite_json(paths.last) { |value| value["receipt_id"] = first_id }
    },
    "missing-receipt-row" => lambda { |copy, _live|
      FileUtils.rm_rf(File.dirname(Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).last))
    },
    "mixed-run-attempt" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/receipt.json")).last
      rewrite_json(path) { |value| value["run_attempt"] = "2" }
    },
    "forged-saved-run" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/run.json")).first
      rewrite_json(path) { |value| value["head_sha"] = "0" * 40 }
    },
    "wrong-live-workflow-path" => lambda { |copy, live|
      paths = [File.join(live, "run.json")] + Dir.glob(File.join(copy, "native-receipts/*/*/run.json"))
      paths.each { |path| rewrite_json(path) { |value| value["path"] = ".github/workflows/not-ci.yml" } }
    },
    "duplicate-live-job-id" => lambda { |copy, live|
      paths = [File.join(live, "jobs.json")] + Dir.glob(File.join(copy, "native-receipts/*/*/jobs.json"))
      paths.each do |path|
        rewrite_json(path) { |value| value.fetch("jobs")[1]["id"] = value.fetch("jobs")[0].fetch("id") }
      end
    },
    "duplicate-live-artifact-id" => lambda { |copy, live|
      page_paths = [File.join(live, "artifacts.json")] +
        Dir.glob(File.join(copy, "native-receipts/*/*/artifacts.json"))
      duplicate = JSON.parse(File.read(page_paths.first)).fetch("artifacts")[0].fetch("id")
      page_paths.each do |path|
        rewrite_json(path) { |value| value.fetch("artifacts")[1]["id"] = duplicate }
      end
      saved = Dir.glob(File.join(copy, "native-receipts/*/*/artifact.json")).sort[1]
      rewrite_json(saved) { |value| value["id"] = duplicate }
    },
    "forged-saved-artifact-bytes" => lambda { |copy, _live|
      path = Dir.glob(File.join(copy, "native-receipts/*/*/artifacts.json")).first
      File.open(path, "a") { |file| file.write("\n") }
    },
    "bad-live-digest" => lambda { |copy, live|
      page_paths = [File.join(live, "artifacts.json")] +
        Dir.glob(File.join(copy, "native-receipts/*/*/artifacts.json"))
      page_paths.each do |path|
        rewrite_json(path) do |value|
          value.fetch("artifacts")[0]["digest"] = "sha256:#{'0' * 64}"
        end
      end
      path = Dir.glob(File.join(copy, "native-receipts/*/*/artifact.json")).first
      rewrite_json(path) { |value| value["digest"] = "sha256:#{'0' * 64}" }
    },
    "missing-results-aggregate" => lambda { |copy, _live|
      path = File.join(copy, "results.md")
      File.write(path, File.read(path).sub("Aggregate: 0", "Aggregate: missing"))
    },
    "wrong-results-final-sha" => lambda { |copy, _live|
      path = File.join(copy, "results.md")
      File.write(path, File.read(path).sub(/Final SHA: [0-9a-f]{40}/, "Final SHA: #{'0' * 40}"))
    },
  }
  expected_errors = {
    "duplicate-live-artifact-id" => "duplicate artifact IDs",
    "bad-live-digest" => "artifact archive digest mismatch",
  }
  mutations.each do |label, mutate|
    copy, copy_expected, copy_anchor, copy_spec, copy_implementation, copy_fixture =
      build_fixture(temporary, label)
    mutate.call(copy, copy_fixture)
    out, err, result = run_validator(copy, copy_expected, copy_anchor, copy_spec,
      copy_implementation, copy_fixture, fake_bin)
    assert(!result.success?, "validator accepted mutation #{label}")
    expected_error = expected_errors[label]
    assert("#{out}#{err}".include?(expected_error),
      "validator rejected #{label} for the wrong reason") if expected_error
  end

  missing_env = { "PATH" => File.join(temporary, "missing-gh") }
  _out, _err, missing = Open3.capture3(missing_env, RbConfig.ruby, VALIDATOR,
    gate, expected, anchor, spec, implementation, ROOT)
  assert(!missing.success?, "validator accepted missing authenticated gh")

  _out, _err, unauthenticated = run_validator(gate, expected, anchor, spec, implementation,
    fixture, fake_bin, "C1B_FAKE_GH_AUTH_FAIL" => "1")
  assert(!unauthenticated.success?, "validator accepted unauthenticated gh")

  _out, _err, api_failure = run_validator(gate, expected, anchor, spec, implementation,
    fixture, fake_bin, "C1B_FAKE_GH_API_FAIL" => "1")
  assert(!api_failure.success?, "validator accepted GitHub API failure")

  _out, _err, absolute_review = run_validator(gate, expected, anchor,
    File.join(gate, spec), implementation, fixture, fake_bin)
  assert(!absolute_review.success?, "validator accepted absolute review path")
end

puts "c1b-evidence-validator-tests=ok"
