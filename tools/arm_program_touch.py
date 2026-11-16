#!/usr/bin/env python3
"""Program recall to Touch to level, end to end in the booted host firmware (D99 follow-up, D117).

`arm_touch_curves.py arm-check` pokes `[ctx + 0x1C]` by hand. This tool does not: it puts a
factory `.ns4p` into the Store record of the I2C EEPROM image (`arm_program_load.build_image`),
boots with it so the firmware recalls the program itself, hooks the Touch handler store
`0xC011AA38` (`str r3, [r2, #0x1C]`, `r2` = piano layer object, `r3` = curveId) and then sends
UART2 MIDI note-ons (`arm_keydown` rig) with the piano sound of the program served through
`arm_piano_load.LibraryStub`. For every velocity it reports

* `T`, the byte the voice init loads at `0xC00AA654` (`lr` after `0xC00AA658`),
* the level call `0xC00A1EB4` arguments `x`, `rec[0x5E]` and the Q23 amplitude it stores.

    python tools/arm_program_touch.py <CODE> <SRAM> --eeprom target/oled/eeprom.bin \
        --ns4b "<backup>.ns4b" --program "Program/Bank A/Pearl Upright.ns4p" \
        --npno "Piano/Upright/Pearl Upright Lrg 6.0.npno" --velocities 1 10 33 64 100 127

Reads the local firmware only; output is derived numbers (never commit the firmware).
"""

from __future__ import annotations

import argparse
import pickle
import sys
import zipfile
from pathlib import Path

# VERIFIED (disassembly `0xC011AA24..0xC011AA38`): the Touch handler stores curveId (`r3`) into
# `[r2 + 0x1C]`, `r2` = `[[arg + 0x94] + 0xC]` = the piano layer object, at this store.
TOUCH_STORE_PC = 0xC011AA38
CTX_PC = 0xC00AA654
T_LOAD_PC = 0xC00AA658
CURVE_IDS = (10, 11, 13)  # edit value 0 / 1 / 2, `piano/touch_curve.rs::CURVE_ID`
# VERIFIED (research `piano-touch-curves.md` s1): level attenuation index of a position `T`.
X_NUM, X_SHIFT, X_MAX = 128 - 0x13, 7, 249


def x_of_t(t: int) -> int:
    """Level attenuation index `X = clamp(((128 - 0x13) T) >> 7, 0, 249)` of a position `T`."""
    return max(0, min(X_MAX, (X_NUM * t) >> X_SHIFT))


def expected_curve_id(edit_value: int) -> int:
    """`curveId` the Touch handler stores for the edit value `0 / 1 / 2`."""
    return CURVE_IDS[edit_value]


def max_abs_error(pairs: list[tuple[int, int]]) -> int:
    """Largest `|a - b|` of `(a, b)` pairs (0 for an empty list)."""
    return max((abs(a - b) for a, b in pairs), default=0)


def parse_model_rows(text: str) -> dict[int, tuple[int, int, list[int]]]:
    """Rows of `touch_model`: `velocity T X amp...` to `{velocity: (T, X, [amp...])}`."""
    rows = {}
    for line in text.splitlines():
        words = line.split()
        if len(words) >= 3 and all(w.lstrip("-").isdigit() for w in words):
            nums = [int(w) for w in words]
            rows[nums[0]] = (nums[1], nums[2], nums[3:])
    return rows


def read_member(ns4b: Path, member: str, limit: int | None = None) -> bytes:
    """Bytes of an archive member (`limit`: first bytes only)."""
    with zipfile.ZipFile(ns4b) as archive, archive.open(member) as handle:
        return handle.read() if limit is None else handle.read(limit)


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - needs the local firmware
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path)
    ap.add_argument("--eeprom", type=Path, required=True)
    ap.add_argument("--ns4b", type=Path, required=True)
    ap.add_argument("--program", required=True, help="archive member of the .ns4p")
    ap.add_argument("--npno", required=True, help="archive member of the piano sound")
    ap.add_argument("--note", type=int, default=60)
    ap.add_argument("--velocities", type=int, nargs="+", default=[1, 10, 33, 64, 100, 127])
    ap.add_argument("--ticks", type=int, default=300)
    ap.add_argument("--cache", type=Path, help="booted state cache (skips the 1 to 4 min boot)")
    ns = ap.parse_args(argv)
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import arm_keydown as kd
    import arm_piano_load as pl
    import arm_program_load as pg
    import arm_screens as sc
    import arm_steady as ast
    import unicorn.arm_const as uc_arm

    program = read_member(ns.ns4b, ns.program)
    stores: list[tuple[int, int]] = []
    session = sc.Session(ns.image, ns.sram)
    emu = session.emu
    kd.install_board_present(emu)
    emu.uc.hook_add(
        emu._u.UC_HOOK_CODE,
        lambda uc, *_: stores.append(
            (uc.reg_read(uc_arm.UC_ARM_REG_R2), uc.reg_read(uc_arm.UC_ARM_REG_R3))
        ),
        begin=TOUCH_STORE_PC,
        end=TOUCH_STORE_PC,
    )
    name = Path(ns.program).stem[:23]
    if ns.cache is not None and ns.cache.exists():
        saved_state, stores = pickle.loads(ns.cache.read_bytes())  # noqa: S301 - our own cache
        session.load_state(saved_state)
    else:
        session.load_eeproms(pg.build_image(ns.eeprom.read_bytes(), program, name))
        session.boot()
        session.run(ns.ticks)
        if ns.cache is not None:
            ns.cache.write_bytes(pickle.dumps((session.state_bytes(), stores)))
    print(f"recall: {len(stores)} Touch stores (ctx, curveId): {[(hex(a), b) for a, b in stores]}")
    emu.install_dsp_ports()
    rig = kd.Rig(emu, kd.Uart2().install(emu))
    npno = pl.read_npno(ns.ns4b, ns.npno)
    pl.LibraryStub({1: pl.device_image(npno, "Library.npno")}, pl.KIND_FULL).install(emu)
    pl.load_sound(emu, pl.managers(emu)[0], 1)
    # VERIFIED (run, D117): the Touch store's `r2` is the piano core object of `arm_piano_load`
    # (`[core + 0x14]` = sound pool); the recall of layer A stores first.
    core = stores[0][0]
    pl.select_sound(emu, 1, core=core)
    print(f"core {core:#x} bound sound {pl.bound_sound(emu, core):#x}")
    pl.install_dsp_ready(emu)
    rig.tick(2)
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
    calls = pl.hook_level_calls(emu)
    blob = ast.snapshot(emu)
    print("velocity T X(model) curveId level-calls[(x, rec5e, out)]")
    for v in ns.velocities:
        ast.restore(emu, blob)
        emu.install_dsp_ports()
        ast.enable_real_timer(emu)
        got.clear()
        ctxs.clear()
        calls.clear()
        kd.capture_note(rig, ns.note, v, {}, on_ticks=40, off_ticks=2)
        ctx = ctxs[0] if ctxs else 0
        curve = emu.read32(ctx + 0x1C) if ctx else -1
        t = got[0] if got else -1
        level = [(c.x, c.rec5e, c.out) for c in calls]
        print(f"{v} {t} {x_of_t(t) if t >= 0 else -1} curveId@ctx+0x1C={curve} {level}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
