#!/usr/bin/env python3
# ruff: noqa: E501
"""Joint `kFx2Amt` x `kFx2Rate` capture: words that both ids write (PHAS 0 `c[-28]`, FLANG `c[-16]`).

The captures of `fx_mod2_dense.py` vary one id at a time, with the other at its type switch
value 64. This tool applies the type, then `Rate = r`, then `Amt = a` (and the reverse order) on
the piano unit and records the last word written to the given DM addresses. Result
(`docs/research/fx-mod2-validation.md` s3): PHAS 0 `c[-28]` and CHOR 5 `c[-0x1C]`, `c[-0x1B]` are
bilinear in rate and amount, FLANG `c[-16]` is `A(a) R(r) / R(64)`.

    uv run --with unicorn python tools/fx_mod2_joint.py CODE.bin SRAM.bin --cache target/settled.snap \
        --type 0 --words 0xb032c,0xb0328 --out target/fx2_joint.csv
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))

#: `(rate, amount)` pairs of the grid.
PAIRS = [(r, a) for r in (0, 20, 64, 100, 127) for a in (0, 20, 64, 100, 127)]
#: Dense mode: three rates (the shared words are linear in the rate, checked at the middle one)
#: against every dense amount of `fx_mod2_dense.DENSE`.
DENSE_RATES = (0, 64, 127)


def last_words(msgs: list, words: set[int]) -> dict[int, int]:
    """The last DSP message word per DM address in `words`."""
    out: dict[int, int] = {}
    for m in msgs:
        if m.dm_word in words:
            out[m.dm_word] = m.word2
    return out


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the firmware
    import arm_edit_capture as cap
    import arm_steady as ast
    import fx_clients as fc

    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--params", type=Path, default=Path("docs/research/arm-edit-params.csv"))
    ap.add_argument("--unit", type=int, default=4)
    ap.add_argument("--type", type=int, required=True)
    ap.add_argument("--words", required=True, help="comma separated DM addresses")
    ap.add_argument("--window", type=int, default=400_000)
    ap.add_argument("--dense", action="store_true", help="3 rates x every dense amount, one order")
    ap.add_argument("--out", type=Path, required=True)
    ns = ap.parse_args(argv)
    emu, blob = fc.fx_snapshot(ns.image, ns.sram, ns.cache)
    allp = cap.load_params(ns.params, ".")
    markers = {p.name: p.edit_id for p in allp if p.name in fc.KIND_MARKERS}
    unit = next(u for u in fc.find_units(emu, markers) if u.index == ns.unit)
    by = {p.name: p for p in cap.load_params(ns.params, "Edt_Effects")}
    t, amt, rate = by["Edt_Effects_kFx2Type"], by["Edt_Effects_kFx2Amt"], by["Edt_Effects_kFx2Rate"]
    watch = {int(x, 0) for x in ns.words.split(",")}
    rows = []
    import fx_mod2_dense as dn

    pairs = [(r, a) for r in DENSE_RATES for a in dn.DENSE] if ns.dense else PAIRS
    for order in ("rate_then_amt",) if ns.dense else ("rate_then_amt", "amt_then_rate"):
        for r, a in pairs:
            ast.restore(emu, blob)
            emu.install_dsp_ports()
            seq = [(t, ns.type)]
            seq += [(rate, r), (amt, a)] if order == "rate_then_amt" else [(amt, a), (rate, r)]
            msgs = []
            for p, v in seq:
                during, later, _ = cap.apply_value(
                    emu, unit.address, p.edit_id, fc.cell_word(p, v), ns.window
                )
                msgs += list(during) + list(later)
            for dm, w in sorted(last_words(msgs, watch).items()):
                rows.append(
                    {
                        "type": ns.type,
                        "order": order,
                        "rate": r,
                        "amount": a,
                        "dm_word": f"{dm:#x}",
                        "word": f"{w:#010x}",
                    }
                )
    with ns.out.open("w", encoding="utf-8", newline="") as fh:
        wr = csv.DictWriter(
            fh,
            fieldnames=["type", "order", "rate", "amount", "dm_word", "word"],
            lineterminator="\n",
        )
        wr.writeheader()
        wr.writerows(rows)
    return 0


if __name__ == "__main__":
    sys.exit(main())
