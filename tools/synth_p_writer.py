#!/usr/bin/env python3
"""The ARM writer of the synth oscillator P block (ratios, weights, gain) per wave type.

Earlier notes (`docs/research/synth-wave-models.md` s9, s10) found no DSP-side writer of the P block
`DM(0xC169A + 0x96 slot)` and concluded that it comes from outside core 2. The ARM writes it: the
strip *apply* function `0xC01CE7D8(strip)` (run for the state marker and after every wave edit)
calls the table driven writers `0xC01C72CC`, `0xC01C7550` and `0xC01C94CC` (`bank0_CODE` rodata
rows at `0xC02F9968 + 0x200`, `0xC02F9E88`, float immediates for the SUPER detune ratios), which
call ~44 one-word senders `0xC01B1E6C .. 0xC01C044C` (`cmp r0, #5` prologue: `(slot, float)`).
The harness never reached it: the edit ids only set the pending type `[strip + 0x98]`, and the
apply function runs only for the marker value. This tool calls the apply function directly for
every type index with the pending words `[strip + 0x9C]`, `[strip + 0xA0]` (wave variant) set:

    uv run --with unicorn --with capstone python tools/synth_p_writer.py capture \\
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin \\
        --cache target/settled.snap --out docs/research/data/synth-p-words.csv

Output columns: `type,a,b,dm,p_index,word,value` (`p_index = dm - 0xC169A`, slot 0 block).
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

# VERIFIED (`docs/research/synth-wave-models.md` s1, org-stage-map s3): slot 0 parameter block
# pointer `P = 0xC169A`; one block of 150 words per slot (`0x96`).
P_BASE = 0xC169A
P_SLOT_WORDS = 0x96
# VERIFIED (run, this tool): apply function of a strip object; inputs `[obj + 0x98]` (type),
# `[obj + 0x9C]`, `[obj + 0xA0]`.
APPLY = 0xC01CE7D8
SENDER = 0xC01CC85C
PENDING_TYPE, PENDING_A, PENDING_B = 0x98, 0x9C, 0xA0
TYPES = 74
FIELDS = ["type", "a", "b", "dm", "p_index", "word", "value"]


def f32(word: int) -> float:
    """Float32 value of a 32-bit word."""
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


# VERIFIED (run): the MULTI voice multiplier record `DM(0xC1596 .. 0xC1598)` is written by the same
# apply function for the MULTI types (`tools/synth_p_writer.py capture`, type 16).
MULTI_WINDOW = (0xC1590, 0xC15AF)


def p_index(dm: int, slot: int = 0) -> int | None:
    """Index `dm - 0xC169A` for the P block of `slot` (`P[-32 .. 117]`) and, for slot 0, the MULTI
    record window (`-266 .. -235`); None elsewhere."""
    rel = dm - (P_BASE + P_SLOT_WORDS * slot)
    if -32 <= rel < P_SLOT_WORDS - 32:
        return rel
    if slot == 0 and MULTI_WINDOW[0] <= dm <= MULTI_WINDOW[1]:
        return dm - P_BASE
    return None


def fold_core2(messages) -> dict[int, int]:  # noqa: ANN001
    """Last word per DM address of the ORG core 2 messages."""
    out: dict[int, int] = {}
    for m in messages:
        if m.chip == "ORG" and m.core == 2:
            out[m.dm_word] = m.word2
    return out


def p_words(words: dict[int, int], slot: int = 0) -> dict[int, int]:
    """`{P index: word}` of the slot's block."""
    return {i: w for dm, w in words.items() if (i := p_index(dm, slot)) is not None}


def rows_for(type_: int, a: int, b: int, block: dict[int, int]) -> list[dict[str, str]]:
    """CSV rows of one run, in address order."""
    return [
        {
            "type": str(type_),
            "a": str(a),
            "b": str(b),
            "dm": f"{P_BASE + i:#x}",
            "p_index": str(i),
            "word": f"{w:#010x}",
            "value": f"{f32(w):.9g}",
        }
        for i, w in sorted(block.items())
    ]


def delta(block: dict[int, int], base: dict[int, int]) -> dict[int, int]:
    """Words of `block` that differ from `base` or that `base` did not write."""
    return {i: w for i, w in block.items() if base.get(i) != w}


def load_csv(path: Path) -> dict[tuple[int, int, int], dict[int, int]]:
    """`{(type, a, b): {p_index: word}}` from a capture file."""
    import csv

    out: dict[tuple[int, int, int], dict[int, int]] = {}
    with path.open(encoding="utf-8") as fh:
        for r in csv.DictReader(fh):
            key = (int(r["type"]), int(r["a"]), int(r["b"]))
            out.setdefault(key, {})[int(r["p_index"])] = int(r["word"], 0)
    return out


def capture(  # pragma: no cover - needs the booted firmware
    code: Path, sram: Path, cache: Path | None, variants: int, out: Path, shard: tuple[int, int]
) -> int:
    import arm_edit_capture as c
    import arm_emu as ae
    import arm_steady as ast
    import unicorn

    emu, _blob = c.settled_emulator(code, sram, cache)
    if not any(lo <= c.SCRATCH <= hi for lo, hi, _ in emu.uc.mem_regions()):
        emu.uc.mem_map(c.SCRATCH, ae.PAGE)
    store = next(s for s in c.first_store_per_layer(c.find_stores(emu)) if s.layer == 4)
    seen: list[int] = []
    emu.uc.hook_add(
        unicorn.UC_HOOK_CODE,
        lambda uc, *_: seen.append(uc.reg_read(unicorn.arm_const.UC_ARM_REG_R0)),
        begin=SENDER,
        end=SENDER,
    )
    cell = c.value_cell(emu, store.address, 0x127)
    emu.write32(cell, 3 << 16)
    c.apply_value(emu, store.address, 0x127, 3 << 16, 2000)
    strip = seen[-1]
    blob = ast.snapshot(emu)
    rows: list[dict[str, str]] = []
    for t in range(shard[0], TYPES, shard[1]):
        base: dict[int, int] = {}
        for a in range(variants):
            for b in range(variants if a == 0 else 1):
                ast.restore(emu, blob)
                emu.install_dsp_ports()
                emu.write32(strip + PENDING_TYPE, t)
                emu.write32(strip + PENDING_A, a)
                emu.write32(strip + PENDING_B, b)
                emu.clear_messages()
                ast.call_preserving(emu, APPLY, strip)
                block = p_words(fold_core2(emu.messages))
                if a == 0 and b == 0:
                    base = block
                    rows += rows_for(t, a, b, block)
                else:
                    rows += rows_for(t, a, b, delta(block, base))
        print(f"type {t}: {len(rows)} rows", flush=True)
    import csv

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
    cp.add_argument("--variants", type=int, default=5)
    cp.add_argument("--shard", default="0/1", help="i/n: types i, i + n, ...")
    cp.add_argument("--out", type=Path, required=True)
    ns = ap.parse_args(argv)
    i, n = (int(x) for x in ns.shard.split("/"))
    return capture(ns.code, ns.sram, ns.cache, ns.variants, ns.out, (i, n))


if __name__ == "__main__":
    sys.exit(main())
