from __future__ import annotations

import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPOSITORY_ROOT / "scripts" / "write_updater_manifest.py"
SOURCE_SHA = "a" * 40


def load_generator():
    if not MODULE_PATH.is_file():
        return None
    spec = importlib.util.spec_from_file_location("write_updater_manifest", MODULE_PATH)
    if spec is None or spec.loader is None:
        return None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class UpdaterManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.mac = self.root / "OpenTake.app.tar.gz"
        self.mac_sig = self.root / "OpenTake.app.tar.gz.sig"
        self.mac_attestation = self.root / "OpenTake.app.tar.gz.attestation.json"
        self.mac_attestation_sig = self.root / "OpenTake.app.tar.gz.attestation.json.sig"
        self.msi = self.root / "OpenTake_1.0.0-beta.3_x64.msi"
        self.msi_sig = self.root / "OpenTake_1.0.0-beta.3_x64.msi.sig"
        self.msi_attestation = self.root / (
            "OpenTake_1.0.0-beta.3_x64.msi.attestation.json"
        )
        self.msi_attestation_sig = self.root / (
            "OpenTake_1.0.0-beta.3_x64.msi.attestation.json.sig"
        )
        self.nsis = self.root / "OpenTake_1.0.0-beta.3_x64-setup.exe"
        self.nsis_sig = self.root / "OpenTake_1.0.0-beta.3_x64-setup.exe.sig"
        self.nsis_attestation = self.root / (
            "OpenTake_1.0.0-beta.3_x64-setup.exe.attestation.json"
        )
        self.nsis_attestation_sig = self.root / (
            "OpenTake_1.0.0-beta.3_x64-setup.exe.attestation.json.sig"
        )
        self.mac.write_bytes(b"mac updater bytes")
        self.mac_sig.write_text("mac-signature\n", encoding="utf-8")
        self.msi.write_bytes(b"windows MSI updater bytes")
        self.msi_sig.write_text("windows-msi-signature\n", encoding="utf-8")
        self.nsis.write_bytes(b"windows NSIS updater bytes")
        self.nsis_sig.write_text("windows-nsis-signature\n", encoding="utf-8")
        self.write_attestation(
            self.mac, self.mac_attestation, "darwin-aarch64"
        )
        self.mac_attestation_sig.write_text(
            "mac-attestation-signature\n", encoding="utf-8"
        )
        self.write_attestation(
            self.msi, self.msi_attestation, "windows-x86_64-msi"
        )
        self.msi_attestation_sig.write_text(
            "windows-msi-attestation-signature\n", encoding="utf-8"
        )
        self.write_attestation(
            self.nsis, self.nsis_attestation, "windows-x86_64-nsis"
        )
        self.nsis_attestation_sig.write_text(
            "windows-nsis-attestation-signature\n", encoding="utf-8"
        )
        self.output = self.root / "updater-v1.0.0-beta.3.json"

    def write_attestation(
        self, artifact: Path, output: Path, platform: str
    ) -> None:
        value = {
            "schemaVersion": 1,
            "repository": "appergb/OpenTake",
            "tag": "v1.0.0-beta.3",
            "version": "1.0.0-beta.3",
            "sourceSha": SOURCE_SHA,
            "platform": platform,
            "assetName": artifact.name,
            "size": artifact.stat().st_size,
            "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
        }
        output.write_text(
            json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )

    def generator(self):
        module = load_generator()
        self.assertIsNotNone(module, "updater manifest generator must exist")
        return module

    def artifacts(self) -> dict[str, tuple[Path, Path, Path, Path]]:
        return {
            "darwin-aarch64": (
                self.mac,
                self.mac_sig,
                self.mac_attestation,
                self.mac_attestation_sig,
            ),
            "windows-x86_64-msi": (
                self.msi,
                self.msi_sig,
                self.msi_attestation,
                self.msi_attestation_sig,
            ),
            "windows-x86_64-nsis": (
                self.nsis,
                self.nsis_sig,
                self.nsis_attestation,
                self.nsis_attestation_sig,
            ),
        }

    def write(self):
        return self.generator().write_manifest(
            repository="appergb/OpenTake",
            tag="v1.0.0-beta.3",
            version="1.0.0-beta.3",
            source_sha=SOURCE_SHA,
            artifacts=self.artifacts(),
            output=self.output,
        )

    def test_writes_exact_tag_specific_tauri_v2_manifest(self) -> None:
        self.write()
        self.assertEqual(
            {
                "version": "1.0.0-beta.3",
                "platforms": {
                    "darwin-aarch64": {
                        "attestationSignature": "mac-attestation-signature",
                        "attestationUrl": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/"
                            "OpenTake.app.tar.gz.attestation.json"
                        ),
                        "signature": "mac-signature",
                        "url": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/OpenTake.app.tar.gz"
                        ),
                    },
                    "windows-x86_64-msi": {
                        "attestationSignature": "windows-msi-attestation-signature",
                        "attestationUrl": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/"
                            "OpenTake_1.0.0-beta.3_x64.msi.attestation.json"
                        ),
                        "signature": "windows-msi-signature",
                        "url": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/"
                            "OpenTake_1.0.0-beta.3_x64.msi"
                        ),
                    },
                    "windows-x86_64-nsis": {
                        "attestationSignature": "windows-nsis-attestation-signature",
                        "attestationUrl": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/"
                            "OpenTake_1.0.0-beta.3_x64-setup.exe.attestation.json"
                        ),
                        "signature": "windows-nsis-signature",
                        "url": (
                            "https://github.com/appergb/OpenTake/releases/download/"
                            "v1.0.0-beta.3/"
                            "OpenTake_1.0.0-beta.3_x64-setup.exe"
                        ),
                    },
                },
            },
            json.loads(self.output.read_text(encoding="utf-8")),
        )

    def test_rejects_any_repository_other_than_fixed_opentake_origin(self) -> None:
        generator = self.generator()
        with self.assertRaisesRegex(generator.ManifestError, "repository"):
            generator.write_manifest(
                repository="attacker/OpenTake",
                tag="v1.0.0-beta.3",
                version="1.0.0-beta.3",
                source_sha=SOURCE_SHA,
                artifacts=self.artifacts(),
                output=self.output,
            )

    def test_rejects_tag_version_mismatch_and_non_beta_version(self) -> None:
        generator = self.generator()
        for tag, version in (
            ("v1.0.0-beta.2", "1.0.0-beta.3"),
            ("v1.0.0", "1.0.0"),
            ("main", "1.0.0-beta.3"),
        ):
            with self.subTest(tag=tag, version=version):
                with self.assertRaisesRegex(generator.ManifestError, "tag|prerelease"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag=tag,
                        version=version,
                        source_sha=SOURCE_SHA,
                        artifacts=self.artifacts(),
                        output=self.output,
                    )

    def test_rejects_semver_build_metadata_before_forming_asset_urls(self) -> None:
        generator = self.generator()
        with self.assertRaisesRegex(generator.ManifestError, "version|metadata"):
            generator.write_manifest(
                repository="appergb/OpenTake",
                tag="v1.0.0-beta.3+hotfix.1",
                version="1.0.0-beta.3+hotfix.1",
                source_sha=SOURCE_SHA,
                artifacts=self.artifacts(),
                output=self.root / "updater-v1.0.0-beta.3+hotfix.1.json",
            )

    def test_requires_exact_real_release_platforms(self) -> None:
        generator = self.generator()
        for artifacts in (
            {"darwin-aarch64": self.artifacts()["darwin-aarch64"]},
            {
                "darwin-aarch64": self.artifacts()["darwin-aarch64"],
                "windows-x86_64": self.artifacts()["windows-x86_64-nsis"],
            },
            {
                **self.artifacts(),
                "darwin-x86_64": self.artifacts()["darwin-aarch64"],
            },
        ):
            with self.subTest(platforms=sorted(artifacts)):
                with self.assertRaisesRegex(generator.ManifestError, "platform"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=artifacts,
                        output=self.output,
                    )

    def test_requires_tauri_v2_updater_asset_shapes(self) -> None:
        generator = self.generator()
        bad_mac = self.root / "OpenTake.dmg"
        bad_mac.write_bytes(b"dmg")
        bad_msi = self.root / "OpenTake-msi.exe"
        bad_msi.write_bytes(b"exe")
        bad_nsis = self.root / "OpenTake-nsis.msi"
        bad_nsis.write_bytes(b"msi")
        for platform, artifact in (
            ("darwin-aarch64", bad_mac),
            ("windows-x86_64-msi", bad_msi),
            ("windows-x86_64-nsis", bad_nsis),
        ):
            artifacts = self.artifacts()
            signature = self.root / f"{artifact.name}.sig"
            signature.write_text("signature\n", encoding="utf-8")
            current = artifacts[platform]
            artifacts[platform] = (artifact, signature, current[2], current[3])
            with self.subTest(platform=platform):
                with self.assertRaisesRegex(generator.ManifestError, "artifact"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=artifacts,
                        output=self.output,
                    )

    def test_rejects_artifact_names_that_require_url_percent_encoding(self) -> None:
        generator = self.generator()
        artifact = self.root / "Open Take_1.0.0-beta.3_x64.msi"
        signature = Path(f"{artifact}.sig")
        attestation = Path(f"{artifact}.attestation.json")
        attestation_signature = Path(f"{attestation}.sig")
        artifact.write_bytes(b"unsafe URL filename")
        signature.write_text("signature\n", encoding="utf-8")
        self.write_attestation(artifact, attestation, "windows-x86_64-msi")
        attestation_signature.write_text("attestation-signature\n", encoding="utf-8")
        artifacts = self.artifacts()
        artifacts["windows-x86_64-msi"] = (
            artifact,
            signature,
            attestation,
            attestation_signature,
        )
        with self.assertRaisesRegex(generator.ManifestError, "filename|URL"):
            generator.write_manifest(
                repository="appergb/OpenTake",
                tag="v1.0.0-beta.3",
                version="1.0.0-beta.3",
                source_sha=SOURCE_SHA,
                artifacts=artifacts,
                output=self.output,
            )

    def test_requires_nonempty_companion_signature_files(self) -> None:
        generator = self.generator()
        wrong_signature = self.root / "detached.sig"
        wrong_signature.write_text("signature", encoding="utf-8")
        empty_signature = self.root / f"{self.mac.name}.sig"
        empty_signature.write_text("", encoding="utf-8")
        for signature in (wrong_signature, empty_signature):
            artifacts = self.artifacts()
            artifacts["darwin-aarch64"] = (
                self.mac,
                signature,
                self.mac_attestation,
                self.mac_attestation_sig,
            )
            with self.subTest(signature=signature.name):
                with self.assertRaisesRegex(generator.ManifestError, "signature"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=artifacts,
                        output=self.output,
                    )

    def test_validator_rejects_forged_manifest_fields(self) -> None:
        generator = self.generator()
        manifest = self.write()
        mutations = (
            ("http URL", lambda value: value["platforms"]["darwin-aarch64"].update(
                url="http://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.3/OpenTake.app.tar.gz"
            )),
            ("wrong repository", lambda value: value["platforms"]["darwin-aarch64"].update(
                url="https://github.com/attacker/OpenTake/releases/download/v1.0.0-beta.3/OpenTake.app.tar.gz"
            )),
            ("wrong tag", lambda value: value["platforms"]["darwin-aarch64"].update(
                url="https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.2/OpenTake.app.tar.gz"
            )),
            ("wrong asset", lambda value: value["platforms"]["darwin-aarch64"].update(
                url="https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.3/other.app.tar.gz"
            )),
            ("wrong version", lambda value: value.update(version="1.0.0-beta.2")),
            ("forged signature", lambda value: value["platforms"]["darwin-aarch64"].update(
                signature="forged"
            )),
            ("wrong attestation URL", lambda value: value["platforms"]["darwin-aarch64"].update(
                attestationUrl="https://github.com/appergb/OpenTake/releases/download/v1.0.0-beta.3/forged.attestation.json"
            )),
            ("forged attestation signature", lambda value: value["platforms"]["darwin-aarch64"].update(
                attestationSignature="forged"
            )),
        )
        for name, mutate in mutations:
            with self.subTest(mutation=name):
                value = json.loads(json.dumps(manifest))
                mutate(value)
                with self.assertRaisesRegex(
                    generator.ManifestError, "version|URL|signature"
                ):
                    generator.validate_manifest(
                        value,
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=self.artifacts(),
                    )

    def test_validator_rejects_missing_or_extra_architecture(self) -> None:
        generator = self.generator()
        manifest = self.write()
        without_windows = json.loads(json.dumps(manifest))
        del without_windows["platforms"]["windows-x86_64-msi"]
        with_extra = json.loads(json.dumps(manifest))
        with_extra["platforms"]["darwin-x86_64"] = dict(
            with_extra["platforms"]["darwin-aarch64"]
        )
        for value in (without_windows, with_extra):
            with self.assertRaisesRegex(generator.ManifestError, "platform"):
                generator.validate_manifest(
                    value,
                    repository="appergb/OpenTake",
                    tag="v1.0.0-beta.3",
                    version="1.0.0-beta.3",
                    source_sha=SOURCE_SHA,
                    artifacts=self.artifacts(),
                )

    def test_rejects_attestation_not_bound_to_release_and_payload(self) -> None:
        generator = self.generator()
        original = json.loads(self.mac_attestation.read_text(encoding="utf-8"))
        mutations = (
            {"schemaVersion": 2},
            {"schemaVersion": True},
            {"repository": "attacker/OpenTake"},
            {"tag": "v1.0.0-beta.2"},
            {"version": "1.0.0-beta.2"},
            {"sourceSha": "b" * 40},
            {"platform": "darwin-x86_64"},
            {"assetName": "old.app.tar.gz"},
            {"size": 1},
            {"sha256": "0" * 64},
            {"extra": True},
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                forged = original | mutation
                self.mac_attestation.write_text(
                    json.dumps(forged, sort_keys=True, separators=(",", ":")) + "\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(generator.ManifestError, "attestation"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=self.artifacts(),
                        output=self.output,
                    )
                self.write_attestation(
                    self.mac, self.mac_attestation, "darwin-aarch64"
                )

    def test_rejects_noncompanion_or_empty_attestation_signature(self) -> None:
        generator = self.generator()
        wrong = self.root / "detached-attestation.sig"
        wrong.write_text("signature", encoding="utf-8")
        empty = self.mac_attestation_sig
        for signature in (wrong, empty):
            with self.subTest(signature=signature.name):
                if signature == empty:
                    empty.write_text("", encoding="utf-8")
                artifacts = self.artifacts()
                artifacts["darwin-aarch64"] = (
                    self.mac,
                    self.mac_sig,
                    self.mac_attestation,
                    signature,
                )
                with self.assertRaisesRegex(generator.ManifestError, "signature"):
                    generator.write_manifest(
                        repository="appergb/OpenTake",
                        tag="v1.0.0-beta.3",
                        version="1.0.0-beta.3",
                        source_sha=SOURCE_SHA,
                        artifacts=artifacts,
                        output=self.output,
                    )
            self.mac_attestation_sig.write_text(
                "mac-attestation-signature\n", encoding="utf-8"
            )


if __name__ == "__main__":
    unittest.main()
