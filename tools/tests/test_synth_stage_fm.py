"""Tests for the FM stage equation models (synthetic memory, no firmware)."""

from __future__ import annotations

import hashlib
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import synth_fm_golden as gold  # noqa: E402
import synth_stage_fm as sf  # noqa: E402
import synth_wave_model as m  # noqa: E402


def memory(seed: int, table_type: int) -> dict[int, int]:
    return gold.memory(seed, table_type)


def run(stage, seed: int, table_type: int) -> dict[int, int]:
    return gold.run(stage, seed, table_type)[1]


def digest(words: dict[int, int]) -> str:
    blob = b"".join(struct.pack("<II", a, w) for a, w in sorted(words.items()))
    return hashlib.sha256(blob).hexdigest()[:16]


def test_sext7_takes_bits_25_to_31_signed() -> None:
    assert sf.sext7(0x7F << 25) == -1
    assert sf.sext7(0x3F << 25) == 63
    assert sf.sext7(0x40 << 25) == -64
    assert sf.sext7(0x1FFFFFF) == 0


def test_phases_step_the_increment_four_times_and_wrap() -> None:
    ph = sf.phases([10, 0xFFFFFFFF], [3, 2])
    assert [p[0] for p in ph] == [13, 16, 19, 22]
    assert [p[1] for p in ph] == [1, 3, 5, 7]


def test_mod_words_fix_by_26_then_shift_5() -> None:
    one = m.ONE
    out = sf.mod_words([[one, one]] * 4, [one, m.HALF])
    assert out[0][0] == m.u32(m.fix(one, 26) << 5)
    assert out[0][1] == m.u32(m.fix(m.HALF, 26) << 5)


def test_lerp_and_fade_hit_their_ends() -> None:
    lo, hi = m.ld(0x3F000000), m.ld(0x3F800000)
    assert m.st(sf.lerp(lo, hi, m.ld(0))) == 0x3F000000
    assert m.st(sf.fade(m.ld(0))) == m.st(m.mul(m.add(sf.FADE_C1, sf.FADE_C1), sf.FADE_C2))


def test_mem_overlay_sees_own_stores_and_broadcast_loads_read_the_pex_word() -> None:
    seen: dict[int, int] = {}
    mm = sf.Mem(lambda a: a, seen.__setitem__)
    mm.put(10, [0x11, 0x22])
    assert mm.lanes(10) == [0x11, 0x22] and seen == {10: 0x11, 11: 0x22}
    assert m.st(mm.bc(0x3F800000)[1]) == 0x3F800000


def test_stage1_writes_62_words_and_is_deterministic() -> None:
    a, b = run(sf.stage1, 1, 49), run(sf.stage1, 1, 49)
    assert len(a) == 62 and a == b
    assert digest(a) != digest(run(sf.stage1, 2, 49))


def test_stage2_writes_52_words_and_is_deterministic() -> None:
    a, b = run(sf.stage2, 1, 50), run(sf.stage2, 1, 50)
    assert len(a) == 52 and a == b
    assert digest(a) != digest(run(sf.stage2, 1, 52))


def test_stage_fingerprints_lock_the_verified_models() -> None:
    # Words equal the listing interpreter (itself equal to sharc-org) over 20 trials x 5 types:
    # `python tools/synth_fm_oracle.py --model ...`, 1240 of 1240 and 1040 of 1040 words.
    assert digest(run(sf.stage1, 1, 49)) == GOLD1
    assert digest(run(sf.stage2, 1, 50)) == GOLD2


GOLD1 = "f6fce486bbd0a8c3"
GOLD2 = "bcd355e625f8da65"


def test_rust_golden_file_is_in_sync_with_the_generator(capsys) -> None:
    gold.main()
    text = capsys.readouterr().out.replace("\r\n", "\n")
    path = Path(__file__).resolve().parents[2] / "crates/nord-core/src/synth/tests_fw_fm_golden.rs"
    assert path.read_text(encoding="utf-8").replace("\r\n", "\n") == text
