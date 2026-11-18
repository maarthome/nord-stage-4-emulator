#!/usr/bin/env python3
"""Combined FX ablation of the program ranking: which FX group combination explains each gap.

Input: `program_rank --ablate-fx` lines (numbers only), one per program. Each line holds the
metrics of the default render, of the firmware-derived render and of the firmware render with every
non-empty combination of the six FX groups put back to the default (63 renders, keys such as
`fx_mod1+fx_mod2`). Score = `program_rank.distance(default, render)`, so a reverted set explains
the gap when it leaves a small score.

    python tools/program_rank_combo.py --ablate-fx fx.jsonl --csv docs/research/data/program-ranking-fx-combos.csv --md

Per program: the full score, the best single group, the best pair, the greedy chain (add the group
that lowers the score most, until all six), the exact best set (all 63 are known, so the greedy
chain is checked against it), the floor (all six FX groups reverted: what the organ tone wheels and
the synth filter engine leave), and the smallest set whose score is at most `EXPLAINED_SCORE`.
"""

from __future__ import annotations

import argparse
import csv
import itertools
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import program_rank as pr  # noqa: E402

FX_GROUPS = ("fx_delay", "fx_compressor", "fx_reverb", "fx_eq", "fx_mod1", "fx_mod2")
EXPLAINED_SCORE = 2.0  # ASSUMED: a set explains the gap when it leaves at most this score (the ranking median is 3.9, 60 of 384 programs are below 1)
NL = chr(10)


def key_of(groups) -> str:
    """Key of a combination as `program_rank` writes it: names in group order joined by `+`."""
    return "+".join(g for g in FX_GROUPS if g in set(groups))


def scores(row: dict) -> dict[str, float | None]:
    """Score left per combination key; the empty set is the full firmware score."""
    out = {"": pr.distance(row["default"], row["firmware"])["score"]}
    for k, m in row["ablate_fx"].items():
        out[k] = pr.distance(row["default"], m)["score"]
    return out


def best(sc: dict, size: int | None = None) -> tuple[str, float | None]:
    """Combination with the smallest score (of `size` groups when given, never the empty set)."""
    cand = {
        k: v for k, v in sc.items() if k and v is not None and (size is None or k.count("+") == size - 1)
    }  # fmt: skip
    if not cand:
        return "", None
    k = min(cand, key=lambda x: (cand[x], x.count("+"), x))
    return k, cand[k]


def greedy(sc: dict) -> list[tuple[str, float | None]]:
    """Add the group that lowers the score most, one at a time: (combination, score left) steps."""
    chosen: list[str] = []
    steps = []
    while len(chosen) < len(FX_GROUPS):
        rest = [g for g in FX_GROUPS if g not in chosen]
        opts = {g: sc.get(key_of(chosen + [g])) for g in rest}
        opts = {g: v for g, v in opts.items() if v is not None}
        if not opts:
            break
        g = min(opts, key=lambda x: (opts[x], x))
        chosen.append(g)
        steps.append((key_of(chosen), opts[g]))
    return steps


def smallest_explaining(sc: dict, limit: float = EXPLAINED_SCORE) -> tuple[str, float | None]:
    """Smallest combination whose score is at most `limit` (lowest score among equals)."""
    ok = {k: v for k, v in sc.items() if k and v is not None and v <= limit}
    if not ok:
        return "none", None
    k = min(ok, key=lambda x: (x.count("+"), ok[x], x))
    return k, ok[k]


def summarize(row: dict) -> dict:
    """One program: full score, single / pair / best / greedy / floor / smallest explaining set."""
    sc = scores(row)
    full = sc[""]
    s1, s2, sb = best(sc, 1), best(sc, 2), best(sc)
    steps = greedy(sc)
    floor = sc.get(key_of(FX_GROUPS))
    expl = smallest_explaining(sc)
    return {
        "entry": row["entry"], "score": full, "single": s1, "pair": s2, "best": sb,
        "greedy": steps, "floor": floor, "explain": expl,
        "pairs": {k: v for k, v in sc.items() if k.count("+") == 1},
    }  # fmt: skip


def f(x, nd=2):
    return "" if x is None else round(x, nd)


def write_csv(rows: list[dict], path: Path) -> None:
    """Committed table: score left per reverted FX combination of every program (numbers only)."""
    keys = sorted({k for r in rows for k in r["all"]}, key=lambda k: (k.count("+"), k))
    with path.open("w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh, lineterminator=NL)
        w.writerow(["entry", "score", *[f"left_without_{k}" for k in keys]])
        for r in rows:
            w.writerow([r["entry"], f(r["score"], 3), *[f(r["all"].get(k), 3) for k in keys]])


def md(rows: list[dict]) -> str:
    """Markdown table for the research note."""
    head = ["#", "Program", "Score", "Best single", "Best pair", "Best set", "Floor (all FX)", "Smallest set <= 2"]
    lines = ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)]
    for i, r in enumerate(rows):
        name = r["entry"].split("/", 1)[1][:-5]

        def cell(t):
            return "-" if t[1] is None else f"{t[0]} ({t[1]:.2f})"

        lines.append(
            f"| {i + 1} | {name} | {f(r['score'])} | {cell(r['single'])} | {cell(r['pair'])} | "
            f"{cell(r['best'])} | {f(r['floor'])} | {cell(r['explain'])} |"
        )
    return NL.join(lines)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("--ablate-fx", type=Path, required=True)
    ap.add_argument("--csv", type=Path)
    ap.add_argument("--md", action="store_true")
    ns = ap.parse_args(argv)
    rows = []
    for line in pr.load_jsonl(ns.ablate_fx):
        s = summarize(line)
        s["all"] = scores(line)
        rows.append(s)
    rows.sort(key=lambda r: (-(r["score"] or 0), r["entry"]))
    if ns.csv:
        write_csv(rows, ns.csv)
    if ns.md:
        print(md(rows))
    print(f"{len(rows)} programs", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
