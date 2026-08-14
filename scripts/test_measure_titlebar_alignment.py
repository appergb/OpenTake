import struct
import subprocess
import sys
import tempfile
import unittest
import zlib
from pathlib import Path


SCRIPT = Path(__file__).with_name("measure_titlebar_alignment.py")


def png_chunk(kind: bytes, data: bytes) -> bytes:
    payload = kind + data
    return struct.pack(">I", len(data)) + payload + struct.pack(">I", zlib.crc32(payload))


def png_bytes(width: int = 400, height: int = 120) -> bytes:
    rows = b"".join(b"\0" + (b"\0\0\0" * width) for _ in range(height))
    return png_with_idat(zlib.compress(rows), width, height)


def png_with_idat(idat: bytes, width: int = 400, height: int = 120) -> bytes:
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + png_chunk(b"IDAT", idat)
        + png_chunk(b"IEND", b"")
    )


def write_png(path: Path, width: int = 400, height: int = 120) -> None:
    path.write_bytes(png_bytes(width, height))


class TitlebarAlignmentCliTests(unittest.TestCase):
    def run_measurement(self, *rectangles: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temp_dir:
            image = Path(temp_dir) / "titlebar.png"
            write_png(image)
            command = [
                sys.executable,
                str(SCRIPT),
                str(image),
                "--scale",
                "2",
                "--traffic-rect",
                "traffic:20,20,100,28",
            ]
            for rectangle in rectangles:
                command.extend(("--icon-rect", rectangle))
            return subprocess.run(command, capture_output=True, text=True, check=False)

    def run_invalid_image(self, payload: bytes) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temp_dir:
            image = Path(temp_dir) / "titlebar.png"
            image.write_bytes(payload)
            return subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    str(image),
                    "--scale",
                    "2",
                    "--traffic-rect",
                    "traffic:20,20,100,28",
                    "--icon-rect",
                    "home:160,22,24,24",
                ],
                capture_output=True,
                text=True,
                check=False,
            )

    def test_accepts_icon_centers_within_one_css_pixel(self) -> None:
        result = self.run_measurement(
            "home:160,22,24,24",
            "chat:196,20,28,28",
            "motion:236,18,32,32",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("maximum deviation: 0.000 CSS px", result.stdout)
        self.assertIn("PASS", result.stdout)

    def test_rejects_an_icon_center_more_than_one_css_pixel_away(self) -> None:
        result = self.run_measurement("settings:300,25,24,24")

        self.assertEqual(result.returncode, 1)
        self.assertIn("1.500 CSS px", result.stdout)
        self.assertIn("FAIL", result.stdout)

    def test_accepts_the_exact_one_css_pixel_boundary(self) -> None:
        result = self.run_measurement("editor:280,22,24,28")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("1.000 CSS px", result.stdout)
        self.assertIn("PASS", result.stdout)

    def test_rejects_sample_rectangles_outside_the_png(self) -> None:
        result = self.run_measurement("export:390,20,24,24")

        self.assertEqual(result.returncode, 2)
        self.assertIn("outside the 400x120 PNG", result.stderr)

    def test_does_not_allow_the_one_css_pixel_gate_to_be_relaxed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            image = Path(temp_dir) / "titlebar.png"
            write_png(image)
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    str(image),
                    "--scale",
                    "2",
                    "--traffic-rect",
                    "traffic:20,20,100,28",
                    "--icon-rect",
                    "settings:300,25,24,24",
                    "--tolerance",
                    "1.001",
                ],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 2)
        self.assertIn("unrecognized arguments: --tolerance 1.001", result.stderr)

    def test_rejects_a_truncated_png_header(self) -> None:
        result = self.run_invalid_image(png_bytes()[:24])

        self.assertEqual(result.returncode, 2)
        self.assertIn("truncated PNG chunk", result.stderr)

    def test_rejects_a_png_with_a_bad_chunk_crc(self) -> None:
        payload = bytearray(png_bytes())
        payload[-1] ^= 1

        result = self.run_invalid_image(bytes(payload))

        self.assertEqual(result.returncode, 2)
        self.assertIn("IEND CRC mismatch", result.stderr)

    def test_rejects_a_png_without_idat_or_iend(self) -> None:
        incomplete = png_bytes()
        idat_offset = incomplete.index(b"IDAT") - 4

        result = self.run_invalid_image(incomplete[:idat_offset])

        self.assertEqual(result.returncode, 2)
        self.assertIn("PNG is missing IDAT and IEND", result.stderr)

    def test_rejects_idat_that_is_not_a_zlib_stream(self) -> None:
        result = self.run_invalid_image(png_with_idat(b"not-a-zlib-stream"))

        self.assertEqual(result.returncode, 2)
        self.assertIn("IDAT is not a valid zlib stream", result.stderr)

    def test_rejects_a_truncated_deflate_stream(self) -> None:
        rows = b"".join(b"\0" + (b"\0\0\0" * 400) for _ in range(120))
        result = self.run_invalid_image(png_with_idat(zlib.compress(rows)[:-2]))

        self.assertEqual(result.returncode, 2)
        self.assertIn("IDAT zlib stream is truncated", result.stderr)

    def test_rejects_decoded_pixels_that_do_not_match_ihdr(self) -> None:
        one_row = b"\0" + (b"\0\0\0" * 400)
        result = self.run_invalid_image(png_with_idat(zlib.compress(one_row)))

        self.assertEqual(result.returncode, 2)
        self.assertIn("decoded pixel data does not match IHDR", result.stderr)


if __name__ == "__main__":
    unittest.main()
