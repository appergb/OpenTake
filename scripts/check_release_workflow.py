#!/usr/bin/env python3
"""Fail-closed contract checks for the tag-driven GitHub release workflow."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tomllib

from workflow_yaml import WorkflowYamlError, parse_workflow_yaml as _parse_workflow_yaml


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "release.yml"
PINNED_ACTIONS = {
    "actions/checkout": "11d5960a326750d5838078e36cf38b85af677262",
    "actions/setup-node": "49933ea5288caeca8642d1e84afbd3f7d6820020",
    "actions/cache": "0057852bfaa89a56745cba8c7296529d2fc39830",
    "actions/upload-artifact": "ea165f8d65b6e75b540449e92b4886f43607fa02",
    "actions/download-artifact": "d3f86a106a0bac45b974a628896c90dbdf5c8093",
    "pnpm/action-setup": "b906affcce14559ad1aafd4ab0e942779e9f58b1",
    "ruby/setup-ruby": "95ef2b042f9d7a56d8268cba8559e2842e2ad01b",
}

APPROVED_STEP_IDENTITIES = {
    "validate": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Validate tag, source SHA, versions, and notes",
        "name:Reassert exact source after validation",
    ),
    "quality": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Free disk space",
        "name:Install Rust toolchain",
        "name:Install system deps (ffmpeg + Tauri/GTK)",
        f"uses:pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        f"uses:actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        f"uses:ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}",
        "name:Cache Cargo dependencies",
        "name:Install locked Motion Canvas dependencies",
        "name:Audit Motion Canvas dependencies and licenses",
        "name:Test and reproduce Motion Canvas runner",
        "name:Validate Windows and release workflow contracts",
        "name:Provisioner unit tests",
        "name:Install locked Web dependencies",
        "name:Rust formatting",
        "name:Rust workspace clippy",
        "name:Rust workspace tests",
        "name:Live playback transport integration",
        "name:Minimal-feature Tauri clippy",
        "name:Web editor behavior suite",
        "name:Web production build",
        "name:Reassert exact source after quality gates",
    ),
    "macos_arm64": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Install Rust toolchain",
        f"uses:pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        f"uses:actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        f"uses:ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}",
        "name:Provision checksum-pinned ARM64 FFmpeg sidecars",
        "name:Verify pinned sidecar supply",
        "name:Install locked Web dependencies",
        "name:Reassert exact source before macOS build",
        "name:Build ad-hoc Tauri app and DMG",
        "name:Verify complete app, sidecars, and DMG",
        "name:Reassert exact source after macOS packaging",
        "name:Create macOS exact-SHA receipt",
        "name:Upload exact-SHA macOS package",
    ),
    "windows_x64": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Install Rust toolchain",
        f"uses:pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        f"uses:actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        f"uses:ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}",
        "name:Provision checksum-pinned Windows FFmpeg sidecars",
        "name:Verify pinned sidecar supply",
        "name:Cache Cargo dependencies",
        "name:Install locked Web dependencies",
        "name:Rust workspace clippy",
        "name:Rust workspace tests",
        "name:Web editor behavior suite",
        "name:Minimal-feature Tauri clippy",
        "name:Web production build",
        "name:Reassert exact source before Windows build",
        "name:Build native MSI and NSIS installers",
        "name:Install NSIS and smoke installed app and sidecars",
        "name:Reassert exact source after Windows packaging",
        "name:Create Windows exact-SHA receipt",
        "name:Upload exact-SHA Windows packages",
    ),
    "publish": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Initialize isolated publish root",
        "name:Download macOS artifact",
        "name:Download Windows artifact",
        "name:Stage and verify the exact release payload",
        "name:Create and verify SHA256SUMS",
        "name:Prepare release notes with provenance",
        "name:Reassert exact source before draft mutation",
        "name:Revalidate remote tag before draft mutation",
        "name:Create or refresh draft prerelease",
        "name:Upload the exact payload to the draft",
        "name:Verify draft target and exact assets",
        "name:Revalidate remote tag before publication",
        "name:Reassert exact source before publication",
        "name:Publish verified prerelease",
        "name:Verify public release through API and checksums",
    ),
}

APPROVED_SIMPLE_RUNS = {
    ("quality", "Install Rust toolchain"): "rustup component add rustfmt clippy",
    ("quality", "Install locked Motion Canvas dependencies"): "npm --prefix plugins/motion-canvas-studio ci --ignore-scripts",
    ("quality", "Provisioner unit tests"): "python3 -B -m unittest discover -s scripts/tests -p 'test_*.py'",
    ("quality", "Install locked Web dependencies"): "pnpm -C web install --frozen-lockfile",
    ("quality", "Rust formatting"): "cargo fmt --all --check",
    ("quality", "Rust workspace clippy"): "cargo clippy --workspace --all-targets -- -D warnings",
    ("quality", "Rust workspace tests"): "cargo test --workspace -- --test-threads=1",
    ("quality", "Minimal-feature Tauri clippy"): "cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings",
    ("quality", "Web editor behavior suite"): "pnpm -C web test",
    ("quality", "Web production build"): "pnpm -C web build",
    ("macos_arm64", "Install Rust toolchain"): "rustup component add rustfmt clippy",
    ("macos_arm64", "Provision checksum-pinned ARM64 FFmpeg sidecars"): "python3 scripts/provision_ffmpeg_sidecars.py --target aarch64-apple-darwin",
    ("macos_arm64", "Verify pinned sidecar supply"): "ruby scripts/tests/packaged-sidecars-test.rb --name packaged_macos_windows_sidecars_resolve_and_execute",
    ("macos_arm64", "Install locked Web dependencies"): "pnpm -C web install --frozen-lockfile",
    ("macos_arm64", "Build ad-hoc Tauri app and DMG"): "./web/node_modules/.bin/tauri build --ci --target aarch64-apple-darwin --bundles app,dmg --config '{\"bundle\":{\"macOS\":{\"signingIdentity\":\"-\"}}}'",
    ("windows_x64", "Install Rust toolchain"): "rustup component add rustfmt clippy",
    ("windows_x64", "Provision checksum-pinned Windows FFmpeg sidecars"): "python scripts/provision_ffmpeg_sidecars.py --target x86_64-pc-windows-msvc",
    ("windows_x64", "Verify pinned sidecar supply"): "ruby scripts/tests/packaged-sidecars-test.rb --name packaged_macos_windows_sidecars_resolve_and_execute",
    ("windows_x64", "Install locked Web dependencies"): "pnpm -C web install --frozen-lockfile",
    ("windows_x64", "Rust workspace clippy"): "cargo clippy --workspace --all-targets -- -D warnings",
    ("windows_x64", "Rust workspace tests"): "cargo test --workspace -- --test-threads=1",
    ("windows_x64", "Web editor behavior suite"): "pnpm -C web test",
    ("windows_x64", "Minimal-feature Tauri clippy"): "cargo clippy -p opentake-tauri --no-default-features --all-targets -- -D warnings",
    ("windows_x64", "Web production build"): "pnpm -C web build",
    ("publish", "Upload the exact payload to the draft"): 'gh release upload "$RELEASE_TAG" "$PUBLISH_ROOT/assets/"* --clobber',
    ("publish", "Publish verified prerelease"): 'gh release edit "$RELEASE_TAG" --draft=false --prerelease --latest=false',
}

APPROVED_COMPLEX_RUN_SHA256 = {
    ("validate", "Validate tag, source SHA, versions, and notes"): "120a1dc8347fb5989f607f6965b1f9ce5167e934a41631d1ac0cee01e0b83876",
    ("validate", "Reassert exact source after validation"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("quality", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("quality", "Free disk space"): "5848415c4d0e696f46965d62a2e17c8b7a0dd45ae600d28102af0b04108d9bf6",
    ("quality", "Install system deps (ffmpeg + Tauri/GTK)"): "ee466d2d3fff1c3703d50f9dabe4d21e1cee4b399924d064c6d2714dae34d16b",
    ("quality", "Audit Motion Canvas dependencies and licenses"): "a3517fae1a8663e519138196c9f3721d8f4df19ac8f115c49a079c4aaa60c8b3",
    ("quality", "Test and reproduce Motion Canvas runner"): "8bcd55de9b045f9d7be6343163a5422cba0ab545f7844da50ca1a7c8623fe640",
    ("quality", "Validate Windows and release workflow contracts"): "45c70d24aae54d1409f0594a65256d7c1397459d97d1fdacfc74d9f8d6f4105f",
    ("quality", "Live playback transport integration"): "461f79546009551e5e7adbf50f869abb9449c2ae7666a66a425c7cd3c24acea9",
    ("quality", "Reassert exact source after quality gates"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("macos_arm64", "Reassert exact source before macOS build"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Verify complete app, sidecars, and DMG"): "4122545856cbbcdec2d7f835c005007d723ffb2432ee18bb1173acc10453123b",
    ("macos_arm64", "Reassert exact source after macOS packaging"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Create macOS exact-SHA receipt"): "67d51abe727feb1d2593927c70e68d9c5c2f64a7db42ab1907dc3e40a4e44ab7",
    ("windows_x64", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("windows_x64", "Reassert exact source before Windows build"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("windows_x64", "Build native MSI and NSIS installers"): "3c814b49684a64df810c052dbf021890df348967d5767bb453800fc36518c6ce",
    ("windows_x64", "Install NSIS and smoke installed app and sidecars"): "3e3909b29d79f338edbe186daf42b341618feb720326eacfac6ea068bf3e8c42",
    ("windows_x64", "Reassert exact source after Windows packaging"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("windows_x64", "Create Windows exact-SHA receipt"): "e80a5bf08f311dfb353c19c4ada500726ed9c5d8e4f817c513d12e791b19f558",
    ("publish", "Assert exact checked-out SHA"): "b1cd768e31e2924c14421c62357ae200ddee50b2a6c2ddc24618717a3c876267",
    ("publish", "Initialize isolated publish root"): "ab0b76ab253b0497067e6d8650a0fd6b37fb3a0ac72b2ff11d1738a88b46c2dd",
    ("publish", "Stage and verify the exact release payload"): "69097e2a084dee0d4a0eb313be47e545b7a8ae59abfc9e52dbe0c1eccb659d11",
    ("publish", "Create and verify SHA256SUMS"): "c205b22264de125ac603558c06b8a93aff726f6eddd8cda07ca7df0862641705",
    ("publish", "Prepare release notes with provenance"): "e2761070b29f27142a750829e0aa831577a7016690a1012d970637983b54ecbc",
    ("publish", "Reassert exact source before draft mutation"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("publish", "Revalidate remote tag before draft mutation"): "2eac9a1203d96969b545c9447b9637c8ad16689d2704b32c7e710e9ae4bff47c",
    ("publish", "Create or refresh draft prerelease"): "e3cee76806b604359715ec91cf1a7c6d7c8919e05dd5b5c99f77b98686debc6b",
    ("publish", "Verify draft target and exact assets"): "440e618009c30184e66a02d4f695bc25af708b09558ec25471e3c52bde417764",
    ("publish", "Revalidate remote tag before publication"): "89cd8010bf65a4a3d7b85b2666e13eeddc4554e24e53eb700592bd6dc77cea6f",
    ("publish", "Reassert exact source before publication"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("publish", "Verify public release through API and checksums"): "bdd6671280f89cfe71fad7c0eb9fecb24a6f66931910ee5075c5c2e9e96f52fe",
}

APPROVED_JOB_SHA256 = {
    "validate": "386c4e21bdcc884a2246eea1d078b12378b25bfef52043e5e9520a2845f51142",
    "quality": "43599825f68088f3cc744220227267f77454e2c7ce389a52b8aee5a492491eec",
    "macos_arm64": "f913508e4f041614a967f680515cc0b8779800f0b2a7b4b6ff29f24f4bb1efd2",
    "windows_x64": "5f012278c20843ba1b38cffb27f3c614716bd92e3fec63a11885b41692be78b5",
    "publish": "89192ac021d48cc8c8ef17b612a2da666d96c01d195c4e2b58eec24b2cc24d1b",
}


class ReleaseStateError(ValueError):
    """The remote release state is unsafe to create, refresh, or publish."""


class RemoteTagError(ValueError):
    """The remote tag advertisement is missing, ambiguous, or malformed."""


def resolve_remote_tag_refs(refs_text: str, expected_tag: str) -> str:
    """Resolve a lightweight or annotated ls-remote tag to its commit SHA."""
    direct_ref = f"refs/tags/{expected_tag}"
    peeled_ref = f"{direct_ref}^{{}}"
    refs: dict[str, str] = {}
    lines = [line for line in refs_text.splitlines() if line]
    if not 1 <= len(lines) <= 2:
        raise RemoteTagError("remote tag lookup must return one or two refs")
    for line in lines:
        fields = line.split()
        if len(fields) != 2:
            raise RemoteTagError("remote tag lookup line is malformed")
        sha, ref = fields
        if ref not in {direct_ref, peeled_ref} or ref in refs:
            raise RemoteTagError("remote tag lookup returned an unexpected ref")
        if re.fullmatch(r"[0-9a-f]{40}", sha) is None:
            raise RemoteTagError("remote tag lookup returned a non-commit object ID")
        refs[ref] = sha
    if direct_ref not in refs:
        raise RemoteTagError("remote tag lookup omitted the direct tag ref")
    return refs.get(peeled_ref, refs[direct_ref])


def resolve_release_state(
    payload: dict[str, object], expected_tag: str, expected_sha: str
) -> dict[str, object]:
    """Normalize one GraphQL release lookup into a side-effect-free draft plan."""
    if not re.fullmatch(r"[0-9a-f]{40}", expected_sha):
        raise ReleaseStateError("expected source SHA must be lowercase 40-hex")
    errors = payload.get("errors")
    if errors:
        raise ReleaseStateError("GraphQL release lookup returned errors")
    data = payload.get("data")
    if not isinstance(data, dict):
        raise ReleaseStateError("GraphQL release lookup has no data object")
    repository = data.get("repository")
    if not isinstance(repository, dict):
        raise ReleaseStateError("GraphQL release lookup has no repository")
    release = repository.get("release")
    if release is None:
        return {
            "action": "create",
            "release_id": None,
            "asset_node_ids": [],
            "asset_names": [],
            "asset_sizes": [],
        }
    if not isinstance(release, dict):
        raise ReleaseStateError("GraphQL release result is malformed")
    if release.get("tagName") != expected_tag:
        raise ReleaseStateError("GraphQL release tag does not match requested tag")
    tag_commit = release.get("tagCommit")
    target = tag_commit.get("oid") if isinstance(tag_commit, dict) else None
    if not isinstance(target, str) or target.lower() != expected_sha:
        raise ReleaseStateError("GraphQL release target does not match source SHA")
    if release.get("isDraft") is not True:
        raise ReleaseStateError(
            "release is already published; refusing to mutate it"
        )
    if release.get("isPrerelease") is not True:
        raise ReleaseStateError("same-tag draft is not marked as a prerelease")

    release_id = release.get("databaseId")
    if not isinstance(release_id, int) or isinstance(release_id, bool):
        raise ReleaseStateError("GraphQL draft release has no numeric database ID")
    connection = release.get("releaseAssets")
    if not isinstance(connection, dict):
        raise ReleaseStateError("GraphQL draft release has no asset connection")
    page_info = connection.get("pageInfo")
    if not isinstance(page_info, dict) or page_info.get("hasNextPage") is not False:
        raise ReleaseStateError("GraphQL draft asset list is incomplete")
    nodes = connection.get("nodes")
    if not isinstance(nodes, list):
        raise ReleaseStateError("GraphQL draft asset list is malformed")

    asset_node_ids: list[str] = []
    asset_names: list[str] = []
    asset_sizes: list[int] = []
    for asset in nodes:
        if not isinstance(asset, dict):
            raise ReleaseStateError("GraphQL draft asset entry is malformed")
        asset_id = asset.get("id")
        name = asset.get("name")
        size = asset.get("size")
        if not isinstance(asset_id, str) or not asset_id:
            raise ReleaseStateError("GraphQL draft asset has no node ID")
        if not isinstance(name, str) or not name:
            raise ReleaseStateError("GraphQL draft asset has no name")
        if not isinstance(size, int) or isinstance(size, bool) or size < 0:
            raise ReleaseStateError("GraphQL draft asset has an invalid size")
        asset_node_ids.append(asset_id)
        asset_names.append(name)
        asset_sizes.append(size)
    return {
        "action": "refresh",
        "release_id": release_id,
        "asset_node_ids": asset_node_ids,
        "asset_names": asset_names,
        "asset_sizes": asset_sizes,
    }


def _as_mapping(value: object) -> dict[str, object] | None:
    return value if isinstance(value, dict) else None


def _as_steps(value: object) -> list[dict[str, object]] | None:
    if not isinstance(value, list) or not all(isinstance(item, dict) for item in value):
        return None
    return value


def _structured_job(document: dict[str, object], name: str) -> dict[str, object] | None:
    jobs = _as_mapping(document.get("jobs"))
    return _as_mapping(jobs.get(name)) if jobs is not None else None


def _structured_step(job: dict[str, object] | None, name: str) -> dict[str, object] | None:
    if job is None:
        return None
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return None
    matches = [step for step in steps if step.get("name") == name]
    return matches[0] if len(matches) == 1 else None


def _run_script(step: dict[str, object] | None) -> str:
    if step is None:
        return ""
    run = step.get("run")
    return run if isinstance(run, str) else ""


def _logical_lines(script: str, continuation: str = "\\") -> list[str]:
    result: list[str] = []
    pending = ""
    heredoc_delimiter: str | None = None
    for raw_line in script.splitlines():
        line = raw_line.strip()
        if heredoc_delimiter is not None:
            if line == heredoc_delimiter:
                heredoc_delimiter = None
            continue
        if not line or line.startswith("#"):
            continue
        if pending:
            line = pending + " " + line
            pending = ""
        if line.endswith(continuation):
            pending = line[: -len(continuation)].rstrip()
            continue
        result.append(line)
        heredoc = re.search(
            r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1", line
        )
        if heredoc is not None:
            heredoc_delimiter = heredoc.group(2)
    if pending:
        result.append(pending)
    return result


def _commands(script: str, *, powershell: bool = False) -> list[tuple[str, ...]]:
    commands: list[tuple[str, ...]] = []
    for line in _logical_lines(script, "`" if powershell else "\\"):
        try:
            tokens = shlex.split(line, comments=True, posix=not powershell)
        except ValueError:
            continue
        if tokens:
            commands.append(tuple(tokens))
    return commands


def _has_command(
    step: dict[str, object] | None,
    expected: tuple[str, ...],
    *,
    powershell: bool = False,
) -> bool:
    return expected in _commands(_run_script(step), powershell=powershell)


def _code_lines(step: dict[str, object] | None) -> list[str]:
    return [
        line.strip()
        for line in _run_script(step).splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


def _has_code_lines(step: dict[str, object] | None, expected: tuple[str, ...]) -> bool:
    lines = set(_code_lines(step))
    return all(line in lines for line in expected)


def _with_mapping(step: dict[str, object] | None) -> dict[str, object] | None:
    return _as_mapping(step.get("with")) if step is not None else None


def _step_identity(step: dict[str, object]) -> str | None:
    name = step.get("name")
    if isinstance(name, str):
        return f"name:{name}"
    uses = step.get("uses")
    if isinstance(uses, str):
        return f"uses:{uses}"
    return None


def _normalized_run_digest(script: str) -> str:
    normalized = "\n".join(
        line.rstrip()
        for line in script.replace("\r\n", "\n").replace("\r", "\n").splitlines()
    ).strip("\n") + "\n"
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()


def _structured_job_digest(job: dict[str, object]) -> str:
    encoded = json.dumps(
        job, ensure_ascii=False, separators=(",", ":"), sort_keys=True
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _git_subcommand(command: tuple[str, ...]) -> str | None:
    if not command:
        return None
    offset = 0
    if command[0] == "sudo":
        offset = 1
    if offset >= len(command) or command[offset] != "git":
        return None
    offset += 1
    while offset < len(command):
        argument = command[offset]
        if argument in {"-C", "-c", "--git-dir", "--work-tree", "--namespace"}:
            offset += 2
            continue
        if argument.startswith("-"):
            offset += 1
            continue
        return argument
    return None



def _all_scalar_strings(value: object) -> list[str]:
    if isinstance(value, str):
        return [value]
    if isinstance(value, dict):
        return [text for child in value.values() for text in _all_scalar_strings(child)]
    if isinstance(value, list):
        return [text for child in value for text in _all_scalar_strings(child)]
    return []


def _step_position(job: dict[str, object] | None, name: str) -> int:
    if job is None:
        return -1
    steps = _as_steps(job.get("steps"))
    if steps is None:
        return -1
    matches = [index for index, step in enumerate(steps) if step.get("name") == name]
    return matches[0] if len(matches) == 1 else -1


def _action_step(
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


def _exact_checkout(
    job: dict[str, object] | None,
    ref: str,
    assertion_name: str,
    expected_variable: str,
) -> tuple[bool, bool]:
    checkouts = _action_step(job, "actions/checkout")
    assertion_position = _step_position(job, assertion_name)
    ordered = len(checkouts) == 1 and checkouts[0][0] == 0 and assertion_position == 1
    if len(checkouts) != 1:
        return False, False
    checkout = checkouts[0][1]
    configured = checkout.get("uses") == (
        f"actions/checkout@{PINNED_ACTIONS['actions/checkout']}"
    ) and _with_mapping(checkout) == {
        "ref": ref,
        "fetch-depth": 0,
        "persist-credentials": False,
    }
    assertion = _structured_step(job, assertion_name)
    assertion_lines = (
        'actual="$(git rev-parse HEAD | tr \'[:upper:]\' \'[:lower:]\')"',
        f'expected="$(printf \'%s\' "${expected_variable}" | tr \'[:upper:]\' \'[:lower:]\')"',
        'test "$actual" = "$expected"',
        'git cat-file -e "${expected}^{commit}"',
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
    )
    asserted = _has_code_lines(assertion, assertion_lines)
    return ordered and configured and asserted, configured


def validate_workflow(workflow: str) -> list[str]:
    try:
        document = _parse_workflow_yaml(workflow)
    except WorkflowYamlError:
        return ["valid unique-key YAML 1.2 workflow"]

    errors: list[str] = []
    if set(document) != {"name", "on", "permissions", "concurrency", "jobs"}:
        errors.append("exact release workflow structure")
    if document.get("name") != "Release":
        errors.append("Release workflow name")

    trigger = _as_mapping(document.get("on"))
    if trigger is None or set(trigger) != {"push", "workflow_dispatch"}:
        errors.append("exact release event set")
    push = _as_mapping(trigger.get("push")) if trigger is not None else None
    dispatch = (
        _as_mapping(trigger.get("workflow_dispatch")) if trigger is not None else None
    )
    inputs = _as_mapping(dispatch.get("inputs")) if dispatch is not None else None
    tag_input = _as_mapping(inputs.get("tag")) if inputs is not None else None
    if (
        push != {"tags": ["v*"]}
        or dispatch is None
        or set(dispatch) != {"inputs"}
        or inputs is None
        or set(inputs) != {"tag"}
        or tag_input is None
        or set(tag_input) != {"description", "required", "type"}
        or tag_input.get("required") is not True
        or tag_input.get("type") != "string"
    ):
        errors.append("tag-only release trigger")

    if document.get("permissions") != {"contents": "read"}:
        errors.append("top-level contents read permission")
    if document.get("concurrency") != {
        "group": "release-${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref_name }}",
        "cancel-in-progress": False,
    }:
        errors.append("per-tag non-cancelling concurrency")

    jobs = _as_mapping(document.get("jobs"))
    expected_job_names = {"validate", "quality", "macos_arm64", "windows_x64", "publish"}
    if jobs is None or set(jobs) != expected_job_names:
        errors.append("exact release job set")
        jobs = jobs or {}
    structured_jobs = {
        name: _as_mapping(jobs.get(name)) for name in expected_job_names
    }

    step_sets_valid = True
    run_templates_valid = True
    job_templates_valid = True
    approved_run_keys = set(APPROVED_SIMPLE_RUNS) | set(APPROVED_COMPLEX_RUN_SHA256)
    actual_run_keys: set[tuple[str, str]] = set()
    for job_name, job in structured_jobs.items():
        if job is None:
            step_sets_valid = False
            run_templates_valid = False
            job_templates_valid = False
            continue
        steps = _as_steps(job.get("steps"))
        if steps is None:
            step_sets_valid = False
            run_templates_valid = False
            job_templates_valid = False
            continue
        identities = tuple(_step_identity(step) for step in steps)
        if identities != APPROVED_STEP_IDENTITIES[job_name]:
            step_sets_valid = False
        if _structured_job_digest(job) != APPROVED_JOB_SHA256[job_name]:
            job_templates_valid = False
        for step in steps:
            run = step.get("run")
            if not isinstance(run, str):
                continue
            name = step.get("name")
            if not isinstance(name, str):
                run_templates_valid = False
                continue
            key = (job_name, name)
            actual_run_keys.add(key)
            if key in APPROVED_SIMPLE_RUNS:
                if run != APPROVED_SIMPLE_RUNS[key]:
                    run_templates_valid = False
            elif key in APPROVED_COMPLEX_RUN_SHA256:
                if _normalized_run_digest(run) != APPROVED_COMPLEX_RUN_SHA256[key]:
                    run_templates_valid = False
            else:
                run_templates_valid = False
    if actual_run_keys != approved_run_keys:
        run_templates_valid = False
    if not step_sets_valid:
        errors.append("approved release step sets")
    if not run_templates_valid:
        errors.append("approved release run templates")
    if not job_templates_valid:
        errors.append("approved complete release job digests")

    ignored = False
    skipped = False
    for job in structured_jobs.values():
        if job is None:
            continue
        if "continue-on-error" in job:
            ignored = True
        if "if" in job:
            skipped = True
        steps = _as_steps(job.get("steps"))
        if steps is None:
            errors.append("release jobs contain structured steps")
            continue
        for step in steps:
            ignored = ignored or "continue-on-error" in step
            skipped = skipped or "if" in step
    if ignored:
        errors.append("no ignored failures")
    if skipped:
        errors.append("required steps are unconditional")
    scalar_strings = _all_scalar_strings(document)
    if any("secrets." in value for value in scalar_strings):
        errors.append("no production secrets")

    action_pins_valid = True
    has_action = False
    forbidden_git_mutation = False
    for job in structured_jobs.values():
        if job is None:
            continue
        for step in _as_steps(job.get("steps")) or []:
            uses = step.get("uses")
            if isinstance(uses, str):
                has_action = True
                action, separator, revision = uses.partition("@")
                if (
                    separator != "@"
                    or PINNED_ACTIONS.get(action) != revision
                    or re.fullmatch(r"[0-9a-f]{40}", revision) is None
                ):
                    action_pins_valid = False
            for command in _commands(_run_script(step)):
                if len(command) >= 2 and command[0] == "git" and command[1] in {"tag", "push"}:
                    forbidden_git_mutation = True
    if not has_action or not action_pins_valid:
        errors.append("external actions pinned to full commit SHA")
    if forbidden_git_mutation:
        errors.append("workflow never creates or moves tags")

    git_source_mutators = {
        "am",
        "apply",
        "checkout",
        "cherry-pick",
        "clean",
        "commit",
        "merge",
        "read-tree",
        "rebase",
        "reset",
        "restore",
        "revert",
        "stash",
        "switch",
        "symbolic-ref",
        "update-index",
        "update-ref",
        "write-tree",
    }
    assertion_names = {
        "validate": "Validate tag, source SHA, versions, and notes",
        "quality": "Assert exact checked-out SHA",
        "macos_arm64": "Assert exact checked-out SHA",
        "windows_x64": "Assert exact checked-out SHA",
        "publish": "Assert exact checked-out SHA",
    }
    source_mutated = False
    for job_name, assertion_name in assertion_names.items():
        job = structured_jobs[job_name]
        if job is None:
            continue
        steps = _as_steps(job.get("steps")) or []
        assertion_position = _step_position(job, assertion_name)
        if assertion_position < 0:
            continue
        for step in steps[assertion_position:]:
            powershell = step.get("shell") == "pwsh" or (
                job_name == "windows_x64" and step.get("shell") is None
            )
            for command in _commands(_run_script(step), powershell=powershell):
                if _git_subcommand(command) in git_source_mutators:
                    source_mutated = True
    if source_mutated:
        errors.append("no git source mutation after SHA assertion")

    validate = structured_jobs["validate"]
    validate_checkouts = _action_step(validate, "actions/checkout")
    validate_checkout_configured = (
        len(validate_checkouts) == 1
        and validate_checkouts[0][1].get("uses")
        == f"actions/checkout@{PINNED_ACTIONS['actions/checkout']}"
        and _with_mapping(validate_checkouts[0][1])
        == {
            "ref": "${{ env.RELEASE_TAG }}",
            "fetch-depth": 0,
            "persist-credentials": False,
        }
    )
    bind = _structured_step(validate, "Validate tag, source SHA, versions, and notes")
    bind_lines = _code_lines(bind)
    validate_order = (
        'source_sha="$(git rev-parse "${RELEASE_TAG}^{commit}" | tr \'[:upper:]\' \'[:lower:]\')"',
        'actual="$(git rev-parse HEAD | tr \'[:upper:]\' \'[:lower:]\')"',
        'test "$actual" = "$source_sha"',
        'git cat-file -e "${source_sha}^{commit}"',
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
        'read -r remote_main remote_ref < <(git ls-remote --exit-code origin refs/heads/main)',
    )
    validate_ordered = (
        len(validate_checkouts) == 1
        and validate_checkouts[0][0] == 0
        and _step_position(validate, "Validate tag, source SHA, versions, and notes") == 1
    )
    if not validate_ordered or not validate_checkout_configured or not all(
        line in bind_lines for line in validate_order
    ):
        errors.append("exactly one ordered checkout and SHA assertion per release job")
    elif [bind_lines.index(line) for line in validate_order] != sorted(
        bind_lines.index(line) for line in validate_order
    ):
        errors.append("exactly one ordered checkout and SHA assertion per release job")
    if not validate_checkout_configured:
        errors.append("validate exact tag checkout")
    if not _has_code_lines(
        bind,
        (
            'if [[ "$source_sha" != "$remote_main" ]]; then',
            'echo "tag commit does not equal current remote main HEAD" >&2',
        ),
    ) or not _has_command(
        bind,
        ("read", "-r", "remote_main", "remote_ref", "<", "<(git", "ls-remote", "--exit-code", "origin", "refs/heads/main)"),
    ):
        errors.append("tag commit equals current remote main HEAD")
    if not _has_code_lines(
        bind,
        (
            'cargo = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))',
            'tauri = json.loads(Path("src-tauri/tauri.conf.json").read_text(encoding="utf-8"))',
            'web = json.loads(Path("web/package.json").read_text(encoding="utf-8"))',
            'notes = Path("docs/releases") / f"{version}.md"',
            'if versions != {version}:',
            'wix_version = tauri["bundle"]["windows"]["wix"]["version"]',
            'if wix_version != "1.0.0.3":',
        ),
    ):
        errors.append("Cargo, Tauri, and Web versions match tag")
    if not _has_code_lines(
        bind,
        (
            'if SEMVER_RE.fullmatch(tag) is None:',
            'if version == "1.0.0-beta.3" and not prerelease:',
            'emit("prerelease", "true")',
        ),
    ):
        errors.append("Beta 3 is a SemVer prerelease")
    if not _has_code_lines(bind, validate_order[1:5]):
        errors.append("validate binds exact clean checkout")

    required_jobs = ("quality", "macos_arm64", "windows_x64")
    for name in ("validate", *required_jobs):
        job = structured_jobs[name]
        if job is not None and "permissions" in job and job.get("permissions") != {
            "contents": "read"
        }:
            errors.append("only publish may elevate contents permission")
            break
    for name in required_jobs:
        job = structured_jobs[name]
        if job is None or job.get("needs") != "validate":
            errors.append(f"{name} depends on validation")
        exact, configured = _exact_checkout(
            job,
            "${{ needs.validate.outputs.source_sha }}",
            "Assert exact checked-out SHA",
            "TARGET_SHA",
        )
        if not exact:
            errors.append("exactly one ordered checkout and SHA assertion per release job")
        if not configured:
            errors.append("every gate checks out the validated SHA")
    publish = structured_jobs["publish"]
    publish_checkout, publish_configured = _exact_checkout(
        publish,
        "${{ needs.validate.outputs.source_sha }}",
        "Assert exact checked-out SHA",
        "RELEASE_SHA",
    )
    if not publish_checkout:
        errors.append("exactly one ordered checkout and SHA assertion per release job")
    if not publish_configured:
        errors.append("every gate checks out the validated SHA")

    reassertions = (
        ("validate", "Reassert exact source after validation"),
        ("quality", "Reassert exact source after quality gates"),
        ("macos_arm64", "Reassert exact source before macOS build"),
        ("macos_arm64", "Reassert exact source after macOS packaging"),
        ("windows_x64", "Reassert exact source before Windows build"),
        ("windows_x64", "Reassert exact source after Windows packaging"),
        ("publish", "Reassert exact source before draft mutation"),
        ("publish", "Reassert exact source before publication"),
    )
    strict_untracked_check = (
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"'
    )
    if any(
        strict_untracked_check
        not in _run_script(_structured_step(structured_jobs[job_name], step_name)).splitlines()
        for job_name, step_name in reassertions
    ):
        errors.append("every source reassert rejects untracked files")

    quality = structured_jobs["quality"]
    quality_commands = (
        ("Install locked Motion Canvas dependencies", ("npm", "--prefix", "plugins/motion-canvas-studio", "ci", "--ignore-scripts")),
        ("Audit Motion Canvas dependencies and licenses", ("npm", "--prefix", "plugins/motion-canvas-studio", "audit", "--audit-level=moderate")),
        ("Audit Motion Canvas dependencies and licenses", ("npm", "--prefix", "plugins/motion-canvas-studio", "run", "licenses")),
        ("Test and reproduce Motion Canvas runner", ("npm", "--prefix", "plugins/motion-canvas-studio", "test")),
        ("Test and reproduce Motion Canvas runner", ("npm", "--prefix", "plugins/motion-canvas-studio", "run", "build")),
        ("Test and reproduce Motion Canvas runner", ("git", "diff", "--exit-code", "--", "plugins/motion-canvas-studio/bundle/runner.html", "plugins/motion-canvas-studio/package-lock.json")),
        ("Validate Windows and release workflow contracts", ("python3", "-B", "scripts/check_windows_product_ci.py")),
        ("Validate Windows and release workflow contracts", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_check_windows_product_ci.py")),
        ("Validate Windows and release workflow contracts", ("python3", "-B", "scripts/check_release_workflow.py")),
        ("Validate Windows and release workflow contracts", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_check_release_workflow.py")),
        ("Provisioner unit tests", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_*.py")),
        ("Rust formatting", ("cargo", "fmt", "--all", "--check")),
        ("Rust workspace clippy", ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings")),
        ("Rust workspace tests", ("cargo", "test", "--workspace", "--", "--test-threads=1")),
        ("Live playback transport integration", ("cargo", "test", "-p", "opentake-tauri", "--features", "playback-engine", "--test", "playback_transport_integration", "--", "--test-threads=1")),
        ("Minimal-feature Tauri clippy", ("cargo", "clippy", "-p", "opentake-tauri", "--no-default-features", "--all-targets", "--", "-D", "warnings")),
        ("Install locked Web dependencies", ("pnpm", "-C", "web", "install", "--frozen-lockfile")),
        ("Web editor behavior suite", ("pnpm", "-C", "web", "test")),
        ("Web production build", ("pnpm", "-C", "web", "build")),
    )
    quality_ok = quality is not None and quality.get("runs-on") == "ubuntu-latest"
    quality_ok = quality_ok and all(
        _has_command(_structured_step(quality, step_name), command)
        for step_name, command in quality_commands
    )
    system_deps = _structured_step(quality, "Install system deps (ffmpeg + Tauri/GTK)")
    quality_ok = quality_ok and _has_command(
        system_deps,
        (
            "sudo", "apt-get", "install", "-y", "ffmpeg", "libwebkit2gtk-4.1-dev",
            "libgtk-3-dev", "libayatana-appindicator3-dev", "librsvg2-dev",
            "libasound2-dev", "libglib2.0-dev", "libsoup-3.0-dev", "patchelf",
            "pkg-config", "fonts-dejavu-core",
        ),
    )
    if not quality_ok:
        errors.append("complete Ubuntu release quality gates")

    ruby_steps = _action_step(quality, "ruby/setup-ruby")
    validator_position = _step_position(quality, "Validate Windows and release workflow contracts")
    if (
        len(ruby_steps) != 1
        or ruby_steps[0][0] <= 1
        or ruby_steps[0][0] >= validator_position
        or ruby_steps[0][1].get("uses")
        != f"ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}"
        or _with_mapping(ruby_steps[0][1]) != {"ruby-version": "3.3"}
    ):
        errors.append("quality pins Ruby Psych before the release validator")

    macos = structured_jobs["macos_arm64"]
    mac_build = _structured_step(macos, "Build ad-hoc Tauri app and DMG")
    mac_verify = _structured_step(macos, "Verify complete app, sidecars, and DMG")
    mac_receipt = _structured_step(macos, "Create macOS exact-SHA receipt")
    mac_uploads = _action_step(macos, "actions/upload-artifact")
    mac_ok = (
        macos is not None
        and macos.get("runs-on") == "macos-14"
        and _as_mapping(macos.get("env", {})).get("APPLE_SIGNING_IDENTITY") == "-"
        and _has_command(_structured_step(macos, "Provision checksum-pinned ARM64 FFmpeg sidecars"), ("python3", "scripts/provision_ffmpeg_sidecars.py", "--target", "aarch64-apple-darwin"))
        and _has_command(_structured_step(macos, "Install locked Web dependencies"), ("pnpm", "-C", "web", "install", "--frozen-lockfile"))
        and _has_command(mac_build, ("./web/node_modules/.bin/tauri", "build", "--ci", "--target", "aarch64-apple-darwin", "--bundles", "app,dmg", "--config", '{"bundle":{"macOS":{"signingIdentity":"-"}}}'))
        and _has_command(mac_verify, ("codesign", "--verify", "--deep", "--strict", "--verbose=2", "$app"))
        and _has_command(mac_verify, ("hdiutil", "verify", "$dmg"))
        and _has_command(mac_verify, ("hdiutil", "attach", "$dmg", "-nobrowse", "-readonly", "-mountpoint", "$mountpoint"))
        and _has_command(mac_verify, ("ruby", "scripts/tests/packaged-sidecars-test.rb", "--name", "packaged_macos_windows_sidecars_resolve_and_execute", "--package", "$mounted_app"))
        and _has_code_lines(mac_verify, ("codesign -dv --verbose=4 \"$app\" 2>&1 | grep -F 'Signature=adhoc'",))
        and _has_code_lines(mac_receipt, ('"schema": "opentake-macos-arm64-receipt-v1",', '"source_sha": os.environ["RECEIPT_SHA"],', '"signature_mode": "ad-hoc",', '"sha256": sha256(artifact),', '"bytes": artifact.stat().st_size,'))
        and len(mac_uploads) == 1
        and mac_uploads[0][1].get("uses") == f"actions/upload-artifact@{PINNED_ACTIONS['actions/upload-artifact']}"
    )
    if not mac_ok:
        errors.append("complete ad-hoc macOS ARM64 bundle gate")

    windows = structured_jobs["windows_x64"]
    windows_build = _structured_step(windows, "Build native MSI and NSIS installers")
    windows_install = _structured_step(windows, "Install NSIS and smoke installed app and sidecars")
    windows_receipt = _structured_step(windows, "Create Windows exact-SHA receipt")
    windows_uploads = _action_step(windows, "actions/upload-artifact")
    windows_commands = (
        ("Provision checksum-pinned Windows FFmpeg sidecars", ("python", "scripts/provision_ffmpeg_sidecars.py", "--target", "x86_64-pc-windows-msvc")),
        ("Rust workspace clippy", ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings")),
        ("Rust workspace tests", ("cargo", "test", "--workspace", "--", "--test-threads=1")),
        ("Web editor behavior suite", ("pnpm", "-C", "web", "test")),
        ("Minimal-feature Tauri clippy", ("cargo", "clippy", "-p", "opentake-tauri", "--no-default-features", "--all-targets", "--", "-D", "warnings")),
        ("Web production build", ("pnpm", "-C", "web", "build")),
    )
    windows_ok = windows is not None and windows.get("runs-on") == "windows-2022"
    windows_ok = windows_ok and all(
        _has_command(_structured_step(windows, step_name), command)
        for step_name, command in windows_commands
    )
    windows_ok = windows_ok and _has_command(
        windows_build,
        ("&", ".\\web\\node_modules\\.bin\\tauri.cmd", "build", "--ci", "--bundles", "msi,nsis"),
        powershell=True,
    )
    windows_ok = windows_ok and _has_code_lines(
        windows_install,
        (
            "if ($msi.Count -ne 1) { throw 'expected exactly one MSI installer' }",
            "if ($installer.Count -ne 1) { throw 'expected exactly one NSIS installer' }",
            "$app = Start-Process -FilePath $application -PassThru",
            "--name packaged_macos_windows_sidecars_resolve_and_execute `",
        ),
    )
    windows_ok = windows_ok and _has_code_lines(
        windows_receipt,
        (
            "schema = 'opentake-windows-release-receipt-v1'",
            "source_sha = $env:RECEIPT_SHA",
            "sha256 = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()",
            "bytes = $_.Length",
        ),
    )
    windows_ok = windows_ok and len(windows_uploads) == 1 and windows_uploads[0][1].get(
        "uses"
    ) == f"actions/upload-artifact@{PINNED_ACTIONS['actions/upload-artifact']}"
    if not windows_ok:
        errors.append("complete Windows x64 installer gate")

    if publish is None or publish.get("needs") != [
        "validate", "quality", "macos_arm64", "windows_x64"
    ]:
        errors.append("publish depends on every required job")
    if publish is None:
        return list(dict.fromkeys(errors))
    if publish.get("permissions") != {"contents": "write"}:
        errors.append("publish-only contents write permission")
    publish_env = _as_mapping(publish.get("env"))
    if publish_env is None or publish_env.get("PYTHONDONTWRITEBYTECODE") != "1":
        errors.append("publish Python helpers cannot write bytecode into the checkout")
    initialize_publish = _structured_step(publish, "Initialize isolated publish root")
    download_macos = _structured_step(publish, "Download macOS artifact")
    download_windows = _structured_step(publish, "Download Windows artifact")
    download_macos_with = _with_mapping(download_macos) or {}
    download_windows_with = _with_mapping(download_windows) or {}
    if not (
        _has_code_lines(
            initialize_publish,
            (
                'publish_root="$RUNNER_TEMP/opentake-release-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT"',
                'printf \'PUBLISH_ROOT=%s\\n\' "$publish_root" >> "$GITHUB_ENV"',
            ),
        )
        and download_macos_with.get("path")
        == "${{ runner.temp }}/opentake-release-${{ github.run_id }}-${{ github.run_attempt }}/input/macos"
        and download_windows_with.get("path")
        == "${{ runner.temp }}/opentake-release-${{ github.run_id }}-${{ github.run_attempt }}/input/windows"
    ):
        errors.append("publish outputs stay outside the worktree")

    stage = _structured_step(publish, "Stage and verify the exact release payload")
    stage_commands = (
        ("test", "${#dmgs[@]}", "-eq", "1"),
        ("test", "${#msis[@]}", "-eq", "1"),
        ("test", "${#exes[@]}", "-eq", "1"),
        ("test", "${#mac_receipts[@]}", "-eq", "1"),
        ("test", "${#windows_receipts[@]}", "-eq", "1"),
    )
    if not all(_has_command(stage, command) for command in stage_commands) or not _has_code_lines(
        stage, ('expected = {"dmg": 1, "msi": 1, "exe": 1, "json": 2}',)
    ):
        errors.append("strict release asset counts")

    checksum = _structured_step(publish, "Create and verify SHA256SUMS")
    checksum_lines = (
        'payload_names=("${asset_names[@]}")',
        'test "${#payload_names[@]}" -eq 5',
        'sha256sum "${payload_names[@]}" > SHA256SUMS',
        'test "$(wc -l < SHA256SUMS | tr -d \' \')" -eq 5',
        'sha256sum --check SHA256SUMS',
        'test "$(wc -l < "$PUBLISH_ROOT/expected-assets.txt" | tr -d \' \')" -eq 6',
    )
    if not _has_code_lines(checksum, checksum_lines):
        errors.append("SHA256SUMS covers and verifies every payload")

    ordered_names = (
        "Revalidate remote tag before draft mutation",
        "Create or refresh draft prerelease",
        "Upload the exact payload to the draft",
        "Verify draft target and exact assets",
        "Revalidate remote tag before publication",
        "Publish verified prerelease",
        "Verify public release through API and checksums",
    )
    ordered_positions = [_step_position(publish, name) for name in ordered_names]
    draft = _structured_step(publish, ordered_names[1])
    upload = _structured_step(publish, ordered_names[2])
    inspect_draft = _structured_step(publish, ordered_names[3])
    rebind_draft = _structured_step(publish, ordered_names[0])
    rebind_public = _structured_step(publish, ordered_names[4])
    make_public = _structured_step(publish, ordered_names[5])
    final = _structured_step(publish, ordered_names[6])
    draft_commands = _commands(_run_script(draft))
    create_command = ("gh", "release", "create", "$RELEASE_TAG", "--verify-tag", "--target", "$RELEASE_SHA", "--title", "OpenTake $RELEASE_VERSION", "--notes-file", "$PUBLISH_ROOT/release-body.md", "--draft", "--prerelease", "--latest=false")
    refresh_command = ("gh", "release", "edit", "$RELEASE_TAG", "--target", "$RELEASE_SHA", "--title", "OpenTake $RELEASE_VERSION", "--notes-file", "$PUBLISH_ROOT/release-body.md", "--draft", "--prerelease", "--latest=false")
    resolver_command = ("python3", "scripts/check_release_workflow.py", "resolve-release-state", "--input", "$PUBLISH_ROOT/existing-release-graphql.json", "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA", "--output", "$PUBLISH_ROOT/release-state.json")
    delete_command = ("gh", "api", "--method", "DELETE", "repos/$GITHUB_REPOSITORY/releases/assets/$asset_id")
    published_guard = ("echo", "release is already published; refusing to mutate it", ">&2")
    draft_ok = (
        min(ordered_positions) >= 0
        and ordered_positions == sorted(ordered_positions)
        and resolver_command in draft_commands
        and create_command in draft_commands
        and refresh_command in draft_commands
        and ("gh", "api", "graphql", "-f", "query=$query", "-f", "owner=$owner", "-f", "name=$repository", "-f", "tag=$RELEASE_TAG", ">", "$PUBLISH_ROOT/existing-release-graphql.json") in draft_commands
        and ("gh", "api", "repos/$GITHUB_REPOSITORY/releases/$release_id", ">", "$PUBLISH_ROOT/existing-draft-rest.json") in draft_commands
        and delete_command in draft_commands
        and published_guard in draft_commands
        and draft_commands.index(resolver_command) < draft_commands.index(delete_command)
        and draft_commands.index(published_guard) < draft_commands.index(delete_command)
    )
    if not draft_ok:
        errors.append("draft exists before release upload")
        errors.append("published same-tag release is immutable")
    if not _has_command(
        upload,
        ("gh", "release", "upload", "$RELEASE_TAG", "$PUBLISH_ROOT/assets/*", "--clobber"),
    ):
        errors.append("draft payload upload supports failed-run retry")
    if not (
        _has_command(inspect_draft, ("python3", "scripts/check_release_workflow.py", "resolve-release-state", "--input", "$PUBLISH_ROOT/draft-release-graphql.json", "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA", "--output", "$PUBLISH_ROOT/draft-state.json"))
        and _has_code_lines(inspect_draft, ('test "$(jq -r \'.action\' "$PUBLISH_ROOT/draft-state.json")" = "refresh"', 'cmp "$PUBLISH_ROOT/expected-assets.txt" "$PUBLISH_ROOT/draft-assets.txt"', "jq -e '.asset_sizes | length == 6 and all(. > 0)' \"$PUBLISH_ROOT/draft-state.json\" >/dev/null"))
    ):
        errors.append("draft API verification")

    def remote_rebind_ok(step: dict[str, object] | None, output: str) -> bool:
        return _has_command(
            step,
            ("git", "ls-remote", "--exit-code", "origin", "refs/tags/$RELEASE_TAG", "refs/tags/$RELEASE_TAG^{}", ">", output),
        ) and _has_command(
            step,
            ("python3", "scripts/check_release_workflow.py", "resolve-remote-tag", "--input", output, "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA"),
        )

    if not remote_rebind_ok(rebind_draft, "$PUBLISH_ROOT/remote-tag-before-draft.txt"):
        errors.append("remote tag rebound before draft mutation")
    if not remote_rebind_ok(rebind_public, "$PUBLISH_ROOT/remote-tag-before-publication.txt"):
        errors.append("remote tag rebound before publication")
    if not _has_command(
        make_public,
        ("gh", "release", "edit", "$RELEASE_TAG", "--draft=false", "--prerelease", "--latest=false"),
    ):
        errors.append("verified prerelease publication")
    if not _has_code_lines(
        final,
        (
            'if release.get("target_commitish") != expected_sha:',
            'if release.get("draft") is not False or release.get("prerelease") is not True:',
            "if expected_names != actual_names:",
            "sha256sum --check SHA256SUMS",
        ),
    ) or not _has_command(final, ("gh", "release", "download", "$RELEASE_TAG", "--dir", "$PUBLISH_ROOT/verified-release")):
        errors.append("final API verification binds target SHA")

    notes = _structured_step(publish, "Prepare release notes with provenance")
    if not _has_code_lines(
        notes,
        (
            "- Source commit: \\`$RELEASE_SHA\\`",
            "- GitHub Actions run: [$GITHUB_RUN_ID/$GITHUB_RUN_ATTEMPT]($GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID)",
            "- Signing limits: the macOS asset uses ad-hoc signing only; it is not Developer ID signed or notarized. Windows installers are not claimed to be Authenticode-signed.",
        ),
    ):
        errors.append("release notes record source, run, and signing limits")

    return list(dict.fromkeys(errors))


def validate_repository_metadata(repository_root: Path) -> list[str]:
    errors: list[str] = []
    cargo_path = repository_root / "Cargo.toml"
    tauri_path = repository_root / "src-tauri" / "tauri.conf.json"
    web_path = repository_root / "web" / "package.json"
    try:
        cargo = tomllib.loads(cargo_path.read_text(encoding="utf-8"))
        tauri = json.loads(tauri_path.read_text(encoding="utf-8"))
        web = json.loads(web_path.read_text(encoding="utf-8"))
        if not all(isinstance(metadata, dict) for metadata in (cargo, tauri, web)):
            raise TypeError("version metadata roots must be objects")
        versions = {
            cargo["workspace"]["package"]["version"],
            tauri["version"],
            web["version"],
        }
        wix_version = tauri["bundle"]["windows"]["wix"]["version"]
        if not all(isinstance(version, str) for version in versions):
            raise TypeError("version metadata values must be strings")
        if not isinstance(wix_version, str):
            raise TypeError("Windows installer version must be a string")
    except (KeyError, OSError, TypeError, UnicodeError, ValueError):
        return ["readable Cargo, Tauri, and Web version metadata"]

    if versions != {"1.0.0-beta.3"}:
        errors.append("repository metadata is OpenTake 1.0.0-beta.3")
    if wix_version != "1.0.0.3":
        errors.append("Windows installer version is 1.0.0.3")
    notes = repository_root / "docs" / "releases" / "1.0.0-beta.3.md"
    try:
        notes_missing = not notes.is_file() or not notes.read_text(
            encoding="utf-8"
        ).strip()
    except (OSError, UnicodeError):
        notes_missing = True
    if notes_missing:
        errors.append("Beta 3 release notes exist")
    return errors


def _resolve_release_state_command(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(prog="check_release_workflow.py resolve-release-state")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args(arguments)
    try:
        payload = json.loads(options.input.read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            raise ReleaseStateError("GraphQL release lookup must be a JSON object")
        plan = resolve_release_state(payload, options.tag, options.sha)
    except (OSError, json.JSONDecodeError, ReleaseStateError) as error:
        raise SystemExit(f"unsafe release state: {error}") from error
    options.output.write_text(
        json.dumps(plan, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def _resolve_remote_tag_command(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(prog="check_release_workflow.py resolve-remote-tag")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True)
    options = parser.parse_args(arguments)
    try:
        resolved = resolve_remote_tag_refs(
            options.input.read_text(encoding="utf-8"), options.tag
        )
    except (OSError, RemoteTagError) as error:
        raise SystemExit(f"unsafe remote tag state: {error}") from error
    if resolved != options.sha:
        raise SystemExit(f"remote tag target mismatch: {resolved} != {options.sha}")
    print(resolved)


def main(arguments: list[str] | None = None) -> None:
    arguments = sys.argv[1:] if arguments is None else arguments
    if arguments:
        if arguments[0] == "resolve-release-state":
            _resolve_release_state_command(arguments[1:])
        elif arguments[0] == "resolve-remote-tag":
            _resolve_remote_tag_command(arguments[1:])
        else:
            raise SystemExit(f"unknown command: {arguments[0]}")
        return
    if not WORKFLOW_PATH.is_file():
        raise SystemExit(f"release workflow is missing: {WORKFLOW_PATH}")
    errors = validate_workflow(WORKFLOW_PATH.read_text(encoding="utf-8"))
    errors.extend(validate_repository_metadata(REPOSITORY_ROOT))
    if errors:
        raise SystemExit("release workflow is missing: " + ", ".join(errors))
    print("Release workflow contract is complete")


if __name__ == "__main__":
    main()
