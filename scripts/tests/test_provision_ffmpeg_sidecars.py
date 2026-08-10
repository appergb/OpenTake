from __future__ import annotations

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile


ROOT = Path(
    os.environ.get("OPENTAKE_REPOSITORY_ROOT", Path(__file__).resolve().parents[2])
).resolve()
MODULE_PATH = Path(
    os.environ.get(
        "OPENTAKE_PROVISIONER_PATH",
        ROOT / "scripts" / "provision_ffmpeg_sidecars.py",
    )
).resolve()
SPEC = importlib.util.spec_from_file_location("provision_ffmpeg_sidecars", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
provisioner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provisioner)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class ProvisionFfmpegSidecarsTests(unittest.TestCase):
    def test_repository_root_can_be_bound_when_tooling_runs_outside_checkout(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            expected_root = Path(directory).resolve()
            with mock.patch.dict(
                os.environ,
                {"OPENTAKE_REPOSITORY_ROOT": str(expected_root)},
            ):
                isolated_spec = importlib.util.spec_from_file_location(
                    "isolated_provision_ffmpeg_sidecars", MODULE_PATH
                )
                assert isolated_spec is not None and isolated_spec.loader is not None
                isolated = importlib.util.module_from_spec(isolated_spec)
                isolated_spec.loader.exec_module(isolated)

            self.assertEqual(isolated.ROOT, expected_root)
            self.assertEqual(
                isolated.BIN_DIR, expected_root / "src-tauri" / "binaries"
            )

    def test_provision_publishes_an_unexecuted_copy_when_windows_locks_images(
        self,
    ) -> None:
        binary = b"redistributable ffmpeg"
        expected_sha = digest(binary)
        record = {
            "url": "https://example.invalid/ffmpeg.exe",
            "sha256": expected_sha,
            "version": "7.0",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary_dir = root / "src-tauri" / "binaries"
            binary_dir.mkdir(parents=True)
            executed_paths: set[Path] = set()
            real_replace = os.replace

            def download_fixture(_url: str, path: Path) -> None:
                path.write_bytes(binary)

            def lock_executed_image(
                path: Path, _expected_sha: str, _version: str
            ) -> None:
                executed_paths.add(path)

            def windows_replace(source: Path, destination: Path) -> None:
                if source in executed_paths:
                    raise PermissionError(
                        32,
                        "The process cannot access the file because it is being used "
                        "by another process",
                        str(source),
                    )
                real_replace(source, destination)

            with (
                mock.patch.object(provisioner, "ROOT", root),
                mock.patch.object(provisioner, "BIN_DIR", binary_dir),
                mock.patch.object(provisioner, "download", download_fixture),
                mock.patch.object(provisioner, "verify", lock_executed_image),
                mock.patch.object(provisioner.os, "replace", windows_replace),
            ):
                provisioner.provision(
                    "ffmpeg", record, "x86_64-pc-windows-msvc"
                )

            final_path = binary_dir / "ffmpeg-x86_64-pc-windows-msvc.exe"
            self.assertEqual(final_path.read_bytes(), binary)
            self.assertTrue(executed_paths)
            self.assertTrue(
                all(binary_dir not in path.parents for path in executed_paths)
            )

    def test_cached_sidecar_is_probed_only_from_a_system_temporary_copy(self) -> None:
        binary = b"redistributable ffmpeg"
        expected_sha = digest(binary)
        record = {
            "url": "https://example.invalid/ffmpeg.exe",
            "sha256": expected_sha,
            "version": "7.0",
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary_dir = root / "src-tauri" / "binaries"
            binary_dir.mkdir(parents=True)
            final_path = binary_dir / "ffmpeg-x86_64-pc-windows-msvc.exe"
            final_path.write_bytes(binary)
            executed_paths: list[Path] = []

            def record_probe(
                path: Path, _expected_sha: str, _version: str
            ) -> None:
                executed_paths.append(path)

            with (
                mock.patch.object(provisioner, "ROOT", root),
                mock.patch.object(provisioner, "BIN_DIR", binary_dir),
                mock.patch.object(provisioner, "verify", record_probe),
            ):
                provisioner.provision(
                    "ffmpeg", record, "x86_64-pc-windows-msvc"
                )

            self.assertEqual(final_path.read_bytes(), binary)
            self.assertEqual(len(executed_paths), 1)
            self.assertNotEqual(executed_paths[0], final_path)
            self.assertNotIn(binary_dir, executed_paths[0].parents)

    def test_materializes_only_the_checksum_pinned_zip_member(self) -> None:
        binary = b"redistributable ffmpeg"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive_path = root / "sidecar.zip"
            destination = root / "ffmpeg"
            with zipfile.ZipFile(archive_path, "w") as archive:
                archive.writestr("ffmpeg", binary)
                archive.writestr("ignored", b"not selected")
            record = {
                "archive": {
                    "format": "zip",
                    "member": "ffmpeg",
                    "sha256": provisioner.sha256(archive_path),
                }
            }

            provisioner.materialize_download(record, archive_path, destination)

            self.assertEqual(destination.read_bytes(), binary)

    def test_rejects_archive_checksum_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive_path = root / "sidecar.zip"
            destination = root / "ffmpeg"
            with zipfile.ZipFile(archive_path, "w") as archive:
                archive.writestr("ffmpeg", b"unexpected")
            record = {
                "archive": {
                    "format": "zip",
                    "member": "ffmpeg",
                    "sha256": "0" * 64,
                }
            }

            with self.assertRaisesRegex(RuntimeError, "archive checksum mismatch"):
                provisioner.materialize_download(record, archive_path, destination)

            self.assertFalse(destination.exists())

    def test_verify_rejects_nonfree_or_unredistributable_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "ffmpeg"
            binary.write_bytes(b"fixture")
            expected_sha = digest(b"fixture")

            with mock.patch.object(
                provisioner.subprocess,
                "check_output",
                return_value="ffmpeg version 7.0\nconfiguration: --enable-nonfree\n",
            ):
                with self.assertRaisesRegex(RuntimeError, "nonfree sidecar"):
                    provisioner.verify(binary, expected_sha, "7.0")

            with mock.patch.object(
                provisioner.subprocess,
                "check_output",
                side_effect=[
                    "ffmpeg version 7.0\nconfiguration: --enable-gpl\n",
                    "This version is not legally redistributable.\n",
                ],
            ):
                with self.assertRaisesRegex(RuntimeError, "sidecar license"):
                    provisioner.verify(binary, expected_sha, "7.0")

            with mock.patch.object(
                provisioner.subprocess,
                "check_output",
                side_effect=[
                    "ffmpeg version 7.0\nconfiguration: --enable-gpl\n",
                    "GNU General Public License version 3 or later\n",
                ],
            ) as metadata:
                provisioner.verify(binary, expected_sha, "7.0")
                self.assertEqual(metadata.call_count, 2)

    def test_apple_silicon_lock_pins_archive_and_binary_hashes(self) -> None:
        lock = json.loads((ROOT / "scripts" / "ffmpeg-sidecars.lock.json").read_text())
        self.assertEqual(lock["schema"], "opentake-ffmpeg-sidecars-v2")
        target = lock["targets"]["aarch64-apple-darwin"]
        for tool in ("ffmpeg", "ffprobe"):
            record = target[tool]
            self.assertEqual(record["version"], "7.0")
            self.assertRegex(record["sha256"], r"^[0-9a-f]{64}$")
            self.assertEqual(record["archive"]["format"], "zip")
            self.assertEqual(record["archive"]["member"], tool)
            self.assertRegex(record["archive"]["sha256"], r"^[0-9a-f]{64}$")


if __name__ == "__main__":
    unittest.main()
