#!/usr/bin/env python3
"""Decode the ARM DSP initialisation script (`bank0_CODE@0xC0503600 ..`) into SY DM words.

Record layout (docs/research/sy-pitch-entry-writer.md s7, read from the data, no ARM function
traced): type 4 is `[4][n][address][n values]` (a block write into the SY1SY2 DM), type 1 is
`[1][value][address]` (a single word, addresses below 0x100000 are DM, others other spaces).
The SY part of the script is the span `0xC0503740 .. 0xC05038B8`: it sets the scan pointer
`DM(0x95C69)`, the width indexed unpacker pointer table `DM(0x95C7B..)` and some constants.

    python tools/ns4_arm_init_script.py target/fw_unpacked/bank0_CODE.bin [--out poke.scn]

The output holds firmware words: keep it under `target/`, never commit it.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

LOAD_BASE = 0xC000_0000
# VERIFIED: span of the SY records (type 4 records at 0xC0503740 .. 0xC05038B0, one further type 4
# record at 0xC050388C); the next type 4 record at 0xC051BDE8 belongs to the SMP image script.
SY_SPAN = (0xC050_3740, 0xC050_38B8)
DM_LIMIT = 0xA0000


def read_words(code: bytes, lo: int, hi: int) -> list[int]:
    """Little endian words of `code` between the load addresses `lo` and `hi`."""
    a, b = lo - LOAD_BASE, hi - LOAD_BASE
    return list(struct.unpack_from(f"<{(b - a) // 4}I", code, a))


def parse_records(words: list[int]) -> list[tuple[int, list[int]]]:
    """Type 4 block records `(address, values)`; stray words (other record types) are skipped."""
    out: list[tuple[int, list[int]]] = []
    i = 0
    while i + 3 <= len(words):
        n, addr = words[i + 1], words[i + 2]
        if (
            words[i] == 4
            and 0 < n <= 0x400
            and 0x90000 <= addr < DM_LIMIT
            and i + 3 + n <= len(words)
        ):
            out.append((addr, words[i + 3 : i + 3 + n]))
            i += 3 + n
        else:
            i += 1
    return out


def sy_records(code: bytes) -> list[tuple[int, list[int]]]:
    return parse_records(read_words(code, *SY_SPAN))


def poke_lines(records: list[tuple[int, list[int]]]) -> list[str]:
    """`poke ADDR VALUE` scenario lines, one per word."""
    return [f"poke 0x{a + k:X} 0x{v:X}" for a, vals in records for k, v in enumerate(vals)]


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("code", type=Path, help="bank0_CODE.bin")
    ap.add_argument("--out", type=Path, help="write the poke lines here (default stdout)")
    args = ap.parse_args(argv)
    lines = poke_lines(sy_records(args.code.read_bytes()))
    text = "\n".join(lines) + "\n"
    if args.out:
        args.out.write_text(text)
    else:
        sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
