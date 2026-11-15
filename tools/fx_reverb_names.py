#!/usr/bin/env python3
"""Value names of `Edt_Effects_kRvbType` and `kRvbColor` in the ARM image (OS v1.66).

The 12 reverb type names and the 3 colour names are consecutive pointers in one table
(`0xC02CD2BC`, `docs/research/fx-reverb-names.md`): index = edit value. This tool finds the table
by its first string and prints `index name`, so a different OS version can be checked.

    python tools/fx_reverb_names.py target/fw_unpacked/bank0_CODE.bin
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path

ARM_BASE = 0xC0000000
# VERIFIED (run of this tool on OS v1.66): the first name of the type list.
FIRST_NAME = b"Spring\x00"
# VERIFIED (same run): 12 type names, then Normal, Dark, Bright for `kRvbColor`.
TYPE_COUNT = 12
EXPECTED_TYPES = [
    "Spring",
    "Spring2",
    "Booth",
    "Booth Predelay",
    "Room",
    "Room Predelay",
    "Stage",
    "Stage Chorale",
    "Hall",
    "Hall Chorale",
    "Cathedral",
    "Cathedral Chorale",
]


def c_string(img: bytes, address: int) -> str:
    """The NUL-terminated string at ARM `address` (empty if outside the image)."""
    off = address - ARM_BASE
    if not 0 <= off < len(img):
        return ""
    return img[off : img.index(b"\0", off)].decode("latin1")


def find_names(img: bytes) -> tuple[int, list[str]]:
    """`(table address, names)`: the pointer table whose first entry is the string "Spring"."""
    start = 0
    while (pos := img.find(FIRST_NAME, start)) >= 0:
        start = pos + 1
        needle = struct.pack("<I", ARM_BASE + pos)
        at = img.find(needle)
        while at >= 0:
            ptrs = struct.unpack_from(f"<{TYPE_COUNT + 3}I", img, at)
            names = [c_string(img, p) for p in ptrs]
            if names[:TYPE_COUNT] == EXPECTED_TYPES:
                return ARM_BASE + at, names
            at = img.find(needle, at + 1)
    return 0, []


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    img = Path(args[0]).read_bytes()
    addr, names = find_names(img)
    if not names:
        print("table not found")
        return 1
    print(f"table at {addr:#x}")
    for i, name in enumerate(names):
        print(f"{i:2d} {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
