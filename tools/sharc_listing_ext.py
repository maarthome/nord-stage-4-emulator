#!/usr/bin/env python3
"""Generator-level extension of `tools/sharc_listing_vm.py`: the instructions of `0x121BBE`.

`docs/research/organ-farfisa-bass.md` s13 / s14. The B3 Bass generator (and the Farfisa
processor) use more than the processor subset: fixed point multiplies with `MRF` (per PE), bit
field extract / deposit, `float Rx by Ry`, `clip`, `abs`, `comp` with a per PE conditional, long
word stores of index registers. The rules are the ones of `nord-sharc` (`core/multiplier.rs`,
`shifter.rs`, `fixed_alu.rs`, `float_conv.rs`, `longword.rs`), checked by the per instruction
trace of the oracle (`tools/organ_listing_check.py gentrace`).
"""

from __future__ import annotations

import re

import sharc_listing_vm as vm

M32 = vm.M32


def wrap80(v: int) -> int:
    """80-bit two's complement wrap of the multiplier result registers."""
    v &= (1 << 80) - 1
    return v - (1 << 80) if v >> 79 else v


def round_mr(v: int) -> int:
    """`nord-sharc` `round_mr`: round to nearest even on the low 32 bits of an MR value."""
    r = v + 0x8000_0000
    if v & M32 == 0x8000_0000:
        r &= ~(1 << 32)
    return wrap80(r & ~M32)


def shift(v: int, s: int, arith: bool) -> int:
    """Left shift by `s` (right when negative), off scale gives 0 or the sign fill."""
    v &= M32
    if s >= 0:
        return (v << s) & M32 if s < 32 else 0
    if -s >= 32:
        return M32 if arith and v >> 31 else 0
    return (vm.sx(v) >> -s) & M32 if arith else v >> -s


def low_mask(n: int) -> int:
    return M32 if n >= 32 else (1 << n) - 1


def fdep(x: int, length: int, pos: int, se: bool) -> int:
    length = min(length, 32)
    field = x & low_mask(length)
    wide = field << pos
    top = pos + length
    if se and length and top <= 32 and field >> (length - 1) & 1:
        wide |= (~0 << top) & M32
    return wide & M32


def fext(x: int, length: int, pos: int, se: bool) -> int:
    length = min(length, 32)
    v = (x >> pos if pos < 32 else 0) & low_mask(length)
    if se and length and pos + length <= 32 and v >> (length - 1) & 1:
        v |= ~low_mask(length) & M32
    return v


NUM = r"(0x[0-9a-f]+|\d+)"
FLOAT_BY = re.compile(r"float ([rf]\d+) by ([rf]\d+)")
FIELD = re.compile(rf"(fext|fdep) (r\d+) by (?:(r\d+)|{NUM}:{NUM})( \(se\))?")
SHIFT = re.compile(r"(ashift|lshift) (r\d+) by (r\d+)")
MULT = re.compile(r"(?:(MRF|MRB)([+-]))?(r\d+)\*(r\d+) ?\((\w+)\)")
LW_STORE = re.compile(r"(dm|pm)\((.*)\)=(\w+) \(LW\)")
MRF_SET = re.compile(r"(MRF|MRB)=(r\d+)\*(r\d+) ?\((\w+)\)")
COMP = re.compile(r"comp\((r\d+),(r\d+)\)")


class GenVm(vm.Vm):
    """`Vm` plus the generator instructions, per PE multiplier registers and flags."""

    def __init__(self, mem: dict[int, int]):
        super().__init__(mem)
        self.mr = [0, 0]  # MRF of PEx, PEy
        self.an = [False, False]
        self.az = [False, False]
        self._pend: list = []

    # ---- compute ---------------------------------------------------------------------------
    def mult(self, pe: int, mr_op: str | None, xt: str, yt: str, mode: str, sel: str) -> int:
        """Result of a fixed multiply: the 32-bit register word (or the new MR for `MRF=`)."""
        file = self.s if pe else self.r
        sgn_x, sgn_y, frac = mode[0] == "s", mode[1] == "s", mode[2] == "f"
        op = lambda t, sg: vm.sx(file[int(t[1:])] >> 8) if sg else file[int(t[1:])] >> 8  # noqa: E731
        p = op(xt, sgn_x) * op(yt, sgn_y)
        if frac and sgn_x and sgn_y:
            p <<= 1
        cur = self.mr[pe]
        v = wrap80(p if mr_op is None else cur + p if mr_op == "+" else cur - p)
        if frac and mode.endswith("r"):
            v = round_mr(v)
        if sel == "MRF":
            return v
        return ((v >> 32) if frac else v) & M32

    def alu(self, expr: str, pe: int) -> int | None:
        v = super().alu(expr, pe)
        if v is not None:
            return v
        e = expr.strip()
        file = self.s if pe else self.r

        def w(t: str) -> int:
            return file[int(t[1:])] >> 8

        if m := FLOAT_BY.fullmatch(e):
            x, scale = vm.sx(w(m[1])), vm.sx(w(m[2]))
            return 0 if x == 0 else vm.round_pack(x < 0, scale, abs(x), False)
        if m := re.fullmatch(r"abs (f\d+)", e):
            f = file[int(m[1][1:])]
            return f & ~(1 << 39) if (f >> 31) & 0xFF else 0
        if m := FIELD.fullmatch(e):
            if m[3]:
                ry = w(m[3])
                length, pos = (ry >> 6) & 63, ry & 63
            else:
                pos, length = int(m[4], 0), int(m[5], 0)
            f = fext if m[1] == "fext" else fdep
            return f(w(m[2]), length, pos, bool(m[6])) << 8
        if m := SHIFT.fullmatch(e):
            return shift(w(m[2]), vm.sx(w(m[3])), m[1] == "ashift") << 8
        if m := re.fullmatch(r"clip (r\d+) by (r\d+)", e):
            lim = abs(vm.sx(w(m[2])))
            return (max(-lim, min(lim, vm.sx(w(m[1])))) & M32) << 8
        if m := re.fullmatch(r"(r\d+) and (r\d+)", e):
            return (w(m[1]) & w(m[2])) << 8
        if m := re.fullmatch(r"(r\d+)([+-])(\d+)", e):
            d = int(m[3])
            return ((w(m[1]) + (d if m[2] == "+" else -d)) & M32) << 8
        if m := MULT.fullmatch(e):
            return self.mult(pe, m[2], m[3], m[4], m[5], "R") << 8
        return None

    # ---- items the base cannot parse -------------------------------------------------------
    def hook(self, item: str, effects: tuple) -> bool:  # noqa: ANN001
        _post, _post32, post_i, writes_mem = effects
        if m := MRF_SET.fullmatch(item):
            for pe in (0, 1):
                self._pend.append(("mr", pe, self.mult(pe, None, m[2], m[3], m[4], "MRF")))
            return True
        if m := COMP.fullmatch(item):
            for pe in (0, 1):
                file = self.s if pe else self.r
                a, b = vm.sx(file[int(m[1][1:])] >> 8), vm.sx(file[int(m[2][1:])] >> 8)
                self._pend.append(("flags", pe, a < b, a == b))
            return True
        if m := LW_STORE.fullmatch(item):
            a, upd = self.addr(m[2])
            if upd:
                post_i.append(upd)
            n = int(m[3][1:])
            names = [m[3][0] + str(n & ~1), m[3][0] + str(n | 1)]
            writes_mem += [(a & ~1, self.get(names[0])), (a | 1, self.get(names[1]))]
            return True
        return False

    # ---- conditional execution and deferred state ------------------------------------------
    def step(self, ins: vm.Insn) -> None:
        self._pend = []
        m = re.match(r"if (\w+) (.*)", ins.text)
        keep = None
        if m:
            if m[1] != "lt":
                raise ValueError(f"condition {m[1]!r} in {ins.text!r}")
            keep = (self.r[:], self.s[:])
            ins = vm.Insn(ins.addr, m[2], vm.split_items(m[2]))
        super().step(ins)
        if keep:  # per PE condition: a false PE keeps its registers
            for pe, old in ((0, keep[0]), (1, keep[1])):
                if not (self.an[pe] and not self.az[pe]):
                    (self.s if pe else self.r)[:] = old
        for p in self._pend:
            if p[0] == "mr":
                self.mr[p[1]] = p[2]
            else:
                self.an[p[1]], self.az[p[1]] = p[2], p[3]
