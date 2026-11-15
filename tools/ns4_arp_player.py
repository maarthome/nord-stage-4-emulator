#!/usr/bin/env python3
"""Firmware evidence for the arpeggiator step length: the record player `0xC023B3EC`.

Fifth research pass on `docs/research/synth-arp-patterns.md` (open question OQ28, bean ns4-rwhu).
The earlier passes found the pattern compiler (`0xC02424DC`, `tools/ns4_arp_gate.py gates`) and the
free-running clock (`0xC01E6368`, `ns4_arp_gate.py clock`) but not the code between them. It is the
**listener method of the pattern object** (vtable `0xC02B79FC`, word 1): the clock calls it with its
64-bit phase (`r2` = low, `r3` = high word) whenever the integer phase changes.

What the player does (disassembly `0xC023B3EC..0xC023B77C`, replayed here on `tools/arm_emu.py`):

1. `phase = low & 0x1FFFFF` (one cycle = `2**21` = 4 firmware beats of `2**19`),
   `cycle = tick >> 21`.
2. It looks for a grid point `g[k]` (the grid at `[obj+0x10]`, 64-bit entries after a count
   word) with `g[k] <= phase < g[k] + 0x10000` that was crossed since the previous call
   (`[obj+0x14]`).
3. The record index is `slot = (cycle * count + k) % [obj+0x24]` (`[obj+0x24]` = steps * 2): **one
   record per grid point**. A record with type 0 / 1 sends "note on" to the listeners, type 3 "note
   off", type 2 nothing; the event carries the arp note index `quotient * ticks + record.tick`.

So the record is played at a grid point, and the grid of the free-running arp spaces its points
`2**18` phase units apart (`tools/ns4_arp_gate.py clock`): half a firmware beat. A step (two
records) is one firmware beat = `30 / D` seconds at the displayed quarter-note BPM `D = R / 2`: a
1/8 note, as the manual (p.36) says. The engine's old `15 / D` s step was twice as fast.

    uv run --with unicorn python tools/ns4_arp_player.py CODE gates
    uv run --with unicorn python tools/ns4_arp_player.py CODE events EqQ.

with `CODE` = `target/fw_unpacked/bank0_CODE.bin`. Evidence and limits:
`docs/research/synth-arp-patterns.md` section 9.
"""

from __future__ import annotations

import argparse
import sys
from collections.abc import Callable
from pathlib import Path

import ns4_arp_gate as ag

PLAYER = 0xC023B3EC  # pattern object vtable word 1 (vtable 0xC02B79FC), also the clock listener
CYCLE_BITS = 21  # one pattern cycle = 2**21 phase units = 4 firmware beats
CYCLE = 1 << CYCLE_BITS
HIT_WINDOW = 0x10000  # a grid point is hit while `g <= phase < g + 0x10000`
STEP_GRID = ag.FREE_RUN_GRID  # 0xC02BBF98: 8 points, spacing 2**18
SHUFFLE_GRID = 0xC02BC1A0  # 8 points: 0, 2/3 beat, 1 beat, 5/3 beat, ... (the object's boot grid)
# Scratch memory of the harness (SRAM window, clear of the compiler harness at
# 0x80040000..0x80060000).
LISTENER = ag.OBJ + 0x4000
VTABLE = ag.OBJ + 0x4100
STUB = ag.OBJ + 0x4200
RING = 0x80050000
RING_BUFFER = 0x80051000
CTX_INDEX = 0x8000C000
RING_TABLE = 0xC05613F4
NOTE_ON, NOTE_OFF = "on", "off"
emu_sinks: dict[
    int, Callable[[object], int]
] = {}  # per emulator: the callback of the listener stub


def read_grid(code: bytes, address: int) -> list[int]:
    """Grid phases at `address` (count word, pad word, then 64-bit entries)."""
    return ag.read_grid(code, address)


def slot_events(grid: list[int], slots: int, cycles: int) -> list[tuple[int, int]]:
    """`(phase, record index)` of every grid point in `cycles` cycles (the player's rule).

    `phase` is the absolute phase of the point (`cycle * 2**21 + g[k]`), the index is
    `(cycle * len(grid) + k) % slots`. The point at phase 0 of the first cycle is not listed: the
    player only reacts to a point that was crossed since its previous call, and the clock starts
    at that phase.
    """
    out = []
    for cycle in range(cycles):
        for k, point in enumerate(grid):
            if cycle == 0 and point == 0:
                continue
            out.append((cycle * CYCLE + point, (cycle * len(grid) + k) % slots))
    return out


def modelled_events(types: list[int], grid: list[int], cycles: int) -> list[tuple[int, str]]:
    """`(phase, "on" | "off")` the player sends for the record types of a compiled pattern.

    Type 3 sends a note-off, type 2 nothing. Type 0 sends a note-on. Type 1 (and any note-on at
    record 0 while the previous note still sounds, flag `[obj+0xBC]`) first sends the pseudo event
    that the listener reads as a note-off (`0xC023B5AC..0xC023B63C`), then the note-on.
    """
    out = []
    sounding = False
    for phase, slot in slot_events(grid, len(types), cycles):
        kind = types[slot]
        if kind == ag.NOTE_OFF:
            out.append((phase, NOTE_OFF))
            sounding = False
        elif kind in ag.NOTE_ON:
            if kind == 1 or (slot == 0 and sounding):
                out.append((phase, NOTE_OFF))
            out.append((phase, NOTE_ON))
            sounding = True
    return out


def phase_seconds(phase: int, table_bpm: int) -> float:
    """Time for the free-running clock to advance `phase` units at a rate-table entry `table_bpm`.

    The clock moves `table_bpm / 60 * 2**19` units per second (`ns4_arp_gate.rate_per_unit`
    times the 24000 units per second time base), so a firmware beat (`2**19`) lasts
    `60 / table_bpm` seconds.
    """
    return phase / ag.PHASE_PER_BEAT * 60.0 / table_bpm


def step_seconds(table_bpm: int) -> float:
    """One pattern step = two records = two grid points (`2 * 2**18`) = one firmware beat."""
    return phase_seconds(ag.SLOTS_PER_STEP * ag.GRID_ENTRY_PHASE, table_bpm)


def player_setup(emu, grid_address: int, types_gate: str) -> list[tuple[int, int, int, int]]:
    """Compiles `types_gate` with the real compiler and wires a fake listener and ring."""
    records = ag.compile_slots(emu, types_gate)
    emu.write32(ag.OBJ + 0x04, 0)  # layer index
    emu.write32(ag.OBJ + 0x10, grid_address)
    emu.write32(ag.OBJ + 0x14, 0)  # previous phase
    emu.write32(ag.OBJ + 0x28, 0)  # not in Gate mode
    emu.uc.mem_write(ag.OBJ + 0xBC, b"\0")
    emu.write32(ag.OBJ + 0xC0, 1)  # one listener
    emu.write32(ag.OBJ + 0xC4, LISTENER)
    emu.write32(LISTENER, VTABLE)
    emu.write32(VTABLE, STUB)
    emu.write32(CTX_INDEX, 0)
    emu.write32(RING_TABLE, RING)
    emu.write32(RING + 0xC, 0xF)
    emu.write32(RING + 0x10, RING_BUFFER)
    return records


def replay(
    emu, grid_address: int, gate: str, cycles: int = 2, resolution: int = 1 << 15
) -> tuple[list[int], list[tuple[int, str, int]]]:
    """Runs the firmware player over `cycles` cycles; returns `(record types, events)`.

    `events` are `(phase of the call, "on" | "off", arp note index)`. The call phase is within
    `resolution` of the grid point (the replay advances the phase in steps of `resolution`).
    """
    from arm_emu import SENTINEL, STACK_TOP

    records = player_setup(emu, grid_address, gate)
    events: list[tuple[int, str, int]] = []
    now = [0]

    def on_event(e) -> int:  # noqa: ANN001
        ev = e.reg(1)
        # Type 1 records send a pseudo event first whose first word is the object pointer (not 1):
        # the Glue listener `0xC0123A18` takes it as a note-off, then gets the real note-on.
        events.append((now[0], NOTE_ON if e.read32(ev) == 1 else NOTE_OFF, e.read32(ev + 4)))
        return 0

    if STUB not in emu.stubs:
        emu.add_stub(STUB, lambda e: emu_sinks[id(e)](e))
    emu_sinks[id(emu)] = on_event
    for phase in range(0, cycles * CYCLE, resolution):
        now[0] = phase
        emu.write32(RING, 0)
        emu.write32(RING + 4, 0)
        emu.set_reg(0, ag.OBJ)
        emu.set_reg(1, 0)
        emu.set_reg(2, phase & 0xFFFFFFFF)
        emu.set_reg(3, phase >> 32)
        emu.set_reg(13, STACK_TOP - 0x1000)
        emu.set_reg(14, SENTINEL)
        emu.run(PLAYER, SENTINEL, 100_000)
    return [rec[0] for rec in records], events


def check_player(emu, grid_address: int, grid: list[int], gate: str, cycles: int = 2) -> list[str]:
    """Differences between the replayed firmware player and `modelled_events` (empty = agree)."""
    types, got = replay(emu, grid_address, gate, cycles)
    want = modelled_events(types, grid, cycles)
    problems = []
    if [kind for _p, kind, _i in got] != [kind for _p, kind in want]:
        problems.append(f"{gate}: event kinds differ: firmware {got} model {want}")
        return problems
    for (call, _kind, _idx), (point, _k) in zip(got, want, strict=True):
        if not point <= call < point + HIT_WINDOW:
            problems.append(f"{gate}: event at call phase {call:#x} for grid point {point:#x}")
    return problems


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the firmware image
    import arm_emu

    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawTextHelpFormatter
    )
    parser.add_argument("code", type=Path)
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("gates")
    ev = sub.add_parser("events")
    ev.add_argument("gate")
    ns = parser.parse_args(argv)
    code = ns.code.read_bytes()
    emu = arm_emu.load_firmware(ns.code)
    grids = {"free-run": STEP_GRID, "boot grid (shuffle)": SHUFFLE_GRID}
    if ns.cmd == "events":
        for name, address in grids.items():
            types, events = replay(emu, address, ns.gate)
            print(f"{name} grid {address:#x}: record types {types}")
            for phase, kind, index in events:
                slots = phase / ag.GRID_ENTRY_PHASE
                print(f"  call phase {phase:#08x} ({slots:5.2f} slots) {kind} {index}")
        return 0
    problems = []
    for name, address in grids.items():
        grid = read_grid(code, address)
        for gate in ("EE", "qq", "EqQ.", "Q.E.", "EEEEEEEE", "QQE.q"):
            problems += check_player(emu, address, grid, gate)
        print(f"{name}: grid {[hex(g) for g in grid]}")
    print("\n".join(problems) or "player events == one record per grid point, all patterns")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
