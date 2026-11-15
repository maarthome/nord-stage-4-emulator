#!/usr/bin/env python3
"""Checks `tools/sharc_listing_vm.py` against the oracle (`sharc-chain`) for one organ processor.

`docs/research/organ-farfisa-bass.md` s13. The routine listing comes from `tools/adi_dis.py` (ADI
`elfdump`), the start state from the boot + install script of `tools/organ_fb_pass.py` plus one
generator-only oracle call, and two comparisons are made:

* `trace`  every instruction of the first call: all R, S, I and M words after the instruction
           against `sharc-chain --trace` (the two Bass frames are 868 rows) and
* memory   after the call every word of the dumped DM ranges (weights, state blocks, delay line,
           output stream) against the oracle's.

Limit: one call. The generator `0x121BBE` also writes processor state words (`DM 0x9559C..0x955AF`,
`0xB45AE..`, `0xC0209`) between calls, so a second call needs the generator listing too.

    python tools/organ_listing_check.py trace IMAGE SCRIPT --entry 0x122296 --end 0x12261D \
        --gen 0x121bbe:0x121e28:I0=0xc00f2,I1=0xb52c7 \
        --proc 0x122296:0x12261d:I0=0xc00ea,I1=0xb472c \
        --exe target/release/sharc-chain
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import sharc_listing_vm as vm

DUMP_RANGES = ((0x95000, 0x1800), (0xB3000, 0x2000), (0xC0000, 0x400), (0xC2B00, 0x300))
SYS_DIR = Path(__file__).resolve().parent


def disassemble(image: str, entry: int, end: int) -> list[vm.Insn]:
    """Listing of `[entry, end)` (pc units are 16-bit parcels: byte = 2 pc)."""
    cmd = [sys.executable, str(SYS_DIR / "adi_dis.py"), image, hex(entry * 2), hex(end * 2 + 0x20)]
    out = subprocess.run(cmd, check=True, capture_output=True, text=True).stdout
    return vm.parse_listing(out)


def chain(exe: str, image: str, script: str, extra: list[str]) -> str:  # pragma: no cover
    cmd = [exe, image, "--script", script, *extra]
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def read_dump(text: str) -> dict[int, int]:
    mem: dict[int, int] = {}
    for line in text.splitlines():
        m = re.match(r"DM\(0x([0-9a-f]+)\.\.\): (.*)", line)
        if m:
            base = int(m[1], 16)
            for k, w in enumerate(m[2].split()):
                mem[base + k] = int(w, 16)
    return mem


def start_memory(  # pragma: no cover - runs the oracle
    a: argparse.Namespace, steps: tuple[str, ...] | None = None
) -> dict[int, int]:
    extra = ["--calls", "1"]
    for step in steps or (a.gen,):
        extra += ["--step", step]
    for base, n in DUMP_RANGES:
        extra += ["--dump", f"{base:#x}:{n:#x}"]
    return read_dump(chain(a.exe, a.image, a.script, extra))


def cmd_trace(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    prog = disassemble(a.image, a.entry, a.end)
    mem = start_memory(a)
    with tempfile.TemporaryDirectory() as t:
        tr = Path(t) / "trace.txt"
        chain(
            a.exe,
            a.image,
            a.script,
            [
                "--calls",
                "1",
                "--step",
                a.gen,
                "--step",
                a.proc,
                "--trace",
                f"{a.entry:#x}:{a.end + 1:#x}",
                str(tr),
            ],
        )
        rows = tr.read_text().splitlines()
    bad = vm.check(mem, prog, rows, a.entry)
    print(f"{len(rows)} rows, {bad} register mismatches")
    machine = vm.Vm(dict(mem))
    machine.m[0:3], machine.m[7], machine.m[10] = [0, 1, 2], 0xFFFFFFFF, 2
    presets = dict(p.split("=") for p in a.proc.split(":")[2].split(","))
    machine.i[0], machine.i[1] = int(presets["I0"], 0), int(presets["I1"], 0)
    machine.run(prog)
    after = start_memory(a, steps=(a.gen, a.proc))
    diff = [
        k for k in sorted(set(after) | set(machine.mem)) if machine.mem.get(k, 0) != after.get(k, 0)
    ]
    print(f"{len(after)} memory words after the call, {len(diff)} differ")
    return 1 if bad or diff else 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("cmd", choices=("trace",))
    ap.add_argument("image")
    ap.add_argument("script")
    ap.add_argument("--entry", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--end", type=lambda s: int(s, 0), required=True)
    ap.add_argument("--gen", required=True)
    ap.add_argument("--proc", required=True)
    ap.add_argument("--exe", default="sharc-chain")
    a = ap.parse_args(argv)
    return cmd_trace(a)


if __name__ == "__main__":
    sys.exit(main())
