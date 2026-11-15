import csv
import os
import sys
import wave

import numpy as np

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import up2_oracle as o  # noqa: E402

RATE = 44100


def _write(path, x, width=2):
    s = np.clip(x, -1, 1)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(x.shape[1])
        w.setsampwidth(width)
        w.setframerate(RATE)
        if width == 2:
            w.writeframes((s * 32767).astype("<i2").tobytes())
        else:
            v = (s * 8388607).astype("<i4")
            w.writeframes(np.frombuffer(v.tobytes(), np.uint8).reshape(-1, 4)[:, :3].tobytes())


def _decay(freq, tau, secs=3.0, amp=0.5):
    t = np.arange(int(RATE * secs)) / RATE
    m = amp * np.exp(-t / tau) * np.sin(2 * np.pi * freq * t)
    return np.stack([m, m], axis=1)


def test_parse_name():
    assert o.parse_name("Astoria Grand DN 021 043") == ("Astoria Grand DN", 21, 43)
    assert o.parse_name("readme") is None
    assert o.parse_name("Soft Grand KO 60 127") is None


def test_decay_times_follow_exponential_envelope():
    # amplitude exp(-t/tau): 20 dB down at tau*ln(10), 40 dB at 2*tau*ln(10)
    tau = 0.5
    m = o.measure(_decay(440.0, tau), RATE)
    assert abs(m["t20_s"] - tau * np.log(10)) < 0.05
    assert abs(m["t40_s"] - 2 * tau * np.log(10)) < 0.05
    assert abs(m["peak_db"] - 20 * np.log10(0.5)) < 0.1


def test_centroid_tracks_frequency():
    low = o.measure(_decay(200.0, 1.0), RATE)["centroid_hz"]
    high = o.measure(_decay(3000.0, 1.0), RATE)["centroid_hz"]
    assert abs(low - 200) < 60 and abs(high - 3000) < 300


def test_tail_db_reflects_noise_floor():
    x = _decay(440.0, 0.2, secs=4.0)
    x[-RATE:] += 0.001
    assert -62 < o.measure(x, RATE)["tail_db"] < -58


def test_catalog_skips_partial_and_unnamed_files_and_reads_24bit(tmp_path):
    _write(tmp_path / "Test Set 060 064.wav", _decay(300.0, 0.3))
    _write(tmp_path / "Test Set 060 127.wav", _decay(300.0, 0.3, amp=0.9), width=3)
    _write(tmp_path / "notes.wav", _decay(300.0, 0.3))
    (tmp_path / "Test Set 061 064.wav").write_bytes(b"RIFF\x00\x00")  # still being written
    out = tmp_path / "out" / "catalog.csv"
    rows = o.build_catalog(tmp_path, out)
    assert [(r.note, r.velocity) for r in rows] == [(60, 64), (60, 127)]
    with out.open() as fh:
        data = list(csv.DictReader(fh))
    assert data[0]["set"] == "Test Set" and set(data[0]) == set(o.FIELDS)
    curve = o.velocity_curve(rows, "Test Set", 60)
    assert curve[0][0] == 64 and curve[1][1] > curve[0][1]


def test_cli_prints_summary(tmp_path, capsys):
    _write(tmp_path / "A 021 043.wav", _decay(100.0, 0.3))
    assert o.main([str(tmp_path), str(tmp_path / "c.csv")]) == 0
    assert "1 files" in capsys.readouterr().out
