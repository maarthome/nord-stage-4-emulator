#!/usr/bin/env python3
"""Extract the files of a Kontakt resource container (``.nkr``) such as the UP2 GUI pictures.

Layout found on ``Ultimate Pianos 2.nkr`` (not encrypted), little endian:

* The file starts with a directory. Each node is ``u16 size, u32 offset, u16 type,
  UTF-16 name, u16 0`` with ``size = 2 + 4 + 2 + 2 * chars + 2``. Type 1 is a folder
  (``Resources``, ``pictures``, ``scripts``, ``data``, ``ir_samples``,
  ``performance_view``; the offset points at a block that holds the child count), type 4
  is a file whose ``offset`` is its data block. Folder nodes are followed by their
  children's blocks of 6-byte headers; this tool reads only the flat file list.
* A data block is a 22-byte header, then the file bytes. Header: magic
  ``FA 05 E9 2A``, ``u16 0x0111``, 4 zero bytes, ``FF 00 00 00``, ``u32 size`` at +14, 4
  zero bytes at +18. In UP2 each picture ``X.PNG`` has a sidecar ``X.txt`` with
  attributes (animation frame count, resize and fixed margins).

VERIFIED on the UP2 file: 149 PNG and 149 txt, every payload starts with the PNG
signature, and the next block starts exactly ``22 + size`` bytes after the previous one
(``tools/tests/test_nkr_extract.py`` rebuilds that rule on a synthetic container). Only
the UP2 file was checked; other libraries may differ (ASSUMED).
"""

from __future__ import annotations

import argparse
import csv
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

MAGIC = bytes.fromhex("fa05e92a")
HEADER = 22
SIZE_AT = 14
TYPE_FOLDER = 1
TYPE_FILE = 4
ENTRY_RE = re.compile(rb"([\x10-\xff])\x00(.{4})(.{2})((?:[\x20-\x7e]\x00)+)\x00\x00", re.S)
SCAN_LIMIT = 1 << 20
PNG_SIG = b"\x89PNG\r\n\x1a\n"
BAD_CHARS = re.compile(r'[<>:"/\\|?*]')


class NkrError(ValueError):
    pass


@dataclass
class Entry:
    name: str
    offset: int
    type: int


def parse_entries(data: bytes) -> list[Entry]:
    """Directory nodes (folders and files), in file order."""
    found: list[tuple[int, Entry]] = []
    for m in ENTRY_RE.finditer(data, 0, min(len(data), SCAN_LIMIT)):
        name = m.group(4).decode("utf-16le")
        if m.group(1)[0] != 2 + 4 + 2 + 2 * len(name) + 2:
            continue
        kind = struct.unpack("<H", m.group(3))[0]
        if kind not in (TYPE_FOLDER, TYPE_FILE):
            continue
        offset = struct.unpack("<I", m.group(2))[0]
        found.append((m.start(), Entry(name, offset, kind)))
    starts = [e.offset for _, e in found if e.type == TYPE_FILE]
    if not starts:
        return []
    data_start = min(starts)
    return [e for pos, e in found if pos < data_start]


def read_file(data: bytes, entry: Entry) -> bytes:
    """Payload of a file node."""
    off = entry.offset
    if data[off : off + 4] != MAGIC:
        raise NkrError(f"{entry.name}: bad block magic at {off}")
    (size,) = struct.unpack_from("<I", data, off + SIZE_AT)
    end = off + HEADER + size
    if end > len(data):
        raise NkrError(f"{entry.name}: block runs past the end of the file")
    return data[off + HEADER : end]


def png_size(payload: bytes) -> tuple[int, int] | None:
    """(width, height) from the IHDR chunk, or None when not a PNG."""
    if payload[:8] != PNG_SIG or payload[12:16] != b"IHDR":
        return None
    return struct.unpack(">II", payload[16:24])


def safe_name(name: str) -> str:
    return BAD_CHARS.sub("_", name)


def extract(data: bytes, out_dir: Path) -> list[dict[str, object]]:
    """Write every file node to ``out_dir`` and return one manifest row per file."""
    out_dir.mkdir(parents=True, exist_ok=True)
    rows: list[dict[str, object]] = []
    for e in parse_entries(data):
        if e.type != TYPE_FILE:
            continue
        payload = read_file(data, e)
        (out_dir / safe_name(e.name)).write_bytes(payload)
        dims = png_size(payload)
        rows.append(
            {
                "name": e.name,
                "bytes": len(payload),
                "width": dims[0] if dims else "",
                "height": dims[1] if dims else "",
                "text": payload.decode("latin-1").replace("\r\n", "; ")
                if e.name.lower().endswith(".txt")
                else "",
            }
        )
    return rows


def write_manifest(path: Path, rows: list[dict[str, object]]) -> None:
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.DictWriter(fh, fieldnames=["name", "bytes", "width", "height", "text"])
        w.writeheader()
        w.writerows(rows)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(
        description="Extract files from a Kontakt .nkr resource container."
    )
    ap.add_argument("nkr", type=Path)
    ap.add_argument("out_dir", type=Path)
    a = ap.parse_args(argv)
    rows = extract(a.nkr.read_bytes(), a.out_dir)
    write_manifest(a.out_dir / "manifest.csv", rows)
    pngs = sum(1 for r in rows if r["width"] != "")
    print(f"{len(rows)} files, {pngs} PNG -> {a.out_dir}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
