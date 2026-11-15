#!/usr/bin/env python3
# ruff: noqa: E501
"""Dense parameter laws of the law-stage Mod 2 types as Rust (`fx/fw_mod2_dense.rs`).

`fx_mod2_laws.py` keeps the 11 captured samples of every `kFx2Amt` / `kFx2Rate` word and the
stage interpolated linearly between them. The dense capture (`tools/fx_mod2_dense.py`,
`docs/research/data/fx-mod2-dense.csv`, piano unit) adds every 8th value and 60..80, so the
interpolation error between knots drops from up to 3.9 % of the range to below 0.7 %
(`docs/research/fx-mod2-validation.md`). This tool maps every dense series to its context offset
`c[o]` (the 11 sample words of `fx-param-fits.csv` identify the series; `dm_word - o` is constant per
type) and writes the knot table.

    python tools/fx_mod2_dense_gen.py [--check]
"""

from __future__ import annotations

import argparse
import csv
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fx_mod2_laws as ml

DENSE_CSV = ml.DATA / "fx-mod2-dense.csv"
RUST = ml.ROOT / "crates" / "nord-core" / "src" / "fx" / "fw_mod2_dense.rs"
#: Types of the law stage (PHAS 0, 1, FLANG 2, 3, ENS 8, 9, SPIN 10, 11).
STAGE_TYPES = (0, 1, 2, 3, 8, 9, 10, 11)


def read_dense() -> dict[tuple[int, str], dict[int, dict[int, int]]]:
    """`(type, "Amt" | "Rate") -> {dm_word: {value: word}}`."""
    out: dict[tuple[int, str], dict[int, dict[int, int]]] = {}
    with DENSE_CSV.open(encoding="utf-8") as f:
        for r in csv.DictReader(f):
            key = (int(r["type"]), r["id"])
            out.setdefault(key, {}).setdefault(int(r["dm_word"], 16), {})[int(r["value"])] = int(
                r["word"], 16
            )
    return out


def knots(dense: dict[tuple[int, str], dict[int, dict[int, int]]]) -> list[int]:
    """The grid of the dense capture: the values of the longest series (single-message series are
    type-switch words, not laws)."""
    return sorted(max((set(s) for ser in dense.values() for s in ser.values()), key=len))


#: Samples of a fitted law that must equal the dense series (the word at v = 1 can depend on the
#: previous value, e.g. SPIN type 11 `c[-21]`: 2 of 11 differ).
MIN_AGREE = 9


def best_match(
    full: dict[int, dict[int, int]], words: list[int], skip: frozenset[int] = frozenset()
) -> tuple[int, dict[int, int]] | None:
    """The dense series that agrees with at least `MIN_AGREE` of the 11 sample `words`; the
    series in `skip` (already taken by another offset) lose ties (ENS `c[2]` and `c[3]` carry
    the same words)."""
    best, score = None, 0
    for dm, ser in full.items():
        if dm in skip:
            continue
        n = sum(ser[v] == w for v, w in zip(ml.SAMPLES, words, strict=True))
        if n > score:
            best, score = (dm, ser), n
    return best if score >= MIN_AGREE else None


def dense_laws() -> dict[int, dict[str, dict[int, tuple[bool, list[int]]]]]:
    """type -> kind -> {ctx offset: (float, words at `knots`)}; the context offset of a dense
    series comes from the sample words of the 11-sample law that equals it."""
    dense = read_dense()
    ks = knots(dense)
    fits = ml.laws()
    out: dict[int, dict[str, dict[int, tuple[bool, list[int]]]]] = {}
    for t in STAGE_TYPES:
        out[t] = {}
        bases: set[int] = set()
        for kind in ("Amt", "Rate"):
            # A variation type starts from its base type's words (same module), so a law the
            # variation capture lacks is the base type's.
            series = {
                **dense.get((t - 1 if t in (3, 9, 11) else t, kind), {}),
                **dense.get((t, kind), {}),
            }
            full = {dm: s for dm, s in sorted(series.items()) if set(ks) <= set(s)}
            laws: dict[int, tuple[bool, list[int]]] = {}
            taken: set[int] = set()
            for off, (flt, words) in fits[t][kind].items():
                hit = best_match(full, words, frozenset(taken)) or best_match(full, words)
                if hit:
                    taken.add(hit[0])
                if hit is None and len(set(words)) == 1:
                    laws[off] = (flt, [words[0]] * len(ks))  # constant: written once
                    continue
                if hit is None:
                    raise SystemExit(
                        f"type {t} {kind} c[{off}]: no dense series equals its samples"
                    )
                bases.add(hit[0] - off)
                laws[off] = (flt, [hit[1][v] for v in ks])
            out[t][kind] = laws
        if len(bases) > 1:
            raise SystemExit(
                f"type {t}: dense series map to several row bases {sorted(map(hex, bases))}"
            )
    return out


JOINT_CSV = ml.DATA / "fx-mod2-joint.csv"
#: Words both ids write that are bilinear in rate (VERIFIED, `docs/research/fx-mod2-validation.md`
#: s3): `(type, dm_word, context offset)`. FLANG `c[-16]` is a product and CHOR 5 a closed form.
JOINT_WORDS = {0: [(0xB032C, -28)]}


def read_joint() -> dict[tuple[int, int], dict[int, dict[int, int]]]:
    """`(type, dm_word) -> {amount: {rate: word}}`."""
    out: dict[tuple[int, int], dict[int, dict[int, int]]] = {}
    with JOINT_CSV.open(encoding="utf-8") as f:
        for r in csv.DictReader(f):
            key = (int(r["type"]), int(r["dm_word"], 16))
            out.setdefault(key, {}).setdefault(int(r["amount"]), {})[int(r["rate"])] = int(
                r["word"], 16
            )
    return out


def rust() -> str:
    dense = read_dense()
    ks = knots(dense)
    laws = dense_laws()
    out = [
        "//! Dense parameter laws of the law-stage Mod 2 types (derived values).",
        "//!",
        "//! GENERATED by `python tools/fx_mod2_dense_gen.py` from `docs/research/data/fx-mod2-dense.csv`",
        "//! (ARM harness capture of the piano unit, `tools/fx_mod2_dense.py`) and `fx-param-fits.csv`;",
        "//! do not edit. VERIFIED at every knot of `KNOTS` (the words the ARM wrote), linear between",
        "//! them (EMPIRICAL: measured error below 0.7 % of the range, `docs/research/fx-mod2-validation.md`).",
        "",
        "/// Values of `kFx2Amt` / `kFx2Rate` at which the ARM writes were captured.",
        "#[rustfmt::skip]",
        f"pub(super) const KNOTS: [u32; {len(ks)}] = {ks};",
        "",
        "/// One context word as a function of a 7 bit parameter: one word per knot.",
        "pub(super) struct DenseLaw {",
        "    pub(super) off: i32,",
        "    pub(super) float: bool,",
        f"    pub(super) words: [u32; {len(ks)}],",
        "}",
        "",
    ]
    for t in STAGE_TYPES:
        for kind in ("Amt", "Rate"):
            out.append(f"/// `kFx2{kind}` dense laws of `kFx2Type` {t}.")
            out.append("#[rustfmt::skip]")
            rows = []
            for off, (flt, words) in sorted(laws[t][kind].items()):
                body = ", ".join(f"{w:#x}" for w in words)
                rows.append(
                    f"    DenseLaw {{ off: {off}, float: {str(flt).lower()}, words: [{body}] }},"
                )
            out.append(
                f"pub(super) const {kind.upper()}_{t}: &[DenseLaw] = &[\n"
                + "\n".join(rows)
                + "\n];"
            )
    joint = read_joint()
    out += [
        "",
        "/// A word that both `kFx2Amt` and `kFx2Rate` write, bilinear in the rate: the word at rate 0",
        "/// and rate 127 for every knot amount (`f(r, a) = f0(a) + (f127(a) - f0(a)) r / 127`, exact to",
        "/// 1.3e-7 of the range at rate 64 on all knots).",
        "pub(super) struct JointLaw {",
        "    pub(super) off: i32,",
        f"    pub(super) rate0: [u32; {len(ks)}],",
        f"    pub(super) rate127: [u32; {len(ks)}],",
        "}",
    ]
    for t, words in JOINT_WORDS.items():
        rows = []
        for dm, off in words:
            grid = joint[(t, dm)]
            r0 = ", ".join(f"{grid[a][0]:#x}" for a in ks)
            r1 = ", ".join(f"{grid[a][127]:#x}" for a in ks)
            rows.append(f"    JointLaw {{ off: {off}, rate0: [{r0}], rate127: [{r1}] }},")
        out.append(f"/// Joint laws of `kFx2Type` {t}.")
        out.append("#[rustfmt::skip]")
        out.append(
            f"pub(super) const JOINT_{t}: &[JointLaw] = &["
            + chr(10)
            + chr(10).join(rows)
            + chr(10)
            + "];"
        )
    out += [
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn the_knots_ascend_and_keep_the_original_samples() {",
        "        assert_eq!((KNOTS[0], KNOTS[KNOTS.len() - 1]), (0, 127));",
        "        assert!(KNOTS.windows(2).all(|w| w[0] < w[1]));",
        "        for v in [0, 1, 2, 15, 31, 47, 63, 79, 95, 111, 127] {",
        "            assert!(KNOTS.contains(&v));",
        "        }",
        '        assert!((60..=80).all(|v| KNOTS.contains(&v)), "CHOR knee region");',
        "    }",
        "}",
    ]
    return "\n".join(out) + "\n"


#: `I1` of the piano unit row per type (`dm_word - c[o]`): CHOR 4 `0xC13A4` (`c[-0x1E]` is
#: `0xC1386`), CHOR 5 `0xC14B8` (`c[-0x1C]` is `0xC149C`), VIBE 6, 7 `0xC17A0` (image row
#: `fw_vibe_stage`, `c[-23]` is `0xC1789`).
CLOSED_BASES = {4: 0xC13A4, 5: 0xC14B8, 6: 0xC17A0, 7: 0xC17A0}
#: Offsets whose closed forms (`fw_mod2.rs`, `fw_vibe_stage.rs`) are checked against the capture.
CLOSED_OFFSETS = {
    4: {"Rate": [-0x20], "Amt": [-0x1E, -0x1D, -3, -2, 1]},
    5: {"Rate": [-0x1E, -0x1D], "Amt": [-0x1C, -0x1B]},
    6: {"Rate": [-23], "Amt": [-2]},
}
TESTS_RUST = ml.ROOT / "crates" / "nord-core" / "src" / "fx" / "tests_mod2_dense_golden.rs"


def golden_rust() -> str:
    """The ARM words of the closed-form types (CHOR 4, 5, VIBE 6) at every dense value."""
    dense = read_dense()
    out = [
        "//! ARM words of the Mod 2 closed-form laws at the dense values (test data).",
        "//!",
        "//! GENERATED by `python tools/fx_mod2_dense_gen.py tests` from",
        "//! `docs/research/data/fx-mod2-dense.csv` (piano unit, ARM harness); do not edit.",
        '//! Each entry is `(type, "Amt" | "Rate", context offset, [(value, word)])`.',
        "",
        "pub(super) type Series = &'static [(u32, u32)];",
        "pub(super) type JointSeries = &'static [(u32, u32, u32)];",
        "",
        "#[rustfmt::skip]",
        "pub(super) const ARM_WORDS: &[(u8, &str, i32, Series)] = &[",
    ]
    for t, kinds in CLOSED_OFFSETS.items():
        for kind, offs in kinds.items():
            for off in offs:
                ser = dense[(t, kind)][CLOSED_BASES[t] + off]
                pts = ", ".join(f"({v}, {w:#x})" for v, w in sorted(ser.items()))
                out.append(f'    ({t}, "{kind}", {off}, &[{pts}]),')
    out.append("];")
    joint = read_joint()
    out += [
        "",
        "/// Words both ids write, at rate 0, 64, 127 for every dense amount: `(type, c[o],",
        "/// [(rate, amount, word)])`.",
        "#[rustfmt::skip]",
        "pub(super) const ARM_JOINT: &[(u8, i32, JointSeries)] = &[",
    ]
    for t, dm, off in (
        (0, 0xB032C, -28),
        (2, 0xB023C, -16),
        (5, 0xC149C, -0x1C),
        (5, 0xC149D, -0x1B),
    ):
        pts = ", ".join(
            f"({r}, {a}, {joint[(t, dm)][a][r]:#x})"
            for a in sorted(joint[(t, dm)])
            for r in sorted(joint[(t, dm)][a])
        )
        out.append(f"    ({t}, {off}, &[{pts}]),")
    out.append("];")
    out += [
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn every_series_covers_the_dense_grid_in_order() {",
        "        assert_eq!(ARM_WORDS.len(), 12);",
        "        for (_, _, _, pts) in ARM_WORDS {",
        "            assert_eq!(pts.len(), 44);",
        "            assert!(pts.windows(2).all(|w| w[0].0 < w[1].0));",
        "        }",
        "    }",
        "}",
    ]
    return "\n".join(out) + "\n"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("cmd", nargs="?", choices=["laws", "tests"], default="laws")
    ap.add_argument("--check", action="store_true")
    ns = ap.parse_args(argv)
    path, text = (RUST, rust()) if ns.cmd == "laws" else (TESTS_RUST, golden_rust())
    if ns.check:
        stale = not path.exists() or path.read_text(encoding="utf-8") != text
        print("stale" if stale else "up to date")
        return 1 if stale else 0
    path.write_text(text, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
