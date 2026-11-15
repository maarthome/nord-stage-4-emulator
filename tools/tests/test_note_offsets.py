import json
import wave

import note_offsets as no
import numpy as np
import pytest

SR = 48_000


def tone(midi: int, start: float, stop: float, total: float, amp: float = 0.2) -> np.ndarray:
    t = np.arange(int(total * SR)) / SR
    f0 = no.midi_to_hz(midi)
    sig = sum((0.7 ** (h - 1)) * np.sin(2 * np.pi * f0 * h * t) for h in (1, 2, 3))
    gate = ((t >= start) & (t < stop)).astype(float)
    return (amp * sig * gate).astype(np.float32)


def test_midi_to_hz():
    assert no.midi_to_hz(69) == pytest.approx(440.0)


def test_partial_bins_stay_below_nyquist():
    assert no.partial_bins(60, SR).size == 9
    assert no.partial_bins(120, SR).size < 9


def test_sustained_tone_offset_is_found_within_100_ms():
    sig = tone(60, 0.5, 2.5, 4.0)
    power, hop = no.stft_power(sig, SR)
    est = no.estimate_offset(power, hop, 60, SR, 0.5, 4.0)
    assert est is not None
    assert abs(est - 2.5) < 0.1


def test_silence_has_no_offset_energy_bound():
    sig = np.zeros(SR * 2, np.float32)
    power, hop = no.stft_power(sig, SR)
    est = no.estimate_offset(power, hop, 60, SR, 0.2, 2.0)
    # all-zero power: reference equals the floor, nothing drops below it, so the cap is returned
    assert est == pytest.approx(2.0)


def test_audio_offsets_stretch_short_transcription_and_respect_next_onset():
    sig = tone(60, 0.5, 2.5, 4.0)
    ev = [
        {"midi": 60, "onset_s": 100.5, "offset_s": 100.55, "velocity": 70},
        {"midi": 62, "onset_s": 100.6, "offset_s": 100.65, "velocity": 70},
        {"midi": 60, "onset_s": 103.0, "offset_s": 103.1, "velocity": 70},
        {"midi": 60, "onset_s": 50.0, "offset_s": 50.1, "velocity": 70},
    ]
    new = no.audio_offsets(ev, sig, SR, 100.0, 4.0)
    assert new[0]["offset_s"] == pytest.approx(102.5, abs=0.1)
    assert new[0]["offset_s"] <= 103.0
    assert new[3]["offset_s"] == 50.1  # outside the window: untouched
    assert all(n["offset_s"] >= e["offset_s"] for n, e in zip(new, ev, strict=True))
    assert new[0] is not ev[0]


def test_merge_retriggers_joins_chatter_and_keeps_separate_notes():
    ev = [
        {"midi": 62, "onset_s": 1.0, "offset_s": 1.05, "velocity": 60},
        {"midi": 62, "onset_s": 1.2, "offset_s": 1.25, "velocity": 80},
        {"midi": 62, "onset_s": 3.0, "offset_s": 3.05, "velocity": 70},
        {"midi": 64, "onset_s": 1.1, "offset_s": 1.15, "velocity": 50},
    ]
    out = no.merge_retriggers(ev, 0.3)
    assert [(e["midi"], e["onset_s"], e["offset_s"]) for e in out] == [
        (62, 1.0, 1.25),
        (64, 1.1, 1.15),
        (62, 3.0, 3.05),
    ]
    assert out[0]["velocity"] == 60
    assert ev[0]["offset_s"] == 1.05  # input untouched
    assert len(no.merge_retriggers(ev, 0.0)) == 4


def test_hold_stats():
    ev = [
        {"midi": 60, "onset_s": 1.0, "offset_s": 2.0},
        {"midi": 61, "onset_s": 1.5, "offset_s": 4.5},
        {"midi": 62, "onset_s": 9.0, "offset_s": 9.1},
    ]
    s = no.hold_stats(ev, 0.0, 5.0)
    assert s["notes"] == 2
    assert s["median_s"] == 2.0
    assert no.hold_stats(ev, 20.0, 1.0)["notes"] == 0


def test_read_wav_mono_averages_channels(tmp_path):
    p = tmp_path / "a.wav"
    left = (np.ones(100) * 16384).astype("<i2")
    right = (np.ones(100) * -16384).astype("<i2")
    with wave.open(str(p), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(np.column_stack([left, right]).tobytes())
    sr, mono = no.read_wav_mono(p)
    assert sr == SR
    assert mono.shape == (100,)
    assert abs(float(mono.max())) < 1e-6
    json.dumps({"ok": True})
