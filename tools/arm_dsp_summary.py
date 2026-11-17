#!/usr/bin/env python3
"""Summarise the ARM -> DSP captures of `arm_dsp_capture.py` and check the sample-pitch `e` (D121).

Reads `target/arm_dsp_all/*.txt.gz` (git-ignored) and the factory programs, writes small tables:

* `per-program` CSV: program, class (synth layers in SAMPLES mode, scene I / II active) and the
  message count per chip and phase group;
* `targets` JSON: per receiving DSP core (`chip core path`) the messages per program (min, median,
  max), the distinct DM words, how many words are constant across programs and which vary;
* `e check` JSON: for the programs with an active SAMPLES layer, every SY message of the key phases,
  hits in the `e` window `DM(0x93500..0x935A0)` and 16-bit halves equal to the modelled entry.

    uv run python tools/arm_dsp_summary.py target/arm_dsp_all factory_programs/Program \
        --out docs/research/data

Evidence and results: `docs/research/arm-dsp-capture.md`.
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import struct
import sys
from collections import Counter, defaultdict
from collections.abc import Iterable
from pathlib import Path

import arm_dsp_capture as cap
import arm_program_routing as apr
import ns4_synth_fields as fields

# VERIFIED (`docs/research/sy-pitch-entry-writer.md` section 1): the `e` window of the SY DM.
E_WINDOW = (0x93500, 0x935A0)
# VERIFIED (`crates/nord-core/src/sinc/pitch_law.rs::UNITY_WORD`, SY oracle, 8 points): the DSP
# step is `ratio(e) * 0.05 / word`.
UNITY_WORD = 0.05
# EMPIRICAL (`tools/arm_sy_voice.py` run, `docs/research/synth-sample-kernel-rule.md` s12): the SY
# pair block word 0 of voice 0 carries the played key's pitch word as a float.
PITCH_WORD_DM = 0x921A6
SYNTH_BASE = 327  # payload byte of the synth block (`arm_zone_programs.CODE_FIELDS`)
IS_OSC_MODE_BIT, SAMPLE_ID_BIT = 8, 26  # layer-relative (`ns4_synth_fields.FIELDS`)
# EMPIRICAL (2026-11-16 capture): path 2 words whose byte address is outside the core windows
# `0x28000000 + 4 w` (`dm` below 0 or past 4 M words) are bulk stores into the DSP external-port
# windows (`0x200xxxxx`, pc `0xC01C0B70`), not parameter words.
DM_WORDS = 0x400000
KEY_PHASES = tuple(f"k{k}" for k in cap.KEYS)
GROUPS = ("load", "idle", "key", "key_off")


def unpack_ratio(e: int) -> float:
    """Ratio of a 16-bit packed entry (`sinc::step_join::unpack_ratio`, VERIFIED there)."""
    exp, man = e >> 11, (e & 0x7FF) / 2048.0
    return 0.625 * man if exp == 0 else 0.625 * 2.0 ** (exp - 1) * (1.0 + man)


def pack_ratio(ratio: float) -> int:
    """Nearest entry of a ratio (`sinc::step_join::pack_ratio`, ASSUMED rounding to nearest)."""
    import math

    if ratio < 0.625:
        code = round(ratio / 0.625 * 2048.0)
    else:
        exp = math.floor(math.log2(ratio / 0.625)) + 1
        man = round((ratio / (0.625 * 2.0 ** (exp - 1)) - 1.0) * 2048.0)
        code = (exp << 11) + man
    return max(0, min(code, 0xF7FF))


def float_of_bits(bits: int) -> float:
    return struct.unpack("<f", struct.pack("<I", bits & 0xFFFFFFFF))[0]


def modelled_entry(pitch_bits: int) -> int:
    """Modelled `e` for the pitch word the ARM sent: `pack_ratio(word / 0.05)` (ASSUMED, D65)."""
    return pack_ratio(float_of_bits(pitch_bits) / UNITY_WORD)


def program_class(payload: bytes) -> dict[str, object]:
    """Synth layers in SAMPLES mode and whether one plays in scene I / II (mask 0 = layer A)."""
    rd = apr.read_bits
    samples = [
        rd(payload, SYNTH_BASE * 8 + b + IS_OSC_MODE_BIT, 1) == 0 for b in fields.LAYER_BASES
    ]
    ids = [
        rd(payload, SYNTH_BASE * 8 + b + SAMPLE_ID_BIT, 32) if s else 0
        for b, s in zip(fields.LAYER_BASES, samples, strict=True)
    ]
    views = apr.scene_views(payload)
    active = []
    for scene in (0, 1):
        enable, mask = views[scene]["synth"]
        mask = mask or 1
        active.append(bool(enable) and any(s and (mask >> i) & 1 for i, s in enumerate(samples)))
    return {
        "sample_layers": samples,
        "sample_ids": ids,
        "sample_scene_1": active[0],
        "sample_scene_2": active[1],
    }


def phase_group(phase: str) -> str:
    """`load`, `idle`, `key` (`k36` ..), `key_off` (`k36_off` ..) or `other`."""
    if phase in ("load", "idle"):
        return phase
    if phase.startswith("k"):
        return "key_off" if phase.endswith("_off") else "key"
    return "other"


def is_bulk(row: dict[str, object]) -> bool:
    """True for a path 2 store outside the DSP core window (bulk data, not a parameter word)."""
    return row["path"] == 2 and not 0 <= int(row["dm"]) < DM_WORDS


def target_name(row: dict[str, object]) -> str:
    """Receiving DSP: `ORG c2` (path 2 core) or `SMP p1` (path 1 DM write)."""
    if row["path"] == 2:
        return f"{row['chip']} c{row['core']}"
    return f"{row['chip']} p1"


def counts_by_group(rows: Iterable[dict[str, object]]) -> dict[str, dict[str, int]]:
    """`{target: {group: n}}` message counts of one program."""
    out: dict[str, Counter] = defaultdict(Counter)
    for r in rows:
        out[target_name(r)][phase_group(str(r["phase"]))] += 1
    return {k: dict(v) for k, v in out.items()}


def chip_total(counts: dict[str, dict[str, int]], chip: str) -> int:
    """Messages of one program to every target of `chip` (`counts_by_group` output)."""
    return sum(
        n for t, g in counts.items() if t.split()[0] == chip for k, n in g.items() if k in GROUPS
    )


def final_words(
    rows: Iterable[dict[str, object]], group: str = "load"
) -> dict[str, dict[int, int]]:
    """`{target: {dm: last value}}` of the messages of one phase group."""
    out: dict[str, dict[int, int]] = defaultdict(dict)
    for r in rows:
        if phase_group(str(r["phase"])) == group:
            out[target_name(r)][int(r["dm"])] = int(r["w2"])
    return out


def ranges(words: Iterable[int]) -> list[tuple[int, int]]:
    """Sorted `(first, last)` runs of consecutive DM words."""
    out: list[list[int]] = []
    for w in sorted(set(words)):
        if out and w == out[-1][1] + 1:
            out[-1][1] = w
        else:
            out.append([w, w])
    return [(a, b) for a, b in out]


def page_histogram(words: Iterable[int], page: int = 0x10000) -> dict[str, int]:
    """`{page base: distinct words}` of DM words (pages of `page` words, hex keys, sorted)."""
    counts = Counter(w // page * page for w in set(words))
    return {hex(k): counts[k] for k in sorted(counts)}


def slim(stats: dict[str, object]) -> dict[str, object]:
    """`compare_words` output with the range list replaced by its length (small committed file)."""
    out = {k: v for k, v in stats.items() if k != "varying_ranges"}
    out["varying_range_count"] = len(stats["varying_ranges"])
    return out


def compare_words(per_program: list[dict[int, int]]) -> dict[str, object]:
    """Across programs: words written by every program, constant ones, varying ones."""
    seen: Counter = Counter()
    values: dict[int, set[int]] = defaultdict(set)
    for words in per_program:
        for dm, v in words.items():
            seen[dm] += 1
            values[dm].add(v)
    n = len(per_program)
    everywhere = [dm for dm, c in seen.items() if c == n]
    constant = [dm for dm in everywhere if len(values[dm]) == 1]
    varying = [dm for dm in seen if len(values[dm]) > 1]
    return {
        "distinct_words": len(seen),
        "in_every_program": len(everywhere),
        "constant": len(constant),
        "varying": len(varying),
        "varying_ranges": ranges(varying),
        "program_specific": len([dm for dm, c in seen.items() if c < n]),
        "pages": page_histogram(seen),
    }


def target_stats(per_program_counts: list[dict[str, dict[str, int]]], group: str) -> dict:
    """`{target: {min, median, max}}` messages per program in a phase group."""
    targets = {t for c in per_program_counts for t in c}
    out = {}
    for t in sorted(targets):
        n = [c.get(t, {}).get(group, 0) for c in per_program_counts]
        out[t] = {"min": min(n), "median": statistics.median(n), "max": max(n)}
    return out


def e_window_hits(rows: Iterable[dict[str, object]]) -> list[dict[str, object]]:
    """Every message (any DSP) addressed into the `e` window."""
    lo, hi = E_WINDOW
    return [r for r in rows if lo <= int(r["dm"]) < hi]


def e_shaped_hits(rows: list[dict[str, object]], slack: int = 2) -> list[dict[str, object]]:
    """Messages of key phase `kN` with a 16-bit half within `slack` of the modelled `e` of key N.

    The modelled entry comes from the pitch word the ARM sent in that phase (`PITCH_WORD_DM`).
    """
    hits = []
    for phase in KEY_PHASES:
        phase_rows = [r for r in rows if r["phase"] == phase]
        pitch = [int(r["w2"]) for r in phase_rows if r["chip"] == "SY" and r["dm"] == PITCH_WORD_DM]
        if not pitch:
            continue
        want = modelled_entry(pitch[-1])
        for r in phase_rows:
            halves = (int(r["w2"]) & 0xFFFF, (int(r["w2"]) >> 16) & 0xFFFF)
            if any(abs(h - want) <= slack and h != 0 for h in halves):
                hits.append({**r, "model_e": want})
    return hits


def sy_words(rows: Iterable[dict[str, object]]) -> dict[str, dict[int, list[int]]]:
    """`{phase: {dm: [values in send order]}}` of the SY messages of the key-on phases."""
    out: dict[str, dict[int, list[int]]] = {p: defaultdict(list) for p in KEY_PHASES}
    for r in rows:
        if r["chip"] == "SY" and r["phase"] in out:
            out[str(r["phase"])][int(r["dm"])].append(int(r["w2"]))
    return {p: dict(d) for p, d in out.items()}


def key_word_table(per_program: list[dict[str, dict[int, list[int]]]]) -> list[dict[str, object]]:
    """Per SY word of the key-on phases: in how many programs, and what it varies with.

    `varies_with_key`: within one program the last value differs between `k36`, `k60`, `k84`;
    `varies_with_program`: the same key gives different values in different programs.
    """
    words = {dm for p in per_program for ph in p.values() for dm in ph}
    out = []
    for dm in sorted(words):
        by_key: dict[str, set[int]] = {ph: set() for ph in KEY_PHASES}
        key_differs = 0
        programs = 0
        for p in per_program:
            last = {ph: p[ph][dm][-1] for ph in KEY_PHASES if dm in p.get(ph, {})}
            if not last:
                continue
            programs += 1
            key_differs += len(set(last.values())) > 1
            for ph, v in last.items():
                by_key[ph].add(v)
        out.append(
            {
                "dm": hex(dm),
                "programs": programs,
                "varies_with_key": key_differs > 0,
                "varies_with_program": any(len(v) > 1 for v in by_key.values()),
                "values": {ph: [hex(v) for v in sorted(vals)[:3]] for ph, vals in by_key.items()},
            }
        )
    return out


def captured(out_dir: Path) -> dict[str, Path]:
    """`{stem: capture file}` of a capture directory."""
    return {p.name.removesuffix(".txt.gz"): p for p in sorted(out_dir.glob("*.txt.gz"))}


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the local captures
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("captures", type=Path)
    ap.add_argument("programs", type=Path)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--ranges", action="store_true", help="keep the varying DM ranges (large)")
    ns = ap.parse_args(argv)
    files = captured(ns.captures)
    programs = {cap.out_name(p): p for p in cap.program_files(ns.programs)}
    table, counts, loads, bulk = [], [], [], []
    e_report: list[dict[str, object]] = []
    sy_per_program: list[dict[str, dict[int, list[int]]]] = []
    for stem, path in files.items():
        prog = programs[stem]
        cls = program_class(prog.read_bytes()[apr.HEADER : apr.HEADER + 820])
        every = list(cap.read_rows(path))
        bulk.append(sum(1 for r in every if is_bulk(r)))
        rows = [r for r in every if not is_bulk(r)]
        c = counts_by_group(rows)
        counts.append(c)
        loads.append(final_words(rows))
        table.append({"program": stem, **{k: v for k, v in cls.items() if k != "sample_ids"}})
        if cls["sample_scene_1"] or cls["sample_scene_2"]:
            sy_per_program.append(sy_words(rows))
            e_report.append(
                {
                    "program": stem,
                    "sample_ids": [hex(i) for i in cls["sample_ids"]],
                    "sy_messages": chip_total(c, "SY"),
                    "window_hits": len(e_window_hits(rows)),
                    "e_shaped_hits": len(e_shaped_hits(rows)),
                    "sy_key_words": sorted(
                        {hex(dm) for ph in sy_words(rows).values() for dm in ph}
                    ),
                }
            )
    ns.out.mkdir(parents=True, exist_ok=True)
    targets = sorted({t for c in counts for t in c})
    summary = {
        "programs": len(files),
        "load": target_stats(counts, "load"),
        "key": target_stats(counts, "key"),
        "words": {
            t: (lambda st: st if ns.ranges else slim(st))(
                compare_words([w.get(t, {}) for w in loads])
            )
            for t in targets
        },
    }
    (ns.out / "arm-dsp-capture-targets.json").write_text(json.dumps(summary, indent=1), "utf-8")
    check = {"programs": e_report, "sy_key_words": key_word_table(sy_per_program)}
    (ns.out / "arm-dsp-capture-e-check.json").write_text(json.dumps(check, indent=1), "utf-8")
    with (ns.out / "arm-dsp-capture-programs.csv").open("w", newline="", encoding="utf-8") as fh:
        wr = csv.writer(fh)
        wr.writerow(
            ["program", "sample_layers", "sample_scene_1", "sample_scene_2", *targets, "bulk"]
        )
        for t_row, c, n_bulk in zip(table, counts, bulk, strict=True):
            wr.writerow(
                [t_row["program"], "".join("S" if s else "." for s in t_row["sample_layers"]),
                 int(t_row["sample_scene_1"]), int(t_row["sample_scene_2"]),
                 *[sum(c.get(t, {}).values()) for t in targets], n_bulk]
            )  # fmt: skip
    print(json.dumps({k: v for k, v in summary.items() if k != "words"}, indent=1))
    print(f"e check: {len(e_report)} sample programs, "
          f"{sum(r['window_hits'] for r in e_report)} window hits, "
          f"{sum(r['e_shaped_hits'] for r in e_report)} e-shaped hits")  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main())
