#!/usr/bin/env python3
"""Agreement of a transcribed note stream with the video-confirmed notes of a clip.

``video_ref.py notes --events-out`` marks a detected note ``confirmed`` when the keyboard in the
video shows the key going down (``VIDEO_SUPPORT_MIN``). Those are the best ground truth the
project has. The score is a plain onset match: a transcribed note matches a confirmed note when
it has the same MIDI number (or the same pitch class, counted separately as an octave match)
and its onset is within ``TOL_S``. Both lists must be in the same clip-relative time base.

``recall`` = confirmed notes found; ``precision`` = transcribed notes that match any confirmed
note, which is only a lower bound because the video confirms the visible part of the keyboard.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Sequence

TOL_S = 0.08


def match_notes(
    truth: Sequence[dict[str, Any]], est: Sequence[dict[str, Any]], tol_s: float = TOL_S
) -> dict[str, Any]:
    """Greedy one-to-one onset matching of ``est`` against ``truth`` (same pitch, within tol)."""
    used: set[int] = set()
    exact = octave = 0
    for t in truth:
        best, best_dt, best_oct = None, tol_s + 1.0, False
        for i, e in enumerate(est):
            if i in used:
                continue
            dt = abs(e["onset_s"] - t["onset_s"])
            if dt > tol_s:
                continue
            same = e["midi"] == t["midi"]
            octv = e["midi"] % 12 == t["midi"] % 12
            if (same or octv) and (dt < best_dt or (same and best_oct)):
                best, best_dt, best_oct = i, dt, not same
        if best is not None:
            used.add(best)
            if best_oct:
                octave += 1
            else:
                exact += 1
    n_t, n_e = len(truth), len(est)
    return {
        "truth": n_t,
        "estimated": n_e,
        "matched_exact": exact,
        "matched_octave": octave,
        "recall": round(exact / n_t, 3) if n_t else None,
        "recall_pitch_class": round((exact + octave) / n_t, 3) if n_t else None,
        "precision_lower_bound": round(exact / n_e, 3) if n_e else None,
    }


def agreement(
    video_events: Path, stream: Path, at_s: float, duration_s: float, tol_s: float = TOL_S
) -> dict[str, Any]:
    """Confirmed notes of a ``video_ref`` clip file against a media-time stream window."""
    vid = json.loads(video_events.read_text(encoding="utf-8"))["events"]
    doc = json.loads(stream.read_text(encoding="utf-8"))
    est = [
        {**e, "onset_s": e["onset_s"] - at_s}
        for e in doc["events"]
        if at_s <= e["onset_s"] < at_s + duration_s
    ]
    confirmed = [e for e in vid if e.get("confirmed")]
    out = match_notes(confirmed, est, tol_s)
    out["video_detected"] = len(vid)
    out["video_confirmed"] = len(confirmed)
    return out


def main(argv: Sequence[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("video_events", type=Path, help="video_ref.py notes --events-out file")
    ap.add_argument("stream", type=Path, help="transcribed stream (media time)")
    ap.add_argument("--at", type=float, required=True)
    ap.add_argument("--duration", type=float, required=True)
    ap.add_argument("--tol", type=float, default=TOL_S, help="Onset tolerance (s).")
    args = ap.parse_args(argv)
    res = agreement(args.video_events, args.stream, args.at, args.duration, args.tol)
    print(json.dumps(res))
    return 0


if __name__ == "__main__":
    sys.exit(main())
