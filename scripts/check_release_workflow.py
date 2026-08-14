#!/usr/bin/env python3
"""Fail-closed contract checks for the tag-driven GitHub release workflow."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tomllib

from workflow_yaml import WorkflowYamlError, parse_workflow_yaml as _parse_workflow_yaml


REPOSITORY_ROOT = Path(
    os.environ.get("OPENTAKE_REPOSITORY_ROOT", Path(__file__).resolve().parents[1])
).resolve()
WORKFLOW_PATH = Path(
    os.environ.get(
        "OPENTAKE_RELEASE_WORKFLOW_PATH",
        REPOSITORY_ROOT / ".github" / "workflows" / "release.yml",
    )
).resolve()
RELEASE_NOTES_PATH = Path(
    os.environ.get(
        "OPENTAKE_RELEASE_NOTES_PATH",
        REPOSITORY_ROOT / "docs" / "releases" / "1.0.0-beta.5.md",
    )
).resolve()
CURRENT_RELEASE_VERSION = "1.0.0-beta.5"
APPROVED_REPOSITORY_IDENTITIES = {
    "1.0.0-beta.4": ("1.0.0.4", "Beta 4"),
    CURRENT_RELEASE_VERSION: ("1.0.0.5", "Beta 5"),
}
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
        "name:Validate Web dependency licenses",
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
        "name:Require updater signing secrets",
        "name:Install Rust toolchain",
        f"uses:pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        f"uses:actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        f"uses:ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}",
        "name:Provision checksum-pinned ARM64 FFmpeg sidecars",
        "name:Verify pinned sidecar supply",
        "name:Install locked Web dependencies",
        "name:Reassert exact source before macOS build",
        "name:Build ad-hoc Tauri app, DMG, and signed updater",
        "name:Verify complete app, sidecars, DMG, and signed updater",
        "name:Reassert exact source after macOS packaging",
        "name:Create and sign macOS updater attestation",
        "name:Create macOS exact-SHA receipt",
        "name:Upload exact-SHA macOS packages and updater",
    ),
    "windows_x64": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Require updater signing secrets",
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
        "name:Build native MSI, NSIS, and signed updater artifacts",
        "name:Install NSIS and smoke installed app, sidecars, and updater artifacts",
        "name:Reassert exact source after Windows packaging",
        "name:Create and sign Windows updater attestations",
        "name:Create Windows exact-SHA receipt",
        "name:Upload exact-SHA Windows packages and updater signatures",
    ),
    "publish": (
        f"uses:actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "name:Assert exact checked-out SHA",
        "name:Initialize isolated publish root",
        "name:Install Minisign verifier",
        "name:Download macOS artifact",
        "name:Download Windows artifact",
        "name:Stage and verify the exact release payload",
        "name:Verify updater signatures against embedded public key",
        "name:Write and verify tag-specific updater manifest",
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
    ("macos_arm64", "Build ad-hoc Tauri app, DMG, and signed updater"): "./web/node_modules/.bin/tauri build --ci --target aarch64-apple-darwin --bundles app,dmg --config '{\"bundle\":{\"createUpdaterArtifacts\":true,\"macOS\":{\"signingIdentity\":\"-\"}}}'",
    ("windows_x64", "Install Rust toolchain"): "rustup component add rustfmt clippy",
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
    ("validate", "Validate tag, source SHA, versions, and notes"): "eefb9d97ad80e8090b817178093824af2b897ae339a144b2e16a91de3f9f8334",
    ("validate", "Reassert exact source after validation"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("quality", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("quality", "Free disk space"): "5848415c4d0e696f46965d62a2e17c8b7a0dd45ae600d28102af0b04108d9bf6",
    ("quality", "Install system deps (ffmpeg + Tauri/GTK)"): "ee466d2d3fff1c3703d50f9dabe4d21e1cee4b399924d064c6d2714dae34d16b",
    ("quality", "Audit Motion Canvas dependencies and licenses"): "a3517fae1a8663e519138196c9f3721d8f4df19ac8f115c49a079c4aaa60c8b3",
    ("quality", "Test and reproduce Motion Canvas runner"): "8bcd55de9b045f9d7be6343163a5422cba0ab545f7844da50ca1a7c8623fe640",
    ("quality", "Validate Windows and release workflow contracts"): "76d3d0db11222f5121e5b404470296695c8cfea0b4e41f0c3a1d853612331e7d",
    ("quality", "Provisioner unit tests"): "f57d4d7d6df403d573d31bbda02589804109596040c2cefcca60f3e9352e891a",
    ("quality", "Validate Web dependency licenses"): "5b914e6aaddab4c9ca0ac03ff0cc03d23b44fc3d66621b8c9fa5eeac07a5cfcd",
    ("quality", "Live playback transport integration"): "461f79546009551e5e7adbf50f869abb9449c2ae7666a66a425c7cd3c24acea9",
    ("quality", "Reassert exact source after quality gates"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("macos_arm64", "Require updater signing secrets"): "d03a04c4866ac7ecb9eba9d53aabbf92de24f452e6b5448d438d863988e797c4",
    ("macos_arm64", "Reassert exact source before macOS build"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Verify complete app, sidecars, DMG, and signed updater"): "17dabc0d92b8958c44f315d7c874f4db97ae3efe1ced73d7994e8654e42e80fe",
    ("macos_arm64", "Reassert exact source after macOS packaging"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("macos_arm64", "Create and sign macOS updater attestation"): "5fd8f5af4dcf6a11740d531a39cd716f93478fb65bb4b4fcbafd88191d245d4a",
    ("macos_arm64", "Create macOS exact-SHA receipt"): "06f89a7122f8257ad14b8ae5fa59426e87ecbae4936f0db69a37ee6cc748bd2a",
    ("windows_x64", "Assert exact checked-out SHA"): "ff0b148eecdf8603712586a6c4a05e752df0b36b5c97a366760f6cba10e58ddd",
    ("windows_x64", "Require updater signing secrets"): "d03a04c4866ac7ecb9eba9d53aabbf92de24f452e6b5448d438d863988e797c4",
    ("windows_x64", "Provision checksum-pinned Windows FFmpeg sidecars"): "0518296c77e12a05d7bd99327daf00782e05aa1697f853d654a0eec0fd449238",
    ("windows_x64", "Reassert exact source before Windows build"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("windows_x64", "Build native MSI, NSIS, and signed updater artifacts"): "cc2f68b639e4d188892836a0b4e0a3819eb216dc83b06f5acf2a3d0640405142",
    ("windows_x64", "Install NSIS and smoke installed app, sidecars, and updater artifacts"): "556af3fe7ee52b0dfde26824abf39b589c897782f1a311a8c64a044dc3cf7010",
    ("windows_x64", "Reassert exact source after Windows packaging"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("windows_x64", "Create and sign Windows updater attestations"): "40c7f2ea8696db15adf49a688a446637df5b1625fe0b39de32502e66af71f2c6",
    ("windows_x64", "Create Windows exact-SHA receipt"): "0072244797bdcbe7a26f57a5a83e5c2ba880ffa96499f18f4c5affd30b0e54f4",
    ("publish", "Assert exact checked-out SHA"): "b1cd768e31e2924c14421c62357ae200ddee50b2a6c2ddc24618717a3c876267",
    ("publish", "Initialize isolated publish root"): "79b191e8b5c59e59d847c59edb73d17069736c54bc1cb56f2588051ab3e3f433",
    ("publish", "Install Minisign verifier"): "444027a1ab2942d223b3e16e286d2de585fdc1175e94e093b012b13aefea2416",
    ("publish", "Stage and verify the exact release payload"): "4cdd6d245891e535e6f11a29371b4b0c7e343702f36d37f56262387f3523b534",
    ("publish", "Verify updater signatures against embedded public key"): "d0da4c84101149e7853b764db8f770d25f61b4fa654f9f927fd813bd22604fe5",
    ("publish", "Write and verify tag-specific updater manifest"): "faef05e038dd082d81e1ec12a8c6f933575e8766e73df1b4ecb28cb794156579",
    ("publish", "Create and verify SHA256SUMS"): "67975ac408cc96209261e8c397402acaac47e1a689d7572d5634d3b8dabd66d8",
    ("publish", "Prepare release notes with provenance"): "25aa357cc5ce5620686b2b66155dfaf4d967c3977eb6cb5dcad628449a161f71",
    ("publish", "Reassert exact source before draft mutation"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("publish", "Revalidate remote tag before draft mutation"): "cd6a3662a7a43dbd837170c734a41f3832f258b14468b3bdbee36b1e42d7a75c",
    ("publish", "Create or refresh draft prerelease"): "f636c39fe330986ae5fe717a5005554fd11a6a9df3e1b8303c4e42c5263b39a7",
    ("publish", "Verify draft target and exact assets"): "99620971b48abd650be72d91bcdd6f2af47ba56446d124c105580ee79bc5d40e",
    ("publish", "Revalidate remote tag before publication"): "502a7ccd2c1d6b81642aa7824c168521caaefd5d6118f4e9cdafa359ce57ce3c",
    ("publish", "Reassert exact source before publication"): "953657d26d2eda8490c18e7030c66ddb19aba64a5c8b19808da9a853fd1bfdd2",
    ("publish", "Verify public release through API and checksums"): "136ae6eb7805f2f0e59f11bc1624860f51defa3d0449aab4b519a2506a554522",
}

APPROVED_JOB_SHA256 = {
    "validate": "ce61e176161ab7ec892d92ed9cf1a3e379c6b5f7b5781bf1addc4f29379a85b2",
    "quality": "d0078d8cd49919be4a1f71f1208c0d05369282c13f5ae5c38142063d648816c2",
    "macos_arm64": "1785d765c96278190c25e312c9e610070619e17b7b2b0d922f0bd234501df525",
    "windows_x64": "63bd70d85e40a3f1177e9059d4674d7f93d4502181fc378f7706e839af953378",
    "publish": "ea3fe6d18a94c0850d3ac7f21c4e23fb8fcf572189f77f659e5b34eab776bd85",
}

EXPECTED_RECOVERY_JOB_CONCLUSIONS = {
    "Validate immutable release source": "success",
    "Release quality gates": "success",
    "macOS ARM64 app and DMG": "success",
    "Windows x64 MSI and NSIS": "failure",
    "Publish verified GitHub prerelease": "skipped",
}
EXPECTED_RECOVERY_WINDOWS_STEPS = (
    ("Set up job", "completed", "success"),
    (
        f"Run actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "completed",
        "success",
    ),
    ("Assert exact checked-out SHA", "completed", "success"),
    ("Require updater signing secrets", "completed", "success"),
    ("Install Rust toolchain", "completed", "success"),
    (
        f"Run pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        "completed",
        "success",
    ),
    (
        f"Run actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        "completed",
        "success",
    ),
    (
        f"Run ruby/setup-ruby@{PINNED_ACTIONS['ruby/setup-ruby']}",
        "completed",
        "success",
    ),
    (
        "Provision checksum-pinned Windows FFmpeg sidecars",
        "completed",
        "failure",
    ),
    ("Verify pinned sidecar supply", "completed", "skipped"),
    ("Cache Cargo dependencies", "completed", "skipped"),
    ("Install locked Web dependencies", "completed", "skipped"),
    ("Rust workspace clippy", "completed", "skipped"),
    ("Rust workspace tests", "completed", "skipped"),
    ("Web editor behavior suite", "completed", "skipped"),
    ("Minimal-feature Tauri clippy", "completed", "skipped"),
    ("Web production build", "completed", "skipped"),
    ("Reassert exact source before Windows build", "completed", "skipped"),
    (
        "Build native MSI, NSIS, and signed updater artifacts",
        "completed",
        "skipped",
    ),
    (
        "Install NSIS and smoke installed app, sidecars, and updater artifacts",
        "completed",
        "skipped",
    ),
    ("Reassert exact source after Windows packaging", "completed", "skipped"),
    ("Create and sign Windows updater attestations", "completed", "skipped"),
    ("Create Windows exact-SHA receipt", "completed", "skipped"),
    (
        "Upload exact-SHA Windows packages and updater signatures",
        "completed",
        "skipped",
    ),
    (
        f"Post Run actions/setup-node@{PINNED_ACTIONS['actions/setup-node']}",
        "completed",
        "skipped",
    ),
    (
        f"Post Run pnpm/action-setup@{PINNED_ACTIONS['pnpm/action-setup']}",
        "completed",
        "success",
    ),
    (
        f"Post Run actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "completed",
        "success",
    ),
    ("Complete job", "completed", "success"),
)
EXPECTED_RECOVERY_WINDOWS_STEP_NUMBERS = (*range(1, 25), 46, 47, 48, 49)
EXPECTED_FAILED_RECOVERY_RUN_ID = 31441693191
EXPECTED_FAILED_RECOVERY_TAG = "v1.0.0-beta.4"
EXPECTED_FAILED_RECOVERY_SOURCE_SHA = (
    "2c4efdff9d2587c90cbcac0919f9d1d333d67d6a"
)
EXPECTED_FAILED_RECOVERY_TOOLING_SHA = (
    "924bc1102a9343e14c3beea2a3622b5d92ebff13"
)
EXPECTED_FAILED_RECOVERY_JOB_CONCLUSIONS = {
    "Validate immutable release source": "success",
    "Release quality gates": "success",
    "macOS ARM64 app and DMG": "success",
    "Windows x64 MSI and NSIS": "success",
    "Publish verified GitHub prerelease": "failure",
}
EXPECTED_FAILED_RECOVERY_JOB_IDS = {
    "Validate immutable release source": 93627558806,
    "Release quality gates": 93627597989,
    "macOS ARM64 app and DMG": 93627597999,
    "Windows x64 MSI and NSIS": 93627598006,
    "Publish verified GitHub prerelease": 93637315893,
}
EXPECTED_FAILED_RECOVERY_PUBLISH_STEPS = (
    ("Set up job", "completed", "success"),
    (
        f"Run actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "completed",
        "success",
    ),
    ("Assert exact checked-out SHA", "completed", "success"),
    ("Initialize isolated publish root", "completed", "success"),
    ("Install Minisign verifier", "completed", "success"),
    ("Download macOS artifact", "completed", "success"),
    ("Download Windows artifact", "completed", "success"),
    ("Stage and verify the exact release payload", "completed", "success"),
    ("Verify updater signatures against embedded public key", "completed", "success"),
    ("Write and verify tag-specific updater manifest", "completed", "success"),
    ("Create and verify SHA256SUMS", "completed", "success"),
    ("Prepare release notes with provenance", "completed", "success"),
    ("Reassert exact source before draft mutation", "completed", "success"),
    ("Revalidate remote tag before draft mutation", "completed", "success"),
    ("Create or refresh draft prerelease", "completed", "failure"),
    ("Upload the exact payload to the draft", "completed", "skipped"),
    ("Verify draft target and exact assets", "completed", "skipped"),
    ("Revalidate remote tag before publication", "completed", "skipped"),
    ("Reassert exact source before publication", "completed", "skipped"),
    ("Publish verified prerelease", "completed", "skipped"),
    ("Verify public release through API and checksums", "completed", "skipped"),
    (
        f"Post Run actions/checkout@{PINNED_ACTIONS['actions/checkout']}",
        "completed",
        "success",
    ),
    ("Complete job", "completed", "success"),
)
EXPECTED_FAILED_RECOVERY_PUBLISH_STEP_NUMBERS = (*range(1, 22), 42, 43)
EXPECTED_FAILED_RECOVERY_ARTIFACTS = {
    f"opentake-windows-x64-{EXPECTED_FAILED_RECOVERY_SOURCE_SHA}": {
        "id": 9084161184,
        "size_in_bytes": 563738459,
        "digest": "sha256:fab634decf8ee1d74432bd003be49474c5790b97e45ad2c81d23aa3bc58613db",
    },
    f"opentake-macos-arm64-{EXPECTED_FAILED_RECOVERY_SOURCE_SHA}": {
        "id": 9083298193,
        "size_in_bytes": 142002569,
        "digest": "sha256:2c5a1ab7f80e59f812b47a37e316c61bc7feedc7b85dd0e20f94a328086ca0b7",
    },
}


class ReleaseStateError(ValueError):
    """The remote release state is unsafe to create, refresh, or publish."""


class RemoteTagError(ValueError):
    """The remote tag advertisement is missing, ambiguous, or malformed."""


class RecoveryRunError(ValueError):
    """The requested failed-run recovery is not bound to a trusted tag push."""


def validate_recovery_run(
    run: dict[str, object],
    jobs: dict[str, object],
    comparison: dict[str, object],
    *,
    expected_run_id: int,
    expected_tag: str,
    expected_sha: str,
    expected_comparison_head_sha: str,
) -> None:
    """Validate one failed tag run before rebuilding its immutable source."""
    if expected_run_id <= 0:
        raise RecoveryRunError("recovery run ID must be positive")
    if re.fullmatch(r"[0-9a-f]{40}", expected_sha) is None:
        raise RecoveryRunError("recovery source SHA must be lowercase 40-hex")
    if re.fullmatch(r"[0-9a-f]{40}", expected_comparison_head_sha) is None:
        raise RecoveryRunError("recovery comparison head must be lowercase 40-hex")
    required_run_fields = {
        "id": expected_run_id,
        "name": "Release",
        "path": ".github/workflows/release.yml",
        "event": "push",
        "status": "completed",
        "conclusion": "failure",
        "head_branch": expected_tag,
        "head_sha": expected_sha,
    }
    if any(run.get(field) != value for field, value in required_run_fields.items()):
        raise RecoveryRunError("recovery run is not the failed exact-tag push")

    run_attempt = run.get("run_attempt")
    total_count = jobs.get("total_count")
    entries = jobs.get("jobs")
    expected_job_count = len(EXPECTED_RECOVERY_JOB_CONCLUSIONS)
    if (
        not isinstance(run_attempt, int)
        or isinstance(run_attempt, bool)
        or run_attempt <= 0
        or total_count != expected_job_count
        or not isinstance(entries, list)
        or len(entries) != expected_job_count
        or not all(isinstance(entry, dict) for entry in entries)
    ):
        raise RecoveryRunError("recovery job list is incomplete or malformed")
    names = [entry.get("name") for entry in entries]
    if (
        not all(isinstance(name, str) for name in names)
        or len(set(names)) != expected_job_count
        or set(names) != set(EXPECTED_RECOVERY_JOB_CONCLUSIONS)
    ):
        raise RecoveryRunError("recovery job set is not the exact failed release")
    by_name = {str(entry["name"]): entry for entry in entries}
    for name, conclusion in EXPECTED_RECOVERY_JOB_CONCLUSIONS.items():
        entry = by_name[name]
        expected_fields = {
            "run_id": expected_run_id,
            "run_attempt": run_attempt,
            "head_sha": expected_sha,
            "status": "completed",
            "conclusion": conclusion,
        }
        if any(entry.get(field) != value for field, value in expected_fields.items()):
            raise RecoveryRunError(
                f"recovery job outcome is not whitelisted: {name}"
            )

    windows_steps = by_name["Windows x64 MSI and NSIS"].get("steps")
    if not isinstance(windows_steps, list) or not all(
        isinstance(step, dict) for step in windows_steps
    ):
        raise RecoveryRunError("failed Windows job steps are missing or malformed")
    step_numbers = tuple(step.get("number") for step in windows_steps)
    step_outcomes = tuple(
        (step.get("name"), step.get("status"), step.get("conclusion"))
        for step in windows_steps
    )
    if (
        any(
            not isinstance(number, int) or isinstance(number, bool)
            for number in step_numbers
        )
        or step_numbers != EXPECTED_RECOVERY_WINDOWS_STEP_NUMBERS
        or step_outcomes != EXPECTED_RECOVERY_WINDOWS_STEPS
    ):
        raise RecoveryRunError(
            "Windows failure is not the exact checksum-pinned sidecar step"
        )

    base = comparison.get("base_commit")
    merge_base = comparison.get("merge_base_commit")
    commits = comparison.get("commits")
    total_commits = comparison.get("total_commits")
    ahead_by = comparison.get("ahead_by")
    behind_by = comparison.get("behind_by")
    if (
        comparison.get("status") != "ahead"
        or not isinstance(total_commits, int)
        or isinstance(total_commits, bool)
        or total_commits <= 0
        or not isinstance(ahead_by, int)
        or isinstance(ahead_by, bool)
        or ahead_by != total_commits
        or not isinstance(behind_by, int)
        or isinstance(behind_by, bool)
        or behind_by != 0
        or not isinstance(base, dict)
        or base.get("sha") != expected_sha
        or not isinstance(merge_base, dict)
        or merge_base.get("sha") != expected_sha
        or not isinstance(commits, list)
        or len(commits) != total_commits
        or not all(isinstance(commit, dict) for commit in commits)
        or commits[-1].get("sha") != expected_comparison_head_sha
    ):
        raise RecoveryRunError(
            "release source is not an ancestor of the approved predecessor tooling"
        )


def validate_failed_recovery_run(
    run: dict[str, object],
    jobs: dict[str, object],
    artifacts: dict[str, object],
    tooling_comparison: dict[str, object],
    *,
    expected_run_id: int,
    expected_tag: str,
    expected_source_sha: str,
    expected_tooling_sha: str,
    expected_current_tooling_sha: str,
) -> None:
    """Validate the one approved failed recovery before chaining another run."""
    if (
        expected_run_id != EXPECTED_FAILED_RECOVERY_RUN_ID
        or expected_tag != EXPECTED_FAILED_RECOVERY_TAG
        or expected_source_sha != EXPECTED_FAILED_RECOVERY_SOURCE_SHA
        or expected_tooling_sha != EXPECTED_FAILED_RECOVERY_TOOLING_SHA
    ):
        raise RecoveryRunError("failed recovery is not on the approved Beta 4 chain")
    if (
        re.fullmatch(r"[0-9a-f]{40}", expected_current_tooling_sha) is None
        or expected_current_tooling_sha == expected_tooling_sha
    ):
        raise RecoveryRunError("current recovery tooling SHA is invalid")

    required_run_fields = {
        "id": EXPECTED_FAILED_RECOVERY_RUN_ID,
        "workflow_id": 330325373,
        "name": "Release",
        "path": ".github/workflows/release.yml",
        "event": "workflow_dispatch",
        "status": "completed",
        "conclusion": "failure",
        "head_branch": "main",
        "head_sha": EXPECTED_FAILED_RECOVERY_TOOLING_SHA,
        "run_attempt": 1,
    }
    if any(run.get(field) != value for field, value in required_run_fields.items()):
        raise RecoveryRunError("failed recovery run is not the approved exact run")
    if isinstance(run.get("run_attempt"), bool):
        raise RecoveryRunError("failed recovery run attempt must be an integer")
    expected_repository = {
        "id": 1275692189,
        "full_name": "appergb/OpenTake",
        "private": False,
    }
    for field in ("repository", "head_repository"):
        repository = run.get(field)
        if (
            not isinstance(repository, dict)
            or repository.get("id") != expected_repository["id"]
            or repository.get("full_name") != expected_repository["full_name"]
            or repository.get("private") is not False
        ):
            raise RecoveryRunError(
                f"failed recovery {field} provenance is not exact"
            )

    total_count = jobs.get("total_count")
    entries = jobs.get("jobs")
    expected_job_count = len(EXPECTED_FAILED_RECOVERY_JOB_CONCLUSIONS)
    if (
        total_count != expected_job_count
        or not isinstance(entries, list)
        or len(entries) != expected_job_count
        or not all(isinstance(entry, dict) for entry in entries)
    ):
        raise RecoveryRunError("failed recovery job list is incomplete or malformed")
    names = [entry.get("name") for entry in entries]
    if (
        not all(isinstance(name, str) for name in names)
        or len(set(names)) != expected_job_count
        or set(names) != set(EXPECTED_FAILED_RECOVERY_JOB_CONCLUSIONS)
    ):
        raise RecoveryRunError("failed recovery job set is not exact")
    by_name = {str(entry["name"]): entry for entry in entries}
    for name, conclusion in EXPECTED_FAILED_RECOVERY_JOB_CONCLUSIONS.items():
        entry = by_name[name]
        expected_fields = {
            "id": EXPECTED_FAILED_RECOVERY_JOB_IDS[name],
            "run_id": EXPECTED_FAILED_RECOVERY_RUN_ID,
            "run_attempt": 1,
            "head_sha": EXPECTED_FAILED_RECOVERY_TOOLING_SHA,
            "status": "completed",
            "conclusion": conclusion,
        }
        if isinstance(entry.get("run_attempt"), bool) or any(
            entry.get(field) != value for field, value in expected_fields.items()
        ):
            raise RecoveryRunError(
                f"failed recovery job outcome is not whitelisted: {name}"
            )

    publish_steps = by_name["Publish verified GitHub prerelease"].get("steps")
    if not isinstance(publish_steps, list) or not all(
        isinstance(step, dict) for step in publish_steps
    ):
        raise RecoveryRunError(
            "failed recovery publish steps are missing or malformed"
        )
    step_numbers = tuple(step.get("number") for step in publish_steps)
    step_outcomes = tuple(
        (step.get("name"), step.get("status"), step.get("conclusion"))
        for step in publish_steps
    )
    if (
        any(
            not isinstance(number, int) or isinstance(number, bool)
            for number in step_numbers
        )
        or step_numbers != EXPECTED_FAILED_RECOVERY_PUBLISH_STEP_NUMBERS
        or step_outcomes != EXPECTED_FAILED_RECOVERY_PUBLISH_STEPS
    ):
        raise RecoveryRunError(
            "failed recovery is not the exact draft-creation permission failure"
        )

    artifact_total_count = artifacts.get("total_count")
    artifact_entries = artifacts.get("artifacts")
    if (
        not isinstance(artifact_total_count, int)
        or isinstance(artifact_total_count, bool)
        or artifact_total_count != len(EXPECTED_FAILED_RECOVERY_ARTIFACTS)
        or not isinstance(artifact_entries, list)
        or len(artifact_entries) != len(EXPECTED_FAILED_RECOVERY_ARTIFACTS)
        or not all(isinstance(entry, dict) for entry in artifact_entries)
    ):
        raise RecoveryRunError("failed recovery artifact list is not exact")
    artifact_names = [entry.get("name") for entry in artifact_entries]
    if (
        not all(isinstance(name, str) for name in artifact_names)
        or len(set(artifact_names)) != len(EXPECTED_FAILED_RECOVERY_ARTIFACTS)
        or set(artifact_names) != set(EXPECTED_FAILED_RECOVERY_ARTIFACTS)
    ):
        raise RecoveryRunError("failed recovery artifact set is not exact")
    artifacts_by_name = {str(entry["name"]): entry for entry in artifact_entries}
    expected_artifact_run = {
        "id": EXPECTED_FAILED_RECOVERY_RUN_ID,
        "repository_id": 1275692189,
        "head_repository_id": 1275692189,
        "head_branch": "main",
        "head_sha": EXPECTED_FAILED_RECOVERY_TOOLING_SHA,
    }
    for name, expected_fields in EXPECTED_FAILED_RECOVERY_ARTIFACTS.items():
        artifact = artifacts_by_name[name]
        artifact_run = artifact.get("workflow_run")
        if (
            artifact.get("expired") is not False
            or any(
                artifact.get(field) != value
                for field, value in expected_fields.items()
            )
            or not isinstance(artifact_run, dict)
            or any(
                artifact_run.get(field) != value
                for field, value in expected_artifact_run.items()
            )
        ):
            raise RecoveryRunError(
                f"failed recovery artifact provenance is not exact: {name}"
            )

    base = tooling_comparison.get("base_commit")
    merge_base = tooling_comparison.get("merge_base_commit")
    commits = tooling_comparison.get("commits")
    total_commits = tooling_comparison.get("total_commits")
    ahead_by = tooling_comparison.get("ahead_by")
    behind_by = tooling_comparison.get("behind_by")
    if (
        tooling_comparison.get("status") != "ahead"
        or not isinstance(total_commits, int)
        or isinstance(total_commits, bool)
        or total_commits <= 0
        or not isinstance(ahead_by, int)
        or isinstance(ahead_by, bool)
        or ahead_by != total_commits
        or not isinstance(behind_by, int)
        or isinstance(behind_by, bool)
        or behind_by != 0
        or not isinstance(base, dict)
        or base.get("sha") != EXPECTED_FAILED_RECOVERY_TOOLING_SHA
        or not isinstance(merge_base, dict)
        or merge_base.get("sha") != EXPECTED_FAILED_RECOVERY_TOOLING_SHA
        or not isinstance(commits, list)
        or len(commits) != total_commits
        or not all(isinstance(commit, dict) for commit in commits)
        or commits[-1].get("sha") != expected_current_tooling_sha
    ):
        raise RecoveryRunError(
            "approved failed-recovery tooling is not an ancestor of current main"
        )


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


def resolve_public_release_state(
    payload: dict[str, object], expected_tag: str, expected_sha: str
) -> dict[str, object]:
    """Validate one published GraphQL release against its immutable tag commit."""
    if re.fullmatch(r"[0-9a-f]{40}", expected_sha) is None:
        raise ReleaseStateError("expected source SHA must be lowercase 40-hex")
    if payload.get("errors"):
        raise ReleaseStateError("GraphQL public release lookup returned errors")
    data = payload.get("data")
    repository = data.get("repository") if isinstance(data, dict) else None
    release = repository.get("release") if isinstance(repository, dict) else None
    if not isinstance(release, dict):
        raise ReleaseStateError("GraphQL public release lookup is malformed")
    if release.get("tagName") != expected_tag:
        raise ReleaseStateError("GraphQL public release tag does not match")
    tag_commit = release.get("tagCommit")
    target = tag_commit.get("oid") if isinstance(tag_commit, dict) else None
    if not isinstance(target, str) or target.lower() != expected_sha:
        raise ReleaseStateError(
            "GraphQL public release tag commit does not match source SHA"
        )
    if release.get("isDraft") is not False or release.get("isPrerelease") is not True:
        raise ReleaseStateError("GraphQL release is not a published prerelease")
    release_id = release.get("databaseId")
    if (
        not isinstance(release_id, int)
        or isinstance(release_id, bool)
        or release_id <= 0
    ):
        raise ReleaseStateError("GraphQL public release has no numeric database ID")
    connection = release.get("releaseAssets")
    if not isinstance(connection, dict):
        raise ReleaseStateError("GraphQL public release has no asset connection")
    page_info = connection.get("pageInfo")
    if not isinstance(page_info, dict) or page_info.get("hasNextPage") is not False:
        raise ReleaseStateError("GraphQL public release asset list is incomplete")
    nodes = connection.get("nodes")
    if not isinstance(nodes, list) or len(nodes) != 17:
        raise ReleaseStateError("GraphQL public release must contain 17 assets")

    asset_node_ids: list[str] = []
    asset_names: list[str] = []
    asset_sizes: list[int] = []
    for asset in nodes:
        if not isinstance(asset, dict):
            raise ReleaseStateError("GraphQL public release asset is malformed")
        asset_id = asset.get("id")
        name = asset.get("name")
        size = asset.get("size")
        if not isinstance(asset_id, str) or not asset_id:
            raise ReleaseStateError("GraphQL public release asset has no node ID")
        if not isinstance(name, str) or not name:
            raise ReleaseStateError("GraphQL public release asset has no name")
        if not isinstance(size, int) or isinstance(size, bool) or size <= 0:
            raise ReleaseStateError("GraphQL public release asset has invalid size")
        asset_node_ids.append(asset_id)
        asset_names.append(name)
        asset_sizes.append(size)
    if len(set(asset_node_ids)) != 17 or len(set(asset_names)) != 17:
        raise ReleaseStateError("GraphQL public release assets are not unique")
    return {
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
    failed_run_input = (
        _as_mapping(inputs.get("failed_run_id")) if inputs is not None else None
    )
    failed_recovery_run_input = (
        _as_mapping(inputs.get("failed_recovery_run_id"))
        if inputs is not None
        else None
    )
    if (
        push != {"tags": ["v*"]}
        or dispatch is None
        or set(dispatch) != {"inputs"}
        or inputs is None
        or set(inputs) != {"tag", "failed_run_id", "failed_recovery_run_id"}
        or tag_input is None
        or set(tag_input) != {"description", "required", "type"}
        or tag_input.get("required") is not True
        or tag_input.get("type") != "string"
        or failed_run_input is None
        or set(failed_run_input) != {"description", "required", "type"}
        or failed_run_input.get("description")
        != "Root failed tag-push Release run ID (31412976593) for the immutable source"
        or failed_run_input.get("required") is not True
        or failed_run_input.get("type") != "string"
        or failed_recovery_run_input is None
        or set(failed_recovery_run_input) != {"description", "required", "type"}
        or failed_recovery_run_input.get("description")
        != "Direct predecessor workflow_dispatch Release run ID (31441693191) whose publish job failed for the same immutable source"
        or failed_recovery_run_input.get("required") is not True
        or failed_recovery_run_input.get("type") != "string"
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
    if any("raw.githubusercontent.com" in value for value in scalar_strings):
        errors.append("release tooling never trusts raw HTTP downloads")
    signing_env = {
        "TAURI_SIGNING_PRIVATE_KEY": "${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD": "${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}",
    }
    secret_steps = {
        ("macos_arm64", "Require updater signing secrets"),
        ("macos_arm64", "Build ad-hoc Tauri app, DMG, and signed updater"),
        ("macos_arm64", "Create and sign macOS updater attestation"),
        ("windows_x64", "Require updater signing secrets"),
        ("windows_x64", "Build native MSI, NSIS, and signed updater artifacts"),
        ("windows_x64", "Create and sign Windows updater attestations"),
    }
    seen_secret_steps: set[tuple[str, str]] = set()
    secret_scope_valid = True
    for job_name, job in structured_jobs.items():
        if job is None:
            continue
        for step in _as_steps(job.get("steps")) or []:
            step_name = step.get("name")
            env = _as_mapping(step.get("env"))
            secret_values = (
                []
                if env is None
                else [value for value in env.values() if "secrets." in str(value)]
            )
            if secret_values:
                key = (job_name, str(step_name))
                if key not in secret_steps or env != signing_env:
                    secret_scope_valid = False
                else:
                    seen_secret_steps.add(key)
    actual_secret_scalars = sorted(
        value for value in scalar_strings if "secrets." in value
    )
    expected_secret_scalars = sorted(
        [signing_env["TAURI_SIGNING_PRIVATE_KEY"]] * 6
        + [signing_env["TAURI_SIGNING_PRIVATE_KEY_PASSWORD"]] * 6
    )
    if not secret_scope_valid or actual_secret_scalars != expected_secret_scalars:
        errors.append("updater signing secrets limited to guard and build steps")
    guard_lines = (
        'test -n "${TAURI_SIGNING_PRIVATE_KEY:-}"',
        'test -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"',
    )
    guards_valid = True
    for job, name in secret_steps:
        if name != "Require updater signing secrets":
            continue
        guard = _structured_step(structured_jobs[job], name)
        if (
            guard is None
            or _as_mapping(guard.get("env")) != signing_env
            or not _has_code_lines(guard, guard_lines)
        ):
            guards_valid = False
    if seen_secret_steps != secret_steps or not guards_valid:
        errors.append("updater signing secrets fail closed")

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
    validate_env = _as_mapping(validate.get("env")) if validate is not None else None
    validate_outputs = (
        _as_mapping(validate.get("outputs")) if validate is not None else None
    )
    recovery_lines = (
        'tooling_sha="$(printf \'%s\' "$RELEASE_TOOLING_SHA" | tr \'[:upper:]\' \'[:lower:]\')"',
        '[[ "$tooling_sha" =~ ^[0-9a-f]{40}$ ]]',
        'if [[ "$GITHUB_EVENT_NAME" = "push" ]]; then',
        'test -z "$FAILED_RUN_ID"',
        'test -z "$FAILED_RECOVERY_RUN_ID"',
        'test "$tooling_sha" = "$source_sha"',
        'elif [[ "$GITHUB_EVENT_NAME" = "workflow_dispatch" ]]; then',
        '[[ "$FAILED_RUN_ID" =~ ^[1-9][0-9]*$ ]]',
        '[[ "$FAILED_RECOVERY_RUN_ID" =~ ^[1-9][0-9]*$ ]]',
        'test "$FAILED_RUN_ID" = "31412976593"',
        'test "$FAILED_RECOVERY_RUN_ID" = "31441693191"',
        'test "$tooling_sha" = "$remote_main"',
        'predecessor_tooling_sha="924bc1102a9343e14c3beea2a3622b5d92ebff13"',
        'if ! git cat-file -e "${tooling_sha}^{commit}" 2>/dev/null; then',
        'git fetch --no-tags --depth=1 origin "$tooling_sha"',
        'test "$(git rev-parse "${tooling_sha}^{commit}")" = "$tooling_sha"',
        'git cat-file blob "$tooling_sha:scripts/check_release_workflow.py" \\',
        'git cat-file blob "$tooling_sha:scripts/workflow_yaml.py" \\',
        'gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RUN_ID" \\',
        'gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RUN_ID/jobs?per_page=100" \\',
        'gh api "repos/$GITHUB_REPOSITORY/compare/$source_sha...$predecessor_tooling_sha" \\',
        'validate-recovery-run \\',
        '--run-id "$FAILED_RUN_ID" \\',
        '--tag "$RELEASE_TAG" \\',
        '--sha "$source_sha" \\',
        '--comparison-head-sha "$predecessor_tooling_sha"',
        'gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RECOVERY_RUN_ID" \\',
        'gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RECOVERY_RUN_ID/jobs?per_page=100" \\',
        'gh api "repos/$GITHUB_REPOSITORY/actions/runs/$FAILED_RECOVERY_RUN_ID/artifacts?per_page=100" \\',
        'gh api "repos/$GITHUB_REPOSITORY/compare/$predecessor_tooling_sha...$remote_main" \\',
        'validate-failed-recovery-run \\',
        '--run-id "$FAILED_RECOVERY_RUN_ID" \\',
        '--source-sha "$source_sha" \\',
        '--tooling-sha "$predecessor_tooling_sha" \\',
        '--current-tooling-sha "$tooling_sha"',
        'printf \'tooling_sha=%s\\n\' "$tooling_sha" >> "$GITHUB_OUTPUT"',
    )
    recovery_provenance = (
        validate_env
        == {
            "RELEASE_TAG": "${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref_name }}",
            "FAILED_RUN_ID": "${{ github.event_name == 'workflow_dispatch' && inputs.failed_run_id || '' }}",
            "FAILED_RECOVERY_RUN_ID": "${{ github.event_name == 'workflow_dispatch' && inputs.failed_recovery_run_id || '' }}",
            "RELEASE_TOOLING_SHA": "${{ github.workflow_sha }}",
        }
        and validate_outputs is not None
        and validate_outputs.get("tooling_sha")
        == "${{ steps.bind.outputs.tooling_sha }}"
        and bind is not None
        and _as_mapping(bind.get("env")) == {"GH_TOKEN": "${{ github.token }}"}
        and _has_code_lines(bind, recovery_lines)
    )
    if not recovery_provenance:
        errors.append("failed-run recovery provenance")
    if not _has_code_lines(
        bind,
        (
            'cargo = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))',
            'tauri = json.loads(Path("src-tauri/tauri.conf.json").read_text(encoding="utf-8"))',
            'web = json.loads(Path("web/package.json").read_text(encoding="utf-8"))',
            'notes = Path("docs/releases") / f"{version}.md"',
            'event_name = os.environ["GITHUB_EVENT_NAME"]',
            'if event_name == "workflow_dispatch":',
            'expected_version = "1.0.0-beta.4"',
            'expected_wix_version = "1.0.0.4"',
            'expected_version = "1.0.0-beta.5"',
            'expected_wix_version = "1.0.0.5"',
            'if versions != {version}:',
            'if version != expected_version:',
            'wix_version = tauri["bundle"]["windows"]["wix"]["version"]',
            'if wix_version != expected_wix_version:',
        ),
    ):
        errors.append("Cargo, Tauri, and Web versions match tag")
    if not _has_code_lines(
        bind,
        (
            'if "+" in tag:',
            'raise SystemExit("SemVer build metadata is unsupported for updater asset URLs")',
            'if SEMVER_RE.fullmatch(tag) is None:',
            'if version == "1.0.0-beta.5" and not prerelease:',
            'emit("prerelease", "true")',
        ),
    ):
        errors.append("SemVer build metadata is unsupported")
    if not _has_code_lines(bind, validate_order[1:5]):
        errors.append("validate binds exact clean checkout")

    required_jobs = ("quality", "macos_arm64", "windows_x64")
    validate_permissions = (
        _as_mapping(validate.get("permissions")) if validate is not None else None
    )
    if validate_permissions != {"actions": "read", "contents": "read"}:
        errors.append("validate-only Actions read permission")
    for name in required_jobs:
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
        ("Validate Windows and release workflow contracts", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_write_updater_attestation.py")),
        ("Validate Windows and release workflow contracts", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_write_updater_manifest.py")),
        ("Provisioner unit tests", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts/tests", "-p", "test_*.py")),
        ("Rust formatting", ("cargo", "fmt", "--all", "--check")),
        ("Rust workspace clippy", ("cargo", "clippy", "--workspace", "--all-targets", "--", "-D", "warnings")),
        ("Rust workspace tests", ("cargo", "test", "--workspace", "--", "--test-threads=1")),
        ("Live playback transport integration", ("cargo", "test", "-p", "opentake-tauri", "--features", "playback-engine", "--test", "playback_transport_integration", "--", "--test-threads=1")),
        ("Minimal-feature Tauri clippy", ("cargo", "clippy", "-p", "opentake-tauri", "--no-default-features", "--all-targets", "--", "-D", "warnings")),
        ("Install locked Web dependencies", ("pnpm", "-C", "web", "install", "--frozen-lockfile")),
        ("Validate Web dependency licenses", ("python3", "-B", "-m", "unittest", "discover", "-s", "scripts", "-p", "test_check_license_inventory.py")),
        ("Validate Web dependency licenses", ("python3", "-B", "scripts/check_license_inventory.py")),
        ("Web editor behavior suite", ("pnpm", "-C", "web", "test")),
        ("Web production build", ("pnpm", "-C", "web", "build")),
    )
    quality_ok = quality is not None and quality.get("runs-on") == "ubuntu-latest"
    quality_ok = quality_ok and all(
        _has_command(_structured_step(quality, step_name), command)
        for step_name, command in quality_commands
    )
    license_step = _structured_step(quality, "Validate Web dependency licenses")
    quality_ok = quality_ok and _has_code_lines(
        license_step,
        (
            'case "$OPENTAKE_EXPECTED_RELEASE_VERSION" in',
            "1.0.0-beta.5)",
            "python3 -B -m unittest discover -s scripts -p 'test_check_license_inventory.py'",
            "python3 -B scripts/check_license_inventory.py",
            "1.0.0-beta.4)",
            'node -e \'const d=require("./web/package.json").dependencies??{}; if(Object.keys(d).some((name)=>name==="codemirror"||name.startsWith("@codemirror/"))) process.exit(1)\'',
        ),
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

    quality_env = _as_mapping(quality.get("env")) if quality is not None else None
    quality_release_contract = _structured_step(
        quality, "Validate Windows and release workflow contracts"
    )
    running_contract_provenance = (
        quality_env is not None
        and quality_env.get("RELEASE_TOOLING_SHA")
        == "${{ needs.validate.outputs.tooling_sha }}"
        and quality_env.get("OPENTAKE_EXPECTED_RELEASE_VERSION")
        == "${{ needs.validate.outputs.version }}"
        and _has_code_lines(
            quality_release_contract,
            (
                'if ! git cat-file -e "${RELEASE_TOOLING_SHA}^{commit}" 2>/dev/null; then',
                'git fetch --no-tags --depth=1 origin "$RELEASE_TOOLING_SHA"',
                'test "$(git rev-parse "${RELEASE_TOOLING_SHA}^{commit}")" = "$RELEASE_TOOLING_SHA"',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/check_release_workflow.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/test_check_release_workflow.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/workflow_yaml.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/provision_ffmpeg_sidecars.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/tests/test_provision_ffmpeg_sidecars.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:.github/workflows/release.yml" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:docs/releases/1.0.0-beta.5.md" \\',
                'OPENTAKE_REPOSITORY_ROOT="$GITHUB_WORKSPACE" \\',
                'OPENTAKE_RELEASE_WORKFLOW_PATH="$tooling_root/release.yml" \\',
                'OPENTAKE_RELEASE_NOTES_PATH="$tooling_root/release-notes.md" \\',
                'PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$tooling_root" \\',
                'python3 -B "$tooling_root/check_release_workflow.py"',
                'python3 -B -m unittest discover -s "$tooling_root" \\',
                "-p 'test_check_release_workflow.py'",
            ),
        )
    )
    if not running_contract_provenance:
        errors.append("exact release tooling provenance")
    provisioner_tests = _structured_step(quality, "Provisioner unit tests")
    recovery_provisioner_tests = _has_code_lines(
        provisioner_tests,
        (
            "python3 -B -m unittest discover -s scripts/tests -p 'test_*.py'",
            'OPENTAKE_REPOSITORY_ROOT="$GITHUB_WORKSPACE" \\',
            'OPENTAKE_PROVISIONER_PATH="$tooling_root/provision_ffmpeg_sidecars.py" \\',
            'PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$tooling_root" \\',
            'python3 -B -m unittest discover \\',
            '-s "$tooling_root" -p \'test_provision_ffmpeg_sidecars.py\'',
        ),
    )
    if not recovery_provisioner_tests:
        errors.append("recovery provisioner tests")

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
    mac_build = _structured_step(
        macos, "Build ad-hoc Tauri app, DMG, and signed updater"
    )
    mac_verify = _structured_step(
        macos, "Verify complete app, sidecars, DMG, and signed updater"
    )
    mac_attestation = _structured_step(
        macos, "Create and sign macOS updater attestation"
    )
    mac_receipt = _structured_step(macos, "Create macOS exact-SHA receipt")
    mac_uploads = _action_step(macos, "actions/upload-artifact")
    mac_ok = (
        macos is not None
        and macos.get("runs-on") == "macos-14"
        and _as_mapping(macos.get("env", {})).get("APPLE_SIGNING_IDENTITY") == "-"
        and mac_build is not None
        and _has_command(_structured_step(macos, "Provision checksum-pinned ARM64 FFmpeg sidecars"), ("python3", "scripts/provision_ffmpeg_sidecars.py", "--target", "aarch64-apple-darwin"))
        and _has_command(_structured_step(macos, "Install locked Web dependencies"), ("pnpm", "-C", "web", "install", "--frozen-lockfile"))
        and _as_mapping(mac_build.get("env")) == signing_env
        and _has_command(mac_build, ("./web/node_modules/.bin/tauri", "build", "--ci", "--target", "aarch64-apple-darwin", "--bundles", "app,dmg", "--config", '{"bundle":{"createUpdaterArtifacts":true,"macOS":{"signingIdentity":"-"}}}'))
        and _has_command(mac_verify, ("codesign", "--verify", "--deep", "--strict", "--verbose=2", "$app"))
        and _has_command(mac_verify, ("hdiutil", "verify", "$dmg"))
        and _has_command(mac_verify, ("hdiutil", "attach", "$dmg", "-nobrowse", "-readonly", "-mountpoint", "$mountpoint"))
        and _has_command(mac_verify, ("ruby", "scripts/tests/packaged-sidecars-test.rb", "--name", "packaged_macos_windows_sidecars_resolve_and_execute", "--package", "$mounted_app"))
        and _has_code_lines(mac_verify, ("codesign -dv --verbose=4 \"$app\" 2>&1 | grep -F 'Signature=adhoc'", 'test "$updater_signature" = "$updater.sig"', 'test -s "$updater"', 'test -s "$updater_signature"'))
        and mac_attestation is not None
        and _as_mapping(mac_attestation.get("env")) == signing_env
        and _has_command(
            mac_attestation,
            (
                "python3", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$RELEASE_TAG",
                "--version", "$RELEASE_VERSION",
                "--source-sha", "$TARGET_SHA",
                "--platform", "darwin-aarch64",
                "--artifact", "$updater",
                "--output", "$attestation",
            ),
        )
        and _has_command(
            mac_attestation,
            ("./web/node_modules/.bin/tauri", "signer", "sign", "$attestation"),
        )
        and _has_code_lines(
            mac_attestation,
            ('attestation="$updater.attestation.json"', 'test -s "$attestation.sig"'),
        )
        and _has_code_lines(mac_receipt, ('"schema": "opentake-macos-arm64-receipt-v2",', '"source_sha": os.environ["RECEIPT_SHA"],', '"platform_signing_mode": "ad-hoc",', '"updater_signature_mode": "tauri-minisign",', '"sha256": sha256(artifact),', '"bytes": artifact.stat().st_size,'))
        and _has_code_lines(
            mac_receipt,
            (
                'if attestations[0] != Path(f"{updaters[0]}.attestation.json"):',
                'if attestation_signatures[0] != Path(f"{attestations[0]}.sig"):',
            ),
        )
        and len(mac_uploads) == 1
        and mac_uploads[0][1].get("uses") == f"actions/upload-artifact@{PINNED_ACTIONS['actions/upload-artifact']}"
    )
    if not mac_ok:
        errors.append("complete ad-hoc macOS ARM64 bundle gate")

    windows = structured_jobs["windows_x64"]
    windows_build = _structured_step(
        windows, "Build native MSI, NSIS, and signed updater artifacts"
    )
    windows_install = _structured_step(
        windows,
        "Install NSIS and smoke installed app, sidecars, and updater artifacts",
    )
    windows_attestation = _structured_step(
        windows, "Create and sign Windows updater attestations"
    )
    windows_receipt = _structured_step(windows, "Create Windows exact-SHA receipt")
    windows_uploads = _action_step(windows, "actions/upload-artifact")
    windows_commands = (
        ("Provision checksum-pinned Windows FFmpeg sidecars", ("python", "$provisioner", "--target", "x86_64-pc-windows-msvc")),
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
    windows_config_lines = (
        "$ErrorActionPreference = 'Stop'",
        "if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) { throw 'RUNNER_TEMP is required' }",
        "$runnerTemp = [System.IO.Path]::GetFullPath($env:RUNNER_TEMP)",
        '$configPath = Join-Path $env:RUNNER_TEMP "opentake-windows-updater-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT.json"',
        "$configPath = [System.IO.Path]::GetFullPath($configPath)",
        "if (-not [System.IO.Path]::IsPathFullyQualified($configPath)) { throw 'Tauri config path is not absolute' }",
        "if (-not [string]::Equals([System.IO.Path]::GetDirectoryName($configPath), $runnerTemp, [System.StringComparison]::OrdinalIgnoreCase)) {",
        "if (Test-Path -LiteralPath $configPath) { throw 'Tauri config path already exists' }",
        "$configJson = '{\"bundle\":{\"createUpdaterArtifacts\":true}}'",
        "$utf8NoBom = [System.Text.UTF8Encoding]::new($false)",
        "[System.IO.File]::WriteAllText($configPath, $configJson, $utf8NoBom)",
        "$configItem = Get-Item -LiteralPath $configPath -Force",
        "if ($configItem.PSIsContainer -or (($configItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {",
        "$configBytes = [System.IO.File]::ReadAllBytes($configPath)",
        "if ($configBytes.Length -ne $utf8NoBom.GetByteCount($configJson)) { throw 'Tauri config is not exact UTF-8 without BOM' }",
        "$configText = [System.IO.File]::ReadAllText($configPath, [System.Text.Encoding]::UTF8)",
        "if ($configText -cne $configJson) { throw 'Tauri config content changed' }",
        "$parsedConfig = [System.IO.File]::ReadAllText($configPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json -AsHashtable",
        "if (@($parsedConfig.Keys).Count -ne 1 -or -not ($parsedConfig.Keys -ccontains 'bundle')) { throw 'Tauri config root is not exact' }",
        "$bundleConfig = $parsedConfig['bundle']",
        "if (-not ($bundleConfig -is [System.Collections.IDictionary]) -or @($bundleConfig.Keys).Count -ne 1 -or -not ($bundleConfig.Keys -ccontains 'createUpdaterArtifacts')) {",
        "if (-not ($bundleConfig['createUpdaterArtifacts'] -is [bool]) -or $bundleConfig['createUpdaterArtifacts'] -ne $true) {",
        "$tauriArguments = @(",
        "'build'",
        "'--ci'",
        "'--bundles'",
        "'msi,nsis'",
        "'--config'",
        "$configPath",
        "& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments",
        "$tauriExitCode = $LASTEXITCODE",
        "Remove-Item -LiteralPath $configPath -Force",
        "if ($tauriExitCode -ne 0) { exit $tauriExitCode }",
    )
    windows_build_script = _run_script(windows_build)
    windows_ok = (
        windows_ok
        and _has_code_lines(windows_build, windows_config_lines)
        and windows_build_script.count(
            "& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments"
        )
        == 1
        and "--config '{" not in windows_build_script
    )
    windows_ok = windows_ok and windows_build is not None and _as_mapping(
        windows_build.get("env")
    ) == signing_env
    windows_ok = (
        windows_ok
        and windows_attestation is not None
        and _as_mapping(windows_attestation.get("env")) == signing_env
        and _has_command(
            windows_attestation,
            (
                "python", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$env:RELEASE_TAG",
                "--version", "$env:RELEASE_VERSION",
                "--source-sha", "$env:TARGET_SHA",
                "--platform", "windows-x86_64-msi",
                "--artifact", "$msi[0].FullName",
                "--output", "$msiAttestation",
            ),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            ("&", ".\\web\\node_modules\\.bin\\tauri.cmd", "signer", "sign", "$msiAttestation"),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            (
                "python", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$env:RELEASE_TAG",
                "--version", "$env:RELEASE_VERSION",
                "--source-sha", "$env:TARGET_SHA",
                "--platform", "windows-x86_64-nsis",
                "--artifact", "$nsis[0].FullName",
                "--output", "$nsisAttestation",
            ),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            ("&", ".\\web\\node_modules\\.bin\\tauri.cmd", "signer", "sign", "$nsisAttestation"),
            powershell=True,
        )
    )
    windows_ok = windows_ok and _has_code_lines(
        windows_install,
        (
            "if ($msi.Count -ne 1) { throw 'expected exactly one MSI installer' }",
            "if ($installer.Count -ne 1) { throw 'expected exactly one NSIS installer' }",
            "if ($msiSignature.Count -ne 1) { throw 'expected exactly one MSI updater signature' }",
            "if ($nsisSignature.Count -ne 1) { throw 'expected exactly one NSIS updater signature' }",
            "$app = Start-Process -FilePath $application -PassThru",
            "--name packaged_macos_windows_sidecars_resolve_and_execute `",
        ),
    )
    windows_ok = windows_ok and _has_code_lines(
        windows_receipt,
        (
            "schema = 'opentake-windows-release-receipt-v2'",
            "source_sha = $env:RECEIPT_SHA",
            "platform_signing_mode = 'unsigned-authenticode'",
            "updater_signature_mode = 'tauri-minisign'",
            "sha256 = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()",
            "bytes = $_.Length",
            "if ($msiAttestation[0].FullName -ne \"$($msi[0].FullName).attestation.json\") { throw 'MSI attestation is not the installer companion' }",
            "if ($msiAttestationSignature[0].FullName -ne \"$($msiAttestation[0].FullName).sig\") { throw 'MSI attestation signature is not its companion' }",
            "if ($nsisAttestation[0].FullName -ne \"$($nsis[0].FullName).attestation.json\") { throw 'NSIS attestation is not the installer companion' }",
            "if ($nsisAttestationSignature[0].FullName -ne \"$($nsisAttestation[0].FullName).sig\") { throw 'NSIS attestation signature is not its companion' }",
        ),
    )
    windows_ok = windows_ok and len(windows_uploads) == 1 and windows_uploads[0][1].get(
        "uses"
    ) == f"actions/upload-artifact@{PINNED_ACTIONS['actions/upload-artifact']}"
    if not windows_ok:
        errors.append("complete Windows x64 installer gate")
    windows_env = _as_mapping(windows.get("env")) if windows is not None else None
    windows_provision = _structured_step(
        windows, "Provision checksum-pinned Windows FFmpeg sidecars"
    )
    windows_tooling_provenance = (
        windows_env is not None
        and windows_env.get("RELEASE_TOOLING_SHA")
        == "${{ needs.validate.outputs.tooling_sha }}"
        and windows_provision is not None
        and windows_provision.get("shell") == "pwsh"
        and _has_code_lines(
            windows_provision,
            (
                "if ($env:RELEASE_TOOLING_SHA -notmatch '^[0-9a-f]{40}$') {",
                'git cat-file -e "$($env:RELEASE_TOOLING_SHA)^{commit}" 2>$null',
                'git fetch --no-tags --depth=1 origin $env:RELEASE_TOOLING_SHA',
                '$resolvedTooling = (git rev-parse "$($env:RELEASE_TOOLING_SHA)^{commit}").Trim().ToLowerInvariant()',
                'git cat-file blob "$($env:RELEASE_TOOLING_SHA):scripts/provision_ffmpeg_sidecars.py" > $provisioner',
                '$env:OPENTAKE_REPOSITORY_ROOT = $env:GITHUB_WORKSPACE',
                'python $provisioner --target x86_64-pc-windows-msvc',
            ),
        )
        and not any(
            "raw.githubusercontent.com" in value
            for value in _all_scalar_strings(windows_provision)
        )
    )
    if not windows_tooling_provenance:
        errors.append("exact release tooling provenance")

    mac_upload_with = (
        _with_mapping(mac_uploads[0][1]) if len(mac_uploads) == 1 else None
    )
    windows_upload_with = (
        _with_mapping(windows_uploads[0][1]) if len(windows_uploads) == 1 else None
    )
    expected_mac_upload = {
        "name": "opentake-macos-arm64-${{ needs.validate.outputs.source_sha }}",
        "path": (
            "target/aarch64-apple-darwin/release/bundle/dmg/*.dmg\n"
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz\n"
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.sig\n"
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.attestation.json\n"
            "target/aarch64-apple-darwin/release/bundle/macos/*.app.tar.gz.attestation.json.sig\n"
            "macos-arm64-receipt.json\n"
        ),
        "if-no-files-found": "error",
        "retention-days": 30,
    }
    expected_windows_upload = {
        "name": "opentake-windows-x64-${{ needs.validate.outputs.source_sha }}",
        "path": (
            "target/release/bundle/msi/*.msi\n"
            "target/release/bundle/msi/*.msi.sig\n"
            "target/release/bundle/msi/*.msi.attestation.json\n"
            "target/release/bundle/msi/*.msi.attestation.json.sig\n"
            "target/release/bundle/nsis/*.exe\n"
            "target/release/bundle/nsis/*.exe.sig\n"
            "target/release/bundle/nsis/*.exe.attestation.json\n"
            "target/release/bundle/nsis/*.exe.attestation.json.sig\n"
            "windows-x64-receipt.json\n"
        ),
        "if-no-files-found": "error",
        "retention-days": 30,
    }
    if (
        mac_upload_with != expected_mac_upload
        or windows_upload_with != expected_windows_upload
    ):
        errors.append("exact signed updater artifact uploads")
    upload_paths = "\n".join(
        str(mapping.get("path", ""))
        for mapping in (mac_upload_with, windows_upload_with)
        if mapping is not None
    ).lower()
    if any(
        marker in upload_paths
        for marker in ("*.key", "*.pem", "private_key", "signing_private")
    ):
        errors.append("private updater signing material is never uploaded")
    signed_bundles_ok = (
        mac_build is not None
        and _as_mapping(mac_build.get("env")) == signing_env
        and _has_command(
            mac_build,
            (
                "./web/node_modules/.bin/tauri",
                "build",
                "--ci",
                "--target",
                "aarch64-apple-darwin",
                "--bundles",
                "app,dmg",
                "--config",
                '{"bundle":{"createUpdaterArtifacts":true,"macOS":{"signingIdentity":"-"}}}',
            ),
        )
        and windows_build is not None
        and _as_mapping(windows_build.get("env")) == signing_env
        and _has_code_lines(windows_build, windows_config_lines)
        and _run_script(windows_build).count(
            "& .\\web\\node_modules\\.bin\\tauri.cmd @tauriArguments"
        )
        == 1
    )
    if not signed_bundles_ok:
        errors.append("signed Tauri v2 updater bundles")

    signed_attestations_ok = (
        mac_attestation is not None
        and _as_mapping(mac_attestation.get("env")) == signing_env
        and _has_command(
            mac_attestation,
            (
                "python3", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$RELEASE_TAG",
                "--version", "$RELEASE_VERSION",
                "--source-sha", "$TARGET_SHA",
                "--platform", "darwin-aarch64",
                "--artifact", "$updater",
                "--output", "$attestation",
            ),
        )
        and _has_command(
            mac_attestation,
            ("./web/node_modules/.bin/tauri", "signer", "sign", "$attestation"),
        )
        and windows_attestation is not None
        and _as_mapping(windows_attestation.get("env")) == signing_env
        and _has_command(
            windows_attestation,
            (
                "python", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$env:RELEASE_TAG",
                "--version", "$env:RELEASE_VERSION",
                "--source-sha", "$env:TARGET_SHA",
                "--platform", "windows-x86_64-msi",
                "--artifact", "$msi[0].FullName",
                "--output", "$msiAttestation",
            ),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            ("&", ".\\web\\node_modules\\.bin\\tauri.cmd", "signer", "sign", "$msiAttestation"),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            (
                "python", "scripts/write_updater_attestation.py",
                "--repository", "appergb/OpenTake",
                "--tag", "$env:RELEASE_TAG",
                "--version", "$env:RELEASE_VERSION",
                "--source-sha", "$env:TARGET_SHA",
                "--platform", "windows-x86_64-nsis",
                "--artifact", "$nsis[0].FullName",
                "--output", "$nsisAttestation",
            ),
            powershell=True,
        )
        and _has_command(
            windows_attestation,
            ("&", ".\\web\\node_modules\\.bin\\tauri.cmd", "signer", "sign", "$nsisAttestation"),
            powershell=True,
        )
    )
    if not signed_attestations_ok:
        errors.append("signed updater attestations bind release identity and payload")

    if publish is None or publish.get("needs") != [
        "validate", "quality", "macos_arm64", "windows_x64"
    ]:
        errors.append("publish depends on every required job")
    if publish is None:
        return list(dict.fromkeys(errors))
    if publish.get("permissions") != {"contents": "write"}:
        errors.append("publish-only contents write permission")
    publish_env = _as_mapping(publish.get("env"))
    if (
        publish_env is None
        or publish_env.get("PYTHONDONTWRITEBYTECODE") != "1"
        or publish_env.get("ROOT_FAILED_RUN_ID")
        != "${{ github.event_name == 'workflow_dispatch' && inputs.failed_run_id || '' }}"
        or publish_env.get("PREDECESSOR_RUN_ID")
        != "${{ github.event_name == 'workflow_dispatch' && inputs.failed_recovery_run_id || '' }}"
    ):
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
                'if ! git cat-file -e "${RELEASE_TOOLING_SHA}^{commit}" 2>/dev/null; then',
                'git fetch --no-tags --depth=1 origin "$RELEASE_TOOLING_SHA"',
                'test "$(git rev-parse "${RELEASE_TOOLING_SHA}^{commit}")" = "$RELEASE_TOOLING_SHA"',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/check_release_workflow.py" \\',
                'git cat-file blob "$RELEASE_TOOLING_SHA:scripts/workflow_yaml.py" \\',
                'test -s "$publish_root/tooling/check_release_workflow.py"',
                'test -s "$publish_root/tooling/workflow_yaml.py"',
                'printf \'PUBLISH_ROOT=%s\\n\' "$publish_root" >> "$GITHUB_ENV"',
            ),
        )
        and "raw.githubusercontent.com" not in _run_script(initialize_publish)
        and download_macos_with.get("path")
        == "${{ runner.temp }}/opentake-release-${{ github.run_id }}-${{ github.run_attempt }}/input/macos"
        and download_windows_with.get("path")
        == "${{ runner.temp }}/opentake-release-${{ github.run_id }}-${{ github.run_attempt }}/input/windows"
    ):
        errors.append("publish outputs stay outside the worktree")
        errors.append("publish helpers use exact tooling commit outside the worktree")

    stage = _structured_step(publish, "Stage and verify the exact release payload")
    stage_commands = (
        ("test", "${#dmgs[@]}", "-eq", "1"),
        ("test", "${#mac_updaters[@]}", "-eq", "1"),
        ("test", "${#mac_signatures[@]}", "-eq", "1"),
        ("test", "${#mac_attestations[@]}", "-eq", "1"),
        ("test", "${#mac_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#msis[@]}", "-eq", "1"),
        ("test", "${#exes[@]}", "-eq", "1"),
        ("test", "${#windows_signatures[@]}", "-eq", "2"),
        ("test", "${#msi_attestations[@]}", "-eq", "1"),
        ("test", "${#msi_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#nsis_attestations[@]}", "-eq", "1"),
        ("test", "${#nsis_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#mac_receipts[@]}", "-eq", "1"),
        ("test", "${#windows_receipts[@]}", "-eq", "1"),
    )
    if not all(_has_command(stage, command) for command in stage_commands):
        errors.append("strict release asset counts")
    signed_stage_commands = stage_commands[1:5] + stage_commands[7:12]
    manifest = _structured_step(
        publish, "Write and verify tag-specific updater manifest"
    )
    manifest_counts = (
        ("test", "${#mac_updaters[@]}", "-eq", "1"),
        ("test", "${#mac_signatures[@]}", "-eq", "1"),
        ("test", "${#mac_attestations[@]}", "-eq", "1"),
        ("test", "${#mac_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#msi_installers[@]}", "-eq", "1"),
        ("test", "${#msi_signatures[@]}", "-eq", "1"),
        ("test", "${#msi_attestations[@]}", "-eq", "1"),
        ("test", "${#msi_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#nsis_installers[@]}", "-eq", "1"),
        ("test", "${#nsis_signatures[@]}", "-eq", "1"),
        ("test", "${#nsis_attestations[@]}", "-eq", "1"),
        ("test", "${#nsis_attestation_signatures[@]}", "-eq", "1"),
        ("test", "${#manifests[@]}", "-eq", "1"),
    )
    if not all(
        _has_command(stage, command) for command in signed_stage_commands
    ) or not all(_has_command(manifest, command) for command in manifest_counts):
        errors.append("strict signed updater asset counts")

    manifest_command = (
        "python3",
        "scripts/write_updater_manifest.py",
        "--repository",
        "appergb/OpenTake",
        "--tag",
        "$RELEASE_TAG",
        "--version",
        "$RELEASE_VERSION",
        "--source-sha",
        "$RELEASE_SHA",
        "--darwin-artifact",
        "${mac_updaters[0]}",
        "--darwin-signature",
        "${mac_signatures[0]}",
        "--darwin-attestation",
        "${mac_attestations[0]}",
        "--darwin-attestation-signature",
        "${mac_attestation_signatures[0]}",
        "--windows-msi-artifact",
        "${msi_installers[0]}",
        "--windows-msi-signature",
        "${msi_signatures[0]}",
        "--windows-msi-attestation",
        "${msi_attestations[0]}",
        "--windows-msi-attestation-signature",
        "${msi_attestation_signatures[0]}",
        "--windows-nsis-artifact",
        "${nsis_installers[0]}",
        "--windows-nsis-signature",
        "${nsis_signatures[0]}",
        "--windows-nsis-attestation",
        "${nsis_attestations[0]}",
        "--windows-nsis-attestation-signature",
        "${nsis_attestation_signatures[0]}",
        "--output",
        "$PUBLISH_ROOT/assets/updater-$RELEASE_TAG.json",
    )
    fixed_manifest_ok = (
        _has_code_lines(bind, ('test "$GITHUB_REPOSITORY" = "appergb/OpenTake"',))
        and _has_command(manifest, manifest_command)
        and _has_code_lines(
            manifest,
            (
                'test "${manifests[0]}" = "$PUBLISH_ROOT/assets/updater-$RELEASE_TAG.json"',
            ),
        )
    )
    if not fixed_manifest_ok:
        errors.append("fixed HTTPS exact-tag updater manifest")

    receipts_agree = _has_code_lines(
        stage,
        (
            'if mac_signatures[0] != Path(f"{mac_updaters[0]}.sig"):',
            'f"{msis[0].name}.sig",',
            'f"{exes[0].name}.sig",',
            '"opentake-macos-arm64-receipt-v2",',
            '"opentake-windows-release-receipt-v2",',
            'if entry.get("sha256") != sha256(artifact):',
            'if entry.get("bytes") != artifact.stat().st_size:',
            'if receipt.get("updater_signature_mode") != "tauri-minisign":',
            'if attestation.get("sourceSha") != source_sha:',
            'if attestation.get("sha256") != sha256(artifact):',
            'if attestation != expected:',
            'msi_attestations[0], msis[0], "windows-x86_64-msi"',
            'nsis_attestations[0], exes[0], "windows-x86_64-nsis"',
        ),
    ) and _has_command(manifest, manifest_command)
    if not receipts_agree:
        errors.append("updater receipts, digests, sizes, and signatures agree")
    attestations_bind = _has_code_lines(
        stage,
        (
            '"schemaVersion": 1,',
            '"repository": "appergb/OpenTake",',
            '"tag": release_tag,',
            '"version": release_version,',
            '"sourceSha": source_sha,',
            '"platform": platform,',
            '"assetName": artifact.name,',
            '"size": artifact.stat().st_size,',
            '"sha256": sha256(artifact),',
            'type(attestation["schemaVersion"]) is not int',
            'or type(attestation["size"]) is not int',
            'or attestation["size"] <= 0',
            'or any(type(attestation[field]) is not str for field in string_fields)',
            'if attestation.get("sourceSha") != source_sha:',
            'if attestation.get("sha256") != sha256(artifact):',
            'if attestation != expected:',
            'if path.read_text(encoding="utf-8") != canonical:',
        ),
    ) and _has_command(manifest, manifest_command)
    if not attestations_bind:
        errors.append("attestations bind exact release identity and updater bytes")

    install_minisign = _structured_step(publish, "Install Minisign verifier")
    verify_signatures = _structured_step(
        publish, "Verify updater signatures against embedded public key"
    )
    signatures_verify = (
        _has_command(install_minisign, ("sudo", "apt-get", "update"))
        and _has_command(
            install_minisign,
            (
                "sudo",
                "apt-get",
                "install",
                "--yes",
                "--no-install-recommends",
                "minisign",
            ),
        )
        and _has_command(
            install_minisign, ("command", "-v", "minisign", ">/dev/null")
        )
        and _has_code_lines(
            verify_signatures,
            (
                'pubkey = config["plugins"]["updater"]["pubkey"]',
                'return base64.b64decode(value, validate=True)',
                '"macos-attestation.sig",',
                '"msi-attestation.sig",',
                '"nsis-attestation.sig",',
                'test "${mac_signatures[0]}" = "${mac_updaters[0]}.sig"',
                'test "${msi_signatures[0]}" = "${msis[0]}.sig"',
                'test "${exe_signatures[0]}" = "${exes[0]}.sig"',
                'test "${mac_attestation_signatures[0]}" = "${mac_attestations[0]}.sig"',
                'test "${msi_attestation_signatures[0]}" = "${msi_attestations[0]}.sig"',
                'test "${nsis_attestation_signatures[0]}" = "${nsis_attestations[0]}.sig"',
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign",
                "-Vm",
                "${mac_updaters[0]}",
                "-p",
                "$verification_root/updater.pub",
                "-x",
                "$verification_root/macos.sig",
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign",
                "-Vm",
                "${msis[0]}",
                "-p",
                "$verification_root/updater.pub",
                "-x",
                "$verification_root/msi.sig",
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign",
                "-Vm",
                "${exes[0]}",
                "-p",
                "$verification_root/updater.pub",
                "-x",
                "$verification_root/nsis.sig",
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign", "-Vm", "${mac_attestations[0]}",
                "-p", "$verification_root/updater.pub",
                "-x", "$verification_root/macos-attestation.sig",
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign", "-Vm", "${msi_attestations[0]}",
                "-p", "$verification_root/updater.pub",
                "-x", "$verification_root/msi-attestation.sig",
            ),
        )
        and _has_command(
            verify_signatures,
            (
                "minisign", "-Vm", "${nsis_attestations[0]}",
                "-p", "$verification_root/updater.pub",
                "-x", "$verification_root/nsis-attestation.sig",
            ),
        )
    )
    if not signatures_verify:
        errors.append("updater signatures verify against embedded public key")

    checksum = _structured_step(publish, "Create and verify SHA256SUMS")
    checksum_lines = (
        'payload_names=("${asset_names[@]}")',
        'test "${#payload_names[@]}" -eq 16',
        'sha256sum "${payload_names[@]}" > SHA256SUMS',
        'test "$(wc -l < SHA256SUMS | tr -d \' \')" -eq 16',
        'sha256sum --check SHA256SUMS',
        'test "$(wc -l < "$PUBLISH_ROOT/expected-assets.txt" | tr -d \' \')" -eq 17',
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
    create_command = ("gh", "release", "create", "$RELEASE_TAG", "--verify-tag", "--title", "OpenTake $RELEASE_VERSION", "--notes-file", "$PUBLISH_ROOT/release-body.md", "--draft", "--prerelease", "--latest=false")
    refresh_command = ("gh", "release", "edit", "$RELEASE_TAG", "--title", "OpenTake $RELEASE_VERSION", "--notes-file", "$PUBLISH_ROOT/release-body.md", "--draft", "--prerelease", "--latest=false")
    resolver_command = ("python3", "-B", "$PUBLISH_ROOT/tooling/check_release_workflow.py", "resolve-release-state", "--input", "$PUBLISH_ROOT/existing-release-graphql.json", "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA", "--output", "$PUBLISH_ROOT/release-state.json")
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
        and _has_code_lines(
            draft,
            (
                'jq -e \'.target_commitish | (type == "string" and length > 0)\' "$PUBLISH_ROOT/existing-draft-rest.json" >/dev/null',
            ),
        )
        and draft_commands.index(resolver_command) < draft_commands.index(delete_command)
        and draft_commands.index(published_guard) < draft_commands.index(delete_command)
    )
    if not draft_ok:
        errors.append("draft exists before release upload")
        errors.append("published same-tag release is immutable")
    if (
        create_command not in draft_commands
        or refresh_command not in draft_commands
        or any(
            token == "--target" or token.startswith("--target=")
            for command in draft_commands
            for token in command
        )
    ):
        errors.append("release mutations omit workflow-protected target_commitish")
    if not _has_command(
        upload,
        ("gh", "release", "upload", "$RELEASE_TAG", "$PUBLISH_ROOT/assets/*", "--clobber"),
    ):
        errors.append("draft payload upload supports failed-run retry")
    if not (
        _has_command(inspect_draft, ("python3", "-B", "$PUBLISH_ROOT/tooling/check_release_workflow.py", "resolve-release-state", "--input", "$PUBLISH_ROOT/draft-release-graphql.json", "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA", "--output", "$PUBLISH_ROOT/draft-state.json"))
        and _has_code_lines(inspect_draft, ('test "$(jq -r \'.action\' "$PUBLISH_ROOT/draft-state.json")" = "refresh"', 'cmp "$PUBLISH_ROOT/expected-assets.txt" "$PUBLISH_ROOT/draft-assets.txt"', "jq -e '.asset_sizes | length == 17 and all(. > 0)' \"$PUBLISH_ROOT/draft-state.json\" >/dev/null"))
    ):
        errors.append("draft API verification")
    draft_payload_verified = (
        _has_code_lines(
            inspect_draft,
            (
                "if remote_sizes != local_sizes:",
                'cmp "$PUBLISH_ROOT/expected-assets.txt" "$PUBLISH_ROOT/verified-draft-assets.txt"',
                'cmp "$PUBLISH_ROOT/assets/SHA256SUMS" "$PUBLISH_ROOT/verified-draft/SHA256SUMS"',
                "sha256sum --check SHA256SUMS",
            ),
        )
        and _has_command(
            inspect_draft,
            (
                "gh",
                "release",
                "download",
                "$RELEASE_TAG",
                "--dir",
                "$PUBLISH_ROOT/verified-draft",
            ),
        )
    )
    if not draft_payload_verified:
        errors.append("draft assets verified by exact name, size, and SHA-256")

    def remote_rebind_ok(step: dict[str, object] | None, output: str) -> bool:
        return _has_command(
            step,
            ("git", "ls-remote", "--exit-code", "origin", "refs/tags/$RELEASE_TAG", "refs/tags/$RELEASE_TAG^{}", ">", output),
        ) and _has_command(
            step,
            ("python3", "-B", "$PUBLISH_ROOT/tooling/check_release_workflow.py", "resolve-remote-tag", "--input", output, "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA"),
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
    if not remote_rebind_ok(
        final, "$PUBLISH_ROOT/remote-tag-after-publication.txt"
    ):
        errors.append("final remote tag rebound after publication")
    final_graphql_ok = (
        _has_command(
            final,
            ("gh", "api", "graphql", "-f", "query=$query", "-f", "owner=$owner", "-f", "name=$repository", "-f", "tag=$RELEASE_TAG", ">", "$PUBLISH_ROOT/public-release-graphql.json"),
        )
        and _has_command(
            final,
            ("python3", "-B", "$PUBLISH_ROOT/tooling/check_release_workflow.py", "resolve-public-release-state", "--input", "$PUBLISH_ROOT/public-release-graphql.json", "--tag", "$RELEASE_TAG", "--sha", "$RELEASE_SHA", "--output", "$PUBLISH_ROOT/public-release-state.json"),
        )
        and _has_code_lines(
            final,
            (
                'cmp "$PUBLISH_ROOT/expected-assets.txt" "$PUBLISH_ROOT/public-graphql-assets.txt"',
                "jq -e '.asset_sizes | length == 17 and all(. > 0)' \"$PUBLISH_ROOT/public-release-state.json\" >/dev/null",
            ),
        )
    )
    if not final_graphql_ok:
        errors.append("final GraphQL verification binds immutable tag commit and assets")
    if not _has_code_lines(
        final,
        (
            'if release.get("draft") is not False or release.get("prerelease") is not True:',
            'target_commitish = release.get("target_commitish")',
            "if not isinstance(target_commitish, str) or not target_commitish:",
            "if expected_names != actual_names:",
            "sha256sum --check SHA256SUMS",
        ),
    ) or not _has_command(final, ("gh", "release", "download", "$RELEASE_TAG", "--dir", "$PUBLISH_ROOT/verified-release")):
        errors.append("final REST schema, exact assets, and checksums")

    notes = _structured_step(publish, "Prepare release notes with provenance")
    if not _has_code_lines(
        notes,
        (
            'notes_sha="$RELEASE_SHA"',
            'if [[ "$RELEASE_TOOLING_SHA" = "$RELEASE_SHA" ]]; then',
            'cp "$NOTES_PATH" "$PUBLISH_ROOT/release-body.md"',
            '[[ "$RELEASE_TOOLING_SHA" =~ ^[0-9a-f]{40}$ ]]',
            'if ! git cat-file -e "${RELEASE_TOOLING_SHA}^{commit}" 2>/dev/null; then',
            'git fetch --no-tags --depth=1 origin "$RELEASE_TOOLING_SHA"',
            'test "$(git rev-parse "${RELEASE_TOOLING_SHA}^{commit}")" = "$RELEASE_TOOLING_SHA"',
            'git cat-file blob "$RELEASE_TOOLING_SHA:$NOTES_PATH" \\',
            'notes_sha="$RELEASE_TOOLING_SHA"',
            'test -s "$PUBLISH_ROOT/release-body.md"',
            'recovery_chain="normal tag push"',
            'test "$ROOT_FAILED_RUN_ID" = "31412976593"',
            'test "$PREDECESSOR_RUN_ID" = "31441693191"',
            'recovery_chain="root run $ROOT_FAILED_RUN_ID; direct predecessor run $PREDECESSOR_RUN_ID"',
            "- Release notes commit: \\`$notes_sha\\`",
            "- Recovery chain: $recovery_chain",
        ),
    ):
        errors.append("recovery release notes use exact tooling commit")
    if not _has_code_lines(
        notes,
        (
            "- Source commit: \\`$RELEASE_SHA\\`",
            "- Release tooling commit: \\`$RELEASE_TOOLING_SHA\\`",
            "- Release notes commit: \\`$notes_sha\\`",
            "- GitHub Actions run: [$GITHUB_RUN_ID/$GITHUB_RUN_ATTEMPT]($GITHUB_SERVER_URL/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID)",
            "- Updater trust: updater packages are signed with the dedicated Tauri updater key; the private key is supplied only from GitHub Actions secrets and is never published.",
            "- Platform signing limits: the macOS app uses ad-hoc signing only; it is not Developer ID signed or notarized. Windows installers are not Authenticode-signed.",
        ),
    ):
        errors.append("release notes record source, run, and signing limits")

    return list(dict.fromkeys(errors))


def validate_release_notes_contract(notes_path: Path) -> list[str]:
    """Require the public notes to describe normal and recovery provenance."""
    try:
        notes = notes_path.read_text(encoding="utf-8")
    except (OSError, UnicodeError):
        return ["Beta 5 release notes document dual-SHA recovery provenance"]
    normalized = " ".join(notes.split())
    required = (
        "正常 tag push",
        "product source SHA 与 release tooling SHA",
        "当前远端 `main` HEAD",
        "`failed_run_id`",
        "`failed_run_id=31412976593`",
        "`failed_recovery_run_id=31441693191`",
        "`2c4efdff9d2587c90cbcac0919f9d1d333d67d6a`",
        "`924bc1102a9343e14c3beea2a3622b5d92ebff13`",
        "`workflow_dispatch` 恢复",
        "原不可变 tag SHA",
        "`github.workflow_sha`",
        "source → `924bc110…`",
        "`924bc110…` → 当前远端 `main`",
        "REST `target_commitish` 只做非空 schema 校验",
        "GraphQL `tagCommit.oid`",
        "不创建、移动或删除 tag",
        "公开 release notes",
        "notes commit",
    )
    if not notes.strip() or any(marker not in normalized for marker in required):
        return ["Beta 5 release notes document dual-SHA recovery provenance"]
    return []


def validate_repository_metadata(
    repository_root: Path, *, expected_version: str | None = None
) -> list[str]:
    errors: list[str] = []
    if expected_version is None:
        expected_version = os.environ.get(
            "OPENTAKE_EXPECTED_RELEASE_VERSION", CURRENT_RELEASE_VERSION
        )
    expected_identity = APPROVED_REPOSITORY_IDENTITIES.get(expected_version)
    if expected_identity is None:
        return ["approved repository release identity"]
    expected_wix_version, release_name = expected_identity
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

    if versions != {expected_version}:
        errors.append(f"repository metadata is OpenTake {expected_version}")
    if wix_version != expected_wix_version:
        errors.append(f"Windows installer version is {expected_wix_version}")
    notes = repository_root / "docs" / "releases" / f"{expected_version}.md"
    try:
        notes_missing = not notes.is_file() or not notes.read_text(
            encoding="utf-8"
        ).strip()
    except (OSError, UnicodeError):
        notes_missing = True
    if notes_missing:
        errors.append(f"{release_name} release notes exist")
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


def _resolve_public_release_state_command(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(
        prog="check_release_workflow.py resolve-public-release-state"
    )
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args(arguments)
    try:
        payload = json.loads(options.input.read_text(encoding="utf-8"))
        if not isinstance(payload, dict):
            raise ReleaseStateError("GraphQL public release must be a JSON object")
        state = resolve_public_release_state(payload, options.tag, options.sha)
    except (OSError, json.JSONDecodeError, ReleaseStateError) as error:
        raise SystemExit(f"unsafe public release state: {error}") from error
    options.output.write_text(
        json.dumps(state, indent=2, sort_keys=True) + "\n", encoding="utf-8"
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


def _validate_recovery_run_command(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(
        prog="check_release_workflow.py validate-recovery-run"
    )
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--jobs", required=True, type=Path)
    parser.add_argument("--comparison", required=True, type=Path)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--comparison-head-sha", required=True)
    options = parser.parse_args(arguments)
    try:
        payloads = [
            json.loads(path.read_text(encoding="utf-8"))
            for path in (options.run, options.jobs, options.comparison)
        ]
        if not all(isinstance(payload, dict) for payload in payloads):
            raise RecoveryRunError("recovery API payloads must be JSON objects")
        validate_recovery_run(
            payloads[0],
            payloads[1],
            payloads[2],
            expected_run_id=options.run_id,
            expected_tag=options.tag,
            expected_sha=options.sha,
            expected_comparison_head_sha=options.comparison_head_sha,
        )
    except (OSError, json.JSONDecodeError, RecoveryRunError) as error:
        raise SystemExit(f"unsafe failed-run recovery: {error}") from error
    print(f"validated failed release run {options.run_id} for {options.tag}")


def _validate_failed_recovery_run_command(arguments: list[str]) -> None:
    parser = argparse.ArgumentParser(
        prog="check_release_workflow.py validate-failed-recovery-run"
    )
    parser.add_argument("--run", required=True, type=Path)
    parser.add_argument("--jobs", required=True, type=Path)
    parser.add_argument("--artifacts", required=True, type=Path)
    parser.add_argument("--tooling-comparison", required=True, type=Path)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--tooling-sha", required=True)
    parser.add_argument("--current-tooling-sha", required=True)
    options = parser.parse_args(arguments)
    try:
        payloads = [
            json.loads(path.read_text(encoding="utf-8"))
            for path in (
                options.run,
                options.jobs,
                options.artifacts,
                options.tooling_comparison,
            )
        ]
        if not all(isinstance(payload, dict) for payload in payloads):
            raise RecoveryRunError(
                "failed-recovery API payloads must be JSON objects"
            )
        validate_failed_recovery_run(
            payloads[0],
            payloads[1],
            payloads[2],
            payloads[3],
            expected_run_id=options.run_id,
            expected_tag=options.tag,
            expected_source_sha=options.source_sha,
            expected_tooling_sha=options.tooling_sha,
            expected_current_tooling_sha=options.current_tooling_sha,
        )
    except (OSError, json.JSONDecodeError, RecoveryRunError) as error:
        raise SystemExit(f"unsafe failed-recovery chain: {error}") from error
    print(
        f"validated failed recovery run {options.run_id} "
        f"for {options.tag} at {options.tooling_sha}"
    )


def main(arguments: list[str] | None = None) -> None:
    arguments = sys.argv[1:] if arguments is None else arguments
    if arguments:
        if arguments[0] == "resolve-release-state":
            _resolve_release_state_command(arguments[1:])
        elif arguments[0] == "resolve-public-release-state":
            _resolve_public_release_state_command(arguments[1:])
        elif arguments[0] == "resolve-remote-tag":
            _resolve_remote_tag_command(arguments[1:])
        elif arguments[0] == "validate-recovery-run":
            _validate_recovery_run_command(arguments[1:])
        elif arguments[0] == "validate-failed-recovery-run":
            _validate_failed_recovery_run_command(arguments[1:])
        else:
            raise SystemExit(f"unknown command: {arguments[0]}")
        return
    if not WORKFLOW_PATH.is_file():
        raise SystemExit(f"release workflow is missing: {WORKFLOW_PATH}")
    errors = validate_workflow(WORKFLOW_PATH.read_text(encoding="utf-8"))
    errors.extend(validate_repository_metadata(REPOSITORY_ROOT))
    errors.extend(validate_release_notes_contract(RELEASE_NOTES_PATH))
    if errors:
        raise SystemExit("release workflow is missing: " + ", ".join(errors))
    print("Release workflow contract is complete")


if __name__ == "__main__":
    main()
