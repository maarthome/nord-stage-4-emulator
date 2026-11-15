#!/usr/bin/env python3
"""ARM capture of the reverb colour switch: the DSP words `kRvbColor` 0, 1, 2 write per `kRvbType`.

For each type the harness applies a different type first (so the change handler always runs),
then the type, then (colours 1 and 2) the colour, on the synth unit L6 of the booted firmware, and
records every DSP word message of the last switch. `fx_rvb_colors.py` reduces the raw rows to the
committed delta table.

    uv run --with unicorn python tools/fx_rvb_color_capture.py CODE.bin SRAM.bin \
        --cache target/settled.snap --types 2,3,4,5,6,7,8,9,10,11 --out capture.csv

Evidence and limits: `docs/research/fx-reverb-colors.md`.
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

#: EMPIRICAL (run, `docs/research/fx-contexts.md` s4): the synth unit whose reverb row is core 2
#: row 0 of the FX image (`docs/research/data/fx-rvbmulti-instance.csv`).
UNIT = 6
FIELDS = "type,color,core,dm_word,word,pc".split(",")


def pre_type(rvb_type: int) -> int:
    """A type different from `rvb_type`, applied first so the type handler runs."""
    return 3 if rvb_type != 3 else 4


def row(rvb_type: int, color: int, m) -> dict[str, object]:  # noqa: ANN001
    """One capture row from a DSP message `m` (fields `core`, `dm_word`, `word2`, `pc`)."""
    return {
        "type": rvb_type,
        "color": color,
        "core": m.core,
        "dm_word": f"{m.dm_word:#x}",
        "word": f"{m.word2:#010x}",
        "pc": f"{m.pc:#x}",
    }


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the firmware
    import arm_edit_capture as cap
    import arm_steady as ast
    import fx_clients as fc

    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--params", type=Path, default=Path("docs/research/arm-edit-params.csv"))
    ap.add_argument("--types", default="2,3,4,5,6,7,8,9,10,11")
    ap.add_argument("--window", type=int, default=20_000_000)
    ap.add_argument("--out", type=Path, required=True)
    ns = ap.parse_args(argv)
    emu, blob = fc.fx_snapshot(ns.image, ns.sram, ns.cache)
    allp = cap.load_params(ns.params, ".")
    markers = {p.name: p.edit_id for p in allp if p.name in fc.KIND_MARKERS}
    unit = next(u for u in fc.find_units(emu, markers) if u.index == UNIT)
    by = {p.name: p for p in cap.load_params(ns.params, fc.DEFAULT_FILTER)}
    tp, cp = by["Edt_Effects_kRvbType"], by["Edt_Effects_kRvbColor"]
    rows = []
    for t in (int(x) for x in ns.types.split(",")):
        for color in (0, 1, 2):
            ast.restore(emu, blob)
            emu.install_dsp_ports()
            cap.apply_value(emu, unit.address, tp.edit_id, pre_type(t), ns.window)
            if color:
                cap.apply_value(emu, unit.address, tp.edit_id, t, ns.window)
                during, later, _ = cap.apply_value(emu, unit.address, cp.edit_id, color, ns.window)
            else:
                during, later, _ = cap.apply_value(emu, unit.address, tp.edit_id, t, ns.window)
            rows += [row(t, color, m) for m in during + later]
            print(t, color, len(during + later), flush=True)
    with ns.out.open("w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=FIELDS)
        w.writeheader()
        w.writerows(rows)
    return 0


if __name__ == "__main__":
    sys.exit(main())
