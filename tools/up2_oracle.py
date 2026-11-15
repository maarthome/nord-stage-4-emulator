#!/usr/bin/env python3
"""Reference-metrics catalog for exported Ultimate Pianos 2 WAV samples.

Files are named ``<set> <MIDI note> <velocity>.wav`` (Kontakt export). For each file
this measures numbers that an emulator render can be compared against: peak and
RMS level, time to fall 20/40 dB below the attack, spectral centroid just after
the attack, and the noise floor at the tail. Output is a CSV that stays under the
git-ignored ``.reference/`` tree (the samples are third-party audio; only numbers
derived from them are written, and the tool itself is the only thing committed).

All metrics are measured on the mono mix (mean of channels), full scale = 1.0.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
import wave
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np

NAME_RE = re.compile(r"^(?P<set>.+) (?P<note>\d{3}) (?P<vel>\d{3})$")
ENV_WINDOW_S = 0.010
CENTROID_WINDOW = 4096
FIELDS = [
    "set",
    "note",
    "velocity",
    "frames",
    "rate",
    "channels",
    "peak_db",
    "rms_100ms_db",
    "t_attack_peak_s",
    "t20_s",
    "t40_s",
    "centroid_hz",
    "tail_db",
]


@dataclass
class Row:
    set: str
    note: int
    velocity: int
    frames: int
    rate: int
    channels: int
    peak_db: float
    rms_100ms_db: float
    t_attack_peak_s: float
    t20_s: float
    t40_s: float
    centroid_hz: float
    tail_db: float


def parse_name(stem: str) -> tuple[str, int, int] | None:
    m = NAME_RE.match(stem)
    return (m["set"], int(m["note"]), int(m["vel"])) if m else None


def read_wav(path: Path) -> tuple[np.ndarray, int]:
    """Return (samples shape (frames, channels) float in [-1, 1], sample rate)."""
    with wave.open(str(path), "rb") as w:
        ch, width, rate, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        raw = w.readframes(n)
    if width == 2:
        x = np.frombuffer(raw, "<i2").astype(np.float64) / 32768.0
    elif width == 3:
        b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
        v = b[:, 0] | (b[:, 1] << 8) | (b[:, 2] << 16)
        x = np.where(v & 0x800000, v - 0x1000000, v) / 8388608.0
    else:
        raise ValueError(f"{path.name}: unsupported sample width {width}")
    return x.reshape(-1, ch), rate


def _db(v: float) -> float:
    return float(20.0 * np.log10(max(v, 1e-9)))


def _time_below(env_db: np.ndarray, start: int, level: float, step_s: float) -> float:
    idx = np.nonzero(env_db[start:] <= level)[0]
    return float(idx[0] * step_s) if idx.size else float("nan")


def measure(x: np.ndarray, rate: int) -> dict[str, float]:
    """Metrics of a (frames, channels) float array."""
    mono = x.mean(axis=1)
    win = max(1, int(ENV_WINDOW_S * rate))
    nwin = len(mono) // win
    if nwin == 0:
        raise ValueError("file shorter than one envelope window")
    env = np.sqrt((mono[: nwin * win].reshape(nwin, win) ** 2).mean(axis=1))
    env_db = 20.0 * np.log10(np.maximum(env, 1e-9))
    k = int(np.argmax(env_db))
    peak_db = env_db[k]
    seg = mono[k * win : k * win + CENTROID_WINDOW]
    if len(seg) > 8:
        spec = np.abs(np.fft.rfft(seg * np.hanning(len(seg))))
        freqs = np.fft.rfftfreq(len(seg), 1.0 / rate)
        centroid = float((spec * freqs).sum() / max(spec.sum(), 1e-12))
    else:
        centroid = float("nan")
    tail = mono[-max(win, rate // 10) :]
    return {
        "peak_db": _db(float(np.abs(x).max())),
        "rms_100ms_db": _db(float(np.sqrt((mono[: rate // 10] ** 2).mean()))),
        "t_attack_peak_s": float(k * ENV_WINDOW_S),
        "t20_s": _time_below(env_db, k, peak_db - 20.0, ENV_WINDOW_S),
        "t40_s": _time_below(env_db, k, peak_db - 40.0, ENV_WINDOW_S),
        "centroid_hz": centroid,
        "tail_db": _db(float(np.sqrt((tail**2).mean()))),
    }


def analyze(path: Path) -> Row | None:
    parsed = parse_name(path.stem)
    if parsed is None:
        return None
    x, rate = read_wav(path)
    m = measure(x, rate)
    return Row(parsed[0], parsed[1], parsed[2], x.shape[0], rate, x.shape[1], **m)


def build_catalog(folder: Path, out_csv: Path) -> list[Row]:
    """Analyze every named WAV in ``folder``; files still being written are skipped."""
    rows: list[Row] = []
    for f in sorted(folder.glob("*.wav")):
        try:
            row = analyze(f)
        except (wave.Error, EOFError, ValueError):
            continue
        if row:
            rows.append(row)
    out_csv.parent.mkdir(parents=True, exist_ok=True)
    with out_csv.open("w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=FIELDS)
        w.writeheader()
        for r in rows:
            w.writerow(
                {k: (f"{v:.4f}" if isinstance(v, float) else v) for k, v in asdict(r).items()}
            )
    return rows


def velocity_curve(rows: list[Row], set_name: str, note: int) -> list[tuple[int, float]]:
    """(velocity, peak_db) pairs for one set and note, ascending velocity."""
    return sorted((r.velocity, r.peak_db) for r in rows if r.set == set_name and r.note == note)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Build the UP2 reference-metrics catalog.")
    ap.add_argument("folder", type=Path, help="folder of exported .wav files")
    ap.add_argument("out_csv", type=Path)
    a = ap.parse_args(argv)
    rows = build_catalog(a.folder, a.out_csv)
    sets = sorted({r.set for r in rows})
    print(f"{len(rows)} files, sets: {', '.join(sets)} -> {a.out_csv}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
