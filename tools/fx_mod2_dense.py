#!/usr/bin/env python3
"""Dense capture of the Mod 2 (`Fx2`) amount and rate words between the 11 sampled values.

`fx_clients.value_grid` samples a 0..127 id at 11 points; the Mod 2 laws of
`crates/nord-core/src/fx/fw_mod2*.rs` interpolate between them. This tool re-runs the slot capture
(`fx_clients.capture_slot`) for the Fx2 slot with a dense grid, so the interpolation can be checked
against the ARM (`docs/research/fx-mod2-validation.md`).

    uv run --with unicorn python tools/fx_mod2_dense.py CODE.bin SRAM.bin \
        --cache target/settled.snap --units 3,4,0 --out target/fx2_dense.csv
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

#: Dense value grid of a 0..127 id: every 8th value, the CHOR knee 60..80, the ends and the 11 original samples.
#: The 11 values of the original capture (`fw_mod2_data.SAMPLES`), so the dense run is comparable.
SAMPLES = (0, 1, 2, 15, 31, 47, 63, 79, 95, 111, 127)
DENSE = sorted(set(range(0, 128, 8)) | set(range(60, 81)) | {3, 126} | set(SAMPLES))


def dense_grid(lo: int, hi: int) -> list[int]:
    """`DENSE` clipped to `[lo, hi]`; a small range (enum ids) is listed whole."""
    if hi - lo < 12:
        return list(range(lo, hi + 1))
    return [v for v in DENSE if lo <= v <= hi]


def f32(word: int) -> float:
    return struct.unpack("<f", struct.pack("<I", word & 0xFFFFFFFF))[0]


def as_number(word: int, is_float: bool) -> float:
    """A word as a float (`is_float`) or a signed integer."""
    if is_float:
        return f32(word)
    return float(word - (1 << 32) if word & 0x80000000 else word)


def looks_float(word: int) -> bool:
    """IEEE exponent in the plausible range of `fx_fit.is_float_word`, or a zero."""
    return word in (0, 0x80000000) or 67 <= (word >> 23) & 0xFF <= 187


def series_of(rows: list[dict[str, str]]) -> dict[tuple[str, str, str, str], dict[int, int]]:
    """`(kind, type, id, dm_word) -> {value: last word}` of the `param` rows of a capture."""
    out: dict[tuple[str, str, str, str], dict[int, int]] = {}
    for r in rows:
        if r["stage"] != "param":
            continue
        key = (r["kind"], r["type"], "Amt" if r["id"].endswith("Amt") else "Rate", r["dm_word"])
        out.setdefault(key, {})[int(r["value"])] = int(r["word"], 16)
    return out


def interp(series: dict[int, int], v: int, is_float: bool, knots: tuple[int, ...] = SAMPLES) -> float:
    """The law of `fw_mod2_law_stage::eval`: linear between the neighbouring `knots`."""
    k = max(i for i, s in enumerate(knots) if s <= v)
    k = min(k, len(knots) - 2)
    x0, x1 = knots[k], knots[k + 1]
    a, b = as_number(series[x0], is_float), as_number(series[x1], is_float)
    return a + (b - a) * (v - x0) / (x1 - x0)


def interpolation_error(series: dict[int, int]) -> tuple[float, int, bool]:
    """Worst `|dense - linear|` over the range of the series, the value where it occurs and the
    word type. Needs the 11 `SAMPLES` in `series`."""
    is_float = all(looks_float(w) for w in series.values())
    nums = {v: as_number(w, is_float) for v, w in series.items()}
    span = (max(nums.values()) - min(nums.values())) or 1.0
    worst, at = 0.0, 0
    for v, x in nums.items():
        e = abs(x - interp(series, v, is_float)) / span
        if e > worst:
            worst, at = e, v
    return worst, at, is_float


def analyze(rows: list[dict[str, str]]) -> list[tuple[str, str, str, str, float, int, bool, int]]:
    """Per series: `(kind, type, id, dm_word, worst relative error, at value, float, range==0)`."""
    out = []
    for (kind, ty, ident, dm), ser in sorted(series_of(rows).items()):
        if not all(v in ser for v in SAMPLES):
            continue
        err, at, flt = interpolation_error(ser)
        out.append((kind, ty, ident, dm, err, at, flt, len(set(ser.values()))))
    return out


EXPORT_FIELDS = ["type", "id", "dm_word", "value", "word"]


def export_rows(rows: list[dict[str, str]], kind: str = "piano") -> list[dict[str, str]]:
    """The `param` rows of one unit kind, last word per `(type, id, dm_word, value)`, sorted;
    `docs/research/data/fx-mod2-dense.csv` is this list for the piano unit."""
    last: dict[tuple[int, str, int, int], str] = {}
    for r in rows:
        if r["stage"] != "param" or r["kind"] != kind:
            continue
        ident = "Amt" if r["id"].endswith("Amt") else "Rate"
        last[(int(r["type"]), ident, int(r["dm_word"], 16), int(r["value"]))] = r["word"]
    return [
        {"type": str(t), "id": i, "dm_word": f"{dm:#x}", "value": str(v), "word": w}
        for (t, i, dm, v), w in sorted(last.items())
    ]


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the firmware
    import arm_edit_capture as cap
    import fx_clients as fc

    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("image", type=Path, nargs="?")
    ap.add_argument("sram", type=Path, nargs="?")
    ap.add_argument("--analyze", type=Path, help="report the interpolation error of a capture csv")
    ap.add_argument("--export", type=Path, help="with --analyze: write the piano rows here")
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--params", type=Path, default=Path("docs/research/arm-edit-params.csv"))
    ap.add_argument("--units", default="3,4,0")
    ap.add_argument("--types", help="comma separated Fx2 types (default all)")
    ap.add_argument("--window", type=int, default=400_000)
    ap.add_argument("--out", type=Path)
    ns = ap.parse_args(argv)
    if ns.analyze:
        import csv

        with ns.analyze.open(encoding="utf-8") as fh:
            rows = list(csv.DictReader(fh))
        if ns.export:
            with ns.export.open("w", encoding="utf-8", newline="") as out:
                w = csv.DictWriter(out, fieldnames=EXPORT_FIELDS, lineterminator="\n")
                w.writeheader()
                w.writerows(export_rows(rows))
        for row in analyze(rows):
            kind, ty, ident, dm, err, at, flt, distinct = row
            print(f"{kind},{ty},{ident},{dm},{err:.3e},{at},{int(flt)},{distinct}")
        return 0
    if not (ns.image and ns.sram and ns.out):
        ap.error("capture needs image, sram and --out")
    fc.value_grid = dense_grid  # capture_slot reads the module global
    emu, blob = fc.fx_snapshot(ns.image, ns.sram, ns.cache)
    allp = cap.load_params(ns.params, ".")
    markers = {p.name: p.edit_id for p in allp if p.name in fc.KIND_MARKERS}
    units = fc.find_units(emu, markers)
    wanted = {int(x, 0) for x in ns.units.split(",")}
    by_name = {p.name: p for p in cap.load_params(ns.params, "Edt_Effects")}
    noise = cap.baseline_keys(emu)
    slot = next(s for s in fc.SLOTS if s.name == "Fx2")
    if ns.types:
        keep = {int(t) for t in ns.types.split(",")}
        tp = by_name[slot.type_id]
        by_name[slot.type_id] = type(tp)(**{**tp.__dict__, "minimum": min(keep), "maximum": max(keep)})
    out: list[dict[str, object]] = []
    for u in (u for u in units if u.index in wanted):
        out += fc.capture_slot(emu, blob, u, slot, by_name, ns.window, noise)
        print(f"unit {u.index}: {len(out)} rows", flush=True)
    fc.write_capture(ns.out, out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
