#!/usr/bin/env python3
"""The System-menu Leslie settings in the ARM firmware: rotor speeds and accelerations per choice.

The edit ids `Edt_Sys_kLeslieHornSpeed` (0x91), `HornAcc` (0x92), `RotorSpeed` (0x93) and
`RotorAcc` (0x94) are handled by one function of the host firmware (`0xC0112AC0..0xC0112FE8`,
switch on the id). Each handler maps the value `0..2` through the word table `0xC02BBC30` and, per
choice, stores doubles into the rotor state block (`docs/research/arm-part-activation.md` s2):

| Id | Stores (block offset) |
|---|---|
| HornSpeed | slow `+0x48`, fast `+0x50` |
| RotorSpeed | slow `+0xB0`, fast `+0xB8` |
| HornAcc | step up `+0x18`, down `+0x28` and `+0x38` (limits `1.0` at `+0x20 / 0x30 / 0x40`) |
| RotorAcc | step up `+0x80`, down `+0x90` and `+0xA0` (limits `1.0` at `+0x88 / 0x98 / 0xA8`) |

The stores are `ldr rN, [pc, #imm]` literal pairs and `strd` instructions, so the table is read by
a tiny evaluator (LDR literal, MOV immediate, STRD, B) with no emulation:

    python tools/arm_leslie_settings.py target/fw_unpacked/bank0_CODE.bin

Names and numbers only; the firmware stays local.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

LOAD_BASE = 0xC0000000
#: VERIFIED (disassembly): word table that maps the menu value to the choice index `0..2`.
CHOICE_TABLE = 0xC02BBC30
#: Entry address of each handler: choice index -> first instruction of its store block.
#: VERIFIED (disassembly of `0xC0112B90..0xC0112FE8`): the `cmp r3, #1/#2/#0` branches of the
#: four handlers `0xC0112BD0` (horn speed), `0xC0112C7C` (rotor speed), `0xC0112C14` (horn
#: acceleration) and `0xC0112CC8` (rotor acceleration).
BLOCKS: dict[str, dict[int, int]] = {
    "HornSpeed": {0: 0xC0112F20, 1: 0xC0112F6C, 2: 0xC0112F50},
    "RotorSpeed": {0: 0xC0112CAC, 1: 0xC0112FCC, 2: 0xC0112FB0},
    "HornAcc": {0: 0xC0112C44, 1: 0xC0112F0C, 2: 0xC0112F3C},
    "RotorAcc": {0: 0xC0112CF8, 1: 0xC0112F9C, 2: 0xC0112F88},
}
#: Where the stored doubles land, per handler: block offset -> field name.
FIELDS: dict[str, dict[int, str]] = {
    "HornSpeed": {0x48: "slow_hz", 0x50: "fast_hz"},
    "RotorSpeed": {0xB0: "slow_hz", 0xB8: "fast_hz"},
    "HornAcc": {0x18: "up_per_update", 0x28: "down_per_update"},
    "RotorAcc": {0x80: "up_per_update", 0x90: "down_per_update"},
}


def decode_double(lo: int, hi: int) -> float:
    """The IEEE double with low word `lo` and high word `hi`."""
    return struct.unpack("<d", struct.pack("<II", lo, hi))[0]


def evaluate(code: bytes, start: int, limit: int = 40) -> dict[int, float]:
    """Run the straight-line store block at `start`; return `{block offset: double}` of its STRDs.

    Supports LDR literal, MOV immediate, STRD immediate and unconditional B; stops at any other
    instruction (the first non-store instruction ends a block).
    """
    regs: dict[int, int] = {}
    stores: dict[int, float] = {}
    pc = start
    for _ in range(limit):
        off = pc - LOAD_BASE
        (w,) = struct.unpack_from("<I", code, off)
        if w & 0x0F7F0000 == 0x051F0000:  # ldr rd, [pc, #+-imm]
            imm = w & 0xFFF
            addr = off + 8 + (imm if w & 0x00800000 else -imm)
            regs[(w >> 12) & 0xF] = struct.unpack_from("<I", code, addr)[0]
        elif w & 0x0FFF0000 == 0x03A00000:  # mov rd, #imm8 ror 2r
            rot = ((w >> 8) & 0xF) * 2
            imm = w & 0xFF
            regs[(w >> 12) & 0xF] = (
                ((imm >> rot) | (imm << (32 - rot))) & 0xFFFFFFFF if rot else imm
            )
        elif w & 0x0E5000F0 == 0x004000F0 and w & 0xF0 == 0xF0 and (w >> 20) & 1 == 0:
            rt = (w >> 12) & 0xF  # strd rt, rt+1, [rn, #imm]
            imm = ((w >> 4) & 0xF0) | (w & 0xF)
            stores[imm] = decode_double(regs[rt], regs[rt + 1])
        elif w & 0x0F000000 == 0x0A000000 and w >> 28 == 0xE:  # b label
            delta = w & 0x00FFFFFF
            delta -= 1 << 24 if delta & 0x00800000 else 0
            pc = pc + 8 + 4 * delta
            continue
        else:
            break
        pc += 4
    return stores


def read_choices(code: bytes) -> list[int]:
    """The menu value to choice index table `0xC02BBC30` (three words)."""
    return list(struct.unpack_from("<3I", code, CHOICE_TABLE - LOAD_BASE))


def settings_table(code: bytes) -> list[dict[str, object]]:
    """One row per (setting, choice): the stored doubles, named by `FIELDS`."""
    rows = []
    for setting, blocks in BLOCKS.items():
        for choice, start in sorted(blocks.items()):
            stores = evaluate(code, start)
            row: dict[str, object] = {"setting": setting, "choice": choice}
            for off, name in FIELDS[setting].items():
                row[name] = stores[off]
            rows.append(row)
    return rows


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - firmware CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ns = ap.parse_args(argv)
    code = ns.image.read_bytes()
    print("value -> choice table", read_choices(code))
    for row in settings_table(code):
        print(row)
    return 0


if __name__ == "__main__":
    sys.exit(main())
