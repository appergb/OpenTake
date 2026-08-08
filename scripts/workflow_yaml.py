#!/usr/bin/env python3
"""Parse GitHub workflows once as strict, unique-key YAML 1.2-style data."""

import json
import subprocess


class WorkflowYamlError(ValueError):
    """The workflow cannot be decoded as a unique-key YAML 1.2 structure."""


_PSYCH_AST_TO_JSON = r"""
require "json"
require "psych"

def scalar_value(node)
  raise "tagged scalars are forbidden" unless node.tag.nil?
  raise "anchored scalars are forbidden" unless node.anchor.nil?
  return node.value unless node.plain
  case node.value
  when "true"
    true
  when "false"
    false
  when "null", "~"
    nil
  when /\A-?(?:0|[1-9][0-9]*)\z/
    Integer(node.value, 10)
  else
    node.value
  end
end

def convert(node)
  case node
  when Psych::Nodes::Mapping
    raise "tagged mappings are forbidden" unless node.tag.nil?
    raise "anchored mappings are forbidden" unless node.anchor.nil?
    result = {}
    node.children.each_slice(2) do |key, value|
      unless key.is_a?(Psych::Nodes::Scalar) && key.tag.nil? && key.anchor.nil?
        raise "mapping keys must be untagged scalars"
      end
      name = key.value
      raise "duplicate mapping key #{name.inspect}" if result.key?(name)
      result[name] = convert(value)
    end
    result
  when Psych::Nodes::Sequence
    raise "tagged sequences are forbidden" unless node.tag.nil?
    raise "anchored sequences are forbidden" unless node.anchor.nil?
    node.children.map { |child| convert(child) }
  when Psych::Nodes::Scalar
    scalar_value(node)
  else
    raise "unsupported YAML node #{node.class}"
  end
end

begin
  stream = Psych.parse_stream(STDIN.read)
  raise "workflow must contain exactly one YAML document" unless stream.children.length == 1
  root = stream.children.fetch(0).root
  raise "workflow document is empty" if root.nil?
  puts JSON.generate(convert(root))
rescue StandardError => error
  warn error.message
  exit 2
end
"""


def parse_workflow_yaml(workflow: str) -> dict[str, object]:
    """Return the sole strict workflow document or raise ``WorkflowYamlError``."""

    try:
        process = subprocess.run(
            ["ruby", "-e", _PSYCH_AST_TO_JSON],
            input=workflow,
            capture_output=True,
            check=False,
            encoding="utf-8",
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise WorkflowYamlError(f"Ruby Psych parser is unavailable: {error}") from error
    if process.returncode != 0:
        detail = process.stderr.strip() or "Ruby Psych rejected the workflow"
        raise WorkflowYamlError(detail)
    try:
        document = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise WorkflowYamlError("Ruby Psych returned malformed JSON") from error
    if not isinstance(document, dict):
        raise WorkflowYamlError("workflow root must be a mapping")
    return document
