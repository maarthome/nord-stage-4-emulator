#!/usr/bin/env python3
"""The ARM generator of the FM band tables (strip types 49..53), ported from the listing.

The strip apply `0xC01CE7D8` builds three curves of 129 floats in a scratch buffer
(`0xC05AFC80`, `+0x204`, `+0x408`) and sends them to `DM(0xE08E6 + ...)` of the DSP core 2. The
table the stages read (`DM(0xE0900 .. 0xE0A68)`, 361 words) is `curve0[26:129] + curve1 + curve2`.
Listing `0xC01CB0DC .. 0xC01CBF38` (soft float calls: `0xC0270C80` `fmul`, `0xC0270A38` `fsub`,
`0xC0270A3C` `fadd`, `0xC0270E18` `fdiv`, `0xC0270BD4` `i2f`, `0xC025CA38` `f2d`, `0xC025D11C`
`d2f`, `0xC025CAF4` `dmul`, `0xC025C740` `dadd`, `0xC025C73C` `drsub`, `0xC025CD60` `ddiv`,
`0xC0271B24` `pow`, `0xC02712D8` `fmax`, `0xC0271E44` `expf`, `0xC025D268` `a < b`,
`0xC025D2A4` `a > b`), per index `j = 0 .. 128` with `s = -132 + 2 j`:

    Sc  = a8 / 2^19 * 8                    (a4 == 0, f32: float(a8) * 1.2014794e-7 * 127)
        = expf((((a8 + 0x8000) >> 16) - 12) * 0.0577622652)    (a4 != 0: 2^((n - 12) / 12))
    E   = f32(s) / 12;   x = f32(2^E 2^-7)
    c0  = max(0, 1 - 0.2 E) / Sc            (type 49), / (0.9 + 0.1 Sc) (type 52), else unscaled
    r   = Sc x;  g = fade(0.6912 - r)  (clip at +-0.2304, (t + 0.2304) 2.17 inside)
    K   = 2 pi r;   c1 = max(0, (c30 - x c24) / K - 0.4138)             (c30 = 0.75 / 1.5)
    sb  = 1.5 - x c28
    c2  = max(0, sb / (x c20 2 pi) - 0.4138)                            (types 49, 53)
        = max(0, sb / (K (m + 0.4138) + x c24) - 0.4138)                (types 50 .. 52,
          m = f(c1, (c18 + c1c) c0 g))

Per type constants `c18, c1c, c20, c24, c28` come from the firmware table at `0xC02F9E88 + 44 t`
(offsets `0xEC, 0xF8, 0x10C, 0x110, 0x114`); they are committed as numbers below (VERIFIED run).
Check: `python tools/synth_fm_generator.py` compares the committed capture
(`docs/research/data/synth-fm-tables.csv`, default `a4 = 0`, `a8 = 0x80000`).
"""

from __future__ import annotations

import math
import struct
import sys
from pathlib import Path

import numpy as np

F32 = np.float32
# VERIFIED (run, listing pool 0xC01CBEC0 / 0xC01CBEC4): a8 -> Sc = a8 * 1.2014794e-7 * 127.
K_A8 = F32(struct.unpack("<f", struct.pack("<I", 0x34010204))[0])
K_127 = F32(127.0)
TWO_PI = F32(struct.unpack("<f", struct.pack("<I", 0x40C90FDB))[0])
OFFSET = F32(struct.unpack("<f", struct.pack("<I", 0x3ED3DDF7))[0])  # 0.4138
CLIP = F32(struct.unpack("<f", struct.pack("<I", 0x3E6BEF0F))[0])  # 0.2304
FADE_TOP = struct.unpack("<d", struct.pack("<Q", 0x3FE61E6968000000))[0]
FADE_GAIN = struct.unpack("<d", struct.pack("<Q", 0x40015C5D6A142504))[0]
LN2_12 = F32(struct.unpack("<f", struct.pack("<I", 0x3D6C9820))[0])
# VERIFIED (run, firmware table 0xC02F9E88 + 44 t): per-type words c18, c1c, c20, c24, c28.
TYPE_WORDS = {
    49: (0x0, 0x41500000, 0x3F800000, 0x3F800000, 0x3F800000),
    50: (0x3EC7AE14, 0x0, 0x3F800000, 0x3F800000, 0x3F800000),
    51: (0x3EC7AE14, 0x0, 0x3F800000, 0x40400000, 0x3F800000),
    52: (0x3D75C28F, 0x0, 0x3F800000, 0x3F800000, 0x3F800000),
    53: (0x0, 0x3FE48E8A, 0x3F800000, 0x3F800000, 0x3F800000),
}
N = 129
TABLE_WORDS = 361
TABLE_BASE = 0xE0900


def _f(word: int) -> np.float32:
    return F32(struct.unpack("<f", struct.pack("<I", word))[0])


def scale(a4: int, a8: int) -> np.float32:
    """`Sc` from the strip words `+0xA4`, `+0xA8` (signed 32-bit)."""
    a8 = a8 - (1 << 32) if a8 >= 1 << 31 else a8
    if a4 == 0:
        return F32(F32(F32(a8) * K_A8) * K_127)
    n = (a8 + 0x8000) >> 16
    return F32(math.exp(float(F32(F32(n) - F32(12.0)) * LN2_12)))


def fade(t: np.float32) -> np.float32:
    """Clip fade of `0.6912 - r` (listing `0xC01CB44C .. 0xC01CB4A8`)."""
    if t > CLIP:
        return F32(1.0)
    if t < -CLIP:
        return F32(0.0)
    return F32(float(F32(t + CLIP)) * FADE_GAIN)


def curves(ty: int, a4: int = 0, a8: int = 0x80000) -> list[list[np.float32]]:
    """The three curves of 129 floats."""
    c18, c1c, c20, c24, c28 = (_f(w) for w in TYPE_WORDS[ty])
    sc = scale(a4, a8)
    c30 = F32(0.75) if ty in (50, 51, 52) else F32(1.5)
    out: list[list[np.float32]] = [[], [], []]
    for j in range(N):
        s = -132 + 2 * j
        e = float(F32(F32(s) / F32(12.0)))
        r4 = F32(1.0 + e * -0.2)
        if r4 < 0:
            r4 = F32(0.0)
        if ty == 49:
            r4 = F32(r4 / sc)
        elif ty == 52:
            r4 = F32(float(r4) / (float(sc) * 0.1 + 0.9))
        x = F32(math.pow(2.0, e) * 2.0**-7)
        r = F32(sc * x)
        sl = F32(x * c24)
        sb = F32(F32(1.5) - F32(x * c28))
        g = fade(F32(FADE_TOP - float(r)))
        k = F32(r * TWO_PI)
        c1 = F32(max(0.0, float(F32(F32(F32(c30 - sl) / k) - OFFSET))))
        if ty in (49, 53):
            den = F32(F32(x * c20) * TWO_PI)
        else:
            m = F32(F32(F32(c18 + c1c) * r4) * g)
            m = F32(min(float(c1), float(m)))
            den = F32(F32(F32(F32(m + OFFSET) * k) + sl))
        c2 = F32(max(0.0, float(F32(F32(sb / den) - OFFSET))))
        out[0].append(r4)
        out[1].append(c1)
        out[2].append(c2)
    return out


def table(ty: int, a4: int = 0, a8: int = 0x80000) -> list[int]:
    """The 361 table words (`DM(0xE0900 ..)`) of a type."""
    c = curves(ty, a4, a8)
    vals = c[0][26:] + c[1] + c[2]
    return [struct.unpack("<I", struct.pack("<f", float(v)))[0] for v in vals]


def main() -> int:  # pragma: no cover - reports against the committed capture
    path = Path(__file__).resolve().parents[1] / "docs/research/data/synth-fm-tables.csv"
    rows: dict[int, list[int]] = {}
    for line in path.read_text().splitlines()[1:]:
        t, dm, word, _ = line.split(",")
        rows.setdefault(int(t), [0] * TABLE_WORDS)[int(dm, 16) - TABLE_BASE] = int(word, 16)
    for ty, ref in sorted(rows.items()):
        got = table(ty)
        bad = sum(1 for a, b in zip(got, ref, strict=True) if abs(a - b) > 1)
        print(f"type {ty}: {TABLE_WORDS - bad}/{TABLE_WORDS} words within 1 ulp")
    return 0


if __name__ == "__main__":
    sys.exit(main())
