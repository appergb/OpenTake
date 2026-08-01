from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
import zipfile


ROOT = Path(__file__).resolve().parents[2]
MODULE_PATH = ROOT / "scripts" / "provision_ffmpeg_sidecars.py"
SPEC = importlib.util.spec_from_file_location("provision_ffmpeg_sidecars", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
provisioner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provisioner)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class ProvisionFfmpegSidecarsTests(unittest.TestCase):
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
