#!/usr/bin/env python3
"""Key gesture timing of the host firmware: hold, auto-repeat and double-tap thresholds.

The host firmware turns the panel key matrix into events with one key object (constructed at
`0xC024C560..0xC024C588`, class constructor `0xC0083278`). Per key `k` it keeps arrays
(`this + 0x1C` repeat count, `0x20` countdown, `0x24` first delay, `0x28` repeat interval,
`0x2C` debounce, `0x30` debounce countdown). The setter `0xC0083248(this, k, delay_ms, repeat_ms)`
stores `ms / 6` (`umull 0xAAAAAAAB`, `lsr 2`) into `0x24` / `0x28`, so every threshold is a whole
number of poll steps and the poll is built for one step per 6 ms. The defaults come from
`0xC0083410` (hold 300 ms, repeat 80 ms, double-tap word 300) and the init routine overrides 49
keys, 4 of them with a repeat.

    python tools/arm_key_timing.py table target/fw_unpacked/bank0_CODE.bin
    python tools/arm_key_timing.py rust target/fw_unpacked/bank0_CODE.bin

Only the table decode reads the firmware (local only: never commit it). Evidence and limits:
`docs/research/firmware-gesture-timings.md`.
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import arm_xref as ax

# VERIFIED (disassembly): the key object setter and the defaults routine.
SETTER = 0xC0083248
DEFAULTS_ROUTINE = 0xC0083410
INIT_START, INIT_END = 0xC024C560, 0xC024CA00
# VERIFIED (`0xC0083250..0xC008326C`: `umull ip, r2, 0xAAAAAAAB, ms; lsr r2, r2, #2`): the divisor
# that converts milliseconds to poll steps is exactly 6 for every `ms` below 2**31.
POLL_DIVISOR = 6


@dataclass(frozen=True)
class KeyTiming:
    """One `setter(this, key, delay_ms, repeat_ms)` call of the init routine."""

    key: int
    hold_ms: int
    repeat_ms: int

    @property
    def hold_steps(self) -> int:
        return steps(self.hold_ms)

    @property
    def repeat_steps(self) -> int:
        return steps(self.repeat_ms)


def steps(ms: int) -> int:
    """Poll steps the firmware stores for `ms` (the `umull 0xAAAAAAAB` quotient)."""
    return ((ms * 0xAAAAAAAB) >> 32) >> 2


def decode_mov_reg(insn: int) -> tuple[int, int] | None:
    """(rd, rm) for a plain `MOV rd, rm`, else None."""
    if insn & 0x0FFF0FF0 == 0x01A00000 and (insn >> 28) != 0xF:
        return (insn >> 12) & 0xF, insn & 0xF
    return None


def decode_defaults(code: bytes, base: int = ax.LOAD_BASE) -> dict[str, int]:
    """Defaults routine: `mov r2,#hold`, `mov r1,#repeat`, then three `str rN, [r0, #off]`."""
    regs: dict[int, int] = {}
    out: dict[str, int] = {}
    addr = DEFAULTS_ROUTINE
    for _ in range(8):
        insn = ax.word_at(code, addr - base)
        mov = ax.decode_mov_imm(insn)
        if mov:
            regs[mov[0]] = mov[1]
        elif insn & 0x0FF00000 == 0x05800000 and (insn >> 16) & 0xF == 0:  # STR rd, [r0, #imm]
            out[f"word{(insn & 0xFFF) // 4}"] = regs.get((insn >> 12) & 0xF, -1)
        elif insn == 0xE12FFF1E:  # bx lr
            break
        addr += 4
    return {
        "hold_ms": out["word0"],
        "repeat_ms": out["word1"],
        "double_tap_word": out["word2"],
    }


def decode_table(code: bytes, base: int = ax.LOAD_BASE) -> list[KeyTiming]:
    """Every `bl SETTER` of the init routine with `r1` (key), `r2` (delay) and `r3` (repeat)."""
    regs: dict[int, int] = {}
    out: list[KeyTiming] = []
    for addr in range(INIT_START, INIT_END, 4):
        insn = ax.word_at(code, addr - base)
        mov = ax.decode_mov_imm(insn)
        if mov:
            regs[mov[0]] = mov[1]
            continue
        mreg = decode_mov_reg(insn)
        if mreg:
            regs[mreg[0]] = regs.get(mreg[1], -1)
            continue
        lit = ax.decode_ldr_literal(insn, addr)
        if lit is not None:
            value = ax.literal_value(code, lit, base)
            if value is not None:
                regs[(insn >> 12) & 0xF] = value
            continue
        if ax.decode_bl(insn, addr) == SETTER:
            out.append(KeyTiming(regs[1], regs[2], regs[3]))
    return out


def rust_rows(table: list[KeyTiming]) -> str:
    """The table as Rust `(key, hold_ms, repeat_ms)` rows for `behaviour/firmware.rs`."""
    return "\n".join(f"    ({t.key}, {t.hold_ms}, {t.repeat_ms})," for t in table)


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the local firmware
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("cmd", choices=["table", "rust"])
    p.add_argument("code", type=Path)
    args = p.parse_args(argv)
    code = args.code.read_bytes()
    table = decode_table(code)
    if args.cmd == "rust":
        print(rust_rows(table))
        return 0
    print("defaults", decode_defaults(code), "divisor", POLL_DIVISOR)
    for t in table:
        print(
            f"key {t.key:3d}  hold {t.hold_ms:4d} ms = {t.hold_steps:3d} steps"
            f"  repeat {t.repeat_ms:3d} ms = {t.repeat_steps:2d} steps"
        )
    return 0


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
