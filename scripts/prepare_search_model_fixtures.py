#!/usr/bin/env python3
"""Fetch opt-in real-model test fixtures using the exported product manifest."""

import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import urllib.request


def fetch(url, destination, expected_hash, expected_size=None):
    def valid(path):
        if not path.is_file() or (expected_size is not None and path.stat().st_size != expected_size):
            return False
        digest = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(chunk)
        return digest.hexdigest() == expected_hash

    if valid(destination):
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as staged:
        temporary = Path(staged.name)
        try:
            with urllib.request.urlopen(url, timeout=120) as response:
                total = 0
                while chunk := response.read(1024 * 1024):
                    total += len(chunk)
                    if total > (expected_size if expected_size is not None else 10_000_000):
                        raise ValueError(f"Oversized fixture: {destination.name}")
                    staged.write(chunk)
            staged.close()
            if not valid(temporary):
                raise ValueError(f"Fixture checksum/size mismatch: {destination.name}")
            temporary.replace(destination)
        finally:
            staged.close()
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    exported = json.loads(args.manifest.read_text(encoding="utf-8-sig"))
    root = args.output.resolve()
    for key in ("imageEncoder", "textEncoder", "tokenizer"):
        asset = exported["manifest"][key]
        target = (root / asset["name"]).resolve()
        if not target.is_relative_to(root):
            raise ValueError("Fixture path must remain inside the output directory")
        fetch(f'{exported["base_url"]}/{asset["name"]}', target, asset["sha256"], asset["bytes"])
    images = (
        ("cats.png", "coco_sample.png", "cf6f3c4befa148732c7453e0de5afab00f682427435fead2d88b07a9615cdac2"),
        ("parrots.png", "hub/parrots.png", "d14e9adf584087f478dc9231c64caf6631d363dfd2188b10a4bd1c0a4020d082"),
    )
    for name, source, digest in images:
        fetch(f"https://huggingface.co/datasets/huggingface/documentation-images/resolve/main/{source}", root / name, digest)
    print("Verified all real-model fixtures")


if __name__ == "__main__":
    main()
