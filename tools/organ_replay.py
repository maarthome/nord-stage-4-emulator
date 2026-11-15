#!/usr/bin/env python3
"""Multi-call replay of the B3 Bass generator + processor listings against the oracle.

`docs/research/organ-farfisa-bass.md` s14. `tools/organ_listing_check.py` reproduces ONE call of
`0x122296`; the generator `0x121BBE` also writes processor state between calls, so this tool runs
both listings (`tools/sharc_listing_ext.py`, ADI `elfdump` text from `tools/adi_dis.py`) call after
call in Python and compares with `sharc-chain`:

* `gentrace`  every instruction of the generator (first call) against `--trace`, then the memory
              after the generator against the oracle's,
* `replay`    N calls of generator + processor: the per call output windows and the memory after
              the last call.

    python tools/organ_replay.py gentrace IMAGE SCRIPT --gen 0x121bbe:0x121e28:I0=0xc00f2,I1=0xb52c7
    python tools/organ_replay.py replay IMAGE SCRIPT --calls 40 \
        --gen 0x121bbe:0x121e28:I0=0xc00f2,I1=0xb52c7 --proc 0x122296:0x12261d:I0=0xc00ea,I1=0xb472c
"""

from __future__ import annotations

import argparse
import sys
import tempfile
from pathlib import Path

import numpy as np
import organ_listing_check as olc
import sharc_listing_ext as ext
import sharc_listing_vm as vm

# VERIFIED (s13): output stream words and the generator ring read by the processor.
WINDOWS = ((0xC00EA, 4), (0xB32E8, 0xD0))


def parse_step(spec: str) -> tuple[int, int, dict[str, int]]:
    entry, end, *rest = spec.split(":")
    regs = dict(p.split("=") for p in rest[0].split(",")) if rest and rest[0] else {}
    return int(entry, 0), int(end, 0), {k.lower(): int(v, 0) for k, v in regs.items()}


def preset(machine: vm.Vm, regs: dict[str, int]) -> None:
    for name, value in regs.items():
        machine.put(name, value)


def run_step(machine: vm.Vm, prog: list[vm.Insn], regs: dict[str, int]) -> None:
    preset(machine, regs)
    machine.run(prog)


def args_for(a: argparse.Namespace, steps: tuple[str, ...], calls: int) -> list[str]:
    extra = ["--calls", str(calls)]
    for step in steps:
        extra += ["--step", step]
    for base, n in olc.DUMP_RANGES:
        extra += ["--dump", f"{base:#x}:{n:#x}"]
    return extra


def memory(a: argparse.Namespace, steps: tuple[str, ...], calls: int = 1) -> dict[int, int]:
    return olc.read_dump(olc.chain(a.exe, a.image, a.script, args_for(a, steps, calls)))


def diff(want: dict[int, int], got: dict[int, int]) -> list[int]:
    return [k for k in sorted(set(want) | set(got)) if want.get(k, 0) != got.get(k, 0)]


def start_vm(mem: dict[int, int], row0: str, machine_cls: type = ext.GenVm) -> vm.Vm:
    """VM with the registers of the first oracle trace row (`pc R0..R15 S0..S15 I0.. M0..`)."""
    _pc, regs = vm.parse_trace_line(row0)
    machine = machine_cls(dict(mem))
    machine.r, machine.s = [v << 8 for v in regs["r"]], [v << 8 for v in regs["s"]]
    machine.i, machine.m = regs["i"][:], regs["m"][:]
    return machine


def trace_rows(a: argparse.Namespace, entry: int, end: int, steps: tuple[str, ...]) -> list[str]:
    with tempfile.TemporaryDirectory() as t:
        tr = Path(t) / "trace.txt"
        extra = ["--calls", "1"]
        for step in steps:
            extra += ["--step", step]
        olc.chain(
            a.exe, a.image, a.script, [*extra, "--trace", f"{entry:#x}:{end + 1:#x}", str(tr)]
        )
        return tr.read_text().splitlines()


def cmd_gentrace(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    entry, end, regs = parse_step(a.gen)
    prog = olc.disassemble(a.image, entry, end)
    mem = memory(a, ())
    rows = trace_rows(a, entry, end, (a.gen,))
    bad = vm.check(mem, prog, rows, entry, ext.GenVm)
    print(f"{len(rows)} rows, {bad} register mismatches")
    machine = start_vm(mem, rows[0])
    machine.run(prog[1:])
    bad_mem = diff(memory(a, (a.gen,)), machine.mem)
    print(
        f"memory after the generator: {len(bad_mem)} words differ {[hex(k) for k in bad_mem[:8]]}"
    )
    return 1 if bad or bad_mem else 0


def oracle_calls(a: argparse.Namespace, calls: int) -> tuple[np.ndarray, dict[int, int]]:
    """`(calls, words)` of the output windows per call and the memory after the last call."""
    with tempfile.TemporaryDirectory() as t:
        out = Path(t) / "out.bin"
        extra = args_for(a, (a.gen, a.proc), calls)
        for base, n in WINDOWS:
            extra += ["--range", f"{base:#x}:{n:#x}"]
        text = olc.chain(a.exe, a.image, a.script, [*extra, "--out", str(out)])
        words = sum(n for _b, n in WINDOWS)
        return np.fromfile(out, dtype="<u4").reshape(calls, words), olc.read_dump(text)


def cmd_replay(a: argparse.Namespace) -> int:  # pragma: no cover - runs the oracle
    g_entry, g_end, g_regs = parse_step(a.gen)
    p_entry, p_end, p_regs = parse_step(a.proc)
    gen = olc.disassemble(a.image, g_entry, g_end)
    proc = olc.disassemble(a.image, p_entry, p_end)
    mem = memory(a, ())
    row0 = trace_rows(a, g_entry, g_end, (a.gen,))[0]
    machine = start_vm(mem, row0)
    want, want_mem = oracle_calls(a, a.calls)
    first_bad = None
    for call in range(a.calls):
        run_step(machine, gen, g_regs)
        run_step(machine, proc, p_regs)
        got = [machine.rd(base + k) for base, n in WINDOWS for k in range(n)]
        if first_bad is None and got != want[call].tolist():
            first_bad = call
            ks = [k for k, (x, y) in enumerate(zip(got, want[call].tolist())) if x != y]
            print(f"call {call}: {len(ks)} window words differ, first at {ks[:6]}")
    bad_mem = diff(want_mem, machine.mem)
    print(
        f"{a.calls} calls: first differing call {first_bad}, memory words differing {len(bad_mem)}"
    )
    return 1 if first_bad is not None or bad_mem else 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("cmd", choices=("gentrace", "replay"))
    ap.add_argument("image")
    ap.add_argument("script")
    ap.add_argument("--gen", required=True)
    ap.add_argument("--proc")
    ap.add_argument("--calls", type=int, default=1)
    ap.add_argument("--exe", default="sharc-chain")
    a = ap.parse_args(argv)
    return cmd_gentrace(a) if a.cmd == "gentrace" else cmd_replay(a)


if __name__ == "__main__":
    sys.exit(main())
