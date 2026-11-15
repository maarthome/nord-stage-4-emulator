#!/usr/bin/env python3
"""The ARM-generated FM band table of strip types 49..53 (kOscBank 1 and 2).

Stages `0x124269` / `0x12453F` read three float curves through the pointer `DM(0xC15A8) = 0xE0926`
(listing `0x248508`: `I5 = 0xE0926 + FEXT(...)`, stride `M5 = 0x81`, rows at `+0`, `+129`,
`+258`). The image holds zeros there. The ARM strip apply `0xC01CE7D8(strip)` writes them: **361
words `0xE0900 .. 0xE0A68`** per type (`docs/research/fm-banks-1-2.md`). Capture (needs the
settled harness):

    uv run --with unicorn --with capstone python tools/synth_fm_tables.py capture \
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin \
        --cache target/settled.snap --out docs/research/data/synth-fm-tables.csv

Output columns: `type,dm,word,value` (types 49..53, all 361 addresses). The pending words `a`,
`b` (wave variant) do not change one of them (checked by the capture).
"""

from __future__ import annotations

import argparse
import csv
import struct
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

# VERIFIED (run, this tool): the ARM writes this window for types 49..53, no other type 0..73.
TABLE_LO, TABLE_HI = 0xE0900, 0xE0A68
TYPES = (49, 50, 51, 52, 53)
# VERIFIED (listing `0x248508`, image word `DM(0xC15A8)`): the pointer the stage adds the index to.
BASE = 0xE0926
ROW_STRIDE = 129
FIELDS = ["type", "dm", "word", "value"]


def f32(word: int) -> float:
    """Float32 value of a 32-bit word."""
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


def window(messages) -> dict[int, int]:  # noqa: ANN001
    """Last word per DM address of the ORG core 2 messages inside the table window."""
    out: dict[int, int] = {}
    for m in messages:
        if m.chip == "ORG" and m.core == 2 and TABLE_LO <= m.dm_word <= TABLE_HI:
            out[m.dm_word] = m.word2 & 0xFFFFFFFF
    return out


def rows_for(type_: int, words: dict[int, int]) -> list[dict[str, str]]:
    return [
        {"type": str(type_), "dm": f"{dm:#x}", "word": f"{w:#010x}", "value": f"{f32(w):.9g}"}
        for dm, w in sorted(words.items())
    ]


def load_csv(path: Path) -> dict[int, dict[int, int]]:
    """`{type: {dm: word}}` from a capture file."""
    out: dict[int, dict[int, int]] = {}
    with path.open(encoding="utf-8") as fh:
        for r in csv.DictReader(fh):
            out.setdefault(int(r["type"]), {})[int(r["dm"], 0)] = int(r["word"], 0)
    return out


def table(words: dict[int, int]) -> list[float]:
    """The 361 values `0xE0900 .. 0xE0A68` as floats (missing words read 0)."""
    return [f32(words.get(a, 0)) for a in range(TABLE_LO, TABLE_HI + 1)]


def capture(  # pragma: no cover - needs the booted firmware
    code: Path, sram: Path, cache: Path | None, out: Path
) -> int:
    import arm_edit_capture as c
    import arm_emu as ae
    import arm_steady as ast
    import synth_p_writer as pw
    import unicorn

    emu, _blob = c.settled_emulator(code, sram, cache)
    if not any(lo <= c.SCRATCH <= hi for lo, hi, _ in emu.uc.mem_regions()):
        emu.uc.mem_map(c.SCRATCH, ae.PAGE)
    store = next(s for s in c.first_store_per_layer(c.find_stores(emu)) if s.layer == 4)
    seen: list[int] = []
    emu.uc.hook_add(
        unicorn.UC_HOOK_CODE,
        lambda uc, *_: seen.append(uc.reg_read(unicorn.arm_const.UC_ARM_REG_R0)),
        begin=pw.SENDER,
        end=pw.SENDER,
    )
    emu.write32(c.value_cell(emu, store.address, 0x127), 3 << 16)
    c.apply_value(emu, store.address, 0x127, 3 << 16, 2000)
    strip = seen[-1]
    blob = ast.snapshot(emu)
    rows: list[dict[str, str]] = []
    for t in TYPES:
        base: dict[int, int] = {}
        for a, b in ((0, 0), (1, 0), (3, 0), (0, 4)):
            ast.restore(emu, blob)
            emu.install_dsp_ports()
            emu.write32(strip + pw.PENDING_TYPE, t)
            emu.write32(strip + pw.PENDING_A, a)
            emu.write32(strip + pw.PENDING_B, b)
            emu.clear_messages()
            ast.call_preserving(emu, pw.APPLY, strip)
            words = window(emu.messages)
            if (a, b) == (0, 0):
                base = words
                rows += rows_for(t, words)
            elif words != base:
                raise SystemExit(f"type {t}: table depends on the pending words ({a}, {b})")
        print(f"type {t}: {len(base)} words", flush=True)
    with out.open("w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, FIELDS, lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    return 0


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    cp = sub.add_parser("capture")
    cp.add_argument("code", type=Path)
    cp.add_argument("sram", type=Path)
    cp.add_argument("--cache", type=Path)
    cp.add_argument("--out", type=Path, required=True)
    ns = ap.parse_args(argv)
    return capture(ns.code, ns.sram, ns.cache, ns.out)


if __name__ == "__main__":
    sys.exit(main())
