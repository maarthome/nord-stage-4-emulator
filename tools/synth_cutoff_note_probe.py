#!/usr/bin/env python3
"""Does the ORG core 2 pass add a note / key-tracking term to the cutoff `R`? (SHARC oracle probe)

The ARM sends one voice word per key to `DM(0xC1DCC + 3 voice + layer)`
(`docs/research/keydown-zones.md` s7: `(layer << 29) | 0x0B << 24 | ((note - 60) & 0xFF) << 16 |
0x80xx00`), the knob word `(knob - 52) << 24` to the layer common block `DM(0xE02E9)` and
the key tracking word `{0, -1/3, -2/3, -1}` Q31 to strip word 38. This tool writes those words to
`sharc-org` (ORG core 2 image) for one channel and reads the cutoff coefficient `R` after the strip
word has settled, then converts it to the table index `i = (R - 0.36083) / 0.0079301`.

    cargo build --release -p nord-sharc
    uv run python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR
    NS4_ORG_IMAGE=DIR/core2_sram.bin python tools/synth_cutoff_note_probe.py [--csv]
"""

from __future__ import annotations

import csv
import math
import sys
from pathlib import Path

import synth_adr_oracle as adr

R_BASE, R_STEP = 0.36083, 0.0079301
PASSES = 60
# VERIFIED (`docs/research/synth-knob-laws.md` s3): kFltKbdTrk 0..3 words in Q31.
TRK_WORDS = (0x00000000, 0xD5555555, 0xAAAAAAAA, 0x80000000)


def voice_word(note: int, g: int = 0x80, on: bool = True, layer: int = 0) -> int:
    """The synth key word of `0xC01EC690` (`keydown-zones.md` s7)."""
    kind = 0x0B if on else 0x08
    return ((layer & 7) << 29) | (kind << 24) | (((note - 60) & 0xFF) << 16) | (g << 8)


FREQ_LAW_CSV = (
    Path(__file__).resolve().parent.parent / "docs/research/data/synth-filter-freq-law.csv"
)


def knob_word(knob: int) -> int:
    """Common block word 1 of `kFltFreq` the ARM writes (`synth-knob-laws.md` s3).

    Top byte `knob - 52` for knobs 0..117; the steeper words above are read from the committed
    ARM table `synth-filter-freq-law.csv` when it is present.
    """
    if knob > 117 and FREQ_LAW_CSV.exists():
        for row in csv.DictReader(FREQ_LAW_CSV.read_text().splitlines()):
            if int(row["knob"]) == knob:
                return int(row["common_word1"], 16)
    return ((knob - 52) & 0xFF) << 24


def table_index(r: float) -> float:
    """Strip table index whose ramp value is `r`."""
    return (r - R_BASE) / R_STEP


def probe(org: str, image: str, knob: int, trk: int, note: int | None) -> float:
    """`R` of the filter lane after `PASSES` passes for the given inputs (`note None`: idle)."""
    word = knob_word(knob)
    msgs = [(0, 3, word), (0, 0x2E9, word), (0, 38, TRK_WORDS[trk])]
    pre = {} if note is None else {adr.CONTROLLER: voice_word(note)}
    rows = adr.run_script(org, image, PASSES, msgs=msgs, pre=pre, bufs=[f"{adr.R_WORDS:#x}:2:f32"])
    return adr.u2f(rows[(PASSES - 1, 0)][1])


# VERIFIED (`docs/research/synth-cutoff-voice-law.md` s2): the F of `R = tan(pi fc / F)`.
FRAME_RATE_HZ = 43_402.34375


def natural_hz(r: float) -> float:
    """`F atan(R) / pi`: the natural frequency the stage stores in `R` (frame rate `F`)."""
    return FRAME_RATE_HZ * math.atan(r) / math.pi


def semitone_offset(r: float, r_ref: float) -> float:
    """Semitones between two cutoffs, measured on `atan(R)` (exactly exponential in the knob)."""
    return 12.0 * math.log2(math.atan(r) / math.atan(r_ref))


def sweep_knobs(org: str, image: str, step: int = 1) -> list[tuple[int, float]]:
    """`(knob, R)` with a voice word of note 60, key tracking 0."""
    return [(k, probe(org, image, k, 0, 60)) for k in range(0, 128, step)]


def main(argv: list[str] | None = None) -> int:
    args = sys.argv[1:] if argv is None else argv
    org, image = adr.default_paths()
    if args[:1] == ["--csv"]:
        print("knob,R,natural_hz")
        for k, r in sweep_knobs(org, image):
            print(f"{k},{r:.8g},{natural_hz(r):.4f}")
        return 0
    print("knob trk note      R        i")
    for knob in (52, 80):
        for trk in (0, 1, 3):
            for note in (None, 36, 60, 84):
                r = probe(org, image, knob, trk, note)
                print(f"{knob:4d} {trk:3d} {str(note):>4} {r:9.5f} {table_index(r):8.2f}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
