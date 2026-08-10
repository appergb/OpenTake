from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPOSITORY_ROOT / "scripts" / "write_updater_attestation.py"
SOURCE_SHA = "a" * 40


def load_generator():
    if not MODULE_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location(
        "write_updater_attestation", MODULE_PATH
    )
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class UpdaterAttestationTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifact = self.root / "OpenTake.app.tar.gz"
        self.artifact.write_bytes(b"signed updater payload")
        self.output = self.root / "OpenTake.app.tar.gz.attestation.json"

    def generator(self):
        module = load_generator()
        self.assertIsNotNone(module, "updater attestation generator must exist")
        return module

    def write(self):
        return self.generator().write_attestation(
            repository="appergb/OpenTake",
            tag="v1.0.0-beta.3",
            version="1.0.0-beta.3",
            source_sha=SOURCE_SHA,
            platform="darwin-aarch64",
            artifact=self.artifact,
            output=self.output,
        )

    def test_writes_exact_canonical_attestation_bound_to_payload(self) -> None:
        value = self.write()
        expected = {
            "schemaVersion": 1,
            "repository": "appergb/OpenTake",
            "tag": "v1.0.0-beta.3",
            "version": "1.0.0-beta.3",
            "sourceSha": SOURCE_SHA,
            "platform": "darwin-aarch64",
            "assetName": self.artifact.name,
            "size": self.artifact.stat().st_size,
            "sha256": hashlib.sha256(self.artifact.read_bytes()).hexdigest(),
        }
        self.assertEqual(expected, value)
        self.assertEqual(
            json.dumps(expected, sort_keys=True, separators=(",", ":")) + "\n",
            self.output.read_text(encoding="utf-8"),
        )

    def test_rejects_wrong_repository_tag_version_source_or_platform(self) -> None:
        generator = self.generator()
        mutations = (
            {"repository": "attacker/OpenTake"},
            {"tag": "v1.0.0-beta.2"},
            {"version": "1.0.0"},
            {
                "tag": "v1.0.0-beta.3+hotfix.1",
                "version": "1.0.0-beta.3+hotfix.1",
            },
            {"source_sha": "A" * 40},
            {"platform": "darwin-x86_64"},
            {"platform": "windows-x86_64"},
        )
        defaults = {
            "repository": "appergb/OpenTake",
            "tag": "v1.0.0-beta.3",
            "version": "1.0.0-beta.3",
            "source_sha": SOURCE_SHA,
            "platform": "darwin-aarch64",
            "artifact": self.artifact,
            "output": self.output,
        }
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with self.assertRaisesRegex(
                    generator.AttestationError,
                    "repository|tag|version|source SHA|platform",
                ):
                    generator.write_attestation(**(defaults | mutation))

    def test_accepts_installer_specific_windows_platforms_and_shapes(self) -> None:
        generator = self.generator()
        for platform, filename in (
            ("windows-x86_64-msi", "OpenTake_1.0.0-beta.3_x64.msi"),
            ("windows-x86_64-nsis", "OpenTake_1.0.0-beta.3_x64-setup.exe"),
        ):
            artifact = self.root / filename
            artifact.write_bytes(platform.encode("ascii"))
            with self.subTest(platform=platform):
                value = generator.write_attestation(
                    repository="appergb/OpenTake",
                    tag="v1.0.0-beta.3",
                    version="1.0.0-beta.3",
                    source_sha=SOURCE_SHA,
                    platform=platform,
                    artifact=artifact,
                    output=Path(f"{artifact}.attestation.json"),
                )
                self.assertEqual(platform, value["platform"])

    def test_rejects_wrong_output_name_and_empty_or_symlink_payload(self) -> None:
        generator = self.generator()
        wrong_output = self.root / "latest.json"
        with self.assertRaisesRegex(generator.AttestationError, "filename"):
            generator.write_attestation(
                repository="appergb/OpenTake",
                tag="v1.0.0-beta.3",
                version="1.0.0-beta.3",
                source_sha=SOURCE_SHA,
                platform="darwin-aarch64",
                artifact=self.artifact,
                output=wrong_output,
            )
        empty = self.root / "empty.app.tar.gz"
        empty.write_bytes(b"")
        for artifact in (empty, self.root / "linked.app.tar.gz"):
            if artifact.name.startswith("linked"):
                artifact.symlink_to(self.artifact)
            with self.subTest(artifact=artifact.name):
                with self.assertRaisesRegex(
                    generator.AttestationError, "regular|empty"
                ):
                    generator.write_attestation(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        platform="darwin-aarch64",
                        artifact=artifact,
                        output=Path(f"{artifact}.attestation.json"),
                    )

    def test_rejects_artifact_names_that_require_url_percent_encoding(self) -> None:
        generator = self.generator()
        artifact = self.root / "Open Take.app.tar.gz"
        artifact.write_bytes(b"unsafe URL filename")
        with self.assertRaisesRegex(generator.AttestationError, "filename|URL"):
            generator.write_attestation(
                repository="appergb/OpenTake",
                tag="v1.0.0-beta.3",
                version="1.0.0-beta.3",
                source_sha=SOURCE_SHA,
                platform="darwin-aarch64",
                artifact=artifact,
                output=Path(f"{artifact}.attestation.json"),
            )


if __name__ == "__main__":
    unittest.main()
