#!/usr/bin/env python3
"""Real SHARC filter stages against the Rust model (`FwCoefs` / `FwFilter`), per stage.

Three checks on the ORG core 2 image (`sharc-org`) and the Rust dumper
`target/release/fw_filter_stage_dump` (`crates/nord-tools/src/bin/fw_filter_stage_dump.rs`):

* `words`: the frame words a sounding voice gives a stage. For each knob the pass control block
  writes the cutoff coefficient `R` (`DM(I3 - 30)`, voice word of note 60, knob word
  `(knob - 52) << 24`) and the resonance word `W28` (`DM(I3 - 28)`). Read from the oracle after a
  settling run; the stage type does not change them.
* `diff`: the stage is installed in the oracle (frame rewriting stores patched out), the frame
  words of `words` are poked, and 1024 samples (an impulse, 255 zeros, then noise) are filtered.
  The same words and input go to the Rust stage. Prints the per-sample difference.
* `corner`: the response of the stage at the real words (Rust stage, equal to the oracle by
  `diff`) against the response of the voice chain (`FwCoefs::from_params` at the displayed cutoff,
  the `--synth-fw-filter` unit) in Hz.

    cargo build --release -p nord-sharc -p nord-tools
    uv run python tools/ns4_dsp_ldr.py target/DSP/dsp_image_2_ORG.bin --sram DIR
    NS4_SHARC_ORG=target/release/sharc-org NS4_ORG_IMAGE=DIR/core2_sram.bin \
        uv run python tools/synth_filter_stage_diff.py diff --csv out.csv

Result and reading: `docs/research/synth-filter-stage-vs-model.md`. Nothing from the firmware is
stored: the CSVs hold derived numbers only.
"""

from __future__ import annotations

import argparse
import csv
import math
import struct
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parent))
import synth_adr_oracle as adr
import synth_cutoff_note_probe as probe
import synth_filter_ladder as lad
import synth_stage_oracle as o

# VERIFIED (docs/research/synth-filter-links.md s1): stage entry per `kFltType`, identity table.
STAGES = {
    "lp2": 0x1253BD,
    "lp4": 0x12566E,
    "ladder": 0x12549D,
    "bp4": 0x125913,
    "hp4": 0x125A50,
}
KNOBS = (24, 40, 52, 64, 80, 100, 118)
RES_KNOBS = (0, 16, 32, 48, 64, 80, 96, 112, 127)
SETTLE_PASSES = 200
SAMPLES = 1024
IMPULSE_TAIL = 256  # samples of the stream before the noise starts
NOISE_AMP = 0.5
# VERIFIED (docs/research/synth-cutoff-voice-law.md s2): the DSP frame rate and the display ratio.
F_HZ = 43_402.34375
DISPLAY_OVER_NATURAL = 1.971
UNIT_2F = 2.0 / DISPLAY_OVER_NATURAL
COMMON_RES = 0xE02EF  # layer A common word 7 (`kFltRes`), docs/research/synth-filter-links.md s2


def f2u(x: float) -> int:
    """Float32 bits of `x`."""
    return struct.unpack("<I", struct.pack("<f", x))[0]


def u2f(w: int) -> float:
    """Float of float32 bits."""
    return struct.unpack("<f", struct.pack("<I", w & 0xFFFFFFFF))[0]


def stream(n: int = SAMPLES, tail: int = IMPULSE_TAIL, amp: float = NOISE_AMP) -> list[float]:
    """Deterministic input: an impulse, `tail - 1` zeros, then LCG noise in `+-amp`."""
    xs = [0.0] * n
    xs[0] = 1.0
    state = 12345
    for i in range(tail, n):
        state = (1103515245 * state + 12345) & 0x7FFFFFFF
        xs[i] = amp * (2.0 * state / 0x7FFFFFFF - 1.0)
    return [u2f(f2u(x)) for x in xs]


def w28_word(position: float) -> int:
    """Resonance word of a table position `idx + fr` (`W28 = idx << 24 | fr 24 bit`)."""
    p = min(max(position, 0.0), 127.999)
    idx = int(p)
    return (idx << 24) | int((p - idx) * (1 << 24))


def settled_position(res_knob: int) -> float:
    """Settled table position of `kFltRes` (VERIFIED, `synth_filter_links.py res`)."""
    return min(128.0 * res_knob / 127.0, 127.999)


def metrics(ir: np.ndarray, kind: str) -> dict[str, float]:
    """Corner (cycles per sample), peak over reference (dB) and slope of an impulse response.

    LP types: reference DC gain, corner the last frequency still above `DC / sqrt 2`.
    HP: reference the gain at 0.45, corner the first frequency above `ref / sqrt 2`.
    BP: reference the peak, corner the geometric centre of the -3 dB band.
    The slope is dB per octave between 2 and 4 times the corner (LP) or half and quarter (HP).
    """
    n = 1 << int(math.ceil(math.log2(len(ir))))
    h = np.abs(np.fft.rfft(ir, 2 * n))
    f = np.arange(len(h)) / (2 * n)
    peak_i = int(np.argmax(h))
    if kind in ("lp2", "lp4", "ladder"):
        ref = h[0]
        above = np.nonzero(h >= ref / math.sqrt(2))[0]
        corner = float(f[above.max()]) if len(above) else 0.0
    elif kind == "hp4":
        ref = h[int(0.45 * 2 * n)]
        above = np.nonzero(h >= ref / math.sqrt(2))[0]
        corner = float(f[above.min()]) if len(above) else 0.0
    else:
        ref = h[peak_i]
        band = np.nonzero(h >= ref / math.sqrt(2))[0]
        corner = math.sqrt(float(f[band.min()]) * float(f[band.max()])) if len(band) else 0.0
    peak_db = 20 * math.log10(h[peak_i] / ref) if ref > 0 and h[peak_i] > 0 else float("nan")
    slope = float("nan")
    if kind != "bp4" and corner > 0:
        a, b = (2, 4) if kind != "hp4" else (0.5, 0.25)
        ia, ib = int(a * corner * 2 * n), int(b * corner * 2 * n)
        if 0 < ia < len(h) and 0 < ib < len(h) and h[ia] > 0 and h[ib] > 0:
            slope = 20 * math.log10(h[ib] / h[ia]) / (1.0 if kind != "hp4" else -1.0)
    return {"corner": corner, "peak_db": peak_db, "slope_db_oct": slope}


class Dumper:
    """Batch of requests to the Rust stage dumper (one process, one line each)."""

    def __init__(self, exe: str) -> None:
        self.exe = exe

    def run(self, lines: list[str]) -> list[list[int]]:
        """Output words (as ints) of every request line."""
        r = subprocess.run(
            [self.exe], input="\n".join(lines) + "\n", capture_output=True, text=True, check=True
        )
        out = []
        for line in r.stdout.splitlines():
            if line.startswith("error"):
                raise RuntimeError(line)
            out.append([int(w, 16) for w in line.split()])
        return out


def settled_words(org: str, image: str, knob: int, res_knob: int, passes: int) -> tuple[int, int]:
    """`(R word, W28 word)` the pass control block gives a sounding voice (note 60)."""
    kw = probe.knob_word(knob)
    res = f2u(-0.5 + res_knob / 127.0)
    extra = {0xE0003: kw, 0xE02E9: kw, COMMON_RES: res, adr.CONTROLLER: probe.voice_word(60)}
    r = o.run_stage(org, image, STAGES["lp4"], {}, [0.0] * (4 * passes), extra=extra)
    fr = r["frames"][-1]
    return fr[-30][0], fr[-28][0]


def oracle_stage(org: str, patched: str, kind: str, w30: int, w28: int, xs: list[float]):
    """Oracle output words of `kind` over `xs` with the frame words poked."""
    frame = {-30: (w30, w30), -28: (w28, w28)}
    if kind == "ladder":
        # the stage reads the coefficient words the previous call stored (frame -18 .. -12): start
        # from the ones this position gives, as `tools/synth_filter_ladder_oracle.py` does
        for off, word in zip((-18, -16, -14, -12), lad.coefficients(w30, w28), strict=True):
            frame[off] = (word, word)
    res = o.run_stage(org, patched, STAGES[kind], frame, xs, watch_frame=(-32, 8))
    return [f2u(v) for v in res["x"]]


def diff_words(a: list[int], b: list[int]) -> dict[str, float]:
    """Per-sample comparison of two output word lists (oracle `a`, Rust `b`)."""
    x = np.array([u2f(w) for w in a], dtype=np.float64)
    y = np.array([u2f(w) for w in b], dtype=np.float64)
    d = np.abs(x - y)
    peak = float(np.max(np.abs(x))) or 1.0
    bad = np.nonzero(np.array(a) != np.array(b))[0]
    return {
        "equal": int(len(a) - len(bad)),
        "total": len(a),
        "first_diff": int(bad[0]) if len(bad) else -1,
        "max_abs": float(d.max()),
        "max_rel_peak": float(d.max()) / peak,
        "peak": peak,
    }


def write_csv(path: str | None, header: list[str], rows: list[list[object]]) -> None:
    """Writes `rows` under `header` when `path` is given (derived numbers only)."""
    if not path:
        return
    with open(path, "w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh, lineterminator="\n")
        w.writerow(header)
        w.writerows(rows)


def knob_r_words(org: str, image: str, jobs: int) -> dict[int, int]:  # pragma: no cover
    """Settled `R` word per knob (voice note 60), from the oracle."""
    with ThreadPoolExecutor(jobs) as ex:
        res = list(ex.map(lambda k: settled_words(org, image, k, 64, SETTLE_PASSES)[0], KNOBS))
    return dict(zip(KNOBS, res, strict=True))


def cmd_diff(ns: argparse.Namespace) -> int:  # pragma: no cover - needs firmware and binaries
    org, image = o.default_paths()
    patched = Path(tempfile.gettempdir(), "core2_stage_diff_patched.bin")
    patched.write_bytes(o.patch_controls(Path(image).read_bytes()))
    xs = stream()
    xw = " ".join(f"{f2u(x):08x}" for x in xs)
    rw = knob_r_words(org, image, ns.jobs)
    jobs = [(kind, k, rk) for kind in STAGES for k in KNOBS for rk in RES_KNOBS]
    lines = [
        f"stage {kind} {rw[k]:08x} {w28_word(settled_position(rk)):08x} {xw}"
        for kind, k, rk in jobs
    ]
    want = Dumper(ns.dumper).run(lines)

    def one(job):
        kind, k, rk = job
        return oracle_stage(org, str(patched), kind, rw[k], w28_word(settled_position(rk)), xs)

    with ThreadPoolExecutor(ns.jobs) as ex:
        got = list(ex.map(one, jobs))
    rows = []
    for (kind, k, rk), g, w in zip(jobs, got, want, strict=True):
        d = diff_words(g, w)
        rows.append(
            [
                kind,
                k,
                rk,
                f"{u2f(rw[k]):.6g}",
                d["equal"],
                d["total"],
                d["first_diff"],
                f"{d['max_abs']:.3g}",
                f"{d['max_rel_peak']:.3g}",
                f"{d['peak']:.4g}",
            ]
        )
    hdr = [
        "stage",
        "knob",
        "res_knob",
        "R",
        "equal",
        "total",
        "first_diff",
        "max_abs",
        "max_rel_peak",
        "peak",
    ]
    for kind in STAGES:
        sub = [r for r in rows if r[0] == kind]
        eq, tot = sum(r[4] for r in sub), sum(r[5] for r in sub)
        worst = max(float(r[8]) for r in sub)
        print(f"{kind:7s} {eq} of {tot} words equal, worst max|d|/peak {worst:.3g}")
    write_csv(ns.csv, hdr, rows)
    return 0


def table_position_of_c(c: float) -> float:
    """Ladder24 table position `p` (0..1) whose row 2 value is `c` (the section pole is `-c`)."""
    rows = [u2f(w) for w in lad.ROWS[2]]
    for i in range(128):
        if rows[i] <= c <= rows[i + 1]:
            return (i + (c - rows[i]) / (rows[i + 1] - rows[i])) / 128.0
    return float("nan")


def cmd_ladder_voice(ns: argparse.Namespace) -> int:  # pragma: no cover - needs firmware
    """Ladder24 coefficient position with and without a voice word (knobs 40 .. 100).

    Without the voice word the stored coefficient `c` follows `R` (position = `R`); with it the
    position stays constant while `R` follows the voice law.
    """
    org, image = o.default_paths()
    print("knob voice R position")
    for knob in (40, 52, 64, 80, 100):
        kw = probe.knob_word(knob)
        for voice in (False, True):
            extra = {0xE0003: kw, 0xE02E9: kw}
            if voice:
                extra[adr.CONTROLLER] = probe.voice_word(60)
            res = o.run_stage(
                org, image, STAGES["ladder"], {}, [0.0] * 240, extra=extra, watch_frame=(-32, 64)
            )
            fr = res["frames"][-1]
            pos = table_position_of_c(u2f(fr[-14][0]))
            print(knob, voice, f"{u2f(fr[-30][0]):.4f}", f"{pos:.4f}")
    return 0


def cmd_words(ns: argparse.Namespace) -> int:  # pragma: no cover - needs firmware
    org, image = o.default_paths()
    print("knob res_knob R W28 position")
    for k in KNOBS:
        for rk in (0, 64, 127):
            r, w = settled_words(org, image, k, rk, ns.passes)
            pos = (w >> 24) + (w & 0xFFFFFF) / (1 << 24)
            print(k, rk, f"{u2f(r):.6g}", f"{w:08x}", f"{pos:.3f}")
    return 0


def cmd_corner(ns: argparse.Namespace) -> int:  # pragma: no cover - needs firmware
    org, image = o.default_paths()
    with open(ns.law, encoding="utf-8") as fh:
        law = {int(r["knob"]): r for r in csv.DictReader(fh)}
    rw = knob_r_words(org, image, ns.jobs)
    n = 65536
    jobs = [(kind, k, rk) for kind in STAGES for k in KNOBS for rk in RES_KNOBS]
    hw, eng = [], []
    for kind, k, rk in jobs:
        disp = float(law[k]["display_hz"])
        hw.append(f"stage {kind} {rw[k]:08x} {w28_word(settled_position(rk)):08x} imp:{n}")
        res_t = settled_position(rk) / 127.0
        eng.append(f"engine {kind} {disp * UNIT_2F:.6g} {res_t:.9g} 48000 imp:{n}")
    dump = Dumper(ns.dumper)
    a, b = dump.run(hw), dump.run(eng)
    rows = []
    for (kind, k, rk), x, y in zip(jobs, a, b, strict=True):
        mh = metrics(np.array([u2f(w) for w in x]), kind)
        me = metrics(np.array([u2f(w) for w in y]), kind)
        disp = float(law[k]["display_hz"])
        hz_hw, hz_en = mh["corner"] * 2 * F_HZ, me["corner"] * 48000.0
        ratio = f"{hz_en / hz_hw:.4g}" if hz_hw > 0 else "nan"
        rows.append(
            [
                kind,
                k,
                rk,
                f"{u2f(rw[k]):.6g}",
                f"{disp:g}",
                f"{hz_hw:.5g}",
                f"{hz_en:.5g}",
                ratio,
                f"{mh['peak_db']:.3f}",
                f"{me['peak_db']:.3f}",
                f"{mh['slope_db_oct']:.2f}",
                f"{me['slope_db_oct']:.2f}",
            ]
        )
    hdr = [
        "stage",
        "knob",
        "res_knob",
        "R",
        "display_hz",
        "hw_corner_hz_2F",
        "model_corner_hz",
        "model_over_hw",
        "hw_peak_db",
        "model_peak_db",
        "hw_slope",
        "model_slope",
    ]
    write_csv(ns.csv, hdr, rows)
    print(",".join(hdr))
    for r in rows:
        if r[2] in (0, 64, 112):
            print(",".join(str(v) for v in r))
    return 0


def main(argv: list[str] | None = None) -> int:  # pragma: no cover - thin CLI
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    ap.add_argument("what", choices=("words", "diff", "corner", "ladder-voice"))
    ap.add_argument("--dumper", default="target/release/fw_filter_stage_dump")
    ap.add_argument("--law", default="docs/research/data/synth-cutoff-voice-law.csv")
    ap.add_argument("--csv")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--passes", type=int, default=SETTLE_PASSES)
    ns = ap.parse_args(argv)
    return {
        "words": cmd_words,
        "diff": cmd_diff,
        "corner": cmd_corner,
        "ladder-voice": cmd_ladder_voice,
    }[ns.what](ns)


if __name__ == "__main__":
    sys.exit(main())
