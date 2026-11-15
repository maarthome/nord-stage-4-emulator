#!/usr/bin/env python3
"""Capture of the B3 Bass voice record writes of the ARM key-down (research s15).

The generator `0x121BBE` runs ten sections per call (`lcntr = 10`) over the per key voice records
`DM 0x954E6..0x95549` (4 pointer / coefficient pairs per section), `0x9559C..0x955AF` (shift
counts) and `0xB4246..0xB42xx` (mask and position words). The key-down of the drawbar and the
percussion item writes them; this tool records every write to those ranges for a key sequence:

    uv run --with unicorn --with capstone --with numpy python tools/organ_bass_voice.py capture \
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin out.csv \
        --keys 24:108 --drawbars 888000000 --cache target/settled.snap

Output rows: `press,key,item,addr,value` (hex address and word); `item` 0 = drawbar, 1 = percussion.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# VERIFIED (research s15): the voice record ranges the generator reads.
RANGES = ((0x954E0, 0x95560), (0x95590, 0x955B0), (0xB4240, 0xB4300))


def in_voice(addr: int) -> bool:
    return any(lo <= addr < hi for lo, hi in RANGES)


def key_list(spec: str) -> list[int]:
    """`24:108` ascending, `108:24` descending, or `60,64,67`."""
    if ":" in spec:
        a, b = (int(x) for x in spec.split(":"))
        return list(range(a, b + 1)) if a <= b else list(range(a, b - 1, -1))
    return [int(x) for x in spec.split(",") if x]


# Slot layout of the first five presses (VERIFIED: asc capture, research s15): press `j` writes the
# 20 words of `A` at `0x954E6 + 20 j`, 4 words at `0x9559C + 4 j` and 12 words at `0xB4246 + 12 j`.
SLOT = ((0x954E6, 20), (0x9559C, 4), (0xB4246, 12))
SLOTS = 5


def slot_words(rows: list[tuple[int, int]], slot: int) -> list[int] | None:
    """Final value (last write wins) of the 36 slot words of `slot`, in `SLOT` order, or `None`."""
    last = dict(rows)
    out = []
    for base, n in SLOT:
        for k in range(n):
            addr = base + n * slot + k
            if addr not in last:
                return None
            out.append(last[addr])
    return out


def build_table(captures: list[Path]) -> dict[int, list[int]]:
    """`key -> 36 words` relocated to slot 0, from fresh-session captures (first five presses)."""
    table: dict[int, list[int]] = {}
    for path in captures:
        by: dict[tuple[int, int], list[tuple[int, int]]] = {}
        for line in path.read_text().splitlines()[1:]:
            press, key, _item, addr, value = line.split(",")
            by.setdefault((int(press), int(key)), []).append((int(addr, 16), int(value, 16)))
        for (press, key), rows in by.items():
            if press < SLOTS and (words := slot_words(rows, press)) is not None:
                table[key] = words
    return table


def cmd_table(a: argparse.Namespace) -> int:
    table = build_table(list(a.captures))
    lines = ["key,words (hex, A 20 | B 4 | C 12 relocated to slot 0)"]
    lines += [f"{k},{' '.join(f'{w:x}' for w in table[k])}" for k in sorted(table)]
    a.out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(table)} keys -> {a.out}")
    return 0


def cmd_capture(a: argparse.Namespace) -> int:  # pragma: no cover - needs the ARM harness
    import arm_edit_capture as aec
    import arm_model_paths as mp
    import arm_steady as ast
    import organ_b3_pass as ob

    emu, _ = aec.settled_emulator(a.code, a.sram, a.cache)
    mp.ensure_scratch(emu)
    store = next(s for s in aec.first_store_per_layer(aec.find_stores(emu)) if s.kind == "organ")
    mp.install_model(emu, store, 5)
    rec = emu.read32(ob.RECORD_TABLE)
    for k, v in enumerate(int(c) for c in a.drawbars):
        emu.write32(rec + ob.DRAWBAR_WORDS + 4 * k, v)
    emu.uc.mem_write(rec + ob.DIRTY, bytes([1]))
    emu.uc.mem_write(rec + ob.BUSY, bytes([1]))
    ast.call_preserving(emu, ob.RECORD_UPDATE, rec, max_insns=20_000_000)
    rows = []
    for press, key in enumerate(key_list(a.keys)):
        for item, off in enumerate((ob.SUB_DRAWBARS, ob.SUB_PERCUSSION)):
            emu.clear_messages()
            ast.call_preserving(
                emu, ob.ITEM_KEYDOWN, emu.read32(rec + off), key, 100, 0, max_insns=20_000_000
            )
            for m in emu.messages:
                d = m.to_dict()
                addr = int(d["dm"], 16)
                if d["chip"] == "PNO" and in_voice(addr):
                    rows.append(f"{press},{key},{item},{addr:x},{int(d['w2'], 16):x}")
    a.out.write_text("press,key,item,addr,value\n" + "\n".join(rows) + "\n", encoding="utf-8")
    print(f"{len(rows)} voice writes -> {a.out}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("capture")
    c.add_argument("code", type=Path)
    c.add_argument("sram", type=Path)
    c.add_argument("out", type=Path)
    c.add_argument("--keys", default="24:108")
    c.add_argument("--drawbars", default="888000000")
    c.add_argument("--cache", type=Path)
    t = sub.add_parser("table")
    t.add_argument("out", type=Path)
    t.add_argument("captures", type=Path, nargs="+")
    ns = ap.parse_args(argv)
    return cmd_capture(ns) if ns.cmd == "capture" else cmd_table(ns)


if __name__ == "__main__":
    sys.exit(main())
