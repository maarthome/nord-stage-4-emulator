#!/usr/bin/env python3
"""Piano transcription of reference-video audio into ``ns4 render-patch --events`` JSON.

Runs Transkun (MIT, https://github.com/yujia-yan/transkun) on the whole soundtrack of a local
video (or on one ``--at/--duration`` clip), converts the MIDI it writes (notes, velocities,
sustain pedal CC64) with ``midi_events.py`` and marks which stretches look like piano with
``clip_flags.py``. Only numbers are written to ``--events-out`` / ``--flags-out`` (no audio);
WAV and MIDI stay in ``--workdir``.

Needs an isolated environment with torch (CUDA) and transkun, outside the repo, for example
``C:\\tmp\\ns4-transcribe\\venv`` (see ``docs/research/piano-transcription.md``). The pure
helpers here and in ``midi_events.py`` / ``clip_flags.py`` import without torch.

Usage::

    python transcribe_clip.py run --media "demo.webm" --name part_a --workdir W \\
        --events-out docs/research/data/piano_events_part_a.json \\
        --programs docs/research/data/video_part_a_programs.json --device cuda
    python transcribe_clip.py slice docs/research/data/piano_events_part_a.json \\
        --at 1708 --duration 25 --out W/velvet.json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import wave
from pathlib import Path
from typing import Any, Sequence

import clip_flags
import midi_events
import numpy as np

SAMPLE_RATE = 44100


def extract_audio(media: Path, wav: Path, at: float | None, duration: float | None) -> None:
    """Writes 44.1 kHz mono 16-bit PCM of ``media`` (optionally one clip) with ffmpeg."""
    cmd = ["ffmpeg", "-v", "error", "-y"]
    if at is not None:
        cmd += ["-ss", str(at)]
    if duration is not None:
        cmd += ["-t", str(duration)]
    cmd += ["-i", str(media), "-vn", "-ac", "1", "-ar", str(SAMPLE_RATE), str(wav)]
    subprocess.run(cmd, check=True)


def read_wav_mono(path: Path) -> tuple[int, np.ndarray]:
    """Reads a 16-bit PCM WAV as float mono in [-1, 1]."""
    with wave.open(str(path), "rb") as w:
        sr, ch, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
        raw = w.readframes(w.getnframes())
    if width != 2:
        raise ValueError(f"{path}: expected 16-bit PCM")
    x = np.frombuffer(raw, dtype="<i2").astype(np.float32) / 32768.0
    return sr, x.reshape(-1, ch).mean(axis=1)


def load_transkun(device: str) -> Any:
    """Loads the pretrained Transkun 2.0 model on ``device`` (needs torch + transkun)."""
    import moduleconf
    import pkg_resources
    import torch

    weight = pkg_resources.resource_filename("transkun", "pretrained/2.0.pt")
    conf = pkg_resources.resource_filename("transkun", "pretrained/2.0.conf")
    mgr = moduleconf.parseFromFile(conf)
    model = mgr["Model"].module.TransKun(conf=mgr["Model"].config).to(device)
    ck = torch.load(weight, map_location=device)
    model.load_state_dict(ck.get("best_state_dict", ck.get("state_dict")), strict=False)
    model.eval()
    torch.set_grad_enabled(False)
    return model


def transcribe_wav(model: Any, wav: Path, midi: Path, device: str) -> None:
    """Runs the model over the whole WAV (it segments internally) and writes ``midi``."""
    import torch
    from transkun.Data import writeMidi

    sr, x = read_wav_mono(wav)
    if sr != model.fs:
        import soxr

        x = soxr.resample(x, sr, model.fs)
    audio = torch.from_numpy(np.ascontiguousarray(x.reshape(-1, 1), dtype=np.float32)).to(device)
    notes = model.transcribe(audio, discardSecondHalf=False)
    writeMidi(notes).write(str(midi))


def build_outputs(
    midi: Path, wav: Path, source: str, offset_s: float, programs: Sequence[dict[str, Any]]
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Events document (with per-note ``snr_db``) and the 1 s window rows for one clip."""
    doc = midi_events.to_events(midi_events.parse_midi(midi.read_bytes()), source)
    sr, mono = read_wav_mono(wav)
    rows = clip_flags.window_rows(mono, sr, doc["events"], programs, offset_s)
    if offset_s:
        doc = midi_events.to_events(
            {"notes": doc["events"], "pedal": doc["pedal"]}, source, offset_s
        )
    return doc, rows


def write_windows_csv(rows: Sequence[dict[str, Any]], path: Path) -> None:
    head = "t,program,class,rms_dbfs,notes,supported,piano_like"
    lines = [head]
    for r in rows:
        sup = "" if r["supported"] is None else r["supported"]
        lines.append(
            f"{r['t']},{r['program'] or ''},{r['class']},{r['rms_dbfs']},{r['notes']},"
            f"{sup},{int(r['piano_like'])}"
        )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def cmd_run(args: argparse.Namespace) -> int:
    work = Path(args.workdir)
    work.mkdir(parents=True, exist_ok=True)
    programs: list[dict[str, Any]] = []
    if args.programs:
        programs = json.loads(Path(args.programs).read_text(encoding="utf-8"))["programs"]
    wav, mid = work / f"{args.name}.wav", work / f"{args.name}.mid"
    offset = args.at or 0.0
    if not wav.exists():
        extract_audio(Path(args.media), wav, args.at, args.duration)
    if not mid.exists():
        transcribe_wav(load_transkun(args.device), wav, mid, args.device)
    doc, rows = build_outputs(mid, wav, "transkun-2.0", offset, programs)
    doc["media"] = Path(args.media).name
    doc["program_ranges"] = clip_flags.program_ranges(rows)
    Path(args.events_out).write_text(json.dumps(doc, separators=(",", ":")), encoding="utf-8")
    if args.flags_out:
        write_windows_csv(rows, Path(args.flags_out))
    print(f"{args.name}: {len(doc['events'])} notes, {len(doc['pedal'])} pedal events")
    return 0


def cmd_slice(args: argparse.Namespace) -> int:
    doc = json.loads(Path(args.events).read_text(encoding="utf-8"))
    cut = midi_events.slice_events(doc, args.at, args.duration)
    cut.pop("program_ranges", None)
    Path(args.out).write_text(json.dumps(cut), encoding="utf-8")
    print(f"{len(cut['events'])} notes, {len(cut['pedal'])} pedal events in {args.out}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run", help="Transcribe a media file (whole soundtrack or one clip).")
    r.add_argument("--media", required=True)
    r.add_argument("--name", required=True, help="Stem for the WAV/MIDI cache in --workdir.")
    r.add_argument("--workdir", required=True)
    r.add_argument("--events-out", required=True)
    r.add_argument("--flags-out", help="Per-second piano-likeness CSV.")
    r.add_argument("--programs", help="video_part_a_programs.json style program list.")
    r.add_argument("--at", type=float, help="Clip start (s); times stay in media time.")
    r.add_argument("--duration", type=float)
    r.add_argument("--device", default="cuda")
    r.set_defaults(fn=cmd_run)
    s = sub.add_parser("slice", help="Cut an events JSON to a window, re-based to 0 s.")
    s.add_argument("events")
    s.add_argument("--at", type=float, required=True)
    s.add_argument("--duration", type=float, required=True)
    s.add_argument("--out", required=True)
    s.set_defaults(fn=cmd_slice)
    return p


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    return int(args.fn(args))


if __name__ == "__main__":
    sys.exit(main())
