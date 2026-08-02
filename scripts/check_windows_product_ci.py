#!/usr/bin/env python3
"""Fail closed when the native Windows product gate loses required coverage."""

import json
import re
from pathlib import Path


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


def _named_job_body(workflow: str, job_name: str) -> str | None:
    match = re.search(
        rf"(?ms)^  {re.escape(job_name)}:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)",
        workflow,
    )
    return None if match is None else match.group("body")


def _job_body(workflow: str) -> str | None:
    return _named_job_body(workflow, "windows-product")


def _steps(job: str) -> list[str]:
    starts = [
        match.start()
        for match in re.finditer(r"(?m)^      - (?=(?:name|uses):)", job)
    ]
    return [
        job[start : starts[index + 1] if index + 1 < len(starts) else len(job)]
        for index, start in enumerate(starts)
    ]


def _active(text: str) -> str:
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def _named_step(steps: list[str], name: str) -> str | None:
    prefix = f"      - name: {name}\n"
    return next((step for step in steps if step.startswith(prefix)), None)


def _uses_step(steps: list[str], action: str) -> str | None:
    prefix = f"      - uses: {action}\n"
    return next((step for step in steps if step.startswith(prefix)), None)


def _has_line(text: str, expected: str) -> bool:
    return any(line.strip() == expected for line in _active(text).splitlines())


def validate_workflow(workflow: str) -> list[str]:
    job = _job_body(workflow)
    if job is None:
        return ["windows-product job"]

    errors: list[str] = []
    steps = _steps(job)

    if not _has_line(job, "runs-on: windows-2022"):
        errors.append("pinned native runner")
    job_conditions = [
        line.strip()
        for line in _active(job).splitlines()
        if line.startswith("    if:")
    ]
    if job_conditions != [NORMAL_JOB_CONDITION]:
        errors.append("product job condition")
    if not _has_line(job, "timeout-minutes: 120"):
        errors.append("bounded runtime")
    if not _has_line(job, TARGET_SHA_EXPRESSION):
        errors.append("exact target SHA")
    if re.search(r"(?m)^\s*continue-on-error:\s*true\s*$", _active(job)):
        errors.append("no ignored failures")
    if any(re.search(r"(?m)^\s*if:\s*", _active(step)) for step in steps):
        errors.append("required steps are unconditional")

    checkout = _uses_step(steps, "actions/checkout@v4")
    checkout_lines = (
        "ref: ${{ env.TARGET_SHA }}",
        "fetch-depth: 0",
        "persist-credentials: false",
    )
    if checkout is None or not all(_has_line(checkout, line) for line in checkout_lines):
        errors.append("exact-SHA checkout")

    assert_step = _named_step(steps, "Assert exact checked-out SHA")
    assert_lines = (
        'actual="$(git rev-parse HEAD | tr \'[:upper:]\' \'[:lower:]\')"',
        'expected="$(printf \'%s\' "$TARGET_SHA" | tr \'[:upper:]\' \'[:lower:]\')"',
        'test "$actual" = "$expected"',
        'git cat-file -e "${expected}^{commit}"',
        'test -z "$(git status --porcelain=v1 --untracked-files=all)"',
        'printf \'sha=%s\\n\' "$actual" >> "$GITHUB_OUTPUT"',
    )
    if assert_step is None or not all(
        _has_line(assert_step, line) for line in assert_lines
    ):
        errors.append("immutable checkout assertion")
    if assert_step is None or not _has_line(assert_step, "id: bind"):
        errors.append("checkout assertion publishes bind output")

    cache = _named_step(steps, "Cache cargo")
    if (
        cache is None
        or not _has_line(cache, "uses: actions/cache@v4")
        or not all(
            _has_line(cache, line)
            for line in ("~/.cargo/registry", "~/.cargo/git")
        )
    ):
        errors.append("cargo dependency cache")
    if cache is None or any(
        line.strip() == "target" or line.strip().startswith(("target/", "target\\"))
        for line in _active(cache).splitlines()
    ):
        errors.append("product cache excludes target")

    required_commands = (
        (
            "Install locked Web dependencies",
            "run: pnpm -C web install --frozen-lockfile",
            "locked web dependencies",
        ),
        (
            "Rust formatting",
            "run: cargo fmt --all --check",
            "Rust formatting command",
        ),
        (
            "Rust workspace clippy",
            "run: cargo clippy --workspace --all-targets -- -D warnings",
            "Rust workspace clippy command",
        ),
        (
            "Web editor behavior suite",
            "run: pnpm -C web test",
            "Web editor tests command",
        ),
        (
            "Rust workspace tests",
            "run: cargo test --workspace -- --test-threads=1",
            "Rust workspace tests command",
        ),
        (
            "Minimal-feature Tauri clippy",
            "run: cargo clippy -p opentake-tauri --no-default-features "
            "--all-targets -- -D warnings",
            "minimal-feature clippy command",
        ),
        (
            "Web production build",
            "run: pnpm -C web build",
            "Web production build command",
        ),
    )
    for step_name, command, label in required_commands:
        step = _named_step(steps, step_name)
        if step is None or not _has_line(step, command):
            errors.append(label)

    chromium_motion = _named_step(steps, "Windows Chromium motion capture regression")
    chromium_motion_fragments = (
        "renderer::tests::chromium_skeleton_reports_unavailable_not_panic",
        "virtual_time_network_csp_timeout_cleanup_and_frame_identity",
        "sandbox_progress_cancel_validated_mp4_result",
    )
    if chromium_motion is None or not all(
        fragment in _active(chromium_motion)
        for fragment in chromium_motion_fragments
    ):
        errors.append("complete Windows Chromium motion regression")

    bundle = _named_step(steps, "Build native MSI and NSIS installers")
    bundle_lines = (
        "Remove-Item 'target/release/bundle/msi' -Recurse -Force -ErrorAction SilentlyContinue",
        "Remove-Item 'target/release/bundle/nsis' -Recurse -Force -ErrorAction SilentlyContinue",
        "& .\\web\\node_modules\\.bin\\tauri.cmd build --ci --bundles msi,nsis",
    )
    if bundle is None or not all(_has_line(bundle, line) for line in bundle_lines):
        errors.append("clean native Tauri bundle")

    installed_product = _named_step(
        steps, "Install NSIS package and execute installed product without PATH"
    )
    installed_product_fragments = (
        "packaged_macos_windows_sidecars_resolve_and_execute",
        "Start-Process -FilePath $application -PassThru",
        "installed OpenTake exited during launch smoke test",
    )
    if installed_product is None or not all(
        fragment in _active(installed_product)
        for fragment in installed_product_fragments
    ):
        errors.append("installed product launch smoke test")

    receipt = _named_step(steps, "Bind installers to the exact source SHA")
    if receipt is None or not _has_line(
        receipt, "RECEIPT_SHA: ${{ steps.bind.outputs.sha }}"
    ):
        errors.append("receipt consumes bind output")
    if receipt is None or not _has_line(receipt, "source_sha = $env:RECEIPT_SHA"):
        errors.append("receipt binds source SHA")
    if receipt is None or not any(
        line.strip().startswith("sha256 = (Get-FileHash")
        for line in _active(receipt).splitlines()
    ):
        errors.append("receipt records SHA-256")
    if receipt is None or not all(
        fragment in _active(receipt)
        for fragment in (
            "target/release/bundle/msi/*.msi",
            "target/release/bundle/nsis/*.exe",
            "Tauri did not produce an MSI installer",
            "Tauri did not produce an NSIS installer",
        )
    ):
        errors.append("receipt requires both installers")
    if receipt is None or not _has_line(
        receipt,
        "$receipt | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8NoBOM windows-product-receipt.json",
    ):
        errors.append("receipt file is written")

    upload = _named_step(steps, "Upload exact-SHA Windows installers")
    if upload is None or not _has_line(upload, "uses: actions/upload-artifact@v4"):
        errors.append("bundle artifact upload")
    if upload is None or "target/release/bundle/msi/*.msi" not in _active(upload):
        errors.append("upload includes MSI")
    if upload is None or "target/release/bundle/nsis/*.exe" not in _active(upload):
        errors.append("upload includes NSIS")
    if upload is None or not _has_line(upload, "windows-product-receipt.json"):
        errors.append("upload includes receipt")
    if upload is None or not _has_line(upload, "if-no-files-found: error"):
        errors.append("missing-artifact failure")

    security_job = _named_job_body(workflow, "windows-security")
    if security_job is None:
        errors.append("Windows cancellation security job")
        return errors
    if "choco install ffmpeg" in _active(security_job):
        errors.append("no external Chocolatey FFmpeg dependency")
    security_steps = _steps(security_job)
    cancellation = _named_step(
        security_steps, "Portable FFmpeg cancellation lifecycle"
    )
    cancellation_fragments = (
        "OPENTAKE_FFMPEG: ${{ github.workspace }}\\src-tauri\\binaries\\ffmpeg-x86_64-pc-windows-msvc.exe",
        "& $env:OPENTAKE_FFMPEG -version",
        "checksum-pinned packaged FFmpeg is not runnable",
        "windows_cancelling_running_pcm_child_reaps_both_pipe_readers",
        "windows_cancelling_mux_wait_reaps_child",
    )
    if cancellation is None or not all(
        fragment in _active(cancellation) for fragment in cancellation_fragments
    ):
        errors.append("pinned FFmpeg cancellation lifecycle")

    return errors


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
    workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
    errors = validate_workflow(workflow)
    errors.extend(validate_tauri_config(TAURI_CONFIG_PATH.read_text(encoding="utf-8")))
    if errors:
        raise SystemExit("windows-product is missing: " + ", ".join(errors))

    print("Windows product CI contract is complete")


if __name__ == "__main__":
    main()
