#!/usr/bin/env python3
"""Parse zone records and the sample file list from a decoded Kontakt ``.nki`` stream.

Input is the output of tools/nki_lz.py. Layout found on ``Ultimate Pianos 2.1.1 Yuri
Total.nki`` (Kontakt 8.4); offsets are relative to the start of the 12 byte record
signature ``FF*7 00 01 00 00 00``. All integers little endian.

Zone record (226 bytes, or 254 for 462 records; the fields below sit at the same
offsets in both)::

    44  u32  zone id (increments)
    64  u16  velocity low        66  u16 velocity high
    68  u16  key low             70  u16 key high
    80  u16  root key
    100 u32  index into the sample file list (+ file_base)
    108 u32  sample rate
    113 u32  frame count   (unaligned; equals the WAV frame count)

Sample file list: entries ``04 <u32 n chars> <UTF-16 name>`` ending in ``.wav``, in
file-index order. ``file_base`` (7 here) is the file index of the first name.
"""

from __future__ import annotations

import argparse
import csv
import re
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

ZONE_SIG = b"\xff" * 7 + b"\x00\x01\x00\x00\x00"
ZONE_MIN = 226
NAME_RE = re.compile(rb"\x04([\x04-\x80])\x00\x00\x00((?:[\x20-\x7e]\x00){4,}?)(?=\x00\x00)")
STEM_RE = re.compile(r"^(?P<set>.+) (?P<note>\d{3}) (?P<vel>\d{3})$")
FIELDS = [
    "zone_id",
    "file_index",
    "name",
    "vel_lo",
    "vel_hi",
    "key_lo",
    "key_hi",
    "root",
    "rate",
    "frames",
]


@dataclass
class Zone:
    zone_id: int
    file_index: int
    vel_lo: int
    vel_hi: int
    key_lo: int
    key_hi: int
    root: int
    rate: int
    frames: int
    name: str = ""


def parse_names(data: bytes, start: int = 0) -> list[str]:
    """Names of ``.wav`` files from the file list, in order."""
    out: list[str] = []
    for m in NAME_RE.finditer(data, start):
        s = m.group(2).decode("utf-16le")
        if len(s) == int.from_bytes(m.group(1), "little") and s.lower().endswith(".wav"):
            out.append(s)
    return out


def parse_zones(data: bytes) -> list[Zone]:
    """Every zone record in file order (no names yet)."""
    zones: list[Zone] = []
    for m in re.finditer(re.escape(ZONE_SIG), data):
        p = m.start()
        if p + ZONE_MIN > len(data):
            continue
        r = data[p : p + ZONE_MIN]
        (zid,) = struct.unpack_from("<I", r, 44)
        vlo, vhi, klo, khi = struct.unpack_from("<4H", r, 64)
        (root,) = struct.unpack_from("<H", r, 80)
        (fidx,) = struct.unpack_from("<I", r, 100)
        (rate,) = struct.unpack_from("<I", r, 108)
        (frames,) = struct.unpack_from("<I", r, 113)
        zones.append(Zone(zid, fidx, vlo, vhi, klo, khi, root, rate, frames))
    return zones


def attach_names(zones: list[Zone], names: list[str], base: int | None = None) -> int:
    """Fill ``Zone.name`` using ``names[file_index - base]``; return the base used."""
    if base is None:
        base = min((z.file_index for z in zones), default=0)
    for z in zones:
        i = z.file_index - base
        z.name = names[i] if 0 <= i < len(names) else ""
    return base


def check_names(zones: list[Zone]) -> tuple[int, int, int]:
    """Compare zones with ``<set> <note> <velocity>.wav`` names.

    Returns (matching, mismatching, unparseable names): a zone matches when the root
    key equals the note and the velocity high bound equals the velocity in the name.
    """
    ok = bad = skip = 0
    for z in zones:
        m = STEM_RE.match(z.name[:-4]) if z.name else None
        if not m:
            skip += 1
        elif int(m["note"]) == z.root and int(m["vel"]) == z.vel_hi:
            ok += 1
        else:
            bad += 1
    return ok, bad, skip


def check_frames(zones: list[Zone], frames_by_stem: dict[str, int]) -> tuple[int, int]:
    """Compare zone frame counts with measured WAV frames (matching, mismatching)."""
    ok = bad = 0
    for z in zones:
        want = frames_by_stem.get(z.name[:-4])
        if want is None:
            continue
        if want == z.frames:
            ok += 1
        else:
            bad += 1
    return ok, bad


def write_csv(path: Path, zones: list[Zone]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(FIELDS)
        for z in zones:
            w.writerow(
                [z.zone_id, z.file_index, z.name, z.vel_lo, z.vel_hi]
                + [z.key_lo, z.key_hi, z.root, z.rate, z.frames]
            )


def load_catalog_frames(path: Path) -> dict[str, int]:
    out: dict[str, int] = {}
    with path.open(encoding="utf-8") as fh:
        for r in csv.DictReader(fh):
            out[f"{r['set']} {int(r['note']):03d} {int(r['velocity']):03d}"] = int(r["frames"])
    return out


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Parse zones from a decoded .nki stream.")
    ap.add_argument("decoded", type=Path)
    ap.add_argument("out_csv", type=Path)
    ap.add_argument("--catalog", type=Path, help="tools/up2_oracle.py CSV to cross-check frames")
    a = ap.parse_args(argv)
    data = a.decoded.read_bytes()
    zones = parse_zones(data)
    list_start = data.rfind(ZONE_SIG) + ZONE_MIN
    base = attach_names(zones, parse_names(data, list_start))
    write_csv(a.out_csv, zones)
    ok, bad, skip = check_names(zones)
    print(f"{len(zones)} zones (file base {base}); names: {ok} match, {bad} differ, {skip} other")
    if a.catalog:
        fo, fb = check_frames(zones, load_catalog_frames(a.catalog))
        print(f"frames vs WAV catalog: {fo} equal, {fb} different")
    return 0


if __name__ == "__main__":
    sys.exit(main())
