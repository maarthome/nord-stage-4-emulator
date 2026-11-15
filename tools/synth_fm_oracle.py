#!/usr/bin/env python3
"""Runs the FM stages `0x124269` / `0x12453F` in `sharc-org` and in the listing interpreter.

The pass is run twice with the same pokes: once with the stage replaced by an `RTS` stage
(`0x124B6A`, gives the memory the stage sees), once with the FM stage. The interpreter
([sharc_text_vm](sharc_text_vm.py)) runs the listing of the stage on the first state; every word
it writes is compared with the second.

    python tools/synth_fm_oracle.py --image SRAM_ORG/core2_sram.bin --stage 0x124269

prints the number of written words that differ over random trials. Needs the local ORG core 2
image and the built `sharc-org` / `sharc-disasm`.
"""

from __future__ import annotations

import argparse
import csv
import random
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
import sharc_text_vm as svm
import synth_fm_tables as ft
import synth_wave_oracle as wo

STAGES = {0x124269: (0x2484D2, 0x248A7E), 0x12453F: (0x248A7E, 0x248F6C)}
RTS_STAGE = 0x124B6A
# VERIFIED (listing `0x2484D2..`): the registers the pass sets for stage A of slot 0.
REGS = {0: wo.PARAM - 0x298 + 0x298, 1: 0xC15B6, 2: wo.RECORD, 3: 0xC0B88}
I0_BASE = 0xC1E02
# windows (start, count) dumped from the oracle: everything the stages read or write
WINDOWS = [
    (0xC0B70, 4),
    (0xC1590, 0x28),
    (0xC1670, 0x40),
    (0xC1DF0, 0x30),
    (0xC1EA0, 0x40),
    (0xC1F60, 0x50),
    (0xC2330, 0x30),
    (0xE0900, 0x16A),
]
PHASE_WORDS = list(range(0xC2336, 0xC2356))


def window_words() -> list[int]:
    return [a for lo, n in WINDOWS for a in range(lo, lo + n)]


def listing(disasm: str, image: str, stage: int) -> list[tuple[int, str]]:  # pragma: no cover
    lo, hi = STAGES[stage]
    out = subprocess.run(
        [disasm, image, hex(lo), hex(hi)], capture_output=True, text=True, check=True
    ).stdout
    return svm.parse_listing(out)


def run_oracle(
    sharc_org: str, image: str, stage: int, pokes: dict[int, int]
) -> dict[int, int]:  # pragma: no cover
    lines = [f"0 {wo.A_ENTRY:#x} {stage:#x}"] + [f"0 {a:#x} {v:#x}" for a, v in pokes.items()]
    with tempfile.TemporaryDirectory() as tmp:
        sp, cp = Path(tmp, "m.txt"), Path(tmp, "o.csv")
        sp.write_text("\n".join(lines) + "\n")
        cmd = [sharc_org, image]
        for dev in wo.DEVS:
            cmd += ["--dev", dev]
        cmd += ["--passes", "1", "--msg-base", "0", "--msg-script", str(sp)]
        for lo, n in WINDOWS:
            cmd += ["--buf", f"{lo:#x}:{n}:f32"]
        cmd += ["--csv", str(cp)]
        r = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if "fault" in r.stdout or not cp.exists():
            raise RuntimeError(r.stdout[-800:] + r.stderr[-800:])
        rows = list(csv.DictReader(cp.open()))
    return {int(row["address"], 16): int(row["raw"], 16) for row in rows}


def random_pokes(seed: int, table: dict[int, int], words: dict[int, int]) -> dict[int, int]:
    """Random lane inputs of one trial plus the FM table and the ARM parameter words."""
    rng = random.Random(seed)
    f = wo.fbits
    p: dict[int, int] = {}
    for i in (0, 1):
        p[wo.PARAM - 32 + i] = f(2 ** rng.uniform(-1, 3))
        p[wo.PARAM - 30 + i] = f(2 ** rng.uniform(-1, 3))
        p[wo.RECORD - 31 + i] = rng.getrandbits(32)
        p[wo.PARAM + 16 + i] = f(rng.uniform(0.5, 1.5))
    for a in PHASE_WORDS:
        p[a] = rng.getrandbits(32)
    p.update(words)
    p.update(table)
    sem = rng.uniform(-24.0, 36.0)
    p[wo.PITCH_SRC] = int(sem * 0x01000000) & 0xFFFFFFFF
    p[wo.S5_SRC] = f(rng.uniform(-0.55, 0.55))
    p[wo.S5_SRC + 1] = p[wo.S5_SRC]
    return p


def flat_reader(image_bytes: bytes, overlay: dict[int, int]):
    def read(a: int) -> int:
        if a in overlay:
            return overlay[a]
        return struct.unpack("<I", image_bytes[a * 4 : a * 4 + 4])[0]

    return read


def model(lines, image_bytes: bytes, pre: dict[int, int]) -> dict[int, int]:
    """Words the listing writes, starting from the image plus the pre-stage dump."""
    overlay = dict(pre)
    written: dict[int, int] = {}

    def write(a: int, w: int) -> None:
        overlay[a] = w
        written[a] = w

    vm = svm.Vm(flat_reader(image_bytes, overlay), write)
    for k, v in REGS.items():
        vm.i[k] = v
    vm.i[0] = I0_BASE
    vm.run(lines)
    return written


def compare(
    sharc_org: str,
    disasm: str,
    image: str,
    trials: int,
    stage: int,
    table_type: int,
    csv_path: Path,
) -> tuple[int, int]:  # pragma: no cover
    tabs = ft.load_csv(csv_path)
    table = tabs[table_type]
    words_csv = wo_words(table_type)
    lines = listing(disasm, image, stage)
    img = Path(image).read_bytes()
    equal = total = 0
    for seed in range(1, trials + 1):
        pokes = random_pokes(seed, table, words_csv)
        pre = run_oracle(sharc_org, image, RTS_STAGE, pokes)
        post = run_oracle(sharc_org, image, stage, pokes)
        got = model(lines, img, pre)
        for a, w in got.items():
            if a in post:
                total += 1
                equal += w == post[a]
    return equal, total


def wo_words(table_type: int) -> dict[int, int]:
    """ARM words of the FM type outside the P block window (`data/synth-p-words.csv`)."""
    path = Path(__file__).resolve().parents[1] / "docs/research/data/synth-p-words.csv"
    out: dict[int, int] = {}
    with path.open(encoding="utf-8") as fh:
        for r in csv.DictReader(fh):
            if int(r["type"]) == table_type and int(r["a"]) == 0 and int(r["b"]) == 0:
                out[int(r["dm"], 0)] = int(r["word"], 0)
    return out


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--image", required=True)
    ap.add_argument("--sharc-org", default="target/release/sharc-org")
    ap.add_argument("--sharc-disasm", default="target/release/sharc-disasm")
    ap.add_argument("--trials", type=int, default=10)
    ap.add_argument("--stage", type=lambda v: int(v, 0), default=0x124269)
    ap.add_argument("--type", type=int, default=49)
    ap.add_argument(
        "--tables",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "docs/research/data/synth-fm-tables.csv",
    )
    ns = ap.parse_args(argv)
    equal, total = compare(
        ns.sharc_org, ns.sharc_disasm, ns.image, ns.trials, ns.stage, ns.type, ns.tables
    )
    print(f"{equal} of {total} written words equal")
    return 0 if equal == total else 1


if __name__ == "__main__":
    sys.exit(main())
