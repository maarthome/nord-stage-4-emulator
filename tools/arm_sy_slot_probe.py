#!/usr/bin/env python3
"""Who fills the SY pitch table entry `e` at `DM(0x9354A + index)`: slot index and watches.

Evidence and tables: `docs/research/sy-pitch-entry-writer.md`. The SY ISR step 3 reads word 0 of
every voice pair block (`DM(0x9219E + 0x16 k)`, SIMD lanes at the even and the odd address),
splits it into a table index (low 16 bits, `FEXT` spec `0x0400`) and the field spec of the entry
(high 16 bits: `0x0400` low half, `0x0410` high half of the table word) and reads
`DM(0x9354A + index)`. This tool shows where the ARM gets that index and who writes the entry:

* `word0(index, high)` / `decode_word0(word)`: the pair word the ARM sends from `0xC01E2B08`
  (`[obj + 0x24] | 0x4000000`, lane x) and `0xC01E2D08` (`[obj + 0x28] | 0x4100000`).
* `probe`: boots the SAMPLES rig (as `arm_sy_voice`), prints the slot object `[0x9062F0F0 + 0 / 4]`
  (the handle table entry `0xC01E22C0..0xC01E22EC` copies into `[sp + 0x7C / 0x78]`), watches writes
  to the slot object and to the voice object fields `+0x24 / +0x28` over load, bind and note-on, and
  lists every SY or SMP message of the note-on that addresses `0x93500..0x935A0`.

    uv run --with unicorn python tools/arm_sy_slot_probe.py target/fw_unpacked/bank0_CODE.bin \
        target/fw_unpacked/bank0_SRAM.bin --cache target/keydown.snap \
        --ns4b ".reference/Nord Stage 4 Factory Restore RevK v1.66.ns4b" \
        --member "Samp Lib/Accordion_Harm/Carina Accordion 1_PS 4.1.nsmp4"

Nothing from the backup is written to disk.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# VERIFIED (listing `0xC01E2B08`, `0xC01E2CFC`, `0xC01E2D08`): the spec halves the ARM ORs into the
# slot index; `0x0400` = `FEXT` position 0 length 16 (low half), `0x0410` = position 16 (high half).
SPEC_LOW, SPEC_HIGH = 0x0400, 0x0410
# VERIFIED (SY listing `0x1C0679..0x1C06A3`, kernel-rule note section 13):
TABLE_BASE, TABLE_WORDS = 0x9354A, 0x44
# VERIFIED (run): the SAMPLES voice object of synth layer A and the slot object it reads.
VOICE_OBJECT, SLOT_OBJECT = 0x90670230, 0x9062F0F0
VOICE_SLOT_FIELDS = (0x24, 0x28)
WINDOW = (0x93500, 0x935A0)


def word0(index: int, high: bool = False) -> int:
    """The pair block word 0 the ARM sends for table `index` (`-1` = unused lane)."""
    index &= 0xFFFFFFFF
    return index | ((SPEC_HIGH if high else SPEC_LOW) << 16)


def decode_word0(word: int) -> tuple[int, int, int]:
    """`(index, spec position, spec length)` as the SY `FEXT` reads word 0."""
    spec = (word >> 16) & 0xFFFF
    return word & 0xFFFF, spec & 0x3F, (spec >> 6) & 0x3F


def entry_address(index: int) -> int:
    """DM word the SY reads for `index` (before the half is picked)."""
    return TABLE_BASE + (index & 0xFFFF)


def in_window(dm_word: int) -> bool:
    """True when a message address falls in the table window `0x93500..0x935A0`."""
    return WINDOW[0] <= dm_word < WINDOW[1]


def probe(code: Path, sram: Path | None, cache: Path | None, ns4b: Path, member: str) -> int:
    """Run the SAMPLES note-on and report the slot object, the watches and the table messages."""
    import arm_edit_capture as aec
    import arm_piano_load as pl
    import arm_sample_probe as sp
    import arm_sy_voice as sv
    import arm_zone_capture as zc

    data = pl.read_npno(ns4b, member, 0x400000)
    setup = zc.prepare(code, sram, cache)
    emu = setup.emu
    writes: list[tuple[str, int, int, int]] = []

    def watch(_uc, _access, address, _size, value, _user) -> None:  # noqa: ANN001
        writes.append(("w", address, value & 0xFFFFFFFF, emu.reg(14)))

    lo, hi = VOICE_OBJECT + VOICE_SLOT_FIELDS[0], VOICE_OBJECT + VOICE_SLOT_FIELDS[1] + 3
    emu.uc.hook_add(emu._u.UC_HOOK_MEM_WRITE, watch, begin=lo, end=hi)
    emu.uc.hook_add(emu._u.UC_HOOK_MEM_WRITE, watch, begin=SLOT_OBJECT, end=SLOT_OBJECT + 7)
    setup.keep({4})
    sp.serve_sample(emu, sp.sample_device_image(data, sp.sample_name(data)))
    if sp.load_sample(emu) != 0:
        raise SystemExit("sample loader failed")
    store = setup.stores[4]
    aec.apply_value(emu, store.address, sp.EDIT_SMP_ID, 1)
    aec.apply_value(emu, store.address, sp.EDIT_MODE, 0)
    sp.bind_layer(emu, 0)
    setup.rig.tick(2)
    print(f"after load and bind: {len(writes)} writes to slot object / voice slot fields")
    slots = [emu.read32(SLOT_OBJECT + o) for o in (0, 4)]
    print("slot object [+0], [+4]:", [hex(s) for s in slots])
    setup.freeze()
    setup.reset()
    raw: list[tuple[int, int]] = []
    orig = sv.decode_messages

    def keep(pairs):  # noqa: ANN001, ANN202
        raw.extend(pairs)
        return orig(pairs)

    sv.decode_messages = keep
    try:
        sv.play_note(setup.rig, 60, ticks=60)
    finally:
        sv.decode_messages = orig
    for kind, address, value, lr in writes:
        print(f"  {kind} {address:#x} <- {value:#x} (lr {lr:#x})")
    hits = [(a, b) for a, b in raw if in_window(a)]
    print(f"note 60: {len(raw)} SY messages, {len(hits)} into {WINDOW[0]:#x}..{WINDOW[1]:#x}")
    for a, b in hits:
        print(f"  {a:#x} <- {b:#x}")
    return 0


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("image", type=Path)
    ap.add_argument("sram", type=Path, nargs="?")
    ap.add_argument("--cache", type=Path)
    ap.add_argument("--ns4b", type=Path, required=True)
    ap.add_argument("--member", required=True)
    ns = ap.parse_args(argv)
    return probe(ns.image, ns.sram, ns.cache, ns.ns4b, ns.member)


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
