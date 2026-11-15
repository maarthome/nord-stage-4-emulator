#!/usr/bin/env python3
"""ARM note messages -> SMP voice record fields -> `sharc-ref` replay (the slew inputs of a note).

    uv run --with unicorn python tools/arm_smp_slew_init.py capture \
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin \
        --cache target/keydown.snap --ns4b ".reference/<backup>.ns4b" \
        --npno "Piano/Grand/Velvet Grand Med 6.1.npno" --velocity 100 --out target/exp/on100.json
    python tools/arm_smp_slew_init.py decode target/exp/on100.json  # (record, field) per message
    python tools/arm_smp_slew_init.py replay target/exp/on100.json target/exp/on100.scn
    target/release/sharc-ref target/oracle/smp.bin target/exp/on100.scn --out target/exp/on100

`capture` runs the real firmware note-on (library stub, `arm_piano_load`) and stores the path 1
messages (derived numbers only: tick, path, DM word, value). `replay` feeds them to the SMP image
in batches of at most 40 pairs (one batch per vector 13 + audio interrupt; the FIFO of the SMP
image takes a bounded number of pairs per interrupt) and captures every slew field of the
records after each interrupt. Findings: `docs/research/smp-slew.md` section 6.
"""

from __future__ import annotations

import argparse
import csv
import json
import sys
from collections.abc import Iterable, Sequence
from pathlib import Path

# VERIFIED (docs/research/piano-dsp-path.md): 114 records of 54 words from DM 0x92848, field f of
# record k is DM BASE + 54 k + f (f in -32..21).
BASE = 0x92848
STRIDE = 54
COUNT = 114
FIELD_LO, FIELD_HI = -32, 21
# The slew inputs and outputs read back after every interrupt.
FIELDS = (-2, -1, 2, 3, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 0, 1)
# EMPIRICAL (run): pairs the SMP FIFO takes per interrupt without losing the tail of a batch.
BATCH = 40
HEADER = (
    "meta rate 48000",
    "warmup 0x1c11e0 100000 flag12=1",
    "poke 0x5201 0x40",
    "fifo 0x92196",
    "irq 13",
    "poke 0x9268B 0",
    "poke 0x9268A 0",
)


def decode(dm_word: int) -> tuple[int, int] | None:
    """`(record, field)` of a DM word inside the voice record array, else `None`."""
    rel = dm_word - BASE
    if not -32 <= rel < COUNT * STRIDE + FIELD_HI + 1:
        return None
    k = (rel + 32) // STRIDE
    f = rel - STRIDE * k
    if f > FIELD_HI:
        k, f = k + 1, rel - STRIDE * (k + 1)
    return (k, f) if 0 <= k < COUNT else None


def addr(record: int, field: int) -> int:
    return BASE + STRIDE * record + field


def replay_scenario(
    msgs: Sequence[Sequence[int]], records: Iterable[int], rows: int, batch: int = BATCH
) -> str:
    """Scenario text: header, `msgs` (`[tick, path, dm_word, value]`, path 1 only) in batches,
    then `rows` interrupts capturing `FIELDS` of `records` (low and high 16 bits)."""
    lines = list(HEADER)
    pairs = [m for m in msgs if m[1] == 1]
    for i in range(0, len(pairs), batch):
        lines.append("irq 13")
        lines += [
            f"msg op={dm >> 28} {dm & 0xFFFFFFF:#x} {w:#x}"
            for _t, _p, dm, w in pairs[i : i + batch]
        ]
        lines.append("irq 17")
    lines += [f"loop {rows}", "irq 17"]
    for k in records:
        for f in FIELDS:
            for part in ("lo", "hi"):
                lines.append(f"capture r{k}_f{f}_{part} {addr(k, f):#x} 1 part={part}")
    lines.append("end")
    return "\n".join(lines) + "\n"


def read_words(csv_path: Path, record: int, field: int) -> list[int]:
    """The captured 32-bit words of one field per interrupt (low and high halves joined)."""
    with csv_path.open() as handle:
        rows = list(csv.reader(line for line in handle if not line.startswith("#")))
    head, data = rows[0], rows[1:]
    lo, hi = head.index(f"r{record}_f{field}_lo"), head.index(f"r{record}_f{field}_hi")
    return [
        ((int(float(r[hi])) << 16) | int(float(r[lo]))) & 0xFFFFFFFF for r in data if r[lo] != ""
    ]


def slew_updates(words: Sequence[int]) -> list[int]:
    """The gain words at every second interrupt (the record is updated every second one)."""
    return [w if w < 1 << 31 else w - (1 << 32) for w in words[::2]]


def capture(ns: argparse.Namespace) -> int:  # pragma: no cover - needs unicorn and local files
    import arm_keydown as kd
    import arm_piano_load as pl

    rig = kd.build_rig(ns.image, ns.sram, ns.cache)
    image = pl.device_image(pl.read_npno(ns.ns4b, ns.npno), "Library.npno")
    pl.LibraryStub({1: image}, pl.KIND_FULL).install(rig.emu)
    pl.load_sound(rig.emu, pl.managers(rig.emu)[0], 1)
    pl.select_sound(rig.emu, 1)
    pl.install_dsp_ready(rig.emu)
    rig.tick(2)
    phases = []
    for label, data, ticks in (
        ("on", kd.note_on(ns.note, ns.velocity), ns.on_ticks),
        ("off", kd.note_off(ns.note), ns.off_ticks),
    ):
        rig.send(data)
        msgs = [[t, m.path, m.dm_word, m.word2] for t, m in rig.tick(ticks)]
        phases.append({"label": label, "msgs": msgs})
    ns.out.write_text(json.dumps(phases))
    print({p["label"]: len(p["msgs"]) for p in phases})
    return 0


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    cap = sub.add_parser("capture")
    cap.add_argument("image", type=Path)
    cap.add_argument("sram", type=Path)
    cap.add_argument("--cache", type=Path)
    cap.add_argument("--ns4b", type=Path, required=True)
    cap.add_argument("--npno", required=True)
    cap.add_argument("--note", type=int, default=60)
    cap.add_argument("--velocity", type=int, default=100)
    cap.add_argument("--on-ticks", type=int, default=60)
    cap.add_argument("--off-ticks", type=int, default=100)
    cap.add_argument("--out", type=Path, required=True)
    dec = sub.add_parser("decode")
    dec.add_argument("json", type=Path)
    rep = sub.add_parser("replay")
    rep.add_argument("json", type=Path)
    rep.add_argument("out", type=Path)
    rep.add_argument("--records", type=int, nargs="+", default=[81])
    rep.add_argument("--rows", type=int, default=30)
    rep.add_argument("--phase", type=int, default=0)
    ns = ap.parse_args(argv)
    if ns.cmd == "capture":
        return capture(ns)
    phases = json.loads(ns.json.read_text())
    if ns.cmd == "decode":
        for p in phases:
            print("==", p["label"])
            for t, _path, dm, w in p["msgs"]:
                where = decode(dm)
                print(f"t{t} dm={dm:#x} {'rec%d F%+d' % where if where else '-'} = {w:#010x}")
        return 0
    text = replay_scenario(phases[ns.phase]["msgs"], ns.records, ns.rows)
    ns.out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {ns.out} ({text.count(chr(10))} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
