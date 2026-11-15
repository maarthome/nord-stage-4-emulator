#!/usr/bin/env python3
# ruff: noqa: E501
"""Generates the Rust tables of the B3 Bass firmware machine from the ADI listings.

`docs/research/organ-farfisa-bass.md` s15. The listings of the generator `0x121BBE` and the
processor `0x122296` are decoded (`tools/sharc_listing_rust.py`: the same regular expression rules as
the Python interpreter that matches the oracle bit for bit) into integer rows:

    python tools/organ_bass_fw_gen.py rust  target/PNO/core1_sram.bin           # rewrite the tables
    python tools/organ_bass_fw_gen.py rust  target/PNO/core1_sram.bin --check   # exit 1 when stale
    python tools/organ_bass_fw_gen.py image target/PNO/core1_sram.bin SCRIPT OUT.csv \
        --exe target/release/sharc-chain                # DM image + start registers after SCRIPT
    python tools/organ_bass_fw_gen.py golden target/PNO/core1_sram.bin SCRIPT OUT.csv --calls 64 \
        --exe target/release/sharc-chain                # oracle output words of the chain

The image and the golden stream come from `sharc-chain` (the oracle); the Python VM replay
(`organ_replay.py`) is the reference for the golden stream and is itself checked against it.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import organ_listing_check as olc
import organ_replay as rp
import sharc_listing_rust as slr

ROOT = Path(__file__).resolve().parents[1]
FW_DIR = ROOT / "crates" / "nord-core" / "src" / "organ" / "b3_bass_fw"
GEN = ("gen", 0x121BBE, 0x121E28, "GEN_ROWS")
PROC = ("proc", 0x122296, 0x12261D, "PROC_ROWS")


def rust_text(
    image: str,
    spec: tuple[str, int, int, str],
    tool: str = "organ_bass_fw_gen.py",
    paths: tuple[str, str] = ("super::Row", "super::super::rows_valid"),
) -> str:
    name, entry, end, const = spec
    rows = slr.decode(olc.disassemble(image, entry, end), entry)
    out = [
        f"//! Decoded rows of the routine `{entry:#x}` ({name}), GENERATED.",
        "//!",
        f"//! `python tools/{tool} rust target/PNO/core1_sram.bin` (`--check` fails when",
        "//! stale). Rows are `[kind, a, b, c, d, e, f, g]` (`tools/sharc_listing_rust.py`): decoded",
        "//! operations of the listing, not firmware bytes; research s15. VERIFIED: the same rules run",
        "//! bit exact against `sharc-chain` in `tools/organ_replay.py` (s14).",
        "",
        f"use {paths[0]};",
        "",
        "#[rustfmt::skip]",
        f"pub(in crate::organ) static {const}: [Row; {len(rows)}] = [",
    ]
    fmt = [f"[{','.join(map(str, r))}]," for r in rows]
    out += ["    " + " ".join(fmt[k : k + 3]) for k in range(0, len(fmt), 3)]
    out += [
        "];",
        "",
        "#[cfg(test)]",
        "mod tests {",
        f"    use super::{const};",
        "",
        "    #[test]",
        "    fn rows_are_structurally_valid() {",
        f"        assert!({paths[1]}(&{const}));",
        "    }",
        "}",
        "",
    ]
    return "\n".join(out)


def rust_files(image: str) -> dict[Path, str]:
    return {FW_DIR / f"prog_{s[0]}.rs": rust_text(image, s) for s in (GEN, PROC)}


def cmd_rust(a: argparse.Namespace) -> int:
    stale = 0
    for path, text in rust_files(a.image).items():
        have = path.read_text(encoding="utf-8") if path.exists() else ""
        if have.replace("\r\n", "\n") != text:
            stale += 1
            if not a.check:
                path.write_text(text, encoding="utf-8", newline="\n")
                print(f"wrote {path.name}")
    if a.check and stale:
        print(f"{stale} generated file(s) are stale")
        return 1
    return 0


def chain_args(a: argparse.Namespace) -> argparse.Namespace:
    a.gen = "0x121bbe:0x121e28:I0=0xc00f2,I1=0xb52c7"
    a.proc = "0x122296:0x12261d:I0=0xc00ea,I1=0xb472c"
    return a


def cmd_image(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    chain_args(a)
    mem = rp.memory(a, ())
    row0 = rp.trace_rows(a, 0x121BBE, 0x121E28, (a.gen,))[0]
    _pc, regs = rp.vm.parse_trace_line(row0)
    lines = [
        "# DM image and start registers of the B3 Bass chain (research s15); `R,name,hex` / `M,addr,words`"
    ]
    for file, vals in (("r", regs["r"]), ("s", regs["s"]), ("i", regs["i"]), ("m", regs["m"])):
        lines += [f"R,{file}{k},{v:x}" for k, v in enumerate(vals) if v]
    words = sorted((k, v) for k, v in mem.items() if v)
    lines += mem_runs(words)
    Path(a.out).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(words)} words, {len(lines)} lines -> {a.out}")
    return 0


def mem_runs(words: list[tuple[int, int]]) -> list[str]:
    """`M,addr,hex hex ...` lines of sorted `(addr, value)` pairs (consecutive addresses merged)."""
    lines: list[str] = []
    run: list[int] = []
    start = 0
    for addr, v in words:
        if run and addr != start + len(run):
            lines.append(f"M,{start:x}," + " ".join(f"{w:x}" for w in run))
            run = []
        if not run:
            start = addr
        run.append(v)
    if run:
        lines.append(f"M,{start:x}," + " ".join(f"{w:x}" for w in run))
    return lines


def cmd_delta(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    """Words that differ between the start memory of `script` and of `script2`."""
    chain_args(a)
    base = rp.memory(a, ())
    a.script = a.script2
    other = rp.memory(a, ())
    words = [(k, other.get(k, 0)) for k in rp.diff(base, other)]
    lines = ["# DM words that differ from the image (key-down of a chord); research s15"]
    Path(a.out).write_text(
        "\n".join(lines + mem_runs(words)) + "\n", encoding="utf-8", newline="\n"
    )
    print(f"{len(words)} words -> {a.out}")
    return 0


def cmd_golden(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    chain_args(a)
    want, _mem = rp.oracle_calls(a, a.calls)
    lines = ["# call,out0,out1 (hex words of DM 0xC00EA and 0xC00EC after each call; oracle)"]
    lines += [f"{c},{row[0]:x},{row[2]:x}" for c, row in enumerate(want.tolist())]
    Path(a.out).write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("cmd", choices=("rust", "image", "golden", "delta"))
    ap.add_argument("image")
    ap.add_argument("script", nargs="?")
    ap.add_argument("out", nargs="?")
    ap.add_argument("--script2")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--calls", type=int, default=64)
    ap.add_argument("--exe", default="sharc-chain")
    a = ap.parse_args(argv)
    return {"rust": cmd_rust, "image": cmd_image, "golden": cmd_golden, "delta": cmd_delta}[a.cmd](
        a
    )


if __name__ == "__main__":
    sys.exit(main())
