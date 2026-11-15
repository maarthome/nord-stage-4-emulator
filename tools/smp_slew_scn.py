#!/usr/bin/env python3
"""`sharc-ref` scenarios for the SMP voice slew loop and the stream edge handler.

    python tools/smp_slew_scn.py random     OUT.scn [--records LO HI] [--seed N]
    python tools/smp_slew_scn.py trajectory OUT.scn
    python tools/smp_slew_scn.py edges      OUT.scn [--counter C]
    target/release/sharc-ref smp.bin OUT.scn --out OUT

`random` pokes random float fields into records LO..HI (default 10..57), captures the whole record
array before and after one audio interrupt; `trajectory` runs 60 updates of record 20 (two
interrupts per update) and captures its gains; `edges` pokes the loop and end fields of every
record and captures the array over three interrupts. The expected results and the Rust model
are in `docs/research/smp-slew.md` and `crates/nord-core/src/piano/smp_slew.rs`.
"""

from __future__ import annotations

import argparse
import random
import struct
import sys
from pathlib import Path

# VERIFIED (docs/research/piano-dsp-path.md): 114 voice records of 54 words from DM 0x92848; field f
# of record k is DM BASE + 54 k + f (f in -32..21).
BASE = 0x92848
STRIDE = 54
COUNT = 114
HEADER = (
    "meta rate 48000",
    "warmup 0x1c11e0 100000 flag12=1",
    "poke 0x5201 0x40",
    "fifo 0x92196",
    "irq 13",
    "poke 0x9268B 0",
    "poke 0x9268A 0",
)
FLOAT_FIELDS = (-2, -1, 2, 3, 4, 5, 11, 12, 13, 14, 15, 16, 17, 18)


def fbits(x: float) -> int:
    """IEEE single bits of ``x``."""
    return struct.unpack("<I", struct.pack("<f", x))[0]


def addr(k: int, f: int) -> int:
    """DM address of field ``f`` of record ``k``."""
    return BASE + STRIDE * k + f


def poke(k: int, f: int, word: int) -> str:
    return f"poke 0x{addr(k, f):X} 0x{word & 0xFFFFFFFF:X}"


def capture_array(prefix: str) -> list[str]:
    """Capture the whole record array as low and high halves (integer words stay exact)."""
    first, count = BASE - 32, COUNT * STRIDE + 32
    return [f"capture {prefix}_{p} 0x{first:X} {count} part={p}" for p in ("lo", "hi")]


def random_scenario(lo: int = 10, hi: int = 57, seed: int = 7) -> str:
    """Random float fields and Q31 multipliers in records ``lo..hi``, one interrupt."""
    rng = random.Random(seed)
    lines = list(HEADER)
    for k in range(lo, hi):
        for f in FLOAT_FIELDS:
            lines.append(poke(k, f, fbits(rng.uniform(0.05, 1.0))))
        for f in (19, 20, -20, -19, 0, 1):
            lines.append(poke(k, f, rng.randrange(0x10000000, 0x70000000)))
    lines += capture_array("b") + ["irq 17"] + capture_array("a")
    return "\n".join(lines) + "\n"


def trajectory_scenario(k: int = 20) -> str:
    """60 updates of one record (two interrupts each); captures F0, F1, F-2, F13..F16 per update."""
    init = {-2: 0.0, -1: 0.04, 2: 0.0, 3: 1.0, 11: 0.0, 12: 0.0, 13: 0.95, 14: 1.0, 15: 1.0}
    init |= {16: 0.9, 17: 0.979, 18: 0.979}
    lines = list(HEADER)
    lines += [poke(k, f, fbits(v)) for f, v in init.items()]
    lines += [poke(k, 19, 0x7FFFFFFF), poke(k, 20, 0x60000000), "loop 60", "irq 17", "irq 17"]
    for f in (0, 1, -2, 13, 14, 15, 16):
        for part in ("lo", "hi"):
            lines.append(f"capture f{f}_{part} 0x{addr(k, f):X} 1 part={part}")
    lines.append("end")
    return "\n".join(lines) + "\n"


def edges_scenario(counter: int = 130) -> str:
    """Loop end 100, loop length 40, history offset 3; counter ``counter + k`` per record."""
    lines = list(HEADER)
    for k in range(COUNT):
        words = {8: 1, -8: 100, -7: 40, -30: counter + k, 6: fbits(0.5), 7: fbits(0.25)}
        words |= {-24: 3, -9: 0, -6: 500, -5: 600, -4: 7, 2: fbits(0.5), 3: fbits(0.5)}
        lines += [poke(k, f, w) for f, w in words.items()]
    for i in range(3):
        lines += capture_array(f"s{i}") + ["irq 17"]
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("kind", choices=("random", "trajectory", "edges"))
    ap.add_argument("out", type=Path)
    ap.add_argument("--records", nargs=2, type=int, default=(10, 57), metavar=("LO", "HI"))
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--counter", type=int, default=130)
    args = ap.parse_args(argv)
    if args.kind == "random":
        text = random_scenario(*args.records, seed=args.seed)
    elif args.kind == "trajectory":
        text = trajectory_scenario()
    else:
        text = edges_scenario(args.counter)
    args.out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {args.out} ({text.count(chr(10))} lines)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
