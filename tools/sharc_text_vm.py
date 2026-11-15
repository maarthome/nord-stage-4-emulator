#!/usr/bin/env python3
"""A small interpreter for SHARC+ SIMD listings (`sharc-disasm` text) used to model the FM stages.

It runs the compute / move forms the Core2 oscillator stages use (float multiply, add, subtract,
`ABS`, `CLIP`, `MIN`, `PASS`, `FLOAT`, `FIX`, `FEXT`, `FDEP`, `ASHIFT`, integer add, register moves
and exchanges, data memory loads and stores with DAG pre / post modify, `DO ... UNTIL LCE`) with
the float semantics of [synth_wave_model](synth_wave_model.py): registers are 40-bit values with a
32-bit significand (round to nearest even), memory words are 32 bit (a store truncates).

SIMD (VERIFIED against `nord-sharc` `moves.rs`, `exec.rs`): every instruction runs on PEx, then on
PEy with the register files exchanged. A data transfer moves the PEy register to / from `addr + 1`;
an immediate register load broadcasts to both files; a register move `Rn = Sm` is executed on both
elements (PEy: `Sn = Rm`), an exchange `Rn <-> Sm` once; DAG registers are single.

    vm = Vm(read_word, write_word); vm.run(listing_lines, i={0: 0xC1E02, ...})

The listing text is local derived data (never committed); the tests use synthetic programs.
"""

from __future__ import annotations

import re
from collections.abc import Callable

import synth_wave_model as m

X = m.X
LINE = re.compile(r"^0x([0-9a-f]+)\s+\S+\s+(?:[0-9a-f]{4}\s+)+(.*)$")


def parse_listing(text: str) -> list[tuple[int, str]]:
    """`(byte address, instruction text)` of each listing line (the trailing verdict is dropped)."""
    out = []
    for line in text.splitlines():
        mt = LINE.match(line.rstrip())
        if mt:
            out.append((int(mt.group(1), 16), mt.group(2).strip()))
    return out


def split_items(text: str) -> list[str]:
    """Top-level comma separated items (commas inside parentheses stay)."""
    items, depth, cur = [], 0, ""
    for ch in text:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            items.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        items.append(cur.strip())
    return items


def num(tok: str) -> int:
    return int(tok, 0)


class Vm:
    """Two register files (`regs[0]` PEx R, `regs[1]` PEy), DAG registers and a word memory."""

    def __init__(
        self,
        read: Callable[[int], int],
        write: Callable[[int, int], None],
        index: dict[int, int] | None = None,
    ) -> None:
        self.read, self.write = read, write
        self.regs: list[list] = [[0] * 16, [0] * 16]
        self.i = [0] * 16
        self.m = [0] * 16
        self.lcntr = 0
        self.steps = 0
        self.index = index or {}
        # MODE1 BDCST1 / BDCST9 (VERIFIED, nord-sharc `dag_modes.rs`): a pre-modify load through
        # I1 / I9 gives both elements the word at the PEx address.
        self.bdcst = {1, 9}

    # --- register access -------------------------------------------------------------------
    @staticmethod
    def fval(v) -> X:
        return m.ld(v) if isinstance(v, int) else v

    @staticmethod
    def ival(v) -> int:
        return m.st(v) if not isinstance(v, int) else v & 0xFFFFFFFF

    def _reg(self, snap, lane: int, tok: str):
        """Value of `Rn` / `Fn` (own file) or `Sn` (the other file) of element `lane`."""
        n = int(tok[1:])
        side = lane ^ (1 if tok[0] == "S" else 0)
        return snap[side][n]

    # --- one instruction -------------------------------------------------------------------
    def step(self, text: str) -> None:
        items = split_items(text)
        snap = [list(self.regs[0]), list(self.regs[1])]
        pend: list[tuple[int, int, object]] = []  # (lane, reg, value)
        mem: list[tuple[int, int]] = []  # (addr, word)
        dag: dict[int, int] = {}
        for item in items:
            self._item(item, snap, pend, mem, dag)
        for lane, n, v in pend:
            self.regs[lane][n] = v
        for a, w in mem:
            self.write(a, w)
        for k, v in dag.items():
            self.i[k] = v & 0xFFFFFFFF

    def _addr(self, spec: str) -> tuple[int, int, int]:
        """`(address, dag register, new index)` of `DM(...)` text: pre or post modify."""
        inner = spec[3:-1]
        a, b = (t.strip() for t in inner.split(","))
        if b[0] == "I":  # DM(offset, Ib): pre modify, no update
            k = int(b[1:])
            addr = (self.i[k] + num(a)) & 0xFFFFFFFF
            return addr, k, self.i[k]
        k = int(a[1:])  # DM(Ia, M or imm): post modify, update
        mod = self.m[int(b[1:])] if b[0] == "M" else num(b)
        return self.i[k], k, (self.i[k] + m.s32(mod)) & 0xFFFFFFFF

    def _item(self, item, snap, pend, mem, dag) -> None:
        fm = re.fullmatch(r"([RSF])(\d+) = (.+)", item)
        # --- data memory transfers
        mt = re.fullmatch(r"DM\((.+)\) = ([RS])(\d+)", item)
        if mt:
            addr, k, nxt = self._addr(f"DM({mt.group(1)})")
            n = int(mt.group(3))
            for lane in (0, 1):
                side = lane ^ (mt.group(2) == "S")
                mem.append((addr + lane, self.ival(snap[side][n])))
            dag[k] = nxt
            return
        mt = re.fullmatch(r"([RS])(\d+) = (DM\(.+\))", item)
        if mt:
            addr, k, nxt = self._addr(mt.group(3))
            n = int(mt.group(2))
            broadcast = k in self.bdcst and not mt.group(3).split(",")[1].strip()[0] == "M"
            for lane in (0, 1):
                tgt = lane ^ (mt.group(1) == "S")
                pend.append((tgt, n, self.read(addr + (0 if broadcast else lane))))
            dag[k] = nxt
            return
        mt = re.fullmatch(r"I(\d+) = DM\((.+)\)", item)
        if mt:
            addr, k, nxt = self._addr(f"DM({mt.group(2)})")
            dag[int(mt.group(1))] = self.read(addr)
            dag.setdefault(k, nxt)
            return
        # --- DAG and control forms
        mt = re.fullmatch(r"I(\d+) = MODIFY\(I(\d+), (\S+)\)", item)
        if mt:
            dag[int(mt.group(1))] = self.i[int(mt.group(2))] + m.s32(num(mt.group(3)))
            return
        mt = re.fullmatch(r"I(\d+) = (0x[0-9a-f]+|\d+)", item)
        if mt:
            dag[int(mt.group(1))] = num(mt.group(2))
            return
        mt = re.fullmatch(r"M(\d+) = (0x[0-9a-f]+|\d+)", item)
        if mt:
            self.m[int(mt.group(1))] = num(mt.group(2))
            return
        mt = re.fullmatch(r"I(\d+) = I(\d+)", item)
        if mt:
            dag[int(mt.group(1))] = self.i[int(mt.group(2))]
            return
        mt = re.fullmatch(r"I(\d+) = R(\d+)", item)
        if mt:
            dag[int(mt.group(1))] = self.ival(snap[0][int(mt.group(2))])
            return
        mt = re.fullmatch(r"R(\d+) <-> S(\d+)", item)
        if mt:  # PEx only: the PEy half of the SIMD exchange is the same exchange
            a, b = int(mt.group(1)), int(mt.group(2))
            pend.append((0, a, snap[1][b]))
            pend.append((1, b, snap[0][a]))
            return
        if fm is None:
            raise ValueError(f"unmodelled item: {item}")
        self._compute(fm, snap, pend, item)

    def _compute(self, fm, snap, pend, item) -> None:
        dk, dn, rhs = fm.group(1), int(fm.group(2)), fm.group(3)
        for lane in (0, 1):
            tgt = lane  # destination file of this element (S forms are handled by the caller)
            if dk == "S":
                raise ValueError(f"S destination unsupported: {item}")

            def r(tok: str, lane=lane):
                return self._reg(snap, lane, tok)

            val = self._rhs(rhs, r)
            pend.append((tgt, dn, val))

    def _rhs(self, rhs: str, r):
        f, iv = self.fval, self.ival
        mt = re.fullmatch(r"(F\d+) ([*+-]) (F\d+)", rhs)
        if mt:
            a, b = f(r(mt.group(1))), f(r(mt.group(3)))
            return {"*": m.mul, "+": m.add, "-": m.sub}[mt.group(2)](a, b)
        mt = re.fullmatch(r"ABS (F\d+)", rhs)
        if mt:
            return abs(f(r(mt.group(1))))
        mt = re.fullmatch(r"PASS (F\d+)", rhs)
        if mt:
            return f(r(mt.group(1)))
        mt = re.fullmatch(r"CLIP (F\d+) BY (F\d+)", rhs)
        if mt:
            return m.clip(f(r(mt.group(1))), f(r(mt.group(2))))
        mt = re.fullmatch(r"MIN ?\((F\d+), (F\d+)\)", rhs)
        if mt:
            return min(f(r(mt.group(1))), f(r(mt.group(2))))
        mt = re.fullmatch(r"FLOAT (R\d+) BY (R\d+)", rhs)
        if mt:
            return m.flt(iv(r(mt.group(1))), m.s32(iv(r(mt.group(2)))))
        mt = re.fullmatch(r"FIX (F\d+) BY ([RF]\d+)", rhs)
        if mt:
            return m.fix(f(r(mt.group(1))), m.s32(iv(r(mt.group(2)))))
        mt = re.fullmatch(r"(R\d+) \+ (R\d+)", rhs)
        if mt:
            return m.u32(iv(r(mt.group(1))) + iv(r(mt.group(2))))
        mt = re.fullmatch(r"ASHIFT (R\d+) BY (-?\d+)", rhs)
        if mt:
            sh = int(mt.group(2))
            x = m.s32(iv(r(mt.group(1))))
            return m.u32(x << sh if sh >= 0 else x >> -sh)
        mt = re.fullmatch(r"FEXT (R\d+) BY (\d+):(\d+)( \(SE\))?", rhs)
        if mt:
            pos, ln = int(mt.group(2)), int(mt.group(3))
            v = (iv(r(mt.group(1))) >> pos) & ((1 << ln) - 1)
            if mt.group(4) and v >> (ln - 1):
                v -= 1 << ln
            return m.u32(v)
        mt = re.fullmatch(r"FDEP (R\d+) BY (\d+):(\d+)", rhs)
        if mt:
            pos, ln = int(mt.group(2)), int(mt.group(3))
            return m.u32((iv(r(mt.group(1))) & ((1 << ln) - 1)) << pos)
        mt = re.fullmatch(r"(0x[0-9a-f]+|-?\d+)", rhs)
        if mt:
            return m.u32(num(rhs))
        mt = re.fullmatch(r"([RS]\d+)", rhs)
        if mt:
            return r(mt.group(1))
        raise ValueError(f"unmodelled compute: {rhs}")

    # --- program ---------------------------------------------------------------------------
    def run(self, lines: list[tuple[int, str]], limit: int = 100_000) -> None:
        """Runs `(address, text)` lines from the first to `RTS`; `DO (PC, n) UNTIL LCE` loops."""
        pos = {a: k for k, (a, _) in enumerate(lines)}
        k = 0
        loops: list[tuple[int, int, int]] = []  # (start index, end index, remaining)
        while k < len(lines):
            addr, text = lines[k]
            self.steps += 1
            if self.steps > limit:
                raise RuntimeError("step limit")
            if text == "RTS":
                return
            mt = re.fullmatch(r"DO \(PC, (\d+)\) UNTIL LCE, LCNTR = (\d+)", text)
            if mt:
                end_addr = addr + 2 * int(mt.group(1))
                loops.append((k + 1, pos[end_addr], int(mt.group(2))))
                k += 1
                continue
            self.step(text)
            if loops and k == loops[-1][1]:
                s, e, rem = loops[-1]
                if rem > 1:
                    loops[-1] = (s, e, rem - 1)
                    k = s
                    continue
                loops.pop()
            k += 1
