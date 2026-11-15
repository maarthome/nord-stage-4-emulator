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
import struct
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


# EMPIRICAL (message list of every captured note): the ARM stores the level and rate words of the
# main voice at these DM words before the first slew update (`docs/research/smp-slew.md` s6).
LEVEL_WORD_DM = 0x9252A
RATE_WORD_DM = 0x9252C
TWO24 = float(1 << 24)


def note_on_words(msgs: Iterable[Sequence[int]]) -> dict[str, float | None]:
    """Level (`F15`) and rate (`F17`) words the ARM stores for a note (path 1 messages).

    The last write to each DM word wins. Returns the levels as floats, and the rate as the decay
    count `d` of `1 - d / 2^24` (the per-update hold decay), `None` when the word was not written.
    """
    last: dict[int, int] = {}
    for _t, path, dm, word in msgs:
        if path == 1:
            last[dm & 0xFFFFFFF] = word
    out: dict[str, float | None] = {"level": None, "rate_d": None}
    if LEVEL_WORD_DM in last:
        out["level"] = struct.unpack("<f", struct.pack("<I", last[LEVEL_WORD_DM] & 0xFFFFFFFF))[0]
    if RATE_WORD_DM in last:
        rate = struct.unpack("<f", struct.pack("<I", last[RATE_WORD_DM] & 0xFFFFFFFF))[0]
        out["rate_d"] = round((1.0 - rate) * TWO24, 1)
    return out


def capture_phases(  # pragma: no cover - needs unicorn and local files
    image: Path,
    sram: Path,
    cache: Path | None,
    ns4b: Path,
    npno: str,
    note: int,
    velocity: int,
    on_ticks: int,
    off_ticks: int,
) -> list[dict]:
    """Runs the firmware note-on / note-off of one stroke and returns the two message phases."""
    import arm_keydown as kd
    import arm_piano_load as pl

    rig = kd.build_rig(image, sram, cache)
    dev = pl.device_image(pl.read_npno(ns4b, npno), "Library.npno")
    pl.LibraryStub({1: dev}, pl.KIND_FULL).install(rig.emu)
    pl.load_sound(rig.emu, pl.managers(rig.emu)[0], 1)
    pl.select_sound(rig.emu, 1)
    pl.install_dsp_ready(rig.emu)
    rig.tick(2)
    phases = []
    for label, data, ticks in (
        ("on", kd.note_on(note, velocity), on_ticks),
        ("off", kd.note_off(note), off_ticks),
    ):
        rig.send(data)
        msgs = [[t, m.path, m.dm_word, m.word2] for t, m in rig.tick(ticks)]
        phases.append({"label": label, "msgs": msgs})
    return phases


def capture(ns: argparse.Namespace) -> int:  # pragma: no cover - needs unicorn and local files
    phases = capture_phases(
        ns.image, ns.sram, ns.cache, ns.ns4b, ns.npno, ns.note, ns.velocity,
        ns.on_ticks, ns.off_ticks,
    )  # fmt: skip
    ns.out.write_text(json.dumps(phases))
    print({p["label"]: len(p["msgs"]) for p in phases})
    return 0


def rates(ns: argparse.Namespace) -> int:  # pragma: no cover - needs unicorn and local files
    """Level and rate words for every sound x velocity x note (one firmware run per stroke)."""
    rows = []
    for npno in ns.npno:
        for velocity in ns.velocities:
            for note in ns.notes:
                ph = capture_phases(
                    ns.image, ns.sram, ns.cache, ns.ns4b, npno, note, velocity, ns.on_ticks, 1
                )
                row = {"npno": npno, "velocity": velocity, "note": note}
                row.update(note_on_words(ph[0]["msgs"]))
                rows.append(row)
                print(json.dumps(row), flush=True)
    ns.out.write_text(json.dumps(rows, indent=1))
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
    rt = sub.add_parser("rates")
    rt.add_argument("image", type=Path)
    rt.add_argument("sram", type=Path)
    rt.add_argument("--cache", type=Path)
    rt.add_argument("--ns4b", type=Path, required=True)
    rt.add_argument("--npno", nargs="+", required=True)
    rt.add_argument("--velocities", type=int, nargs="+", default=[20, 64, 100, 127])
    rt.add_argument("--notes", type=int, nargs="+", default=[60])
    rt.add_argument("--on-ticks", type=int, default=60)
    rt.add_argument("--out", type=Path, required=True)
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
    if ns.cmd == "rates":
        return rates(ns)
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
