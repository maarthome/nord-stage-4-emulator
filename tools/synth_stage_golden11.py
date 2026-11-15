#!/usr/bin/env python3
"""Golden vectors of the verified-words unison stages for the Rust port (`fw_stages/words.rs`).

    python tools/synth_stage_golden11.py > crates/nord-core/src/synth/tests_fw_stage_golden11.rs

The bit-exact Python stage models (`synth_stage_unison.py`, `synth_stage_super1.py`; 320 of 320
oracle words each, `tools/synth_wave_oracle.py`) are fed the words the ARM writes per wave type
(`docs/research/data/synth-p-words.csv`, `docs/research/synth-p-writer.md`): the sixteen unison
ratios, the lane mix groups, `P[16]` / `P[17]` and, for MULTI, the multiplier record. Per case the
lane x output of four consecutive samples is recorded; the Rust stage must reproduce it from the
same words with its own float phases. Only derived values appear.

Cases: `(category, wave)` pairs of the CSV, two pitches, two spread words.
"""

from __future__ import annotations

import csv
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
import synth_stage_golden as g
import synth_stage_super1 as s1
import synth_stage_unison as un
import synth_wave_model as m

ROOT = Path(__file__).resolve().parents[1]
WORDS = ROOT / "docs" / "research" / "data" / "synth-p-words.csv"
TYPES = ROOT / "docs" / "research" / "data" / "synth-wave-types.csv"

# (name, category, wave, stage fn): SUPER 0, SUPER 1 and three MULTI types (octave record 1/1/0.1,
# 2/4/1 with all three voices weighted, 1/1.5/0.1)
CASES = (
    ("super", 6, 0, un.stage_super),
    ("super1", 6, 1, s1.stage_super1),
    ("multi", 5, 0, un.stage_multi),
    ("multi", 5, 4, un.stage_multi),
    ("multi", 5, 2, un.stage_multi),
)
PITCHES = (0.003, 0.015)  # cycles per sample
SPREADS = (0.2, 0.7)


def load_words() -> dict[int, dict[int, int]]:
    """`type -> {p_index: bits}` of the `a = b = 0` rows of slot 0."""
    out: dict[int, dict[int, int]] = {}
    with WORDS.open(encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if r["a"] == "0" and r["b"] == "0":
                out.setdefault(int(r["type"]), {})[int(r["p_index"])] = int(r["word"], 16)
    return out


def type_of(category: int, wave: int) -> int:
    with TYPES.open(encoding="utf-8") as f:
        for r in csv.DictReader(f):
            if (
                r["mode"] == "1"
                and r["bank"] == "0"
                and (int(r["cat"]), int(r["wave"]))
                == (
                    category,
                    wave,
                )
            ):
                return int(r["type"])
    raise KeyError((category, wave))


def phase_word(case: int, lane: int, voice: int) -> int:
    """Start phase of a voice (24 significant bits; the Rust test rebuilds it the same way)."""
    return g.phase_word(case * 16 + lane * 8 + voice)


def lanes_for(words: dict[int, int], pitch: float, spread: float, case: int) -> list[m.Lane]:
    zero = m.f2u(0.0)
    p = lambda i: words.get(i, zero)  # noqa: E731
    groups = tuple(tuple(p(-16 + 4 * j + k) for k in range(4)) for j in range(8))
    octaves = (p(-260), p(-259), p(-258))
    lanes = []
    for lane in (0, 1):
        ln = m.Lane(
            pitch=m.f2u(2.0 * g.f32(pitch)),
            phase1=0,
            phase2=0,
            shape_a=zero,
            shape_b=zero,
            ratio1=p(-32 + lane),
            ratio2=p(-30 + lane),
            weights=(zero,) * 4,
            out_gain=p(16 + lane),
            spread=m.f2u(spread),
        )
        ln.ratios = tuple(p(-32 + 2 * j + lane) for j in range(8))
        ln.phases = tuple(phase_word(case, lane, j) for j in range(8))
        ln.groups = groups
        ln.octaves = octaves
        lanes.append(ln)
    return lanes


def rows() -> list[tuple[int, int, int, list[int]]]:
    """`(case id, pitch bits, spread bits, [4 output words of lane x])`."""
    words = load_words()
    out = []
    for n, (_, cat, wave, fn) in enumerate(CASES):
        w = words[type_of(cat, wave)]
        for pitch in PITCHES:
            for spread in SPREADS:
                lanes = lanes_for(w, pitch, spread, n)
                res, _, _ = fn(lanes)
                out.append((n, m.f2u(g.f32(pitch)), m.f2u(g.f32(spread)), list(res[0])))
    return out


def rust() -> str:
    data = rows()
    parts = [
        "//! Firmware parity of the verified-words unison stages (`fw_stages/words.rs`).\n"
        "//!\n"
        "//! Derived values only, made by `python tools/synth_stage_golden11.py > <file>`\n"
        "//! from the bit-exact models fed the ARM words (`docs/research/synth-p-writer.md`).\n",
        g.words_rust("CASE", [r[0] for r in data]),
        g.words_rust("DT", [r[1] for r in data]),
        g.words_rust("S5", [r[2] for r in data]),
        g.words_rust("OUT", [w & 0xFFFFFFFF for r in data for w in r[3]]),
    ]
    tail = (
        "\n#[test]\nfn golden_tables_have_the_documented_shape() {\n"
        f"    assert_eq!(CASE.len(), {len(data)});\n"
        "    assert_eq!(OUT.len(), 4 * CASE.len());\n"
        "    assert!(OUT.iter().all(|&w| f32::from_bits(w).is_finite()));\n}\n"
    )
    return "\n".join(parts) + tail


if __name__ == "__main__":
    sys.stdout.write(rust())
