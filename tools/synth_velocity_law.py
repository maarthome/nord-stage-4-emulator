#!/usr/bin/env python3
"""Synth velocity laws of the ORG core 2 strip (amp `kAmpEnvVel`, filter envelope `kFltEnvVel`).

The ARM key message carries `g = key_velocity_g(vel)` (`docs/research/keydown-zones.md` s7) in
byte 1 (`0x80 + g`, the controller word `DM(0xC1DCC)` of the slot). The ORG core 2 pass turns
it into

* the amp velocity gain `V(setting, g)` at `DM(0xC10B4)` (slot 0), from the strip words
  `kAmpEnvVel`: `a = strip 0 word 53`, `b = strip 6 word 53` (`synth-knob-laws.csv`):
  `bits(V) = bits(a) + ((bits(a + b) - bits(a)) * G >> 7)` with `G = g`, and `G = 128` for
  `g = 127` (a geometric blend, 1 ulp);
* the filter-envelope velocity factor `F = G / 128` at `DM(0xC2723)` when `kFltEnvVel` is on
  (strip 0 word 19 = 0.0, strip 6 word 19 = 1.0), else 1.0.

    python tools/synth_velocity_law.py --check   # model against the committed CSV (no firmware)
    python tools/synth_velocity_law.py oracle    # re-measure with `sharc-org` (local ORG image)

The firmware image is read only by the `oracle` command and never stored. Evidence:
`docs/research/synth-velocity.md`.
"""

from __future__ import annotations

import argparse
import csv
import struct
import sys
from pathlib import Path

CSV_PATH = Path(__file__).resolve().parent.parent / "docs/research/data/synth-velocity-law.csv"

# VERIFIED: `kAmpEnvVel` words of strip 0 (a) and strip 6 (b) for the settings 0..3
# (docs/research/data/synth-knob-laws.csv rows 0x131, `q16` cell).
AMP_A = (0x3F800000, 0x3F004DCE, 0x3E809BCC, 0x3E00E9F9)
AMP_B = (0x00000000, 0x3F41FB22, 0x3FAAB6D5, 0x3FEF4782)
SETTINGS = (0, 1, 2, 3)


def f2u(x: float) -> int:
    """float32 bits of a number."""
    return struct.unpack("<I", struct.pack("<f", x))[0]


def u2f(u: int) -> float:
    """float of float32 bits."""
    return struct.unpack("<f", struct.pack("<I", u & 0xFFFFFFFF))[0]


def effective_g(g: int) -> int:
    """`G`: the key byte `g = 127` acts as 128 (one extra step), VERIFIED against the oracle."""
    return 128 if g >= 127 else g


def amp_word(setting: int, g: int) -> int:
    """float32 bits of the amp velocity gain `V` for a setting `0..3` and key byte `g` `0..127`."""
    a = AMP_A[setting]
    top = f2u(u2f(a) + u2f(AMP_B[setting]))
    return a + (((top - a) * effective_g(g)) >> 7)


def filter_word(on: bool, g: int) -> int:
    """float32 bits of the filter-envelope velocity factor."""
    return f2u(effective_g(g) / 128.0) if on else f2u(1.0)


def model_rows() -> list[tuple[str, int, int, int]]:
    """`(law, setting, g, word)` rows of the model: amp for 4 settings, filter off / on."""
    rows = [("amp", s, g, amp_word(s, g)) for s in SETTINGS for g in range(128)]
    rows += [
        ("flt", int(on), g, filter_word(bool(on), g)) for on in (False, True) for g in range(128)
    ]
    return rows


def read_csv(path: Path) -> list[tuple[str, int, int, int]]:
    """Rows of the committed CSV."""
    with path.open(newline="") as fh:
        return [
            (r["law"], int(r["setting"]), int(r["g"]), int(r["word"], 16))
            for r in csv.DictReader(fh)
        ]


def write_csv(path: Path, rows: list[tuple[str, int, int, int]]) -> None:
    """Writes `law,setting,g,word` rows."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(["law", "setting", "g", "word"])
        for law, s, g, word in rows:
            w.writerow([law, s, g, f"{word:#010x}"])


def compare(
    measured: list[tuple[str, int, int, int]], model: list[tuple[str, int, int, int]]
) -> int:
    """Largest bit difference of two row lists (law, setting and g must agree)."""
    if [r[:3] for r in measured] != [r[:3] for r in model]:
        raise ValueError("row keys differ")
    return max(abs(m[3] - d[3]) for m, d in zip(measured, model))


def oracle_rows(sharc_org: str, image: str) -> list[tuple[str, int, int, int]]:  # pragma: no cover
    """Re-measures every row with the ORG core 2 oracle (needs `sharc-org`, local image)."""
    import synth_adr_oracle as o

    lag = 4

    def series(msgs: list[tuple[int, int, int]], address: int) -> list[int]:
        passes = 128 * lag + 8
        pokes = [(1 + g * lag, o.CONTROLLER, 0x01000000 | ((0x80 + g) << 8)) for g in range(1, 128)]
        rows = o.run_script(
            sharc_org,
            image,
            passes,
            pre={o.CONTROLLER: 0x01000000 | (0x80 << 8)},
            pokes=pokes,
            msgs=msgs,
            bufs=[f"{address:#x}:1:f32"],
        )
        return [rows[(g * lag + lag, 0)][0] for g in range(128)]

    out: list[tuple[str, int, int, int]] = []
    for s in SETTINGS:
        words = series([(0, 53, AMP_A[s]), (0, 425, AMP_B[s])], 0xC10B4)
        out += [("amp", s, g, w) for g, w in enumerate(words)]
    for on in (0, 1):
        pair = (0, f2u(1.0)) if on else (f2u(1.0), 0)
        words = series([(0, 19, pair[0]), (0, 391, pair[1])], 0xC2723)
        out += [("flt", on, g, w) for g, w in enumerate(words)]
    return out


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("cmd", nargs="?", choices=("oracle",))
    ap.add_argument("--check", action="store_true", help="compare the model with the committed CSV")
    ap.add_argument("--out", type=Path, default=CSV_PATH)
    ns = ap.parse_args(argv)
    if ns.cmd == "oracle":
        import synth_adr_oracle as o

        org, image = o.default_paths()
        rows = oracle_rows(org, image)
        write_csv(ns.out, rows)
        print(f"wrote {len(rows)} rows, max err vs model {compare(rows, model_rows())} ulp")
        return 0
    err = compare(read_csv(CSV_PATH), model_rows())
    print(f"model vs CSV: max error {err} bit(s)")
    return int(err > 1)


if __name__ == "__main__":
    sys.exit(main())
