#!/usr/bin/env python3
"""Damper-down voice list of the firmware: note-on with and without the sustain pedal.

Loads one factory `.npno` through the real firmware (`arm_piano_load`: stubbed library, real
loader, binder and key path), then plays the same note twice from the settled snapshot, once
plain and once after a CC64 = 127 on UART2, and prints one derived-numbers CSV row:

    sound, class, root, records per stroke kind at the root (0 normal, 1 damper-down, 2 key-up),
    level calls with the pedal up / down, whether they are identical, message counts of the
    note-on / note-off / pedal-up phases.

    uv run --with unicorn python tools/arm_piano_pedal.py target/fw_unpacked/bank0_CODE.bin \
        target/fw_unpacked/bank0_SRAM.bin --cache target/keydown.snap \
        --ns4b ".reference/Nord Stage 4 Factory Restore RevK v1.66.ns4b" \
        --npno "Piano/Grand/Velvet Grand Med 6.1.npno" --note 60 --velocity 90

Findings: `docs/research/piano-extra-voices.md`. Only derived numbers are printed (no file bytes).
"""

from __future__ import annotations

import argparse
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

# VERIFIED (`crates/nord-core/src/piano/npno_level.rs`, same constants): backup-file layout of
# an `.npno`: key map, class byte, stroke counts per root, 118-byte stroke records.
KEYMAP = 0xB8
CLASS_BYTE = 0x44
COUNTS = 0x64E
RECORDS = 0x75E
RECORD_LEN = 0x76
LEVEL_FIELD = 0x34
KIND_BYTE = 4
# VERIFIED (run, `docs/research/piano-extra-voices.md` s2): the record byte 4 (`& 3`, the
# firmware's "layer") is the stroke kind: 0 normal, 1 damper-down, 2 key-up.
KIND_NAMES = ("normal", "damper", "keyup")
SUSTAIN_ON = bytes([0xB0, 64, 127])
SUSTAIN_OFF = bytes([0xB0, 64, 0])


@dataclass(frozen=True)
class Record:
    """Derived fields of one stroke record."""

    index: int
    root: int
    kind: int
    level_offset: int
    threshold: int


def parse_records(npno: bytes) -> list[Record]:
    """Stroke records in file order (ascending root, `counts[root]` records each)."""
    counts = struct.unpack(">128H", npno[COUNTS : COUNTS + 256])
    records: list[Record] = []
    for root in range(128):
        for _ in range(counts[root]):
            at = RECORDS + len(records) * RECORD_LEN
            raw = npno[at : at + RECORD_LEN]
            (offset,) = struct.unpack(">h", raw[LEVEL_FIELD : LEVEL_FIELD + 2])
            records.append(Record(len(records), root, raw[KIND_BYTE] & 3, offset, raw[0x74]))
    return records


def kind_counts(records: list[Record], root: int) -> tuple[int, int, int]:
    """Records of `root` per stroke kind (normal, damper-down, key-up)."""
    out = [0, 0, 0]
    for rec in records:
        if rec.root == root and rec.kind < 3:
            out[rec.kind] += 1
    return out[0], out[1], out[2]


def extra_voice_kind(counts: tuple[int, int, int], cls: int) -> str:
    """Kind of the first extra voice (VERIFIED rule, `piano-sound-load.md` s10): classes 1, 2, 3,
    5, 6 start one; it plays the damper-down stroke when the root has one, else a second
    normal stroke at the velocity position `T + 20`; other classes have none."""
    if cls not in (1, 2, 3, 5, 6):
        return "none"
    return "damper-stroke" if counts[1] > 0 else "normal+20"


def row(name: str, cls: int, root: int, counts: tuple[int, int, int], fields: list[str]) -> str:
    """One CSV row: sound, class, root, kind counts, then the run fields."""
    head = [name, str(cls), str(root), *map(str, counts), extra_voice_kind(counts, cls)]
    return ",".join(head + fields)


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    import arm_keydown as kd
    import arm_piano_load as pl

    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path, nargs="?")
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--ns4b", type=Path, required=True)
    ap.add_argument("--npno", required=True)
    ap.add_argument("--note", type=int, default=60)
    ap.add_argument("--velocity", type=int, default=90)
    ns = ap.parse_args(argv)
    raw = pl.read_npno(ns.ns4b, ns.npno)
    records = parse_records(raw)
    root = raw[KEYMAP + ns.note]
    counts = kind_counts(records, root)

    def run(pedal: bool) -> tuple[list, list[int]]:
        rig = kd.build_rig(ns.image, ns.sram, ns.cache)
        pl.LibraryStub({1: pl.device_image(raw, "Library.npno")}, pl.KIND_FULL).install(rig.emu)
        pl.load_sound(rig.emu, pl.managers(rig.emu)[0], 1)
        pl.select_sound(rig.emu, 1)
        pl.install_dsp_ready(rig.emu)
        calls = pl.hook_level_calls(rig.emu)
        rig.tick(2)
        if pedal:
            rig.send(SUSTAIN_ON)
            rig.tick(10)
        rig.send(kd.note_on(ns.note, ns.velocity))
        n_on = len(rig.tick(40))
        rig.send(kd.note_off(ns.note))
        n_off = len(rig.tick(10))
        rig.send(SUSTAIN_OFF)
        n_up = len(rig.tick(10))
        return [(c.x, c.rec5e, c.key, c.out) for c in calls], [n_on, n_off, n_up]

    up, msgs_up = run(False)
    down, msgs_down = run(True)
    fields = [
        f"calls_up={up}".replace(",", ";"),
        f"calls_down={down}".replace(",", ";"),
        f"same={up == down}",
        "msgs_up=" + "/".join(map(str, msgs_up)),
        "msgs_down=" + "/".join(map(str, msgs_down)),
    ]
    print(row(ns.npno.rsplit("/", 1)[-1], raw[CLASS_BYTE], root, counts, fields))
    return 0


if __name__ == "__main__":
    sys.exit(main())
