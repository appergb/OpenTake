#!/usr/bin/env python3
"""Fail closed when the native Windows product gate loses required coverage."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Callable

from workflow_yaml import WorkflowYamlError, parse_workflow_yaml


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
TAURI_CONFIG_PATH = REPOSITORY_ROOT / "src-tauri" / "tauri.conf.json"
NORMAL_JOB_CONDITION = (
    "if: github.event_name != 'workflow_dispatch' || inputs.red_task == 'none'"
)
TARGET_SHA_EXPRESSION = (
    "TARGET_SHA: ${{ github.event_name == 'workflow_dispatch' && inputs.commit_sha || "
    "github.event_name == 'pull_request' && github.event.pull_request.head.sha || github.sha }}"
)
ACTION_PINS = MappingProxyType(
    {
        "actions/checkout": "11d5960a326750d5838078e36cf38b85af677262",
        "actions/setup-node": "49933ea5288caeca8642d1e84afbd3f7d6820020",
        "actions/cache": "0057852bfaa89a56745cba8c7296529d2fc39830",
        "actions/upload-artifact": "ea165f8d65b6e75b540449e92b4886f43607fa02",
        "pnpm/action-setup": "b906affcce14559ad1aafd4ab0e942779e9f58b1",
        "ruby/setup-ruby": "95ef2b042f9d7a56d8268cba8559e2842e2ad01b",
    }
)
EXPECTED_CHECKOUT_REFS = MappingProxyType(
    {
        "motion-canvas": (None,),
        "rust": (None,),
        "windows-product": ("${{ env.TARGET_SHA }}",),
        "windows-security": ("${{ env.TARGET_SHA }}",),
        "web": (None,),
        "windows-library-security": ("${{ env.TARGET_SHA }}",),
        "safe-filesystem": ("${{ env.TARGET_SHA }}",),
        "windows-red-evidence": (
            "${{ env.DISPATCHER_SHA }}",
            "${{ env.TARGET_SHA }}",
        ),
    }
)
def _pinned_action(action: str) -> str:
    return f"{action}@{ACTION_PINS[action]}"


def validate_action_pin_ownership(
    resolve_commit: Callable[[str, str], str],
) -> list[str]:
    errors: list[str] = []
    for repository, expected_sha in ACTION_PINS.items():
        try:
            resolved_sha = resolve_commit(repository, expected_sha)
        except Exception:
            resolved_sha = ""
        if resolved_sha != expected_sha:
            errors.append(f"{repository} pin is not a commit in its repository")
    return errors


def _resolve_github_commit(repository: str, sha: str) -> str:
    result = subprocess.run(
        [
            "gh",
            "api",
            f"repos/{repository}/commits/{sha}",
            "--jq",
            ".sha",
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return result.stdout.strip()


@dataclass(frozen=True)
class GateContract:
    name: str
    step_id: str
    shell: str
    step_sha256: str
    receipt_id: str
    receipt_command: str

    def __post_init__(self) -> None:
        if not all(
            (self.name, self.step_id, self.shell, self.receipt_id, self.receipt_command)
        ):
            raise ValueError("gate contract fields must be non-empty")
        if re.fullmatch(r"[0-9a-f]{64}", self.step_sha256) is None:
            raise ValueError("gate contract digest must be immutable SHA-256")


@dataclass(frozen=True)
class JobContract:
    job_name: str
    label: str
    receipt_label: str
    receipt_step_sha256: str
    enforce_step_sha256: str
    gates: tuple[GateContract, ...]

    def __post_init__(self) -> None:
        if any(
            re.fullmatch(r"[0-9a-f]{64}", digest) is None
            for digest in (self.receipt_step_sha256, self.enforce_step_sha256)
        ):
            raise ValueError("receipt step digests must be immutable SHA-256")
        if len(self.gates) != 4:
            raise ValueError(f"{self.job_name} must bind exactly four critical gates")
        for attribute in ("name", "step_id", "receipt_id"):
            values = [getattr(gate, attribute) for gate in self.gates]
            if len(values) != len(set(values)):
                raise ValueError(f"{self.job_name} has duplicate gate {attribute}")

    @property
    def input_step_name(self) -> str:
        return f"Validate immutable Windows {self.label} SHA input"

    @property
    def bind_step_name(self) -> str:
        return f"Assert exact Windows {self.label} checkout"

    @property
    def bind_id(self) -> str:
        return f"bind-{self.label}"

    @property
    def receipt_step_name(self) -> str:
        return f"Build Windows {self.receipt_label} JSON receipt"

    @property
    def receipt_slug(self) -> str:
        return self.receipt_label.replace(" ", "-")

    @property
    def receipt_schema(self) -> str:
        return f"opentake-windows-{self.receipt_slug}-receipt-v1"

    @property
    def receipt_path(self) -> str:
        return f"windows-{self.receipt_slug}-receipt/receipt.json"

    @property
    def upload_step_name(self) -> str:
        return f"Upload exact-SHA Windows {self.receipt_label} receipt"

    @property
    def artifact_name(self) -> str:
        return f"windows-{self.receipt_slug}-${{{{ steps.{self.bind_id}.outputs.sha }}}}"

    @property
    def enforce_step_name(self) -> str:
        return f"Enforce Windows {self.receipt_label} aggregate"


@dataclass(frozen=True)
class ShaBoundJobContract:
    job_name: str
    bind_step_name: str
    before_reassert_name: str
    after_reassert_name: str
    first_gate_name: str
    job_sha256: str
    step_identities: tuple[str, ...]

    def __post_init__(self) -> None:
        if re.fullmatch(r"[0-9a-f]{64}", self.job_sha256) is None:
            raise ValueError("job contract digest must be immutable SHA-256")
        if len(self.step_identities) != len(set(self.step_identities)):
            raise ValueError(f"{self.job_name} step identities must be unique")


SPECIALIST_CONTRACTS = (
    JobContract(
        job_name="windows-security",
        label="security",
        receipt_label="security",
        receipt_step_sha256=(
            "fba3fe1ca73b067454770fe798116c5798182724bffe4558a1d0eb2ad1401c5d"
        ),
        enforce_step_sha256=(
            "4ab2ff4776426b4ce82d0d858b154f13c798344b870535471b8b7d5974117cdc"
        ),
        gates=(
            GateContract(
                name="Portable FFmpeg cancellation lifecycle",
                step_id="security-cancellation",
                shell="pwsh",
                step_sha256="a54cf70f4c3e46bd950d0aa5297f146fe9ab4098f4a73029a8bc01c806017817",
                receipt_id="cancellation",
                receipt_command="packaged FFmpeg -version; cargo test opentake-media Windows cancellation lifecycle",
            ),
            GateContract(
                name="Race-free helper process-tree containment",
                step_id="security-process-tree",
                shell="pwsh",
                step_sha256="dc35fee96b2bbcaa39449d9baa11062b39cb49f834fe8f05a44d947f5f0decad",
                receipt_id="process-tree",
                receipt_command="cargo test -p opentake-media --lib process_tree::tests::windows_suspended_job_contains_fast_exit_descendant -- --exact --nocapture --test-threads=1",
            ),
            GateContract(
                name="Verify portable Tauri test image imports",
                step_id="security-native-imports",
                shell="pwsh",
                step_sha256="eb033e2a269a3b3121d1a8a32d5f2c36bfcec12c1dd859abf5aae79e75e4d6bd",
                receipt_id="native-imports",
                receipt_command="cargo test -p opentake-tauri --lib --no-run; verify imports, manifest, and native exports",
            ),
            GateContract(
                name="Reserved output identity and reparse safety",
                step_id="security-reparse",
                shell="pwsh",
                step_sha256="9b2cb3d2a1d5b036a6e9d304adb0d8959be1996906a0a0a2baab6e5a9d1456c4",
                receipt_id="reparse-safety",
                receipt_command="cargo test opentake-tauri Windows retained-output and reparse safety contracts",
            ),
        ),
    ),
    JobContract(
        job_name="windows-library-security",
        label="library",
        receipt_label="library security",
        receipt_step_sha256=(
            "ba155d7be2752bbbcb8478809fc61797134311203efd0be1e00394f82101e378"
        ),
        enforce_step_sha256=(
            "8840a28182ab9f2e79c7321a5621e78a9c156e62982cb29577b61c1fe0ce0c67"
        ),
        gates=(
            GateContract(
                name="Test retained-handle and junction defenses",
                step_id="library-media",
                shell="pwsh",
                step_sha256="db5958a2fd760240ca83031afc3b95957399347cc6f6fbb5fac460ea3104406b",
                receipt_id="media-library",
                receipt_command="cargo test -p opentake-media library::tests -- --test-threads=1",
            ),
            GateContract(
                name="Test complete bundle publication and recovery",
                step_id="library-project",
                shell="pwsh",
                step_sha256="750491bc9cad44421c91c8018f409aa77989003765a4691e5fcb454ad82a0007",
                receipt_id="project",
                receipt_command="cargo test -p opentake-project -- --test-threads=1",
            ),
            GateContract(
                name="Test Tauri project-library commit guards",
                step_id="library-tauri",
                shell="pwsh",
                step_sha256="f0dfbb62a5bc8e89d3a24c5d0d3887d2cbbf78bc34f3468d673d8f960fec658c",
                receipt_id="tauri-library",
                receipt_command="cargo test -p opentake-tauri library::tests -- --test-threads=1",
            ),
            GateContract(
                name="Clippy capability-backed library",
                step_id="library-clippy",
                shell="pwsh",
                step_sha256="32578d93d1ad7c315c5931c645e71acbd41869b52d002cac0a5c3ab7522f12ee",
                receipt_id="media-clippy",
                receipt_command="cargo clippy -p opentake-media --all-targets -- -D warnings",
            ),
        ),
    ),
)

SHA_BOUND_JOB_CONTRACTS = (
    ShaBoundJobContract(
        job_name="windows-product",
        bind_step_name="Assert exact checked-out SHA",
        before_reassert_name="Re-assert immutable Windows product source before gates",
        after_reassert_name="Re-assert immutable Windows product source after gates",
        first_gate_name="Rust formatting",
        job_sha256="e55c11b2d14037a8edc227bb7f2527f02ec7cec6e0c0e7194ee3bfaf6b7d8ed6",
        step_identities=(
            "name:Validate immutable SHA input",
            f"uses:{_pinned_action('actions/checkout')}",
            "name:Assert exact checked-out SHA",
            "name:Install Rust toolchain",
            f"uses:{_pinned_action('pnpm/action-setup')}",
            f"uses:{_pinned_action('actions/setup-node')}",
            f"uses:{_pinned_action('ruby/setup-ruby')}",
            "name:Provision checksum-pinned packaged FFmpeg sidecars",
            "name:Verify sidecar supply and probe/decode/encode boundary without PATH",
            "name:Cache cargo",
            "name:Install locked Web dependencies",
            "name:Re-assert immutable Windows product source before gates",
            "name:Rust formatting",
            "name:Rust workspace clippy",
            "name:Windows Chromium motion capture regression",
            "name:Web editor behavior suite",
            "name:Rust workspace tests",
            "name:Minimal-feature Tauri clippy",
            "name:Web production build",
            "name:Build native MSI and NSIS installers",
            "name:Install NSIS package and execute installed product without PATH",
            "name:Re-assert immutable Windows product source after gates",
            "name:Bind installers to the exact source SHA",
            "name:Upload exact-SHA Windows installers",
        ),
    ),
    ShaBoundJobContract(
        job_name="windows-security",
        bind_step_name="Assert exact Windows security checkout",
        before_reassert_name="Re-assert immutable Windows security source before gates",
        after_reassert_name="Re-assert immutable Windows security source after gates",
        first_gate_name="Portable FFmpeg cancellation lifecycle",
        job_sha256="5ea53859a3df822e3e99eb6a36bf95e3919c21bfa564fcfa4a77bc168bc218ac",
        step_identities=(
            "name:Validate immutable Windows security SHA input",
            f"uses:{_pinned_action('actions/checkout')}",
            "name:Assert exact Windows security checkout",
            "name:Install Rust toolchain",
            "name:Provision checksum-pinned packaged FFmpeg sidecars",
            "name:Cache cargo",
            "name:Re-assert immutable Windows security source before gates",
            "name:Portable FFmpeg cancellation lifecycle",
            "name:Race-free helper process-tree containment",
            "name:Verify portable Tauri test image imports",
            "name:Reserved output identity and reparse safety",
            "name:Re-assert immutable Windows security source after gates",
            "name:Build Windows security JSON receipt",
            "name:Upload exact-SHA Windows security receipt",
            "name:Enforce Windows security aggregate",
        ),
    ),
    ShaBoundJobContract(
        job_name="windows-library-security",
        bind_step_name="Assert exact Windows library checkout",
        before_reassert_name="Re-assert immutable Windows library source before gates",
        after_reassert_name="Re-assert immutable Windows library source after gates",
        first_gate_name="Test retained-handle and junction defenses",
        job_sha256="1207afe5e7582db13cc0f7ed950d72aa3bc1fc6b890c67039e4b728cc6146264",
        step_identities=(
            "name:Validate immutable Windows library SHA input",
            f"uses:{_pinned_action('actions/checkout')}",
            "name:Assert exact Windows library checkout",
            "name:Install Rust toolchain",
            "name:Provision checksum-pinned packaged FFmpeg sidecars",
            "name:Cache cargo",
            "name:Re-assert immutable Windows library source before gates",
            "name:Test retained-handle and junction defenses",
            "name:Test complete bundle publication and recovery",
            "name:Test Tauri project-library commit guards",
            "name:Clippy capability-backed library",
            "name:Re-assert immutable Windows library source after gates",
            "name:Build Windows library security JSON receipt",
            "name:Upload exact-SHA Windows library security receipt",
            "name:Enforce Windows library security aggregate",
        ),
    ),
    ShaBoundJobContract(
        job_name="safe-filesystem",
        bind_step_name="Assert exact checked-out SHA",
        before_reassert_name="Re-assert immutable target before native gates",
        after_reassert_name="Re-assert immutable target after native gates",
        first_gate_name="Run all native gates and retain every exit",
        job_sha256="cabf2d7d2c0b46f98151df30851436a4b1237e2027d979295d8671a08b3c3299",
        step_identities=(
            "name:Validate immutable SHA input",
            f"uses:{_pinned_action('actions/checkout')}",
            "name:Assert exact checked-out SHA",
            "name:Install Rust components",
            "name:Cache cargo",
            "name:Parse Windows expected-RED harness",
            "name:Re-assert immutable target before native gates",
            "name:Run all native gates and retain every exit",
            "name:Re-assert immutable target after native gates",
            "name:Build exclusive JSON receipt",
            "name:Upload immutable native receipt",
            "name:Enforce native aggregate",
        ),
    ),
    ShaBoundJobContract(
        job_name="windows-red-evidence",
        bind_step_name="Assert exact RED commit and parent",
        before_reassert_name="Re-assert immutable RED sources before focused gate",
        after_reassert_name="Re-assert immutable RED sources after focused gate",
        first_gate_name="Run focused expected-RED contract",
        job_sha256="dad4ce7b8b0b9ed7c0de6f353679192169e4c0e4647bba721f60bb65546f08ec",
        step_identities=(
            "name:Validate immutable RED inputs",
            "name:Checkout trusted RED dispatcher",
            "name:Assert trusted RED dispatcher",
            "name:Checkout RED target",
            "name:Assert exact RED commit and parent",
            "name:Re-assert immutable RED sources before focused gate",
            "name:Run focused expected-RED contract",
            "name:Re-assert immutable RED sources after focused gate",
            "name:Upload immutable Windows RED receipt",
        ),
    ),
)


def _as_mapping(value: object) -> dict[str, object] | None:
    return value if isinstance(value, dict) else None


def _as_steps(value: object) -> list[dict[str, object]] | None:
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        return None
    return value


def _structured_job(
    document: dict[str, object], name: str
) -> dict[str, object] | None:
    jobs = _as_mapping(document.get("jobs"))
    return _as_mapping(jobs.get(name)) if jobs is not None else None


def _structured_step(
    job: dict[str, object] | None, name: str
) -> dict[str, object] | None:
    if job is None:
        return None
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return None
    matches = [step for step in steps if step.get("name") == name]
    return matches[0] if len(matches) == 1 else None


def _step_position(job: dict[str, object] | None, name: str) -> int:
    if job is None:
        return -1
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return -1
    matches = [index for index, step in enumerate(steps) if step.get("name") == name]
    return matches[0] if len(matches) == 1 else -1


def _step_identity(step: dict[str, object]) -> str | None:
    name = step.get("name")
    if isinstance(name, str):
        return f"name:{name}"
    uses = step.get("uses")
    if isinstance(uses, str):
        return f"uses:{uses}"
    return None


def _structured_digest(value: dict[str, object]) -> str:
    encoded = json.dumps(
        value, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _run_script(step: dict[str, object] | None) -> str:
    if step is None:
        return ""
    script = step.get("run")
    return script if isinstance(script, str) else ""


def _code_lines(step: dict[str, object] | None) -> list[str]:
    return [
        line.strip()
        for line in _run_script(step).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


def _has_code_lines(
    step: dict[str, object] | None, expected: tuple[str, ...]
) -> bool:
    lines = set(_code_lines(step))
    return all(line in lines for line in expected)


def _with_mapping(step: dict[str, object] | None) -> dict[str, object] | None:
    return _as_mapping(step.get("with")) if step is not None else None


def _action_steps(
    job: dict[str, object] | None, action: str
) -> list[tuple[int, dict[str, object]]]:
    if job is None:
        return []
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return []
    return [
        (index, step)
        for index, step in enumerate(steps)
        if isinstance(step.get("uses"), str)
        and step["uses"].split("@", 1)[0] == action
    ]


def _collect_uses(
    document: dict[str, object],
) -> tuple[tuple[str, str, dict[str, object]], ...]:
    jobs = _as_mapping(document.get("jobs"))
    if jobs is None:
        raise ValueError("jobs must be a mapping")
    records: list[tuple[str, str, dict[str, object]]] = []
    for job_name, job_value in jobs.items():
        job = _as_mapping(job_value)
        if job is None:
            raise ValueError("job must be a mapping")
        if "uses" in job:
            uses = job.get("uses")
            with_values = job.get("with")
            if not isinstance(uses, str) or (
                with_values is not None and not isinstance(with_values, dict)
            ):
                raise ValueError("job uses contract has invalid types")
            records.append(
                (job_name, uses, {} if with_values is None else with_values)
            )
        steps = _as_steps(job.get("steps"))
        if steps is None:
            raise ValueError("steps must be a sequence of mappings")
        for step in steps:
            if "uses" not in step:
                continue
            uses = step.get("uses")
            with_values = step.get("with")
            if not isinstance(uses, str) or (
                with_values is not None and not isinstance(with_values, dict)
            ):
                raise ValueError("step uses contract has invalid types")
            records.append(
                (job_name, uses, {} if with_values is None else with_values)
            )
    return tuple(records)


_FORBIDDEN_SOURCE_MUTATION = re.compile(
    r"(?i)(?<![A-Za-z0-9_-])git(?:\.exe)?"
    r"(?:\s+(?:-[Cc]\s+\S+|--git-dir(?:=|\s+)\S+|--work-tree(?:=|\s+)\S+))*"
    r"\s+(checkout|switch|reset|restore|clean|read-tree)\b"
)


def _source_mutation_after_bind(
    job: dict[str, object], bind_step_name: str
) -> bool:
    steps = _as_steps(job.get("steps"))
    bind_position = _step_position(job, bind_step_name)
    if steps is None or bind_position < 0:
        return True
    return any(
        _FORBIDDEN_SOURCE_MUTATION.search(_run_script(step)) is not None
        for step in steps[bind_position + 1 :]
    )


_REASSERT_LINES = (
    "set -euo pipefail",
    "actual=\"$(git rev-parse HEAD | tr '[:upper:]' '[:lower:]')\"",
    "expected=\"$(printf '%s' \"$BOUND_SHA\" | tr '[:upper:]' '[:lower:]')\"",
    'test "$actual" = "$expected"',
    'git cat-file -e "${expected}^{commit}"',
    'test "$(git rev-parse \'HEAD^{tree}\')" = "$(git rev-parse "${expected}^{tree}")"',
    "git diff --cached --quiet --exit-code",
    "git diff --quiet --exit-code",
    'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
)

_UNTRACKED_STATUS_LINE = (
    'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
)
_RED_UNTRACKED_STATUS_LINES = (
    "if (@(git -C c1b-dispatcher status --porcelain=v1 --untracked-files=all).Count -ne 0) { throw 'RED dispatcher worktree is not clean' }",
    "if (@(git -C c1b-target status --porcelain=v1 --untracked-files=all).Count -ne 0) { throw 'RED target worktree is not clean' }",
)


def _valid_reassert_step(
    step: dict[str, object] | None, bound_output: str, *, always: bool
) -> bool:
    expected_keys = {"name", "shell", "env", "run"}
    if always:
        expected_keys.add("if")
    return bool(
        step is not None
        and set(step) == expected_keys
        and step.get("shell") == "bash"
        and step.get("env") == {"BOUND_SHA": bound_output}
        and (not always or step.get("if") == "always()")
        and _has_code_lines(step, _REASSERT_LINES)
    )


def _valid_red_reassert_step(
    step: dict[str, object] | None, *, always: bool
) -> bool:
    expected_keys = {"name", "shell", "run"}
    if always:
        expected_keys.add("if")
    script = _run_script(step)
    required = (
        "git -C c1b-dispatcher rev-parse HEAD",
        "git -C c1b-target rev-parse HEAD",
        "steps.bind-dispatcher.outputs.sha",
        "steps.bind-red.outputs.sha",
        "git -C c1b-dispatcher rev-parse 'HEAD^{tree}'",
        "git -C c1b-target rev-parse 'HEAD^{tree}'",
        "git -C c1b-dispatcher diff --cached --quiet --exit-code",
        "git -C c1b-dispatcher diff --quiet --exit-code",
        "git -C c1b-target diff --cached --quiet --exit-code",
        "git -C c1b-target diff --quiet --exit-code",
    )
    return bool(
        step is not None
        and set(step) == expected_keys
        and step.get("shell") == "pwsh"
        and (not always or step.get("if") == "always()")
        and all(fragment in script for fragment in required)
        and _has_code_lines(step, _RED_UNTRACKED_STATUS_LINES)
    )


def _validate_action_pins_document(document: dict[str, object]) -> list[str]:
    try:
        records = _collect_uses(document)
    except ValueError:
        return ["all external actions pinned"]
    seen: set[str] = set()
    for _job_name, uses, _with_values in records:
        match = re.fullmatch(
            r"([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)@([^\s#]+)", uses
        )
        if match is None:
            return ["all external actions pinned"]
        action, revision = match.groups()
        if (
            ACTION_PINS.get(action) != revision
            or re.fullmatch(r"[0-9a-f]{40}", revision) is None
        ):
            return ["all external actions pinned"]
        seen.add(action)
    return [] if seen == set(ACTION_PINS) else ["all external actions pinned"]


def _validate_checkout_actions_document(document: dict[str, object]) -> list[str]:
    try:
        records = _collect_uses(document)
    except ValueError:
        return ["checkout action counts"]
    actual: dict[str, list[dict[str, object]]] = {}
    for job_name, uses, with_values in records:
        if uses.split("@", 1)[0] == "actions/checkout":
            actual.setdefault(job_name, []).append(with_values)
    if set(actual) != set(EXPECTED_CHECKOUT_REFS):
        return ["checkout action counts"]
    for job_name, expected_refs in EXPECTED_CHECKOUT_REFS.items():
        checkouts = actual[job_name]
        if len(checkouts) != len(expected_refs):
            return ["checkout action counts"]
        refs = tuple(checkout.get("ref") for checkout in checkouts)
        if refs != expected_refs or any(ref == "main" for ref in refs):
            return ["checkout refs immutable"]
    return []


_BOUND_OUTPUTS = {
    "windows-product": "${{ steps.bind.outputs.sha }}",
    "windows-security": "${{ steps.bind-security.outputs.sha }}",
    "windows-library-security": "${{ steps.bind-library.outputs.sha }}",
    "safe-filesystem": "${{ steps.bind.outputs.sha }}",
}
_MUTATION_LABELS = {
    "windows-product": "product",
    "windows-security": "security",
    "windows-library-security": "library",
    "safe-filesystem": "safe-filesystem",
    "windows-red-evidence": "RED",
}


def _validate_sha_bound_job_contracts(
    document: dict[str, object],
) -> list[str]:
    errors: list[str] = []
    for contract in SHA_BOUND_JOB_CONTRACTS:
        job = _structured_job(document, contract.job_name)
        steps = _as_steps(job.get("steps")) if job is not None else None
        if job is None or steps is None:
            errors.append("exact SHA-bound Windows job step identities")
            errors.append("exact SHA-bound Windows job digests")
            continue
        identities = tuple(_step_identity(step) for step in steps)
        if identities != contract.step_identities:
            errors.append("exact SHA-bound Windows job step identities")
        if _structured_digest(job) != contract.job_sha256:
            errors.append("exact SHA-bound Windows job digests")

        bind_position = _step_position(job, contract.bind_step_name)
        before_position = _step_position(job, contract.before_reassert_name)
        first_gate_position = _step_position(job, contract.first_gate_name)
        after_position = _step_position(job, contract.after_reassert_name)
        if not (
            0 <= bind_position < before_position
            and before_position + 1 == first_gate_position
            and first_gate_position < after_position
        ):
            errors.append("SHA-bound Windows reassert order")

        before = _structured_step(job, contract.before_reassert_name)
        after = _structured_step(job, contract.after_reassert_name)
        if contract.job_name == "windows-red-evidence":
            reassertions_valid = _valid_red_reassert_step(
                before, always=False
            ) and _valid_red_reassert_step(after, always=True)
            untracked_guards_valid = _has_code_lines(
                before, _RED_UNTRACKED_STATUS_LINES
            ) and _has_code_lines(after, _RED_UNTRACKED_STATUS_LINES)
        else:
            bound_output = _BOUND_OUTPUTS[contract.job_name]
            reassertions_valid = _valid_reassert_step(
                before, bound_output, always=False
            ) and _valid_reassert_step(
                after,
                bound_output,
                always=contract.job_name != "windows-product",
            )
            guarded_steps = [before, after]
            if contract.job_name in {
                "windows-product",
                "windows-security",
                "windows-library-security",
            }:
                guarded_steps.append(_structured_step(job, contract.bind_step_name))
            untracked_guards_valid = all(
                _has_code_lines(step, (_UNTRACKED_STATUS_LINE,))
                for step in guarded_steps
            )
        if not reassertions_valid:
            errors.append("complete SHA-bound Windows source reassertions")
        if not untracked_guards_valid:
            errors.append("complete SHA-bound Windows untracked source guards")

        mutation_scan_bind = (
            "Assert trusted RED dispatcher"
            if contract.job_name == "windows-red-evidence"
            else contract.bind_step_name
        )
        if _source_mutation_after_bind(job, mutation_scan_bind):
            label = _MUTATION_LABELS[contract.job_name]
            errors.append(f"no source mutation after {label} SHA bind")
    return errors


def _validate_structured_specialist(
    document: dict[str, object], contract: JobContract
) -> list[str]:
    job = _structured_job(document, contract.job_name)
    if job is None:
        return [f"{contract.label} job"]
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return [f"{contract.label} job"]

    errors: list[str] = []
    prefix = contract.label
    if job.get("runs-on") != "windows-latest":
        errors.append(f"{prefix} native runner")
    if job.get("timeout-minutes") != 60:
        errors.append(f"{prefix} bounded runtime")
    if _as_mapping(job.get("env")) is None or job["env"].get(
        "TARGET_SHA"
    ) != TARGET_SHA_EXPRESSION.removeprefix("TARGET_SHA: "):
        errors.append(f"{prefix} exact target SHA")
    if job.get("continue-on-error") is True or any(
        step.get("continue-on-error") is True for step in steps
    ):
        errors.append(f"{prefix} no ignored failures")

    input_step = _structured_step(job, contract.input_step_name)
    if not _has_code_lines(
        input_step, ('[[ "$TARGET_SHA" =~ ^[0-9a-fA-F]{40}$ ]]',)
    ):
        errors.append(f"{prefix} immutable SHA validation")

    checkouts = _action_steps(job, "actions/checkout")
    checkout = checkouts[0][1] if len(checkouts) == 1 else None
    if (
        checkout is None
        or checkout.get("uses") != _pinned_action("actions/checkout")
        or _with_mapping(checkout)
        != {
            "ref": "${{ env.TARGET_SHA }}",
            "fetch-depth": 0,
            "persist-credentials": False,
        }
    ):
        errors.append(f"{prefix} exact-SHA checkout")

    bind = _structured_step(job, contract.bind_step_name)
    bind_lines = (
        'actual="$(git rev-parse HEAD | tr \'[:upper:]\' \'[:lower:]\')"',
        'expected="$(printf \'%s\' "$TARGET_SHA" | tr \'[:upper:]\' \'[:lower:]\')"',
        'test "$actual" = "$expected"',
        'git cat-file -e "${expected}^{commit}"',
        'test "$(git rev-parse \'HEAD^{tree}\')" = "$(git rev-parse "${expected}^{tree}")"',
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
        'printf \'sha=%s\\n\' "$actual" >> "$GITHUB_OUTPUT"',
    )
    if (
        bind is None
        or bind.get("id") != contract.bind_id
        or bind.get("shell") != "bash"
        or not _has_code_lines(bind, bind_lines)
    ):
        errors.append(f"{prefix} immutable checkout assertion")

    gate_steps: list[dict[str, object]] = []
    gates_exact = True
    for gate in contract.gates:
        gate_step = _structured_step(job, gate.name)
        if gate_step is None:
            gates_exact = False
            continue
        gate_steps.append(gate_step)
        if (
            gate_step.get("id") != gate.step_id
            or gate_step.get("shell") != gate.shell
            or "if" in gate_step
            or _structured_digest(gate_step) != gate.step_sha256
        ):
            gates_exact = False
    if len(gate_steps) != len(contract.gates) or not gates_exact:
        errors.append(f"exact {prefix} gate contracts")

    receipt = _structured_step(job, contract.receipt_step_name)
    receipt_script = _run_script(receipt)
    receipt_fragments = (
        f"schema = '{contract.receipt_schema}'",
        "repository = '${{ github.repository }}'",
        "run_id = '${{ github.run_id }}'",
        "run_attempt = '${{ github.run_attempt }}'",
        "job = '${{ github.job }}'",
        "requested_sha = $env:TARGET_SHA.ToLowerInvariant()",
        "checked_out_sha = $env:RECEIPT_SHA.ToLowerInvariant()",
        "runner_os = '${{ runner.os }}'",
        "runner_arch = '${{ runner.arch }}'",
        "commands = @($commands)",
        "aggregate_exit = $aggregate",
        f"Set-Content -Encoding utf8NoBOM '{contract.receipt_path}'",
    )
    receipt_env = _as_mapping(receipt.get("env")) if receipt is not None else None
    if (
        receipt is None
        or receipt.get("if") != "always()"
        or receipt_env
        != {
            "RECEIPT_SHA": f"${{{{ steps.{contract.bind_id}.outputs.sha }}}}"
        }
        or not all(fragment in receipt_script for fragment in receipt_fragments)
    ):
        errors.append(f"{prefix} SHA-bound receipt")
    if receipt is None or _structured_digest(receipt) != contract.receipt_step_sha256:
        errors.append(f"exact {prefix} receipt build contract")
    if receipt is None or receipt.get("if") != "always()":
        errors.append(f"{prefix} receipt always runs")

    expected_receipt_rows = [
        (gate.receipt_id, gate.receipt_command, gate.step_id)
        for gate in contract.gates
    ]
    actual_receipt_rows = re.findall(
        r"\[ordered\]@\{\s*"
        r"id = '([^']+)'\s*"
        r"command = '([^']+)'\s*"
        r"result = '\$\{\{ steps\.([a-z0-9-]+)\.outcome \}\}'\s*"
        r"\}",
        receipt_script,
        flags=re.DOTALL,
    )
    if actual_receipt_rows != expected_receipt_rows:
        errors.append(f"{prefix} receipt gate binding")

    upload = _structured_step(job, contract.upload_step_name)
    upload_with = _with_mapping(upload)
    upload_path = upload_with.get("path") if upload_with is not None else None
    if (
        upload is None
        or upload.get("if") != "always()"
        or upload.get("uses") != _pinned_action("actions/upload-artifact")
        or upload_with is None
        or upload_with.get("name") != contract.artifact_name
        or not isinstance(upload_path, str)
        or contract.receipt_path not in upload_path.splitlines()
        or upload_with.get("if-no-files-found") != "error"
    ):
        errors.append(f"{prefix} SHA-bound receipt artifact")

    enforce = _structured_step(job, contract.enforce_step_name)
    enforce_script = _run_script(enforce)
    enforce_fragments = (
        contract.receipt_path,
        "Test-Path $path -PathType Leaf",
        f"steps.{contract.bind_id}.outputs.sha",
        "aggregate_exit",
        "result -ne 'success'",
    )
    if (
        enforce is None
        or enforce.get("if") != "always()"
        or not all(fragment in enforce_script for fragment in enforce_fragments)
    ):
        errors.append(f"{prefix} aggregate enforcement")
    if enforce is None or _structured_digest(enforce) != contract.enforce_step_sha256:
        errors.append(f"exact {prefix} receipt enforce contract")

    ordered_names = [
        contract.input_step_name,
        contract.bind_step_name,
        f"Re-assert immutable Windows {prefix} source before gates",
        *(gate.name for gate in contract.gates),
        f"Re-assert immutable Windows {prefix} source after gates",
        contract.receipt_step_name,
        contract.upload_step_name,
        contract.enforce_step_name,
    ]
    positions = [_step_position(job, name) for name in ordered_names]
    checkout_position = checkouts[0][0] if len(checkouts) == 1 else -1
    positions.insert(1, checkout_position)
    if any(position < 0 for position in positions) or positions != sorted(positions):
        errors.append(f"{prefix} gate order")
    return errors


def _validate_structured_product(document: dict[str, object]) -> list[str]:
    job = _structured_job(document, "windows-product")
    if job is None:
        return ["windows-product job"]
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return ["windows-product job"]
    errors: list[str] = []

    if job.get("runs-on") != "windows-2022":
        errors.append("pinned native runner")
    if job.get("if") != NORMAL_JOB_CONDITION.removeprefix("if: "):
        errors.append("product job condition")
    if job.get("timeout-minutes") != 120:
        errors.append("bounded runtime")
    env = _as_mapping(job.get("env"))
    if env is None or env.get("TARGET_SHA") != TARGET_SHA_EXPRESSION.removeprefix(
        "TARGET_SHA: "
    ):
        errors.append("exact target SHA")
    if job.get("continue-on-error") is True or any(
        step.get("continue-on-error") is True for step in steps
    ):
        errors.append("no ignored failures")
    if any("if" in step for step in steps):
        errors.append("required steps are unconditional")

    checkouts = _action_steps(job, "actions/checkout")
    checkout = checkouts[0][1] if len(checkouts) == 1 else None
    if (
        checkout is None
        or checkout.get("uses") != _pinned_action("actions/checkout")
        or _with_mapping(checkout)
        != {
            "ref": "${{ env.TARGET_SHA }}",
            "fetch-depth": 0,
            "persist-credentials": False,
        }
    ):
        errors.append("exact-SHA checkout")

    assert_step = _structured_step(job, "Assert exact checked-out SHA")
    assert_lines = (
        'actual="$(git rev-parse HEAD | tr \'[:upper:]\' \'[:lower:]\')"',
        'expected="$(printf \'%s\' "$TARGET_SHA" | tr \'[:upper:]\' \'[:lower:]\')"',
        'test "$actual" = "$expected"',
        'git cat-file -e "${expected}^{commit}"',
        'test "$(git rev-parse \'HEAD^{tree}\')" = "$(git rev-parse "${expected}^{tree}")"',
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
        'printf \'sha=%s\\n\' "$actual" >> "$GITHUB_OUTPUT"',
    )
    if not _has_code_lines(assert_step, assert_lines):
        errors.append("immutable checkout assertion")
    if assert_step is None or assert_step.get("id") != "bind":
        errors.append("checkout assertion publishes bind output")

    cache = _structured_step(job, "Cache cargo")
    cache_with = _with_mapping(cache)
    cache_path = cache_with.get("path") if cache_with is not None else None
    if (
        cache is None
        or cache.get("uses") != _pinned_action("actions/cache")
        or not isinstance(cache_path, str)
        or not {"~/.cargo/registry", "~/.cargo/git"}.issubset(
            set(cache_path.splitlines())
        )
    ):
        errors.append("cargo dependency cache")
    if not isinstance(cache_path, str) or any(
        line.strip() == "target"
        or line.strip().startswith(("target/", "target\\"))
        for line in cache_path.splitlines()
    ):
        errors.append("product cache excludes target")

    required_commands = (
        (
            "Install locked Web dependencies",
            "pnpm -C web install --frozen-lockfile",
            "locked web dependencies",
        ),
        ("Rust formatting", "cargo fmt --all --check", "Rust formatting command"),
        (
            "Rust workspace clippy",
            "cargo clippy --workspace --all-targets -- -D warnings",
            "Rust workspace clippy command",
        ),
        ("Web editor behavior suite", "pnpm -C web test", "Web editor tests command"),
        (
            "Rust workspace tests",
            "cargo test --workspace -- --test-threads=1",
            "Rust workspace tests command",
        ),
        (
            "Minimal-feature Tauri clippy",
            "cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings",
            "minimal-feature clippy command",
        ),
        ("Web production build", "pnpm -C web build", "Web production build command"),
    )
    for step_name, command, label in required_commands:
        if _run_script(_structured_step(job, step_name)) != command:
            errors.append(label)

    chromium_script = _run_script(
        _structured_step(job, "Windows Chromium motion capture regression")
    )
    if not all(
        fragment in chromium_script
        for fragment in (
            "renderer::tests::chromium_skeleton_reports_unavailable_not_panic",
            "virtual_time_network_csp_timeout_cleanup_and_frame_identity",
            "sandbox_progress_cancel_validated_mp4_result",
        )
    ):
        errors.append("complete Windows Chromium motion regression")

    bundle = _structured_step(job, "Build native MSI and NSIS installers")
    if not _has_code_lines(
        bundle,
        (
            "Remove-Item 'target/release/bundle/msi' -Recurse -Force "
            "-ErrorAction SilentlyContinue",
            "Remove-Item 'target/release/bundle/nsis' -Recurse -Force "
            "-ErrorAction SilentlyContinue",
            "& .\\web\\node_modules\\.bin\\tauri.cmd build --ci --bundles msi,nsis",
        ),
    ):
        errors.append("clean native Tauri bundle")

    installed_script = _run_script(
        _structured_step(
            job, "Install NSIS package and execute installed product without PATH"
        )
    )
    if not all(
        fragment in installed_script
        for fragment in (
            "packaged_macos_windows_sidecars_resolve_and_execute",
            "Start-Process -FilePath $application -PassThru",
            "installed OpenTake exited during launch smoke test",
        )
    ):
        errors.append("installed product launch smoke test")

    receipt = _structured_step(job, "Bind installers to the exact source SHA")
    receipt_env = _as_mapping(receipt.get("env")) if receipt is not None else None
    receipt_script = _run_script(receipt)
    if receipt_env is None or receipt_env.get("RECEIPT_SHA") != (
        "${{ steps.bind.outputs.sha }}"
    ):
        errors.append("receipt consumes bind output")
    if "source_sha = $env:RECEIPT_SHA" not in receipt_script:
        errors.append("receipt binds source SHA")
    if not any(
        line.startswith("sha256 = (Get-FileHash") for line in _code_lines(receipt)
    ):
        errors.append("receipt records SHA-256")
    if not all(
        fragment in receipt_script
        for fragment in (
            "target/release/bundle/msi/*.msi",
            "target/release/bundle/nsis/*.exe",
            "Tauri did not produce an MSI installer",
            "Tauri did not produce an NSIS installer",
        )
    ):
        errors.append("receipt requires both installers")
    receipt_write = (
        "$receipt | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8NoBOM "
        "windows-product-receipt.json"
    )
    if receipt_write not in _code_lines(receipt):
        errors.append("receipt file is written")

    upload = _structured_step(job, "Upload exact-SHA Windows installers")
    upload_with = _with_mapping(upload)
    upload_path = upload_with.get("path") if upload_with is not None else None
    if upload is None or upload.get("uses") != _pinned_action(
        "actions/upload-artifact"
    ):
        errors.append("bundle artifact upload")
    if not isinstance(upload_path, str) or (
        "target/release/bundle/msi/*.msi" not in upload_path.splitlines()
    ):
        errors.append("upload includes MSI")
    if not isinstance(upload_path, str) or (
        "target/release/bundle/nsis/*.exe" not in upload_path.splitlines()
    ):
        errors.append("upload includes NSIS")
    if not isinstance(upload_path, str) or (
        "windows-product-receipt.json" not in upload_path.splitlines()
    ):
        errors.append("upload includes receipt")
    if upload_with is None or upload_with.get("if-no-files-found") != "error":
        errors.append("missing-artifact failure")
    return errors


def validate_workflow(workflow: str) -> list[str]:
    try:
        document = parse_workflow_yaml(workflow)
    except WorkflowYamlError:
        return ["valid unique-key YAML 1.2 workflow"]

    errors = _validate_action_pins_document(document)
    errors.extend(_validate_checkout_actions_document(document))
    errors.extend(_validate_sha_bound_job_contracts(document))
    errors.extend(_validate_structured_product(document))
    for specialist_contract in SPECIALIST_CONTRACTS:
        errors.extend(_validate_structured_specialist(document, specialist_contract))
    return list(dict.fromkeys(errors))


def validate_tauri_config(config_text: str) -> list[str]:
    try:
        config = json.loads(config_text)
    except json.JSONDecodeError:
        return ["valid Tauri JSON config"]

    app_version = config.get("version")
    wix_version = (
        config.get("bundle", {}).get("windows", {}).get("wix", {}).get("version")
    )
    errors: list[str] = []
    if not isinstance(app_version, str) or not app_version:
        errors.append("public Beta app version")
    if not isinstance(wix_version, str) or not re.fullmatch(
        r"\d+\.\d+\.\d+(?:\.\d+)?", wix_version
    ):
        errors.append("MSI-compatible numeric Beta version")
        return errors

    wix_fields = [int(field) for field in wix_version.split(".")]
    if (
        wix_fields[0] > 255
        or wix_fields[1] > 255
        or any(field > 65_535 for field in wix_fields[2:])
    ):
        errors.append("MSI-compatible numeric Beta version")
    if isinstance(app_version, str):
        app_core = app_version.split("+", 1)[0].split("-", 1)[0]
        if app_core != ".".join(str(field) for field in wix_fields[:3]):
            errors.append("MSI version tracks public app version")
    return errors


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--workflow",
        type=Path,
        default=WORKFLOW_PATH,
        help="workflow path to validate (defaults to the repository CI workflow)",
    )
    parser.add_argument(
        "--workflow-only",
        action="store_true",
        help="skip the repository Tauri configuration check",
    )
    parser.add_argument(
        "--verify-action-pins-online",
        action="store_true",
        help="verify every pinned SHA through its official GitHub repository REST endpoint",
    )
    args = parser.parse_args()
    if args.verify_action_pins_online:
        errors = validate_action_pin_ownership(_resolve_github_commit)
        if errors:
            raise SystemExit("action pin ownership failed: " + ", ".join(errors))
        print("External action pin ownership is REST-verified")
        return

    workflow = args.workflow.read_text(encoding="utf-8")
    errors = validate_workflow(workflow)
    if not args.workflow_only:
        errors.extend(
            validate_tauri_config(TAURI_CONFIG_PATH.read_text(encoding="utf-8"))
        )
    if errors:
        raise SystemExit("windows-product is missing: " + ", ".join(errors))

    print("Windows product CI contract is complete")


if __name__ == "__main__":
    main()
