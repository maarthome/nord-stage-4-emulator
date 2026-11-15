#!/usr/bin/env python3
"""Menu choice lists of the Synth read from the display strings of the firmware.

The display strings of a choice list sit back to back in `bank0_CODE.bin`, in menu order, before
the label of the menu item:

* `Pitch Stick Range` (`Edt_Syn_kPitchStickRange`, ids `0..9`, default 1): ten `N semi` strings;
* `Vibrato Delay` (`Edt_Syn_kVibDly`, ids `1..30`, default 7): thirty `N.N s` strings.

    python tools/ns4_fw_choices.py target/fw_unpacked/bank0_CODE.bin

The id ranges are in `docs/research/arm-edit-params.csv`. Names and numbers only; the firmware
stays local.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

LABEL = b"Pitch Stick Range\x00"
CHOICE = re.compile(rb"(-?\d+) semi\x00")
VIB_LABEL = b"Vibrato Delay\x00"
VIB_CHOICE = re.compile(rb"(\d\.\d) s\x00")
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


def vib_delay_list(blob: bytes) -> list[float]:
    """The Vibrato Delay choice values in menu order (seconds)."""
    at = blob.find(VIB_LABEL)
    if at < 0:
        raise ValueError("label 'Vibrato Delay' not found")
    window = blob[max(0, at - 0x200) : at]
    return [float(m.group(1)) for m in VIB_CHOICE.finditer(window)]


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - firmware CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ns = ap.parse_args(argv)
    blob = ns.image.read_bytes()
    print("pitch stick range", pstick_range_list(blob), "label at", hex(blob.find(LABEL)))
    print("vibrato delay", vib_delay_list(blob), "label at", hex(blob.find(VIB_LABEL)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
