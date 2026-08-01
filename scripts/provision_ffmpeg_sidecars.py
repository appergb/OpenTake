#!/usr/bin/env python3
"""Provision checksum-pinned FFmpeg sidecars for the selected Rust target."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile


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
    if "--enable-nonfree" in output:
        raise RuntimeError(f"unredistributable nonfree sidecar rejected: {path}")
    first_line = output.splitlines()[0]
    fields = first_line.split()
    reported = fields[2] if len(fields) >= 3 else ""
    if reported != version and not reported.startswith(f"{version}-"):
        raise RuntimeError(f"unexpected sidecar version for {path}: {first_line}")
    license_output = subprocess.check_output(
        [str(path), "-L"], text=True, stderr=subprocess.STDOUT
    )
    if "not legally redistributable" in license_output.lower():
        raise RuntimeError(f"unredistributable sidecar license rejected: {path}")


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


def materialize_download(
    record: dict[str, object], download_path: Path, destination_path: Path
) -> None:
    archive = record.get("archive")
    if archive is None:
        os.replace(download_path, destination_path)
        return
    if not isinstance(archive, dict) or archive.get("format") != "zip":
        raise RuntimeError("unsupported sidecar archive format")
    expected_archive_sha = archive.get("sha256")
    member = archive.get("member")
    if not isinstance(expected_archive_sha, str) or not isinstance(member, str):
        raise RuntimeError("sidecar archive requires string member and sha256")
    actual_archive_sha = sha256(download_path)
    if actual_archive_sha != expected_archive_sha:
        raise RuntimeError(
            "sidecar archive checksum mismatch: "
            f"{actual_archive_sha} != {expected_archive_sha}"
        )
    with zipfile.ZipFile(download_path) as archive_file:
        matches = [
            item
            for item in archive_file.infolist()
            if item.filename == member and not item.is_dir()
        ]
        if len(matches) != 1:
            raise RuntimeError(f"sidecar archive must contain exactly one {member!r}")
        if matches[0].file_size > 256 * 1024 * 1024:
            raise RuntimeError("sidecar archive member exceeds the 256 MiB limit")
        with archive_file.open(matches[0]) as source, destination_path.open("wb") as target:
            shutil.copyfileobj(source, target, length=1024 * 1024)


def provision(tool: str, record: dict[str, object], target: str) -> None:
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    final_path = destination(tool, target)
    expected_sha = record.get("sha256")
    version = record.get("version")
    url = record.get("url")
    if not all(isinstance(value, str) for value in (expected_sha, version, url)):
        raise RuntimeError(f"invalid sidecar lock record for {tool}/{target}")
    assert isinstance(expected_sha, str)
    assert isinstance(version, str)
    assert isinstance(url, str)
    if final_path.is_file() and sha256(final_path) == expected_sha:
        verify(final_path, expected_sha, version)
        print(f"verified {final_path.relative_to(ROOT)}")
        return

    with tempfile.NamedTemporaryFile(
        prefix=f".{tool}-{target}-", dir=BIN_DIR, delete=False
    ) as stream:
        temporary_path = Path(stream.name)
    archive_path: Path | None = None

    try:
        if record.get("archive") is None:
            download(url, temporary_path)
        else:
            with tempfile.NamedTemporaryFile(
                prefix=f".{tool}-{target}-archive-", dir=BIN_DIR, delete=False
            ) as stream:
                archive_path = Path(stream.name)
            download(url, archive_path)
            materialize_download(record, archive_path, temporary_path)
        actual_sha = sha256(temporary_path)
        if actual_sha != expected_sha:
            raise RuntimeError(
                f"download checksum mismatch for {tool}: {actual_sha} != {expected_sha}"
            )
        if "windows" not in target:
            temporary_path.chmod(
                temporary_path.stat().st_mode
                | stat.S_IXUSR
                | stat.S_IXGRP
                | stat.S_IXOTH
            )
        verify(temporary_path, expected_sha, version)
        os.replace(temporary_path, final_path)
        verify(final_path, expected_sha, version)
        print(f"provisioned {final_path.relative_to(ROOT)}")
    finally:
        if temporary_path.exists():
            temporary_path.unlink()
        if archive_path is not None and archive_path.exists():
            archive_path.unlink()


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
