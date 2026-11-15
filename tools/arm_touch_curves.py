#!/usr/bin/env python3
"""Program Touch velocity curves of the Piano section, from the host firmware (`bank0_CODE.bin`).

Edit id `0x159` (`Edt_Pno_kKbdTouch`, values 0..=2, default 1) stores `curveId` into field `+0x1C`
of the piano layer object (handler `0xC011AA24`, table `0xC02BBF98 + 0x678`: ids 10, 11, 13). The
voice init `0xC00AA520` reads the velocity position of a note as
`T = byte[0xC0532664 + (curveId << 10) + r8]`, with
`r8 = s16[0xC02B3128 + 0x18 + 2 * (v + 128 * 6)]` (curve 6 of the 8 internal velocity curves is in
force at the default program). This tool composes `T(v)` for `v = 1..=127` and prints it as Rust
arrays plus the fingerprint the Rust unit test pins:

    python tools/arm_touch_curves.py table target/fw_unpacked/bank0_CODE.bin
    python tools/arm_touch_curves.py rust  target/fw_unpacked/bank0_CODE.bin
    python tools/arm_touch_curves.py check target/fw_unpacked/bank0_CODE.bin

`arm-check` runs the real firmware (`arm_keydown` rig, `arm_piano_load`): it loads a piano sound,
sets `[ctx + 0x1C]` to each curve id, sends a MIDI note-on over UART2 and compares the byte the ARM
loads at `0xC00AA658` with the composed table:

    uv run --with unicorn --with capstone python tools/arm_touch_curves.py arm-check \
        target/fw_unpacked/bank0_CODE.bin target/fw_unpacked/bank0_SRAM.bin \
        --cache target/keydown.snap --ns4b "<backup>.ns4b" \
        --npno "Piano/Clavinet/Clavinet D6  6.1.npno"

Only the firmware image is read (local, never commit it).
Evidence: `docs/research/piano-touch-curves.md`.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

LOAD_BASE = 0xC0000000
# VERIFIED (disassembly `0xC011AA24..0xC011AA38`, run): `curveId = u32[TOUCH_TABLE + 4 * value]`.
TOUCH_TABLE = 0xC02BBF98 + 0x678
# VERIFIED (disassembly `0xC00AA644..0xC00AA654`): byte table, one 1024-byte curve per id.
T_TABLE = 0xC0532664
# VERIFIED (disassembly `0xC00D2090..0xC00D209C`, run): `s16` internal velocity curves, 128
# entries each, read at `+0x18`; curve 6 is the default (`[manager + 0x134]`).
R8_TABLE = 0xC02B3128
R8_OFFSET = 0x18
DEFAULT_R8_CURVE = 6
EDIT_VALUES = (0, 1, 2)
# The `ldrb lr, [ip, r8]` of the voice init sits at `0xC00AA654`; `T` is in `lr` after it.
CTX_PC = 0xC00AA654
T_LOAD_PC = 0xC00AA658
# Velocities of the ARM check.
CHECK_VELOCITIES = (1, 2, 10, 33, 64, 90, 100, 120, 127)


def read_u32(code: bytes, address: int) -> int:
    return struct.unpack_from("<I", code, address - LOAD_BASE)[0]


def curve_ids(code: bytes) -> list[int]:
    """`curveId` of the edit values 0, 1 and 2."""
    return [read_u32(code, TOUCH_TABLE + 4 * v) for v in EDIT_VALUES]


def r8_of(code: bytes, velocity: int, curve: int = DEFAULT_R8_CURVE) -> int:
    """Internal velocity of a MIDI velocity (`1..=127`)."""
    address = R8_TABLE + R8_OFFSET + 2 * (velocity + 128 * curve) - LOAD_BASE
    return struct.unpack_from("<h", code, address)[0]


def t_of(code: bytes, curve_id: int, velocity: int) -> int:
    """Velocity position `T` of `velocity` under `curve_id`."""
    return code[T_TABLE - LOAD_BASE + (curve_id << 10) + r8_of(code, velocity)]


def table(code: bytes, curve_id: int) -> list[int]:
    """`T(v)` for `v = 1..=127`."""
    return [t_of(code, curve_id, v) for v in range(1, 128)]


def fingerprint(tables: list[list[int]]) -> int:
    """FNV-1a over the tables in order (same as the Rust unit test)."""
    h = 0xCBF29CE484222325
    for t in tables:
        for b in t:
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def rust_array(name: str, values: list[int], width: int = 92) -> str:
    """A Rust `const` byte array, wrapped like rustfmt."""
    lines, row = [], ""
    for v in values:
        item = f"{v}, "
        if len(row) + len(item) > width - 4:
            lines.append("    " + row.rstrip())
            row = ""
        row += item
    if row:
        lines.append("    " + row.rstrip())
    return f"const {name}: [u8; {len(values)}] = [\n" + "\n".join(lines) + "\n];"


def extract(path: Path) -> tuple[list[int], list[list[int]]]:
    """`(curve ids, T tables)` of the three edit values."""
    code = path.read_bytes()
    ids = curve_ids(code)
    return ids, [table(code, i) for i in ids]


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("cmd", choices=("table", "rust", "check", "arm-check"))
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path, nargs="?")
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--ns4b", type=Path)
    ap.add_argument("--npno")
    ns = ap.parse_args(argv)
    ids, tables = extract(ns.image)
    if ns.cmd == "table":
        for i, t in zip(ids, tables):
            print(f"curve id {i}: T(1)={t[0]} T(64)={t[63]} T(127)={t[126]}")
            print(" ", t)
        return 0
    if ns.cmd == "rust":
        print(rust_array("T_HEAVY", tables[0]))
        print(rust_array("T_LIGHT", tables[2]))
    print(f"fingerprint {fingerprint(tables):#018x} ids {ids}")
    if ns.cmd == "arm-check":
        return arm_check(ns, ids, tables)
    return 0


def arm_check(
    ns: argparse.Namespace, ids: list[int], tables: list[list[int]]
) -> int:  # pragma: no cover - needs the local firmware
    """Real-firmware check, see the module note (needs `unicorn`, `capstone` and the firmware)."""
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import arm_keydown as kd
    import arm_piano_load as pl
    import arm_steady as ast
    import unicorn.arm_const as uc_arm

    rig = kd.build_rig(ns.image, ns.sram, ns.cache)
    emu = rig.emu
    npno = pl.read_npno(ns.ns4b, ns.npno)
    pl.LibraryStub({1: pl.device_image(npno, "Library.npno")}, pl.KIND_FULL).install(emu)
    pl.load_sound(emu, pl.managers(emu)[0], 1)
    pl.select_sound(emu, 1)
    pl.install_dsp_ready(emu)
    rig.tick(2)
    blob = ast.snapshot(emu)
    got: list[int] = []
    ctxs: list[int] = []
    emu.uc.hook_add(
        emu._u.UC_HOOK_CODE,
        lambda uc, *_: got.append(uc.reg_read(uc_arm.UC_ARM_REG_LR)),
        begin=T_LOAD_PC,
        end=T_LOAD_PC,
    )
    emu.uc.hook_add(
        emu._u.UC_HOOK_CODE,
        lambda uc, *_: ctxs.append(uc.reg_read(uc_arm.UC_ARM_REG_R6)),
        begin=CTX_PC,
        end=CTX_PC,
    )
    kd.capture_note(rig, 60, 64, {}, on_ticks=40, off_ticks=2)
    ctx = ctxs[0]
    bad = n = 0
    for curve_id, t in zip(ids, tables):
        for v in CHECK_VELOCITIES:
            ast.restore(emu, blob)
            emu.install_dsp_ports()
            ast.enable_real_timer(emu)
            emu.write32(ctx + 0x1C, curve_id)
            got.clear()
            kd.capture_note(rig, 60, v, {}, on_ticks=40, off_ticks=2)
            n += 1
            bad += not (got and got[0] == t[v - 1])
            print(f"curve {curve_id} velocity {v}: ARM {got[:1]} table {t[v - 1]}")
    print(f"mismatches {bad} of {n}")
    return int(bad > 0)


if __name__ == "__main__":
    sys.exit(main())
