#!/usr/bin/env python3
"""Executes an ADI `elfdump` listing of a PNO Core 1 routine (SIMD mode, 40-bit float) in Python.

This is the instruction-level reference for the bit-exact organ routine models
(`tools/organ_bass_model.py`, `docs/research/organ-farfisa-bass.md` s13): the listing text of
`tools/adi_dis.py` is parsed and run on a register file and a flat word memory, and the result is
compared with the per-instruction register trace of the oracle (`sharc-chain --trace`).

Supported subset (everything the Bass and Farfisa processors use): float add / sub / mul / pass on
PEx and PEy, integer add, register moves and swaps, immediates, DM / PM loads and stores with
pre / post modify and circular buffers, long-word loads, `modify`, `lcntr` loops, `rts`.
SIMD (MODE1 PEYEN) rules are the ones of `nord-sharc` (`core/moves.rs`, `core/float_ops.rs`): a
data register transfer moves the PEy twin to `addr + 1`, an S register transfers with the roles
swapped, floats are 40 bit (32 bit significand, round to nearest even), loads clear the extension
byte, moves keep it, stores write the upper 32 bits.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

M32 = 0xFFFFFFFF
ALL_ONES = 0xFF_FFFF_FFFF
LINE = re.compile(r"^([0-9a-f]{8})\s+((?:[0-9a-f]{4} ?)+)\s+(.*?);?\s*$")


def sx(v: int) -> int:
    """Signed value of a 32-bit word."""
    v &= M32
    return v - (1 << 32) if v & 0x80000000 else v


def unpack(v: int) -> tuple[bool, str, int, int]:
    """`(neg, class, exp, man)` of a 40-bit float (`nord-sharc` `core/float.rs`): man is 1.31."""
    neg, e, frac = v >> 39 == 1, (v >> 31) & 0xFF, v & 0x7FFF_FFFF
    cls = "zero" if e == 0 else ("inf" if frac == 0 else "nan") if e == 255 else "norm"
    return neg, cls, e - 127, ((1 << 31) | frac) if cls == "norm" else 0


def round_pack(neg: bool, e2: int, m: int, sticky: bool) -> int:
    """Rounds `m * 2^e2` to a 32-bit significand (nearest even) and encodes the 40-bit float."""
    sign = int(neg) << 39
    if m == 0:
        return sign
    msb = m.bit_length() - 1
    exp = e2 + msb
    shift = msb + 1 - 32
    if shift > 0:
        rem, half = m & ((1 << shift) - 1), 1 << (shift - 1)
        q = m >> shift
        if rem > half or (rem == half and (sticky or q & 1)):
            q += 1
    else:
        q = m << -shift
    if q == 1 << 32:
        q >>= 1
        exp += 1
    if exp > 127:
        return sign | (0xFF << 31)
    if exp < -126:
        return sign
    return sign | ((exp + 127) << 31) | (q & 0x7FFF_FFFF)


def f40_add(a: int, b: int, sub: bool = False) -> int:
    """`Fx + Fy` / `Fx - Fy` on 40-bit floats."""
    xn, xc, xe, xm = unpack(a)
    yn, yc, ye, ym = unpack(b)
    yn ^= sub
    if "nan" in (xc, yc) or (xc == yc == "inf" and xn != yn):
        return ALL_ONES
    if xc == "inf":
        return (int(xn) << 39) | (0xFF << 31)
    if yc == "inf":
        return (int(yn) << 39) | (0xFF << 31)
    if xc == yc == "zero":
        return int(xn and yn) << 39
    if xc == "zero":
        return round_pack(yn, ye - 31, ym, False)
    if yc == "zero":
        return round_pack(xn, xe - 31, xm, False)
    if (xe, xm) < (ye, ym):
        xn, xe, xm, yn, ye, ym = yn, ye, ym, xn, xe, xm
    d = xe - ye
    mb, full = xm << 64, ym << 64
    ms, sticky = (0, True) if d >= 128 else (full >> d, full & ((1 << d) - 1) != 0)
    m = mb + ms if xn == yn else mb - ms - int(sticky)
    if m == 0 and not sticky:
        return 0
    return round_pack(xn, xe - 95, m, sticky)


def f40_mul(a: int, b: int) -> int:
    """`Fx * Fy` on 40-bit floats."""
    xn, xc, xe, xm = unpack(a)
    yn, yc, ye, ym = unpack(b)
    neg = xn ^ yn
    if "nan" in (xc, yc) or {xc, yc} == {"inf", "zero"}:
        return 0x7F_FFFF_FFFF | (int(neg) << 39)
    if "inf" in (xc, yc):
        return (int(neg) << 39) | (0xFF << 31)
    if "zero" in (xc, yc):
        return int(neg) << 39
    return round_pack(neg, xe + ye - 62, xm * ym, False)


def f40_pass(a: int) -> int:
    """`PASS Fx` (a denormal reads as zero)."""
    neg, cls, e, man = unpack(a)
    if cls == "norm":
        return (int(neg) << 39) | ((e + 127) << 31) | (man & 0x7FFF_FFFF)
    return int(neg) << 39 if cls == "zero" else ALL_ONES


def f32_bits_of(x: float) -> int:
    """32-bit IEEE pattern of a float (used to build 40-bit operands from f32 data)."""
    import struct

    return struct.unpack("<I", struct.pack("<f", x))[0]


@dataclass
class Insn:
    addr: int  # parcel offset from the routine start
    text: str
    items: list[str] = field(default_factory=list)


def split_items(text: str) -> list[str]:
    """Comma separated items of one instruction, commas inside parentheses kept."""
    out, depth, cur = [], 0, ""
    for ch in text:
        depth += ch == "("
        depth -= ch == ")"
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    out.append(cur.strip())
    return [i for i in out if i]


def parse_listing(text: str) -> list[Insn]:
    """Instructions of an `adi_dis.py` listing (`offset parcels text` lines)."""
    prog = []
    for line in text.splitlines():
        m = LINE.match(line)
        if m:
            body = m[3].strip()
            prog.append(Insn(int(m[1], 16), body, split_items(body)))
    return prog


def twin(name: str) -> str:
    """The opposite-element register of a data register (`r3` <-> `s3`), others unchanged."""
    if name[0] in "rs" and name[1:].isdigit():
        return ("s" if name[0] == "r" else "r") + name[1:]
    return name


class Vm:
    """Register file, memory and the listing interpreter (PEx = R, PEy = S, both always on).

    `r` and `s` hold 40-bit floats (`value32 << 8 | extension`).
    """

    def __init__(self, mem: dict[int, int]):
        self.mem = mem
        self.r = [0] * 16
        self.s = [0] * 16
        self.i = [0] * 16
        self.m = [0] * 16
        self.l = [0] * 16
        self.b = [0] * 16
        self.lcntr = 0

    # ---- registers ------------------------------------------------------------------------
    def get40(self, name: str) -> int:
        if name[0] in "rs":
            return (self.r if name[0] == "r" else self.s)[int(name[1:])]
        return self.get(name) << 8

    def get(self, name: str) -> int:
        if name == "lcntr":
            return self.lcntr
        v = getattr(self, name[0])[int(name[1:])]
        return v >> 8 if name[0] in "rs" else v

    def put(self, name: str, v: int) -> None:
        """Writes a 32-bit word (a data register loses its extension byte)."""
        v &= M32
        if name == "lcntr":
            self.lcntr = v
        elif name[0] in "rs":
            getattr(self, name[0])[int(name[1:])] = v << 8
        else:
            getattr(self, name[0])[int(name[1:])] = v
            if name[0] == "b":
                self.i[int(name[1:])] = v

    def put40(self, name: str, v: int) -> None:
        getattr(self, name[0])[int(name[1:])] = v & ALL_ONES

    # ---- memory ---------------------------------------------------------------------------
    def rd(self, a: int) -> int:
        return self.mem.get(a & M32, 0)

    def wr(self, a: int, v: int) -> None:
        self.mem[a & M32] = v & M32

    def wrap(self, idx: int, a: int) -> int:
        n, base = self.l[idx], self.b[idx]
        if n == 0:
            return a & M32
        if a >= base + n:
            a -= n
        elif a < base:
            a += n
        return a & M32

    def addr(self, spec: str) -> tuple[int, tuple[int, int] | None]:
        """`(effective address, (i index, new I))` of the operand text inside `dm(...)`."""
        parts = [p.strip() for p in spec.split(",")]
        if len(parts) == 1:
            return int(parts[0], 0) & M32, None
        a, b = parts
        if a[0] == "i":  # post modify
            ia = int(a[1:])
            step = sx(self.m[int(b[1:])]) if b[0] == "m" else sx(int(b, 0))
            return self.i[ia], (ia, self.wrap(ia, self.i[ia] + step))
        ib = int(b[1:])  # pre modify without update
        off = sx(self.m[int(a[1:])]) if a[0] == "m" else sx(int(a, 0))
        return self.wrap(ib, self.i[ib] + off), None

    @staticmethod
    def step_twin(spec: str) -> int:
        """Offset of the PEy word: 0 for a broadcast access through I1 (MODE1 BDCST1), else 1."""
        return 0 if re.search(r"\bi1\b", spec) else 1

    # ---- float / integer compute ----------------------------------------------------------
    def alu(self, expr: str, pe: int) -> int | None:
        """40-bit result of the float or integer `expr` on element `pe`, else `None`."""
        expr = expr.strip()

        def reg(tok: str) -> int:
            file = self.s if pe else self.r
            return file[int(tok[1:])]

        m = re.fullmatch(r"pass (f\d+)", expr)
        if m:
            return f40_pass(reg(m[1]))
        m = re.fullmatch(r"(f\d+)([*+-])(f\d+)", expr)
        if m:
            a, b = reg(m[1]), reg(m[3])
            return f40_mul(a, b) if m[2] == "*" else f40_add(a, b, m[2] == "-")
        m = re.fullmatch(r"(r\d+)([+-])(r\d+)", expr)
        if m:
            a, b = reg(m[1]) >> 8, reg(m[3]) >> 8
            return ((a + b if m[2] == "+" else a - b) & M32) << 8
        return None

    # ---- one instruction ------------------------------------------------------------------
    def step(self, ins: Insn) -> None:
        """Runs one instruction: every item reads the old state, all writes happen at the end."""
        post: list[tuple[str, int]] = []  # 40-bit register writes
        post32: list[tuple[str, int]] = []  # 32-bit register writes
        post_i: list[tuple[int, int]] = []
        writes_mem: list[tuple[int, int]] = []
        for item in ins.items:
            if item in ("nop", "rts (db)", "rts"):
                continue
            m = re.fullmatch(r"(\w+)<->(\w+)", item)
            if m:
                a, b = m[1], m[2]
                post += [(a, self.get40(b)), (b, self.get40(a))]
                continue
            m = re.fullmatch(r"(dm|pm)\((.*)\)=(\w+)", item)
            if m:
                a, upd = self.addr(m[2])
                if upd:
                    post_i.append(upd)
                reg = m[3]
                writes_mem.append((a, self.get(reg)))
                writes_mem.append((a + 1, self.get(twin(reg))))
                continue
            m = re.fullmatch(r"(\w+)=(dm|pm)\((.*?)\)( \(LW\))?", item)
            if m:
                a, upd = self.addr(m[3])
                if upd:
                    post_i.append(upd)
                reg = m[1]
                if m[4]:
                    n = int(reg[1:])
                    post32 += [(reg[0] + str(n & ~1), self.rd(a & ~1))]
                    post32 += [(reg[0] + str(n | 1), self.rd(a | 1))]
                elif reg[0] in "rs":
                    post32 += [(reg, self.rd(a)), (twin(reg), self.rd(a + self.step_twin(m[3])))]
                else:
                    post32.append((reg, self.rd(a)))
                continue
            m = re.fullmatch(r"(\w+)=modify ?\((\w+),(\w+)\)", item)
            if m:
                ia = int(m[2][1:])
                step = sx(self.m[int(m[3][1:])]) if m[3][0] == "m" else sx(int(m[3], 0))
                post32.append((m[1], self.wrap(ia, self.i[ia] + step)))
                continue
            m = re.fullmatch(r"(\w+)=(.+)", item)
            if not m:
                raise ValueError(f"cannot parse {item!r} in {ins.text!r}")
            dst, src = m[1], m[2].strip()
            if dst[0] in "rfs" and dst[1:].isdigit():
                # PEx writes the named register, PEy its twin; `fN` names register `rN`
                names = ("s" if dst[0] == "s" else "r") + dst[1:]
                for pe, target in ((0, names), (1, twin(names))):
                    v = self.alu(src, pe)
                    if v is None:
                        if re.fullmatch(r"[rsf]\d+", src):
                            sname = ("s" if src[0] == "s" else "r") + src[1:]
                            v = self.get40(twin(sname) if pe else sname)
                        elif re.fullmatch(r"[a-z]+\d*", src):
                            v = self.get(src) << 8
                        else:
                            v = (int(src, 0) & M32) << 8
                    post.append((target, v))
            elif re.fullmatch(r"[a-z]+\d*", src):
                post32.append((dst, self.get(src)))
            else:
                post32.append((dst, int(src, 0) & M32))
        for a, v in writes_mem:
            self.wr(a, v)
        for name, v in post:
            self.put40(name, v)
        for name, v in post32:
            self.put(name, v)
        for ia, v in post_i:
            self.i[ia] = v

    # ---- program --------------------------------------------------------------------------
    def run(self, prog: list[Insn], on_step=None) -> None:  # noqa: ANN001
        """Runs `prog` up to its `rts`; `on_step(insn)` is called after every instruction."""
        at = {p.addr: k for k, p in enumerate(prog)}
        stack: list[list[int]] = []  # [start index, end addr, remaining count]
        k = 0
        while k < len(prog):
            ins = prog[k]
            m = re.fullmatch(r"lcntr=(\w+), do \(pc,(\w+)\) until lce", ins.text)
            if m:
                self.lcntr = int(m[1], 0)
                end = ins.addr + (int(m[2], 0) & 0x7FFFFF)
                if end not in at:
                    raise ValueError(f"loop end {end:#x} is not an instruction")
                stack.append([k + 1, end, self.lcntr])
                if on_step:
                    on_step(ins)
                k += 1
                continue
            if ins.text.startswith("rts"):
                return
            self.step(ins)
            if on_step:
                on_step(ins)
            if stack and ins.addr == stack[-1][1]:
                stack[-1][2] -= 1
                self.lcntr = stack[-1][2]
                if stack[-1][2] > 0:
                    k = stack[-1][0]
                    continue
                stack.pop()
            k += 1


def load_listing(path: str | Path) -> list[Insn]:
    """Parses the listing file written by `tools/adi_dis.py`."""
    return parse_listing(Path(path).read_text(encoding="utf-8"))


def parse_trace_line(line: str) -> tuple[int, dict[str, list[int]]]:
    """`(pc, {r, s, i, m})` of a `sharc-chain --trace` row (32-bit words, hex)."""
    v = [int(x, 16) for x in line.split()]
    return v[0], {"r": v[1:17], "s": v[17:33], "i": v[33:49], "m": v[49:65]}


def check(mem: dict[int, int], prog: list[Insn], trace: list[str], base: int) -> int:
    """Runs `prog[1:]` from the state after the first trace row and counts register mismatches.

    The first listing instruction (`i2 = dm(..)`) is undone by starting from its trace row; later
    rows are compared after every instruction and the state is resynchronised on a difference,
    so each wrong rule is reported once.
    """
    _pc, regs = parse_trace_line(trace[0])
    vm = Vm(dict(mem))
    vm.r, vm.s = [v << 8 for v in regs["r"]], [v << 8 for v in regs["s"]]
    vm.i, vm.m = regs["i"][:], regs["m"][:]
    bad = 0
    rows = iter(trace[1:])

    def on_step(ins: Insn) -> None:
        nonlocal bad
        row = next(rows, None)
        if row is None:
            return
        pc, want = parse_trace_line(row)
        got = {"r": [v >> 8 for v in vm.r], "s": [v >> 8 for v in vm.s], "i": vm.i, "m": vm.m}
        if pc != base + ins.addr and bad < 3:
            print(f"pc {pc:x} != {base + ins.addr:x} at {ins.text}")
        for k in "rsim":
            if got[k] != want[k]:
                bad += 1
                if bad <= 8:
                    d = [
                        (j, f"{a:x}", f"{b:x}")
                        for j, (a, b) in enumerate(zip(got[k], want[k]))
                        if a != b
                    ]
                    print(f"pc {pc:x} {ins.text}: {k} {d}")
                if k in "rs":
                    getattr(vm, k)[:] = [v << 8 for v in want[k]]
                else:
                    getattr(vm, k)[:] = want[k]

    vm.run(prog[1:], on_step)
    return bad
