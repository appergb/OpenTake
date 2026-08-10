#!/usr/bin/env python3
"""Write a deterministic signed-updater identity attestation."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile


EXPECTED_REPOSITORY = "appergb/OpenTake"
EXPECTED_PLATFORMS = frozenset(
    {"darwin-aarch64", "windows-x86_64-msi", "windows-x86_64-nsis"}
)
ATTESTATION_KEYS = frozenset(
    {
        "schemaVersion",
        "repository",
        "tag",
        "version",
        "sourceSha",
        "platform",
        "assetName",
        "size",
        "sha256",
    }
)
_NUMERIC = r"(?:0|[1-9][0-9]*)"
_IDENTIFIER = r"(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
_SEMVER_RE = re.compile(
    rf"^{_NUMERIC}\.{_NUMERIC}\.{_NUMERIC}"
    rf"(?:-{_IDENTIFIER}(?:\.{_IDENTIFIER})*)$"
)
_SOURCE_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_SAFE_ASSET_NAME_RE = re.compile(r"^[0-9A-Za-z._-]+$")


class AttestationError(ValueError):
    """Attestation input does not satisfy the updater release contract."""


def _validate_identity(
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    platform: str,
) -> None:
    if repository != EXPECTED_REPOSITORY:
        raise AttestationError(f"unexpected attestation repository: {repository}")
    if _SEMVER_RE.fullmatch(version) is None:
        raise AttestationError(f"attestation version must be a SemVer prerelease: {version}")
    if tag != f"v{version}":
        raise AttestationError(f"attestation tag/version mismatch: {tag} != v{version}")
    if _SOURCE_SHA_RE.fullmatch(source_sha) is None:
        raise AttestationError(f"invalid attestation source SHA: {source_sha}")
    if platform not in EXPECTED_PLATFORMS:
        raise AttestationError(f"unexpected attestation platform: {platform}")


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_attestation(
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    platform: str,
    artifact: Path,
) -> dict[str, object]:
    _validate_identity(repository, tag, version, source_sha, platform)
    artifact = Path(artifact)
    if artifact.is_symlink() or not artifact.is_file():
        raise AttestationError(f"updater artifact must be a regular file: {artifact}")
    if artifact.stat().st_size <= 0:
        raise AttestationError(f"updater artifact is empty: {artifact}")
    if _SAFE_ASSET_NAME_RE.fullmatch(artifact.name) is None:
        raise AttestationError(
            f"updater artifact filename is not URL-safe: {artifact.name}"
        )
    if platform == "darwin-aarch64" and not artifact.name.endswith(".app.tar.gz"):
        raise AttestationError(f"unexpected macOS updater artifact: {artifact.name}")
    if platform == "windows-x86_64-msi" and not artifact.name.endswith(".msi"):
        raise AttestationError(f"unexpected Windows MSI updater artifact: {artifact.name}")
    if platform == "windows-x86_64-nsis" and not artifact.name.endswith(".exe"):
        raise AttestationError(f"unexpected Windows NSIS updater artifact: {artifact.name}")
    return {
        "schemaVersion": 1,
        "repository": repository,
        "tag": tag,
        "version": version,
        "sourceSha": source_sha,
        "platform": platform,
        "assetName": artifact.name,
        "size": artifact.stat().st_size,
        "sha256": _sha256(artifact),
    }


def write_attestation(
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    platform: str,
    artifact: Path,
    output: Path,
) -> dict[str, object]:
    artifact = Path(artifact)
    output = Path(output)
    if output != Path(f"{artifact}.attestation.json"):
        raise AttestationError(f"unexpected attestation filename: {output.name}")
    value = build_attestation(
        repository=repository,
        tag=tag,
        version=version,
        source_sha=source_sha,
        platform=platform,
        artifact=artifact,
    )
    if set(value) != ATTESTATION_KEYS:
        raise AttestationError("attestation field set is not exact")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w",
            encoding="utf-8",
            dir=output.parent,
            prefix=f".{output.name}.",
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            stream.write(json.dumps(value, sort_keys=True, separators=(",", ":")))
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(0o644)
        os.replace(temporary, output)
        temporary = None
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
    written = json.loads(output.read_text(encoding="utf-8"))
    if written != value or set(written) != ATTESTATION_KEYS:
        raise AttestationError("written attestation failed exact reload validation")
    return value


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    write_attestation(
        repository=options.repository,
        tag=options.tag,
        version=options.version,
        source_sha=options.source_sha,
        platform=options.platform,
        artifact=options.artifact,
        output=options.output,
    )


if __name__ == "__main__":
    main()
