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

Strip fields that change the table (`probe`): see `docs/research/fm-banks-1-2.md` s6.

Output columns: `type,dm,word,value` (types 49..53, all 361 addresses). The pending words `a`,
`b` (wave variant) do not change one of them (checked by the capture); strip `+0xA4` and `+0xA8` do.
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


def changed_offsets(
    base: dict[int, int], runs: dict[tuple[int, int], dict[int, int]]
) -> dict[int, list[int]]:
    """Strip offsets whose poke changed the table: `{offset: [poked values]}`.

    `runs` maps `(offset, value)` to the table words of that run; `base` is the unpoked table.
    """
    out: dict[int, list[int]] = {}
    for (off, val), words in sorted(runs.items()):
        if words != base:
            out.setdefault(off, []).append(val)
    return out


def probe(  # pragma: no cover - needs the booted firmware
    code: Path, sram: Path, cache: Path | None, types: tuple[int, ...]
) -> int:
    """Poke every strip offset the apply reads (five values) and report which change the table.

    Result (VERIFIED run, types 49 and 52, 38 offsets read): only strip `+0x00` (not the type, the
    strip object kind), `+0xA4` (a flag) and `+0xA8` (a scale, default `0x80000`) change a word.
    """
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

    def run(t: int, pokes: list[tuple[int, int]]) -> dict[int, int]:
        ast.restore(emu, blob)
        emu.install_dsp_ports()
        emu.write32(strip + pw.PENDING_TYPE, t)
        emu.write32(strip + pw.PENDING_A, 0)
        emu.write32(strip + pw.PENDING_B, 0)
        for off, val in pokes:
            emu.write32(strip + off, val)
        emu.clear_messages()
        ast.call_preserving(emu, pw.APPLY, strip)
        return window(emu.messages)

    reads: set[int] = set()
    hook = emu.uc.hook_add(
        unicorn.UC_HOOK_MEM_READ,
        lambda uc, ac, addr, size, val, ud: (
            reads.add((addr - strip) & ~3) if strip <= addr < strip + 0x400 else None
        ),
    )
    run(types[0], [])  # the reads of one apply are the offsets worth poking
    emu.uc.hook_del(hook)
    skip = {pw.PENDING_TYPE, pw.PENDING_A, pw.PENDING_B}
    for t in types:
        base = run(t, [])
        runs = {
            (off, val): run(t, [(off, val)])
            for off in sorted(reads - skip)
            for val in (0, 1, 0x3F800000, 0x40000000, 0x7FFFFFFF)
        }
        got = {hex(o): [hex(v) for v in vs] for o, vs in changed_offsets(base, runs).items()}
        print(f"type {t}: {len(reads)} offsets read, changed by {got}", flush=True)
    return 0


def partial_words(bank: int, wave: int) -> tuple[int, int] | None:
    """Strip `(+0xA4, +0xA8)` the oscillator wave edit stores for an FM bank (`None`: no FM wave).

    VERIFIED (run, `select`, 5 categories x 86 waves): the wave edit `kOscWave` handler stores
    `+0xA8` at `0xC00185B0` and `+0xA4` at `0xC00185D8` (caller `0xC003D7BC`). Bank 1 (`FM-H`,
    waves 0..24 = partial ratios 0.5, 1, 2 .. 24, label table `bank0_CODE@0x2D82A8`): `a4 = 0`,
    `a8 = ratio 2^16`. Bank 2 (`FM-I`, waves 0..60 = `wave - 12` semitones): `a4 = 1`,
    `a8 = wave 2^16`. Larger waves fall back to type 2.
    """
    if bank == 1 and 0 <= wave <= 24:
        return 0, 0x8000 if wave == 0 else wave << 16
    if bank == 2 and 0 <= wave <= 60:
        return 1, wave << 16
    return None


def select(  # pragma: no cover - needs the booted firmware
    code: Path, sram: Path, cache: Path | None, out: Path
) -> int:
    """Run the wave edit for every FM `(bank, category, wave)` and record type, `a4`, `a8`."""
    import arm_edit_capture as c
    import arm_emu as ae
    import arm_steady as ast
    import unicorn

    emu, blob = c.settled_emulator(code, sram, cache)
    stores = c.first_store_per_layer(c.find_stores(emu))
    store = next(s for s in stores if s.layer == 4)
    if not any(lo <= c.SCRATCH <= hi for lo, hi, _ in emu.uc.mem_regions()):
        emu.uc.mem_map(c.SCRATCH, ae.PAGE)
    objs: list[int] = []
    emu.uc.hook_add(
        unicorn.UC_HOOK_CODE,
        lambda uc, *_: objs.append(uc.reg_read(unicorn.arm_const.UC_ARM_REG_R0)),
        begin=0xC01CC85C,
        end=0xC01CC85C,
    )
    rows = ["bank,category,wave,type,a4,a8"]
    for bank, top in ((1, 25), (2, 61)):
        for cat in range(5):
            for wave in range(top + 1):
                ast.restore(emu, blob)
                emu.install_dsp_ports()
                objs.clear()
                for key, v in ((0xFA, 1), (0x125, bank), (0x126, cat)):
                    emu.write32(c.value_cell(emu, store.address, key), v)
                c.apply_value(emu, store.address, 0x127, wave, 2000)
                o = objs[-1]
                rows.append(
                    f"{bank},{cat},{wave},{emu.read32(o + 0x98)},"
                    f"{emu.read32(o + 0xA4)},{emu.read32(o + 0xA8):#x}"
                )
        print(f"bank {bank} done", flush=True)
    out.write_text("\n".join(rows) + "\n", encoding="utf-8")
    return 0


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    cp = sub.add_parser("capture")
    cp.add_argument("code", type=Path)
    cp.add_argument("sram", type=Path)
    cp.add_argument("--cache", type=Path)
    cp.add_argument("--out", type=Path, required=True)
    pp = sub.add_parser("probe")
    pp.add_argument("code", type=Path)
    pp.add_argument("sram", type=Path)
    pp.add_argument("--cache", type=Path)
    pp.add_argument("--types", type=int, nargs="+", default=[49, 52])
    sp = sub.add_parser("select")
    sp.add_argument("code", type=Path)
    sp.add_argument("sram", type=Path)
    sp.add_argument("--cache", type=Path)
    sp.add_argument("--out", type=Path, required=True)
    ns = ap.parse_args(argv)
    if ns.cmd == "select":
        return select(ns.code, ns.sram, ns.cache, ns.out)
    if ns.cmd == "probe":
        return probe(ns.code, ns.sram, ns.cache, tuple(ns.types))
    return capture(ns.code, ns.sram, ns.cache, ns.out)


if __name__ == "__main__":
    sys.exit(main())
