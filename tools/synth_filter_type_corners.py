#!/usr/bin/env python3
"""Corner frequency of each synth filter stage relative to its nominal cutoff (derived table).

Every stage of the family (`0x1253BD` 12 dB low pass, `0x12566E` 24 dB low pass, `0x125913` band
pass, `0x125A50` high pass) is a bilinear section with the nominal cutoff `atan(R) / pi` cycles
per sample, `R` the frame word the ARM cutoff word fixes independently of the type
(`docs/research/synth-cutoff-voice-law.md`). What differs per type is where the response crosses
-3 dB relative to that nominal cutoff, and that depends on the damping `P(idx)` of the table
position `idx`. This tool evaluates the closed-form responses of the section equations
(`tools/synth_filter_model.py`, bit exact to the SHARC oracle) on a grid and writes the table
`docs/research/data/synth-filter-type-corners.csv`:

    python tools/synth_filter_type_corners.py            # print the CSV
    python tools/synth_filter_type_corners.py --write    # regenerate the committed table
    python tools/synth_filter_type_corners.py --check    # fail if the committed table differs

Columns: `type`, `idx`, `R`, `nominal` (cycles per sample), `f3_over_nominal` (low pass / high
pass: the -3 dB frequency relative to the passband gain; band pass: the lower and upper -3 dB
frequencies relative to the peak, as `lo/hi`), `peak_over_nominal`, `peak_db` (peak over the
passband gain).
"""

from __future__ import annotations

import csv
import io
import math
import struct
import sys
from pathlib import Path

DATA = Path(__file__).resolve().parents[1] / "docs/research/data"
TABLES_CSV = DATA / "synth-filter-tables.csv"
TABLE_D_CSV = DATA / "synth-filter-table-d.csv"
OUT_CSV = DATA / "synth-filter-type-corners.csv"

TYPES = ("lp12", "lp24", "bp4", "hp4")
# VERIFIED (`docs/research/synth-filter-type-corners.md`): 47.4 .. 80.6 is the position range
# the firmware resonance word reaches; the rows outside it show the full table.
IDX_GRID = (0, 16, 32, 47, 56, 64, 72, 81, 96, 112, 127)
R_GRID = (0.0102, 0.1, 0.36)
GRID_POINTS = 6000


def _f32(word: int) -> float:
    return struct.unpack("<f", struct.pack("<I", word))[0]


def load_tables() -> dict[str, list[float]]:
    """Tables A, C, B (`synth-filter-tables.csv`) and D (`synth-filter-table-d.csv`) as floats."""
    out: dict[str, list[float]] = {"A": [], "C": [], "B": [], "D": []}
    with TABLES_CSV.open(newline="", encoding="utf-8") as fh:
        for row in csv.DictReader(fh):
            out["A"].append(float(row["A_0xB0D0F"]))
            out["C"].append(float(row["C_0xB0D90"]))
            out["B"].append(float(row["B_0xB1015"]))
    with TABLE_D_CSV.open(newline="", encoding="utf-8") as fh:
        out["D"] = [_f32(int(row["word"], 16)) for row in csv.DictReader(fh)]
    return out


def section_gk(r: float, p: float) -> tuple[float, float, float]:
    """`g = 2 R N`, `k = P N`, `h = 1 - g^2 / 4 - g k / 2` with `N = 1 / sqrt(1 + R P + R^2)`."""
    n = (1.0 + r * p + r * r) ** -0.5
    g, k = 2.0 * r * n, p * n
    return g, k, 1.0 - 0.25 * g * g - 0.5 * g * k


def section_response(z: complex, g: float, k: float, h: float, kind: str) -> complex:
    """Transfer function of one section at `z` (`kind`: `lp`, `bp`, `hp`)."""
    s = 1.0 + z * g * g / (z - 1.0) ** 2 + k * g / (z - 1.0)
    if kind == "hp":
        return h / s
    v = (z + 1.0) / (2.0 * z) / s  # the input average `(u + b) / 2` with `b = u / z`
    if kind == "bp":
        return z * g * v / (z - 1.0)
    return g * g * v * (z + 1.0) / (2.0 * (z - 1.0) ** 2)


def stage_params(
    tables: dict[str, list[float]], kind: str, idx: int
) -> tuple[float, float, int, str]:
    """`(cin, damping P, sections, section kind)` of a stage at the integer table index."""
    if kind == "lp12":
        return tables["A"][idx], tables["D"][idx], 1, "lp"
    if kind == "lp24":
        return tables["A"][idx], tables["B"][idx], 2, "lp"
    if kind == "bp4":
        return tables["C"][idx], tables["B"][idx], 2, "bp"
    return tables["A"][idx], tables["B"][idx], 2, "hp"


def gain_at(tables: dict[str, list[float]], kind: str, r: float, idx: int, f: float) -> float:
    """Magnitude of the stage (with its input gain) at `f` cycles per sample, clip not active."""
    cin, p, secs, sk = stage_params(tables, kind, idx)
    g, k, h = section_gk(r, p)
    z = complex(math.cos(2.0 * math.pi * f), math.sin(2.0 * math.pi * f))
    return abs(cin * section_response(z, g, k, h, sk) ** secs)


def corner_row(tables: dict[str, list[float]], kind: str, r: float, idx: int) -> dict[str, str]:
    """One CSV row: the -3 dB corner, peak position and peak height relative to the nominal."""
    fs = [0.5 * i / GRID_POINTS for i in range(1, GRID_POINTS)]
    m = [gain_at(tables, kind, r, idx, f) for f in fs]
    ip = max(range(len(m)), key=m.__getitem__)
    nom = math.atan(r) / math.pi
    if kind in ("lp12", "lp24"):
        ref = gain_at(tables, kind, r, idx, 1e-6)
    elif kind == "hp4":
        ref = m[-1]
    else:
        ref = m[ip]
    thr = ref / math.sqrt(2.0)
    if kind == "bp4":
        lo = next((fs[i] for i in range(ip, -1, -1) if m[i] < thr), None)
        hi = next((fs[i] for i in range(ip, len(m)) if m[i] < thr), None)
        f3 = "/".join("" if v is None else f"{v / nom:.4f}" for v in (lo, hi))
    elif kind == "hp4":
        v = next((fs[i] for i in range(ip, -1, -1) if m[i] < thr), None)
        f3 = "" if v is None else f"{v / nom:.4f}"
    else:
        v = next((fs[i] for i in range(ip, len(m)) if m[i] < thr), None)
        f3 = "" if v is None else f"{v / nom:.4f}"
    return {
        "type": kind,
        "idx": str(idx),
        "R": f"{r:g}",
        "nominal": f"{nom:.6f}",
        "f3_over_nominal": f3,
        "peak_over_nominal": f"{fs[ip] / nom:.4f}",
        "peak_db": f"{20.0 * math.log10(m[ip] / ref):.2f}",
    }


FIELDS = ("type", "idx", "R", "nominal", "f3_over_nominal", "peak_over_nominal", "peak_db")


def table_text() -> str:
    """The committed CSV as text (LF line ends)."""
    tables = load_tables()
    buf = io.StringIO()
    w = csv.DictWriter(buf, fieldnames=FIELDS, lineterminator="\n")
    w.writeheader()
    for kind in TYPES:
        for r in R_GRID:
            for idx in IDX_GRID:
                w.writerow(corner_row(tables, kind, r, idx))
    return buf.getvalue()


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    text = table_text()
    if args[:1] == ["--check"]:
        have = OUT_CSV.read_text(encoding="utf-8").replace("\r\n", "\n")
        if have != text:
            print(f"{OUT_CSV} differs from the generated table", file=sys.stderr)
            return 1
        print("ok: table matches")
        return 0
    if args[:1] == ["--write"]:
        OUT_CSV.write_text(text, encoding="utf-8", newline="\n")
        print(f"wrote {OUT_CSV}")
        return 0
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
