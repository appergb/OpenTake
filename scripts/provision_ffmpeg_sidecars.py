#!/usr/bin/env python3
"""Provision checksum-pinned FFmpeg sidecars for the selected Rust target."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import time
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = ROOT / "scripts" / "ffmpeg-sidecars.lock.json"
BIN_DIR = ROOT / "src-tauri" / "binaries"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def host_target() -> str:
    return subprocess.check_output(
        ["rustc", "--print", "host-tuple"], text=True
    ).strip()


def destination(tool: str, target: str) -> Path:
    extension = ".exe" if "windows" in target else ""
    return BIN_DIR / f"{tool}-{target}{extension}"


def verify(path: Path, expected_sha: str, version: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise RuntimeError(f"sidecar is not a regular non-symlink file: {path}")
    actual_sha = sha256(path)
    if actual_sha != expected_sha:
        raise RuntimeError(
            f"sidecar checksum mismatch for {path}: {actual_sha} != {expected_sha}"
        )
    output = subprocess.check_output(
        [str(path), "-version"], text=True, stderr=subprocess.STDOUT
    )
    first_line = output.splitlines()[0]
    fields = first_line.split()
    reported = fields[2] if len(fields) >= 3 else ""
    if reported != version and not reported.startswith(f"{version}-"):
        raise RuntimeError(f"unexpected sidecar version for {path}: {first_line}")


def download(url: str, destination_path: Path) -> None:
    last_error: Exception | None = None
    for attempt in range(1, 5):
        try:
            request = urllib.request.Request(
                url, headers={"User-Agent": "OpenTake-sidecar-provisioner/1"}
            )
            with urllib.request.urlopen(request, timeout=120) as response:
                with destination_path.open("wb") as stream:
                    while chunk := response.read(1024 * 1024):
                        stream.write(chunk)
            return
        except OSError as error:
            last_error = error
            if destination_path.exists():
                destination_path.unlink()
            if attempt < 4:
                time.sleep(attempt)
    raise RuntimeError(f"download failed after 4 attempts: {last_error}")


def provision(tool: str, record: dict[str, str], target: str) -> None:
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    final_path = destination(tool, target)
    if final_path.is_file() and sha256(final_path) == record["sha256"]:
        verify(final_path, record["sha256"], record["version"])
        print(f"verified {final_path.relative_to(ROOT)}")
        return

    with tempfile.NamedTemporaryFile(
        prefix=f".{tool}-{target}-", dir=BIN_DIR, delete=False
    ) as stream:
        temporary_path = Path(stream.name)

    try:
        download(record["url"], temporary_path)
        actual_sha = sha256(temporary_path)
        if actual_sha != record["sha256"]:
            raise RuntimeError(
                f"download checksum mismatch for {tool}: {actual_sha} != {record['sha256']}"
            )
        if "windows" not in target:
            temporary_path.chmod(
                temporary_path.stat().st_mode
                | stat.S_IXUSR
                | stat.S_IXGRP
                | stat.S_IXOTH
            )
        os.replace(temporary_path, final_path)
        verify(final_path, record["sha256"], record["version"])
        print(f"provisioned {final_path.relative_to(ROOT)}")
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", default=host_target())
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()

    lock = json.loads(LOCK_PATH.read_text(encoding="utf-8"))
    try:
        target = lock["targets"][args.target]
    except KeyError as error:
        raise SystemExit(f"unsupported packaged sidecar target: {args.target}") from error

    BIN_DIR.mkdir(parents=True, exist_ok=True)
    for partial in BIN_DIR.glob(f".*-{args.target}-*"):
        if partial.is_file() and not partial.is_symlink():
            partial.unlink()

    for tool in ("ffmpeg", "ffprobe"):
        path = destination(tool, args.target)
        if args.verify_only:
            verify(path, target[tool]["sha256"], target[tool]["version"])
            print(f"verified {path.relative_to(ROOT)}")
        else:
            provision(tool, target[tool], args.target)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"error: {error}", file=sys.stderr)
        sys.exit(1)
