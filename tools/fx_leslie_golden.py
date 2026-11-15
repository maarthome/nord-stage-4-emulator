#!/usr/bin/env python3
"""Golden output words of the Leslie processor from the `sharc-fx` oracle, as Rust test data.

Four scenarios with the LCG noise of `fx_leslie_run.lcg_words` (exact in `f32`): the rotor running
at the speed words the ARM sends (`0x22B00 / 0x26000 / 0x15D00`) and at 12 times those (the phase
wraps several times), both with the frozen history ring (the state of the first oracle runs); the
slow speed words with the pass routine's ring advance (4 words per call, re-based every 8
calls, second buffer too: `RESET8`) and with a plain 4 word advance that wraps at the ring length
(`ADV`, `fx-leslie.md` s10). Each scenario keeps the first 64 calls' output words and the FNV-1a 64
hash of all `4 x calls` words. Derived numbers only.

    python tools/fx_leslie_golden.py PNO_core1_sram.bin pno_core1.script > \\
        crates/nord-core/src/fx/tests_fw_leslie_golden_data.rs
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
import fx_biquad_check as fb
import fx_leslie_check as lc
import fx_leslie_ring as ring
import fx_leslie_run as lr
from fx_leslie_words import fnv

#: Leslie drive words `0xB2F2D / 0xB2F2E / 0xB2F31 / 0xB2F32`: the ARM writes `v / 127` into all
#: four (`tools/fx_clients.py --slots`, `Edt_OrgCom_kEffectsLeslieDrv`,
#: `docs/research/organ-preamp-search.md` s10).
DRIVE_ADDRS = (0xB2F2D, 0xB2F2E, 0xB2F31, 0xB2F32)
#: `v = 31` (`B3 Chords n Solo` stores 2.44 of 10): `31 / 127` as `f32`.
DRIVE_WORD = 0x3E79F3E8
#: (name, speed multiplier, ring stride, ring period, drive word); stride 0 is the frozen ring.
SCENARIOS = (
    ("SLOW", 1, 0, 0, 0),
    ("X12", 12, 0, 0, 0),
    ("RESET8", 1, 4, 8, 0),
    ("ADV", 1, 4, 0, 0),
    ("DRIVE", 1, 0, 0, DRIVE_WORD),
)
CALLS = 2000
KEEP = 64


def oracle_outputs(
    pno1: Path,
    script: Path,
    mult: int,
    calls: int,
    binary: Path,
    stride: int = 0,
    period: int = 0,
    drive: int = 0,
) -> list[int]:
    """Output words of every call (four per call) with the LCG input."""
    lc.MULT[0] = mult
    extra = [t for a, v in lc.rotor_words().items() for t in ("--poke", f"0:{a:#x}={v:#x}")]
    extra += ring.ring_pokes(calls, stride, period, second=True)
    if drive:
        extra += [t for a in DRIVE_ADDRS for t in ("--poke", f"0:{a:#x}={drive:#x}")]
    args = lr.build_args(script, calls, extra, lcg=True)
    with tempfile.NamedTemporaryFile("w", suffix=".args", delete=False) as f:
        f.write("\n".join(args))
    out = subprocess.run(
        [str(binary), str(pno1), f"@{f.name}"], capture_output=True, text=True, check=True
    ).stdout
    dumps = fb.parse_dumps(out)
    return [d[lr.IO + j] for d in dumps for j in range(4)]


def rust(name: str, mult: int, words: list[int], drive: int = 0) -> str:
    keep = words[: 4 * KEEP]
    body = [f"    {', '.join(f'{w:#x}' for w in keep[i : i + 8])}," for i in range(0, len(keep), 8)]
    return "\n".join(
        [
            f"/// Speed words x{mult}: output words of the first {KEEP} calls.",
            f"pub(super) const {name}_FIRST: [u32; {len(keep)}] = [",
            *body,
            "];",
            f"/// FNV-1a 64 of all {len(words)} output words ({len(words) // 4} calls).",
            f"pub(super) const {name}_FNV: u64 = {fnv(words):#x};",
            f"pub(super) const {name}_MULT: u32 = {mult};",
            *([f"pub(super) const {name}_DRIVE: u32 = {drive:#x};"] if drive else []),
            "",
        ]
    )


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("pno_core1", type=Path)
    ap.add_argument("script", type=Path)
    ap.add_argument("--binary", type=Path, default=Path("target/release/sharc-fx.exe"))
    ns = ap.parse_args(argv)
    out = [
        "//! Oracle output of the Leslie processor on LCG noise (`tools/fx_leslie_golden.py`).",
        "//! Derived numbers; VERIFIED: the `sharc-fx` oracle with the boot log of OS v1.66.",
        "",
        f"pub(super) const CALLS: usize = {CALLS};",
        "",
    ]
    for name, mult, stride, period, drive in SCENARIOS:
        words = oracle_outputs(
            ns.pno_core1, ns.script, mult, CALLS, ns.binary, stride, period, drive
        )
        out.append(rust(name, mult, words, drive))
    out += [
        "#[cfg(test)]",
        "mod tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn the_golden_blocks_have_the_documented_shape() {",
        f"        assert_eq!(SLOW_FIRST.len(), {4 * KEEP});",
        f"        assert_eq!(X12_FIRST.len(), {4 * KEEP});",
        f"        assert_eq!(RESET8_FIRST.len(), {4 * KEEP});",
        f"        assert_eq!(ADV_FIRST.len(), {4 * KEEP});",
        f"        assert_eq!(DRIVE_FIRST.len(), {4 * KEEP});",
        "        assert_ne!(SLOW_FNV, DRIVE_FNV);",
        "        assert_eq!(X12_MULT, 12 * SLOW_MULT);",
        "        assert_ne!(SLOW_FNV, X12_FNV);",
        "        assert_ne!(SLOW_FNV, RESET8_FNV);",
        "        assert_ne!(RESET8_FNV, ADV_FNV);",
        "    }",
        "}",
        "",
    ]
    sys.stdout.write("\n".join(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
