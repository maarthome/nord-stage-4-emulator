#!/usr/bin/env python3
"""Parse group records (names, volume, modulator inventory) from a decoded Kontakt ``.nki``.

Input is the output of tools/nki_lz.py. Layout found on ``Ultimate Pianos 2.1.1 Yuri
Total.nki`` (Kontakt 8.4), little endian:

* A group record starts with its name as ``u32 n chars, UTF-16 name`` (names look like
  ``astoria .grand #dn``). Consecutive records are about 4850 bytes apart; the record
  length is not fixed, so a record runs to the next group name.
* Parameters and modulators appear as ``u32 n, ASCII identifier`` strings
  (``volume``, ``pan``, ``pitch``, ``ENV_AHDSR_VOLUME``, ``LFO_SAW_TREMOLO``,
  ``VEL_VOLUME``, ``KP_VOLUME``, ``KSP_CUTOFF`` ...). ``volume`` appears several times
  per group; the 4 bytes after its third occurrence are a little endian float, the
  group volume (normalized Kontakt value).

EMPIRICAL: occurrence 3 is the only one that varies across the 181 groups (0.78027 in 140
groups, 0.84029 in 41); the others are 1.0 everywhere. Which occurrence is the group
volume, and that 0.78027 is 0 dB, are inferences (ASSUMED) with no Kontakt readout to
confirm them.

What is not decoded: envelope times, LFO rates, modulation intensities, filter and
effect values, and the membership of zones in groups (see docs/research/nki-lz-stream.md).
"""

from __future__ import annotations

import argparse
import csv
import re
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path

NAME_RE = re.compile(rb"([\x04-\x60])\x00\x00\x00((?:[\x20-\x7e]\x00){4,}?)(?=\x00\x00)")
GROUP_RE = re.compile(r" #[a-z]{2}$")
IDENT_RE = re.compile(rb"([\x03-\x40])\x00\x00\x00([A-Za-z_][A-Za-z0-9_]{2,60})")
VOLUME_OCCURRENCE = 2  # EMPIRICAL: the only `volume` occurrence that varies by group
MODULATOR_PREFIXES = ("ENV_", "LFO_", "VEL_", "KP_", "KSP_", "STEP_", "MOD_")


@dataclass
class Group:
    name: str
    offset: int
    volume: float | None
    modulators: list[str] = field(default_factory=list)


def find_group_names(data: bytes, start: int, end: int) -> list[tuple[int, str]]:
    """(offset of the name length field, name) for every group name in ``data[start:end]``."""
    out: list[tuple[int, str]] = []
    for m in NAME_RE.finditer(data, start, end):
        name = m.group(2).decode("utf-16le")
        if len(name) == m.group(1)[0] and GROUP_RE.search(name):
            out.append((m.start(), name))
    return out


def identifiers(record: bytes) -> list[tuple[int, str]]:
    """Length-prefixed ASCII identifiers in a record, as (offset, text)."""
    out: list[tuple[int, str]] = []
    for m in IDENT_RE.finditer(record):
        n = m.group(1)[0]
        text = m.group(2)
        if len(text) >= n:
            out.append((m.start(), text[:n].decode("ascii")))
    return out


def parse_group(data: bytes, offset: int, name: str, end: int) -> Group:
    body_start = offset + 4 + 2 * len(name)
    rec = data[body_start:end]
    idents = identifiers(rec)
    volume = None
    seen = 0
    for off, text in idents:
        if text != "volume":
            continue
        if seen == VOLUME_OCCURRENCE:
            pos = off + 4 + len(text)
            if pos + 4 <= len(rec):
                (volume,) = struct.unpack_from("<f", rec, pos)
            break
        seen += 1
    mods = [t for _, t in idents if t.startswith(MODULATOR_PREFIXES)]
    return Group(name, offset, volume, mods)


def parse_groups(data: bytes, start: int, end: int) -> list[Group]:
    """All groups in ``data[start:end]``."""
    names = find_group_names(data, start, end)
    groups: list[Group] = []
    for i, (off, name) in enumerate(names):
        stop = names[i + 1][0] if i + 1 < len(names) else end
        groups.append(parse_group(data, off, name, stop))
    return groups


def write_csv(path: Path, groups: list[Group]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(["name", "offset", "volume", "modulators"])
        for g in groups:
            vol = "" if g.volume is None else f"{g.volume:.6f}"
            w.writerow([g.name, g.offset, vol, " ".join(g.modulators)])


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Parse group records from a decoded .nki stream.")
    ap.add_argument("decoded", type=Path)
    ap.add_argument("script_len", type=int, help="length of the leading KSP script in bytes")
    ap.add_argument("out_csv", type=Path)
    a = ap.parse_args(argv)
    data = a.decoded.read_bytes()
    zone_start = data.find(b"\xff" * 7 + b"\x00\x01\x00\x00\x00", a.script_len)
    end = zone_start if zone_start > 0 else len(data)
    groups = parse_groups(data, a.script_len, end)
    write_csv(a.out_csv, groups)
    vols = [g.volume for g in groups if g.volume is not None]
    rng = f"{min(vols):.3f}..{max(vols):.3f}" if vols else "n/a"
    print(f"{len(groups)} groups, volume range {rng}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
