#!/usr/bin/env python3
"""Measure packaged macOS title-bar control alignment from PNG sample rectangles."""

from __future__ import annotations

import argparse
import math
import struct
import sys
import zlib
from dataclasses import dataclass
from pathlib import Path


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
MAX_PNG_BYTES = 64 * 1024 * 1024
MAX_PNG_CHUNKS = 4096
MAX_DECODED_BYTES = 256 * 1024 * 1024
ALIGNMENT_TOLERANCE_CSS_PX = 1.0
KNOWN_CRITICAL_CHUNKS = {b"IHDR", b"PLTE", b"IDAT", b"IEND"}
VALID_BIT_DEPTHS = {
    0: {1, 2, 4, 8, 16},
    2: {8, 16},
    3: {1, 2, 4, 8},
    4: {8, 16},
    6: {8, 16},
}


@dataclass(frozen=True)
class SampleRect:
    name: str
    x: float
    y: float
    width: float
    height: float

    @property
    def center_y(self) -> float:
        return self.y + self.height / 2


def parse_rect(raw: str) -> SampleRect:
    try:
        name, coordinates = raw.split(":", 1)
        values = tuple(float(value) for value in coordinates.split(","))
    except ValueError as error:
        raise argparse.ArgumentTypeError(
            "rectangle must use NAME:X,Y,WIDTH,HEIGHT"
        ) from error
    if not name or len(values) != 4 or not all(math.isfinite(value) for value in values):
        raise argparse.ArgumentTypeError("rectangle must use NAME:X,Y,WIDTH,HEIGHT")
    x, y, width, height = values
    if x < 0 or y < 0 or width <= 0 or height <= 0:
        raise argparse.ArgumentTypeError("rectangle coordinates must be non-negative and sized")
    return SampleRect(name=name, x=x, y=y, width=width, height=height)


def png_dimensions(path: Path) -> tuple[int, int]:
    try:
        file_size = path.stat().st_size
        if file_size > MAX_PNG_BYTES:
            raise ValueError(f"PNG exceeds the {MAX_PNG_BYTES}-byte evidence limit")
        with path.open("rb") as image:
            if image.read(8) != PNG_SIGNATURE:
                raise ValueError("file does not have a PNG signature")

            width = 0
            height = 0
            saw_ihdr = False
            saw_plte = False
            saw_idat = False
            idat_closed = False
            bit_depth = 0
            color_type = -1
            row_stride = 0
            expected_decoded = 0
            decoded_count = 0
            decoded_rows = 0
            decoded_pending = bytearray()
            decompressor = None

            def consume_decoded(data: bytes) -> None:
                nonlocal decoded_count, decoded_rows
                decoded_count += len(data)
                if decoded_count > expected_decoded:
                    raise ValueError("PNG decoded pixel data does not match IHDR")
                decoded_pending.extend(data)
                consumed = 0
                while len(decoded_pending) - consumed >= row_stride:
                    if decoded_pending[consumed] > 4:
                        raise ValueError("PNG scanline uses an invalid filter type")
                    consumed += row_stride
                    decoded_rows += 1
                if consumed:
                    del decoded_pending[:consumed]

            def feed_idat(data: bytes) -> None:
                if decompressor is None:
                    raise ValueError("PNG IDAT decoder was not initialized")
                compressed = data
                while compressed:
                    before = len(compressed)
                    output_limit = min(
                        1024 * 1024,
                        max(1, expected_decoded - decoded_count + 1),
                    )
                    try:
                        output = decompressor.decompress(compressed, output_limit)
                    except zlib.error as error:
                        raise ValueError("PNG IDAT is not a valid zlib stream") from error
                    compressed = decompressor.unconsumed_tail
                    consume_decoded(output)
                    if decompressor.unused_data:
                        raise ValueError("PNG IDAT contains bytes after the zlib stream")
                    if compressed and not output and len(compressed) >= before:
                        raise ValueError("PNG IDAT decoder made no progress")

            for _ in range(MAX_PNG_CHUNKS):
                chunk_header = image.read(8)
                if not chunk_header:
                    missing = []
                    if not saw_idat:
                        missing.append("IDAT")
                    missing.append("IEND")
                    raise ValueError(f"PNG is missing {' and '.join(missing)}")
                if len(chunk_header) != 8:
                    raise ValueError("truncated PNG chunk header")
                chunk_length, chunk_type = struct.unpack(">I4s", chunk_header)
                if not all(
                    ord("A") <= value <= ord("Z") or ord("a") <= value <= ord("z")
                    for value in chunk_type
                ):
                    raise ValueError("PNG chunk type contains non-letter bytes")
                if chunk_type[0] & 0x20 == 0 and chunk_type not in KNOWN_CRITICAL_CHUNKS:
                    raise ValueError(f"unknown critical PNG chunk {chunk_type!r}")
                if chunk_length > MAX_PNG_BYTES:
                    raise ValueError("PNG chunk exceeds the evidence size limit")
                if not saw_ihdr and (chunk_type != b"IHDR" or chunk_length != 13):
                    raise ValueError("PNG must begin with one 13-byte IHDR chunk")
                if saw_ihdr and chunk_type == b"IHDR":
                    raise ValueError("PNG contains more than one IHDR chunk")
                if chunk_type == b"IDAT":
                    if idat_closed:
                        raise ValueError("PNG IDAT chunks must be consecutive")
                    if not saw_idat:
                        if color_type == 3 and not saw_plte:
                            raise ValueError("indexed-color PNG requires PLTE before IDAT")
                        decompressor = zlib.decompressobj()

                crc = zlib.crc32(chunk_type)
                remaining = chunk_length
                captured = bytearray()
                while remaining:
                    block = image.read(min(remaining, 64 * 1024))
                    if not block:
                        raise ValueError("truncated PNG chunk data")
                    crc = zlib.crc32(block, crc)
                    if chunk_type == b"IHDR":
                        captured.extend(block)
                    elif chunk_type == b"IDAT":
                        feed_idat(block)
                    remaining -= len(block)
                stored_crc = image.read(4)
                if len(stored_crc) != 4:
                    raise ValueError("truncated PNG chunk CRC")
                if struct.unpack(">I", stored_crc)[0] != crc & 0xFFFFFFFF:
                    raise ValueError(f"{chunk_type.decode('ascii')} CRC mismatch")

                if chunk_type == b"IHDR":
                    width, height, bit_depth, color_type, compression, filtering, interlace = (
                        struct.unpack(">IIBBBBB", captured)
                    )
                    if width == 0 or height == 0 or width >= 2**31 or height >= 2**31:
                        raise ValueError("PNG dimensions are outside the PNG specification")
                    if bit_depth not in VALID_BIT_DEPTHS.get(color_type, set()):
                        raise ValueError("PNG bit depth and color type are incompatible")
                    if compression != 0 or filtering != 0 or interlace not in (0, 1):
                        raise ValueError("PNG IHDR uses an unsupported encoding method")
                    if interlace != 0:
                        raise ValueError("packaged evidence PNG must be non-interlaced")
                    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}[color_type]
                    row_stride = 1 + (width * channels * bit_depth + 7) // 8
                    expected_decoded = row_stride * height
                    if expected_decoded > MAX_DECODED_BYTES:
                        raise ValueError(
                            f"PNG decoded pixels exceed the {MAX_DECODED_BYTES}-byte evidence limit"
                        )
                    saw_ihdr = True
                elif chunk_type == b"PLTE":
                    if saw_plte or saw_idat:
                        raise ValueError("PNG PLTE must appear at most once before IDAT")
                    if color_type in (0, 4):
                        raise ValueError("grayscale PNG must not contain PLTE")
                    if chunk_length == 0 or chunk_length % 3 != 0 or chunk_length > 768:
                        raise ValueError("PNG PLTE has an invalid palette size")
                    if color_type == 3 and chunk_length // 3 > 2**bit_depth:
                        raise ValueError("PNG PLTE exceeds the indexed bit depth")
                    saw_plte = True
                elif chunk_type == b"IDAT":
                    saw_idat = True
                elif chunk_type == b"IEND":
                    if chunk_length != 0:
                        raise ValueError("PNG IEND chunk must be empty")
                    if not saw_idat:
                        raise ValueError("PNG is missing IDAT")
                    try:
                        consume_decoded(decompressor.flush())
                    except zlib.error as error:
                        raise ValueError("PNG IDAT is not a valid zlib stream") from error
                    if not decompressor.eof:
                        raise ValueError("PNG IDAT zlib stream is truncated")
                    if decompressor.unused_data:
                        raise ValueError("PNG IDAT contains bytes after the zlib stream")
                    if (
                        decoded_count != expected_decoded
                        or decoded_rows != height
                        or decoded_pending
                    ):
                        raise ValueError("PNG decoded pixel data does not match IHDR")
                    if image.read(1):
                        raise ValueError("PNG contains trailing bytes after IEND")
                    return width, height
                elif saw_idat:
                    idat_closed = True

            raise ValueError(f"PNG exceeds the {MAX_PNG_CHUNKS}-chunk evidence limit")
    except OSError as error:
        raise ValueError(f"cannot read PNG: {error}") from error


def validate_rect(rect: SampleRect, image_width: int, image_height: int) -> None:
    if rect.x + rect.width > image_width or rect.y + rect.height > image_height:
        raise ValueError(
            f"{rect.name} rectangle is outside the {image_width}x{image_height} PNG"
        )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Compare traffic-light and title-bar icon vertical centers measured "
            "from a packaged-app PNG. Coordinates are physical image pixels."
        )
    )
    parser.add_argument("image", type=Path, help="packaged-app PNG")
    parser.add_argument("--scale", type=float, required=True, help="PNG pixels per CSS pixel")
    parser.add_argument(
        "--traffic-rect",
        type=parse_rect,
        required=True,
        help="traffic-light group as NAME:X,Y,WIDTH,HEIGHT",
    )
    parser.add_argument(
        "--icon-rect",
        type=parse_rect,
        action="append",
        required=True,
        help="title-bar icon sample as NAME:X,Y,WIDTH,HEIGHT; repeat for each icon",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if not math.isfinite(args.scale) or args.scale <= 0:
        parser.error("--scale must be a positive finite number")

    try:
        width, height = png_dimensions(args.image)
        samples = [args.traffic_rect, *args.icon_rect]
        if len({sample.name for sample in samples}) != len(samples):
            raise ValueError("sample names must be unique")
        for sample in samples:
            validate_rect(sample, width, height)
    except ValueError as error:
        parser.error(str(error))

    traffic_center = args.traffic_rect.center_y / args.scale
    deviations: list[float] = []
    print(f"image: {args.image} ({width}x{height}, scale {args.scale:g}x)")
    print(f"{args.traffic_rect.name}: center {traffic_center:.3f} CSS px")
    for icon in args.icon_rect:
        center = icon.center_y / args.scale
        deviation = abs(center - traffic_center)
        deviations.append(deviation)
        print(f"{icon.name}: center {center:.3f} CSS px; deviation {deviation:.3f} CSS px")

    maximum = max(deviations)
    passed = maximum <= ALIGNMENT_TOLERANCE_CSS_PX
    print(f"maximum deviation: {maximum:.3f} CSS px")
    print(
        "PASS"
        if passed
        else f"FAIL (tolerance {ALIGNMENT_TOLERANCE_CSS_PX:.3f} CSS px)"
    )
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
