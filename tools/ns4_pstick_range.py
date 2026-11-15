#!/usr/bin/env python3
"""The Pitch Stick Range choices of the Synth (`Edt_Syn_kPitchStickRange`), from firmware strings.

The ten display strings of the choice list sit back to back in `bank0_CODE.bin`, in menu order,
before the label `Pitch Stick Range`:

    python tools/ns4_pstick_range.py target/fw_unpacked/bank0_CODE.bin

The edit id has the range `0..9` (ten choices, default 1) in `docs/research/arm-edit-params.csv`.
Names and numbers only; the firmware stays local.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

LABEL = b"Pitch Stick Range\x00"
CHOICE = re.compile(rb"(-?\d+) semi\x00")
#: The list is ten strings of at most 8 characters plus padding; this window covers it.
WINDOW = 0x90


def choices_before(blob: bytes, label_at: int) -> list[int]:
    """The `N semi` strings in the window right before the label at `label_at`, in order."""
    window = blob[max(0, label_at - WINDOW) : label_at]
    return [int(m.group(1)) for m in CHOICE.finditer(window)]


def pstick_range_list(blob: bytes) -> list[int]:
    """The Pitch Stick Range choice values in menu order (semitones)."""
    at = blob.find(LABEL)
    if at < 0:
        raise ValueError("label 'Pitch Stick Range' not found")
    return choices_before(blob, at)


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - firmware CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ns = ap.parse_args(argv)
    blob = ns.image.read_bytes()
    print(pstick_range_list(blob), "label at", hex(blob.find(LABEL)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
