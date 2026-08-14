import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import check_license_inventory as inventory


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
EXPECTED_PACKAGES = {
    "codemirror": "6.0.2",
    "@codemirror/lang-html": "6.4.12",
    "@codemirror/lang-css": "6.3.1",
    "@codemirror/theme-one-dark": "6.1.3",
}
LICENSE_TEXT = """MIT License

Copyright (C) 2018-2021 by Marijn Haverbeke <marijn@haverbeke.berlin> and others

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
"""


def _notice_row(package: str, contract: inventory.PackageContract) -> str:
    repository = contract.repository
    return (
        f"| `{package}` | `{contract.version}` | [{repository}]({repository}) | MIT | "
        f"`web/node_modules/{package}/LICENSE` |"
    )


def _write_valid_fixture(root: Path) -> None:
    dependencies = {
        package: contract.version
        for package, contract in inventory.CODEMIRROR_PACKAGES.items()
    }
    web = root / "web"
    web.mkdir(parents=True)
    (web / "package.json").write_text(
        json.dumps({"dependencies": dependencies}), encoding="utf-8"
    )

    importer_lines: list[str] = []
    package_lines: list[str] = []
    snapshot_lines: list[str] = []
    notice_lines = ["# Third-party notices", ""]
    for package, contract in inventory.CODEMIRROR_PACKAGES.items():
        key = f"'{package}'" if package.startswith("@") else package
        importer_lines.extend(
            [
                f"      {key}:",
                f"        specifier: {contract.version}",
                f"        version: {contract.version}",
            ]
        )
        package_key = (
            f"'{package}@{contract.version}'"
            if package.startswith("@")
            else f"{package}@{contract.version}"
        )
        package_lines.extend([f"  {package_key}:", "    resolution: {}"])
        snapshot_lines.extend([f"  {package_key}:", "    dependencies: {}"])
        notice_lines.extend([_notice_row(package, contract), contract.copyright])

        package_dir = web / "node_modules" / Path(*package.split("/"))
        package_dir.mkdir(parents=True)
        (package_dir / "package.json").write_text(
            json.dumps(
                {
                    "name": package,
                    "version": contract.version,
                    "license": "MIT",
                    "repository": {"type": "git", "url": contract.repository + ".git"},
                }
            ),
            encoding="utf-8",
        )
        (package_dir / "LICENSE").write_text(
            LICENSE_TEXT, encoding="utf-8"
        )

    (web / "pnpm-lock.yaml").write_text(
        "lockfileVersion: '9.0'\nimporters:\n  .:\n    dependencies:\n"
        + "\n".join(importer_lines)
        + "\npackages:\n"
        + "\n".join(package_lines)
        + "\nsnapshots:\n"
        + "\n".join(snapshot_lines)
        + "\n",
        encoding="utf-8",
    )
    (root / "THIRD_PARTY_NOTICES.md").write_text(
        "\n".join(notice_lines) + "\n" + LICENSE_TEXT, encoding="utf-8"
    )


class LicenseInventoryTests(unittest.TestCase):
    def test_contract_requires_the_exact_four_direct_packages(self) -> None:
        self.assertEqual(
            EXPECTED_PACKAGES,
            {
                package: contract.version
                for package, contract in inventory.CODEMIRROR_PACKAGES.items()
            },
        )

    def test_repository_inventory_is_valid(self) -> None:
        self.assertEqual([], inventory.validate_inventory(REPOSITORY_ROOT))

    def test_missing_notice_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write_valid_fixture(root)
            notices_path = root / "THIRD_PARTY_NOTICES.md"
            notices = notices_path.read_text(encoding="utf-8")
            package = "@codemirror/lang-css"
            contract = inventory.CODEMIRROR_PACKAGES[package]
            notices_path.write_text(
                notices.replace(_notice_row(package, contract), "", 1),
                encoding="utf-8",
            )
            self.assertIn(
                f"{package}@{contract.version} third-party notice is missing",
                inventory.validate_inventory(root),
            )

    def test_changed_lock_version_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write_valid_fixture(root)
            lockfile_path = root / "web" / "pnpm-lock.yaml"
            lockfile = lockfile_path.read_text(encoding="utf-8")
            lockfile_path.write_text(
                lockfile.replace("version: 6.0.2", "version: 6.0.1", 1),
                encoding="utf-8",
            )
            self.assertIn(
                "codemirror lock importer must resolve exactly 6.0.2",
                inventory.validate_inventory(root),
            )

    def test_runtime_dependency_moved_to_dev_dependencies_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write_valid_fixture(root)
            lockfile_path = root / "web" / "pnpm-lock.yaml"
            lockfile = lockfile_path.read_text(encoding="utf-8")
            stanza = (
                "      codemirror:\n"
                "        specifier: 6.0.2\n"
                "        version: 6.0.2\n"
            )
            self.assertIn(stanza, lockfile)
            mutated = lockfile.replace(stanza, "", 1).replace(
                "\npackages:\n",
                "\n    devDependencies:\n" + stanza + "packages:\n",
                1,
            )
            lockfile_path.write_text(mutated, encoding="utf-8")
            self.assertIn(
                "codemirror lock importer must resolve exactly 6.0.2",
                inventory.validate_inventory(root),
            )

    def test_missing_package_or_snapshot_record_is_rejected(self) -> None:
        for section, record, expected in (
            (
                "packages",
                "  codemirror@6.0.2:\n    resolution: {}\n",
                "codemirror@6.0.2 package resolution record is missing",
            ),
            (
                "snapshots",
                "  codemirror@6.0.2:\n    dependencies: {}\n",
                "codemirror@6.0.2 snapshot record is missing",
            ),
        ):
            with self.subTest(section=section), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                _write_valid_fixture(root)
                lockfile_path = root / "web" / "pnpm-lock.yaml"
                lockfile = lockfile_path.read_text(encoding="utf-8")
                self.assertIn(record, lockfile)
                lockfile_path.write_text(
                    lockfile.replace(record, "", 1), encoding="utf-8"
                )
                self.assertIn(expected, inventory.validate_inventory(root))

    def test_installed_repository_and_full_license_are_verified(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write_valid_fixture(root)
            package_dir = root / "web" / "node_modules" / "codemirror"
            manifest_path = package_dir / "package.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["repository"]["url"] = "https://example.invalid/lookalike.git"
            manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
            self.assertIn(
                "installed codemirror repository does not match the official source",
                inventory.validate_inventory(root),
            )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _write_valid_fixture(root)
            license_path = root / "web" / "node_modules" / "codemirror" / "LICENSE"
            license_path.write_text("MIT License\n", encoding="utf-8")
            self.assertIn(
                "installed codemirror LICENSE does not match its published license",
                inventory.validate_inventory(root),
            )


if __name__ == "__main__":
    unittest.main()
