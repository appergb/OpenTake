#!/usr/bin/env python3
"""Validate the release-critical CodeMirror dependency and notice inventory."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True)
class PackageContract:
    version: str
    repository: str
    copyright: str
    license_sha256: str


CODEMIRROR_PACKAGES = {
    "codemirror": PackageContract(
        version="6.0.2",
        repository="https://github.com/codemirror/basic-setup",
        copyright="Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others",
        license_sha256="05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07",
    ),
    "@codemirror/lang-html": PackageContract(
        version="6.4.12",
        repository="https://code.haverbeke.berlin/codemirror/lang-html",
        copyright="Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others",
        license_sha256="05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07",
    ),
    "@codemirror/lang-css": PackageContract(
        version="6.3.1",
        repository="https://github.com/codemirror/lang-css",
        copyright="Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others",
        license_sha256="05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07",
    ),
    "@codemirror/state": PackageContract(
        version="6.7.1",
        repository="https://code.haverbeke.berlin/codemirror/state",
        copyright="Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others",
        license_sha256="05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07",
    ),
    "@codemirror/theme-one-dark": PackageContract(
        version="6.1.3",
        repository="https://github.com/codemirror/theme-one-dark",
        copyright="Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others",
        license_sha256="05c6130cda97e7600ca91427a41e8a065efcf82365fc0293e7de80faec494c07",
    ),
}


def _package_directory(node_modules: Path, package: str) -> Path:
    return node_modules.joinpath(*package.split("/"))


def _mapping_body(document: str, name: str, indent: int) -> str:
    indentation = " " * indent
    match = re.search(
        rf"(?ms)^{indentation}{re.escape(name)}:\n"
        rf"(?P<body>.*?)(?=^{indentation}\S[^\n]*:\n|\Z)",
        document,
    )
    return "" if match is None else match.group("body")


def _lock_importer_entry(lockfile: str, package: str) -> tuple[str, str] | None:
    importers = _top_level_section(lockfile, "importers")
    root_importer = _mapping_body(importers, ".", 2)
    runtime_dependencies = _mapping_body(root_importer, "dependencies", 4)
    key = f"'{package}'" if package.startswith("@") else package
    match = re.search(
        rf"(?m)^      {re.escape(key)}:\n"
        rf"        specifier: (?P<specifier>[^\n]+)\n"
        rf"        version: (?P<version>[^\s(]+)",
        runtime_dependencies,
    )
    if match is None:
        return None
    return match.group("specifier"), match.group("version")


def _top_level_section(lockfile: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^{re.escape(name)}:\n(?P<body>.*?)(?=^[A-Za-z][A-Za-z0-9_-]*:\n|\Z)",
        lockfile,
    )
    return "" if match is None else match.group("body")


def _has_section_package(section: str, package: str, version: str) -> bool:
    key = re.escape(f"{package}@{version}")
    return re.search(rf"(?m)^  ['\"]?{key}['\"]?:", section) is not None


def _normalize_repository(repository: object) -> str:
    if isinstance(repository, dict):
        repository = repository.get("url")
    if not isinstance(repository, str):
        return ""
    normalized = repository.strip()
    if normalized.startswith("git+"):
        normalized = normalized[4:]
    if normalized.endswith(".git"):
        normalized = normalized[:-4]
    return normalized.rstrip("/")


def validate_inventory(repository_root: Path = REPOSITORY_ROOT) -> list[str]:
    errors: list[str] = []
    package_json_path = repository_root / "web" / "package.json"
    lockfile_path = repository_root / "web" / "pnpm-lock.yaml"
    notices_path = repository_root / "THIRD_PARTY_NOTICES.md"

    try:
        package_json = json.loads(package_json_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return ["web/package.json must be readable valid JSON"]

    try:
        lockfile = lockfile_path.read_text(encoding="utf-8")
    except OSError:
        lockfile = ""
        errors.append("web/pnpm-lock.yaml must be readable")

    try:
        notices = notices_path.read_text(encoding="utf-8")
    except OSError:
        notices = ""
        errors.append("THIRD_PARTY_NOTICES.md must be readable")

    dependencies = package_json.get("dependencies")
    if not isinstance(dependencies, dict):
        dependencies = {}
    direct_codemirror = {
        package
        for package in dependencies
        if package == "codemirror" or package.startswith("@codemirror/")
    }
    if direct_codemirror != set(CODEMIRROR_PACKAGES):
        errors.append(
            "direct CodeMirror dependency set must exactly match the license contract"
        )

    node_modules = repository_root / "web" / "node_modules"
    package_records = _top_level_section(lockfile, "packages")
    snapshot_records = _top_level_section(lockfile, "snapshots")
    for package, contract in CODEMIRROR_PACKAGES.items():
        expected_license = f"web/node_modules/{package}/LICENSE"
        expected_notice = (
            f"| `{package}` | `{contract.version}` | "
            f"[{contract.repository}]({contract.repository}) | MIT | "
            f"`{expected_license}` |"
        )

        if dependencies.get(package) != contract.version:
            errors.append(f"{package} must be an exact {contract.version} dependency")

        importer = _lock_importer_entry(lockfile, package)
        if importer != (contract.version, contract.version):
            errors.append(f"{package} lock importer must resolve exactly {contract.version}")
        if not _has_section_package(package_records, package, contract.version):
            errors.append(
                f"{package}@{contract.version} package resolution record is missing"
            )
        if not _has_section_package(snapshot_records, package, contract.version):
            errors.append(f"{package}@{contract.version} snapshot record is missing")

        package_directory = _package_directory(node_modules, package)
        installed_manifest = package_directory / "package.json"
        installed_license = package_directory / "LICENSE"
        try:
            installed = json.loads(installed_manifest.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            errors.append(f"installed {package}@{contract.version} manifest is missing")
        else:
            if installed.get("version") != contract.version:
                errors.append(f"installed {package} version must be {contract.version}")
            if installed.get("license") != "MIT":
                errors.append(f"installed {package} license must be MIT")
            if _normalize_repository(installed.get("repository")) != contract.repository:
                errors.append(
                    f"installed {package} repository does not match the official source"
                )
        try:
            license_bytes = installed_license.read_bytes()
            license_text = license_bytes.decode("utf-8")
        except OSError:
            errors.append(f"installed {package} LICENSE file is missing")
        except UnicodeDecodeError:
            errors.append(f"installed {package} LICENSE must be UTF-8")
        else:
            if hashlib.sha256(license_bytes).hexdigest() != contract.license_sha256:
                errors.append(
                    f"installed {package} LICENSE does not match its published license"
                )
            if license_text.strip() not in notices:
                errors.append(
                    f"{package}@{contract.version} full MIT license notice is missing"
                )

        if expected_notice not in notices:
            errors.append(f"{package}@{contract.version} third-party notice is missing")
        if contract.copyright not in notices:
            errors.append(f"{package}@{contract.version} copyright notice is missing")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository-root", type=Path, default=REPOSITORY_ROOT)
    args = parser.parse_args()
    errors = validate_inventory(args.repository_root.resolve())
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print("CodeMirror dependency and license inventory is valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
