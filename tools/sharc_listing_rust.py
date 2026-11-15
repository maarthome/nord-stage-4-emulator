#!/usr/bin/env python3
"""Decodes an ADI listing (`tools/adi_dis.py`) into the row table of the Rust SHARC machine.

`docs/research/organ-farfisa-bass.md` s15. The Python interpreter (`sharc_listing_vm.py`,
`sharc_listing_ext.py`) runs a listing bit exact against the oracle by matching the instruction
text with regular expressions; this module applies the **same** rules once and emits the result as
integer rows, so `crates/nord-core/src/organ/b3_bass_fw/` can run the same routine without text:

* row `[kind, a, b, c, d, e, f, g]` (eight `i32`),
* `INS n cond jcond target` opens an instruction of `n` item rows (reads see the old state, writes
  land at the end, `cond` = `if lt`); a jump is an `INS` with no items, `jcond` 1 = always, 2 =
  `if ne`, 3 = `if eq` and `target` the absolute index of the row it continues at (forward, inside
  the same loop body); `LOOP count n` repeats the next `n` rows (`lcntr` / `do ... until lce`).

Rows carry no firmware bytes, only the decoded operation of each instruction (D22: derived).
"""

from __future__ import annotations

import re

import sharc_listing_ext as ext
import sharc_listing_vm as vm

# row kinds (mirrored in `b3_bass_fw/machine.rs`)
INS, LOOP, SWAP, STORE, LOAD, MODIFY, MOVE_N, MOVE_NI, MOVE_D, MOVE_DN, MOVE_DI, ALU = range(12)
MRF_SET, COMP = 12, 13
# ALU operations
(PASS, FMUL, FADD, FSUB, IADD, ISUB, FLOATBY, ABS, FEXT_R, FEXT_I, FDEP_R, FDEP_I) = range(12)
ASHIFT, LSHIFT, CLIP, AND, ADDI, MULT = range(12, 18)

NAMES = "rsimlb"
KIND_BASE = {"i": 32, "m": 48, "l": 64, "b": 80}
LCNTR = 96


def reg_id(name: str) -> int:
    """Register number: data registers `r` 0..15, `s` 16..31, then `i`, `m`, `l`, `b`, `lcntr`."""
    if name == "lcntr":
        return LCNTR
    n = int(name[1:])
    if name[0] in "rf":
        return n
    return 16 + n if name[0] == "s" else KIND_BASE[name[0]] + n


def is_data(name: str) -> bool:
    return name[0] in "rsf" and name[1:].isdigit()


def addr_fields(spec: str) -> tuple[int, int, int]:
    """`(kind, p, q)` of a `dm(...)` operand: 0 abs, 1 post m, 2 post imm, 3 pre m, 4 pre imm."""
    parts = [p.strip() for p in spec.split(",")]
    if len(parts) == 1:
        return 0, vm.sx(int(parts[0], 0)), 0
    a, b = parts
    if a[0] == "i":
        ia = int(a[1:])
        return (1, ia, int(b[1:])) if b[0] == "m" else (2, ia, vm.sx(int(b, 0)))
    ib = int(b[1:])
    return (3, ib, int(a[1:])) if a[0] == "m" else (4, ib, vm.sx(int(a, 0)))


def mode_bits(mode: str) -> int:
    return (
        (mode[0] == "s") | (mode[1] == "s") << 1 | (mode[2] == "f") << 2 | mode.endswith("r") << 3
    )


def alu_row(dst: int, src: str) -> list[int] | None:
    """`[ALU, dst, op, x, y, p, q, 0]` of a compute expression, or `None`."""
    e = src.strip()
    n = lambda t: int(t[1:])  # noqa: E731
    if m := re.fullmatch(r"pass (f\d+)", e):
        return [ALU, dst, PASS, n(m[1]), 0, 0, 0, 0]
    if m := re.fullmatch(r"(f\d+)([*+-])(f\d+)", e):
        op = {"*": FMUL, "+": FADD, "-": FSUB}[m[2]]
        return [ALU, dst, op, n(m[1]), n(m[3]), 0, 0, 0]
    if m := re.fullmatch(r"(r\d+)([+-])(r\d+)", e):
        return [ALU, dst, IADD if m[2] == "+" else ISUB, n(m[1]), n(m[3]), 0, 0, 0]
    if m := ext.FLOAT_BY.fullmatch(e):
        return [ALU, dst, FLOATBY, n(m[1]), n(m[2]), 0, 0, 0]
    if m := re.fullmatch(r"abs (f\d+)", e):
        return [ALU, dst, ABS, n(m[1]), 0, 0, 0, 0]
    if m := ext.FIELD.fullmatch(e):
        se = int(bool(m[6]))
        if m[3]:
            return [ALU, dst, FEXT_R if m[1] == "fext" else FDEP_R, n(m[2]), n(m[3]), se, 0, 0]
        op = FEXT_I if m[1] == "fext" else FDEP_I
        return [ALU, dst, op, n(m[2]), int(m[4], 0), int(m[5], 0), se, 0]
    if m := ext.SHIFT.fullmatch(e):
        return [ALU, dst, ASHIFT if m[1] == "ashift" else LSHIFT, n(m[2]), n(m[3]), 0, 0, 0]
    if m := re.fullmatch(r"clip (r\d+) by (r\d+)", e):
        return [ALU, dst, CLIP, n(m[1]), n(m[2]), 0, 0, 0]
    if m := re.fullmatch(r"(r\d+) and (r\d+)", e):
        return [ALU, dst, AND, n(m[1]), n(m[2]), 0, 0, 0]
    if m := re.fullmatch(r"(r\d+)([+-])(\d+)", e):
        return [ALU, dst, ADDI, n(m[1]), int(m[3]) * (1 if m[2] == "+" else -1), 0, 0, 0]
    if m := ext.MULT.fullmatch(e):
        mrop = {None: 0, "+": 1, "-": 2}[m[2]]
        return [ALU, dst, MULT, n(m[3]), n(m[4]), mrop, mode_bits(m[5]), 0]
    return None


def item_row(item: str) -> list[int]:
    """The row of one instruction item (the rules of `Vm.step` and `GenVm.hook`)."""
    if m := ext.MRF_SET.fullmatch(item):
        return [MRF_SET, int(m[2][1:]), int(m[3][1:]), mode_bits(m[4]), 0, 0, 0, 0]
    if m := ext.COMP.fullmatch(item):
        return [COMP, int(m[1][1:]), int(m[2][1:]), 0, 0, 0, 0, 0]
    if m := ext.LW_STORE.fullmatch(item):
        return [STORE, reg_id(m[3]), *addr_fields(m[2]), 1, 0, 0]
    if m := re.fullmatch(r"(\w+)<->(\w+)", item):
        assert is_data(m[1]) and is_data(m[2]), item
        return [SWAP, reg_id(m[1]), reg_id(m[2]), 0, 0, 0, 0, 0]
    if m := re.fullmatch(r"(dm|pm)\((.*)\)=(\w+)", item):
        return [STORE, reg_id(m[3]), *addr_fields(m[2]), 0, 0, 0]
    if m := re.fullmatch(r"(\w+)=(dm|pm)\((.*?)\)( \(LW\))?", item):
        bcast = int(vm.Vm.step_twin(m[3]) == 0)
        return [LOAD, reg_id(m[1]), *addr_fields(m[3]), int(bool(m[4])) | bcast << 1, 0, 0]
    if m := re.fullmatch(r"(\w+)=modify ?\((\w+),(\w+)\)", item):
        by_m = m[3][0] == "m"
        val = int(m[3][1:]) if by_m else vm.sx(int(m[3], 0))
        return [MODIFY, reg_id(m[1]), int(m[2][1:]), int(by_m), val, 0, 0, 0]
    m = re.fullmatch(r"(\w+)=(.+)", item)
    assert m, item
    dst, src = m[1], m[2].strip()
    if is_data(dst):
        did = reg_id("s" + dst[1:] if dst[0] == "s" else "r" + dst[1:])
        if row := alu_row(did, src):
            return row
        if re.fullmatch(r"[rsf]\d+", src):
            return [
                MOVE_D,
                did,
                reg_id("s" + src[1:] if src[0] == "s" else "r" + src[1:]),
                0,
                0,
                0,
                0,
                0,
            ]
        if re.fullmatch(r"[a-z]+\d*", src):
            return [MOVE_DN, did, reg_id(src), 0, 0, 0, 0, 0]
        return [MOVE_DI, did, vm.sx(int(src, 0)), 0, 0, 0, 0, 0]
    if re.fullmatch(r"[a-z]+\d*", src):
        return [MOVE_N, reg_id(dst), reg_id(src), 0, 0, 0, 0, 0]
    return [MOVE_NI, reg_id(dst), vm.sx(int(src, 0)), 0, 0, 0, 0, 0]


JUMP_COND = {None: 1, "ne": 2, "eq": 3}


def decode(prog: list[vm.Insn], base: int = 0) -> list[list[int]]:
    """Rows of a listing up to its `rts` (loops become `LOOP count n` + `n` rows).

    `base` is the word address of the first instruction (absolute `jump` targets).
    """
    at = {p.addr: k for k, p in enumerate(prog)}
    rows: list[list[int]] = []
    row_of: dict[int, int] = {}  # listing offset -> index of the instruction's first row
    fixups: list[tuple[int, int]] = []  # (row index of a jump, target offset)
    open_loops: list[tuple[int, int]] = []  # (row index of LOOP, index of the last instruction)
    for k, ins in enumerate(prog):
        row_of[ins.addr] = len(rows)
        if ins.text.startswith("rts"):
            break
        if m := re.fullmatch(r"lcntr=(\w+), do \(pc,(\w+)\) until lce", ins.text):
            end = at[ins.addr + (int(m[2], 0) & 0x7FFFFF)]
            open_loops.append((len(rows), end))
            rows.append([LOOP, int(m[1], 0), 0, 0, 0, 0, 0, 0])
            continue
        if j := vm.JUMP.fullmatch(ins.text):
            fixups.append((len(rows), int(j[2], 0) - base))
            rows.append([INS, 0, 0, JUMP_COND[j[1]], 0, 0, 0, 0])
            while open_loops and open_loops[-1][1] == k:
                start, _ = open_loops.pop()
                rows[start][2] = len(rows) - start - 1
            continue
        text, cond = ins.text, 0
        if c := re.match(r"if (\w+) (.*)", text):
            assert c[1] == "lt", text
            text, cond = c[2], 1
        items = [i for i in vm.split_items(text) if i not in ("nop", "rts (db)", "rts")]
        body = [item_row(i) for i in items]
        rows.append([INS, len(body), cond, 0, 0, 0, 0, 0])
        rows.extend(body)
        while open_loops and open_loops[-1][1] == k:
            start, _ = open_loops.pop()
            rows[start][2] = len(rows) - start - 1
    assert not open_loops
    for at_row, target in fixups:
        rows[at_row][4] = row_of[target]
    return rows
