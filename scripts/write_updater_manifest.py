#!/usr/bin/env python3
"""Write and verify the tag-specific Tauri v2 updater manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
from typing import Mapping
from urllib.parse import quote


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


class ManifestError(ValueError):
    """The updater manifest inputs do not satisfy the release trust contract."""


def _validate_release_identity(
    repository: str, tag: str, version: str, source_sha: str
) -> None:
    if repository != EXPECTED_REPOSITORY:
        raise ManifestError(f"unexpected updater repository: {repository}")
    if _SEMVER_RE.fullmatch(version) is None:
        raise ManifestError(f"updater version must be a SemVer prerelease: {version}")
    if tag != f"v{version}":
        raise ManifestError(f"updater tag/version mismatch: {tag} != v{version}")
    if _SOURCE_SHA_RE.fullmatch(source_sha) is None:
        raise ManifestError(f"invalid updater source SHA: {source_sha}")


def _read_signature(artifact: Path, signature: Path) -> str:
    if signature != Path(f"{artifact}.sig"):
        raise ManifestError(f"signature is not the artifact companion: {signature}")
    for kind, path in (("artifact", artifact), ("signature", signature)):
        if path.is_symlink() or not path.is_file():
            raise ManifestError(f"{kind} must be a regular file: {path}")
    if artifact.stat().st_size <= 0:
        raise ManifestError(f"updater artifact is empty: {artifact}")
    if signature.stat().st_size > 16 * 1024:
        raise ManifestError(f"updater signature is unexpectedly large: {signature}")
    try:
        value = signature.read_text(encoding="utf-8").strip()
    except UnicodeError as error:
        raise ManifestError(f"updater signature is not UTF-8: {signature}") from error
    if not value or "\x00" in value:
        raise ManifestError(f"updater signature is empty or malformed: {signature}")
    return value


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _validate_attestation(
    *,
    attestation_path: Path,
    artifact: Path,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    platform: str,
) -> None:
    if attestation_path != Path(f"{artifact}.attestation.json"):
        raise ManifestError(
            f"attestation is not the artifact companion: {attestation_path}"
        )
    if attestation_path.is_symlink() or not attestation_path.is_file():
        raise ManifestError(f"attestation must be a regular file: {attestation_path}")
    try:
        attestation = json.loads(attestation_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError) as error:
        raise ManifestError(f"attestation is not valid JSON: {attestation_path}") from error
    if not isinstance(attestation, dict) or set(attestation) != ATTESTATION_KEYS:
        raise ManifestError("attestation field set is not exact")
    string_fields = (
        "repository",
        "tag",
        "version",
        "sourceSha",
        "platform",
        "assetName",
        "sha256",
    )
    if (
        type(attestation["schemaVersion"]) is not int
        or type(attestation["size"]) is not int
        or attestation["size"] <= 0
        or any(type(attestation[field]) is not str for field in string_fields)
    ):
        raise ManifestError("attestation field types are not strict")
    expected = {
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
    if attestation != expected:
        raise ManifestError(
            f"attestation does not bind exact release and payload: {attestation_path}"
        )


def _validate_artifacts(
    artifacts: Mapping[str, tuple[Path, Path, Path, Path]],
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
) -> dict[str, tuple[Path, str, Path, str]]:
    if set(artifacts) != EXPECTED_PLATFORMS:
        raise ManifestError(
            f"updater platforms must be exactly {sorted(EXPECTED_PLATFORMS)}"
        )
    validated: dict[str, tuple[Path, str, Path, str]] = {}
    for platform in sorted(EXPECTED_PLATFORMS):
        artifact, signature, attestation, attestation_signature = artifacts[platform]
        artifact = Path(artifact)
        signature = Path(signature)
        attestation = Path(attestation)
        attestation_signature = Path(attestation_signature)
        if _SAFE_ASSET_NAME_RE.fullmatch(artifact.name) is None:
            raise ManifestError(
                f"updater artifact filename is not URL-safe: {artifact.name}"
            )
        if platform == "darwin-aarch64" and not artifact.name.endswith(
            ".app.tar.gz"
        ):
            raise ManifestError(f"unexpected macOS updater artifact: {artifact.name}")
        if platform == "windows-x86_64-msi" and not artifact.name.endswith(".msi"):
            raise ManifestError(f"unexpected Windows MSI updater artifact: {artifact.name}")
        if platform == "windows-x86_64-nsis" and not artifact.name.endswith(".exe"):
            raise ManifestError(f"unexpected Windows NSIS updater artifact: {artifact.name}")
        package_signature = _read_signature(artifact, signature)
        _validate_attestation(
            attestation_path=attestation,
            artifact=artifact,
            repository=repository,
            tag=tag,
            version=version,
            source_sha=source_sha,
            platform=platform,
        )
        attestation_signature_value = _read_signature(
            attestation, attestation_signature
        )
        validated[platform] = (
            artifact,
            package_signature,
            attestation,
            attestation_signature_value,
        )
    return validated


def _asset_url(repository: str, tag: str, artifact: Path) -> str:
    return (
        f"https://github.com/{repository}/releases/download/"
        f"{quote(tag, safe='')}/{quote(artifact.name, safe='')}"
    )


def build_manifest(
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    artifacts: Mapping[str, tuple[Path, Path, Path, Path]],
) -> dict[str, object]:
    _validate_release_identity(repository, tag, version, source_sha)
    validated = _validate_artifacts(
        artifacts,
        repository=repository,
        tag=tag,
        version=version,
        source_sha=source_sha,
    )
    return {
        "version": version,
        "platforms": {
            platform: {
                "attestationSignature": attestation_signature,
                "attestationUrl": _asset_url(repository, tag, attestation),
                "signature": package_signature,
                "url": _asset_url(repository, tag, artifact),
            }
            for platform, (
                artifact,
                package_signature,
                attestation,
                attestation_signature,
            ) in sorted(validated.items())
        },
    }


def validate_manifest(
    manifest: object,
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    artifacts: Mapping[str, tuple[Path, Path, Path, Path]],
) -> None:
    expected = build_manifest(
        repository=repository,
        tag=tag,
        version=version,
        source_sha=source_sha,
        artifacts=artifacts,
    )
    if not isinstance(manifest, dict):
        raise ManifestError("updater manifest must be a JSON object")
    if manifest.get("version") != version:
        raise ManifestError("updater manifest version does not match its tag")
    platforms = manifest.get("platforms")
    if not isinstance(platforms, dict) or set(platforms) != EXPECTED_PLATFORMS:
        raise ManifestError("updater manifest platform set is not exact")
    for platform in sorted(EXPECTED_PLATFORMS):
        entry = platforms.get(platform)
        expected_entry = expected["platforms"][platform]  # type: ignore[index]
        if not isinstance(entry, dict):
            raise ManifestError(f"updater platform entry is malformed: {platform}")
        if entry.get("url") != expected_entry["url"]:
            raise ManifestError(f"updater artifact URL mismatch: {platform}")
        if entry.get("signature") != expected_entry["signature"]:
            raise ManifestError(f"updater signature mismatch: {platform}")
        if entry.get("attestationUrl") != expected_entry["attestationUrl"]:
            raise ManifestError(f"updater attestation URL mismatch: {platform}")
        if (
            entry.get("attestationSignature")
            != expected_entry["attestationSignature"]
        ):
            raise ManifestError(f"updater attestation signature mismatch: {platform}")
        if set(entry) != {
            "url",
            "signature",
            "attestationUrl",
            "attestationSignature",
        }:
            raise ManifestError(f"updater platform entry has extra fields: {platform}")
    if set(manifest) != {"version", "platforms"}:
        raise ManifestError("updater manifest has unexpected top-level fields")


def write_manifest(
    *,
    repository: str,
    tag: str,
    version: str,
    source_sha: str,
    artifacts: Mapping[str, tuple[Path, Path, Path, Path]],
    output: Path,
) -> dict[str, object]:
    output = Path(output)
    _validate_release_identity(repository, tag, version, source_sha)
    if output.name != f"updater-{tag}.json":
        raise ManifestError(f"unexpected updater manifest filename: {output.name}")
    manifest = build_manifest(
        repository=repository,
        tag=tag,
        version=version,
        source_sha=source_sha,
        artifacts=artifacts,
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        encoding="utf-8",
        dir=output.parent,
        prefix=f".{output.name}.",
        delete=False,
    ) as stream:
        temporary = Path(stream.name)
        json.dump(manifest, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    try:
        temporary.chmod(0o644)
        os.replace(temporary, output)
    finally:
        temporary.unlink(missing_ok=True)
    written = json.loads(output.read_text(encoding="utf-8"))
    validate_manifest(
        written,
        repository=repository,
        tag=tag,
        version=version,
        source_sha=source_sha,
        artifacts=artifacts,
    )
    return written


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--darwin-artifact", type=Path, required=True)
    parser.add_argument("--darwin-signature", type=Path, required=True)
    parser.add_argument("--darwin-attestation", type=Path, required=True)
    parser.add_argument("--darwin-attestation-signature", type=Path, required=True)
    parser.add_argument("--windows-msi-artifact", type=Path, required=True)
    parser.add_argument("--windows-msi-signature", type=Path, required=True)
    parser.add_argument("--windows-msi-attestation", type=Path, required=True)
    parser.add_argument("--windows-msi-attestation-signature", type=Path, required=True)
    parser.add_argument("--windows-nsis-artifact", type=Path, required=True)
    parser.add_argument("--windows-nsis-signature", type=Path, required=True)
    parser.add_argument("--windows-nsis-attestation", type=Path, required=True)
    parser.add_argument("--windows-nsis-attestation-signature", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    write_manifest(
        repository=options.repository,
        tag=options.tag,
        version=options.version,
        source_sha=options.source_sha,
        artifacts={
            "darwin-aarch64": (
                options.darwin_artifact,
                options.darwin_signature,
                options.darwin_attestation,
                options.darwin_attestation_signature,
            ),
            "windows-x86_64-msi": (
                options.windows_msi_artifact,
                options.windows_msi_signature,
                options.windows_msi_attestation,
                options.windows_msi_attestation_signature,
            ),
            "windows-x86_64-nsis": (
                options.windows_nsis_artifact,
                options.windows_nsis_signature,
                options.windows_nsis_attestation,
                options.windows_nsis_attestation_signature,
            ),
        },
        output=options.output,
    )


if __name__ == "__main__":
    main()
